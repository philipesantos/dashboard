#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
//! The test sign-in service of feature 005 (`specs/005-plugin-credentials/research.md`, R15): a
//! demonstration notes service at `https://notes.test`, on an HTTPS test server reached through
//! the seam, with an authorization code flow with PKCE, rotating refresh tokens, revocation, an
//! identity operation, and notes that only an account reads. It is scripted per test to refuse,
//! answer another account, or send the credential back, and it plays the person in the browser.
//! Every token, refresh token, code and verifier it issues or sees is a fresh random value,
//! kept so [`scan`] can look for it everywhere it must not be.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::sync::{Arc, Mutex, PoisonError};

use sha2::{Digest, Sha256};

use super::servers::{Received, Reply, Server, TestPki};

/// The host of the service, as `local/notes` declares it.
pub const HOST: &str = "notes.test";

/// How the service misbehaves, set per test.
#[derive(Debug, Clone, Default)]
pub struct Script {
    /// The token address refuses every exchange.
    pub token_refused: bool,
    /// The token address refuses every refresh.
    pub refresh_refused: bool,
    /// `/me` answers `401` to every token.
    pub identity_refused: bool,
    /// `/notes` answers this status to every request that carries a valid token.
    pub notes_status: Option<u16>,
    /// `/notes` sends the `Authorization` value back.
    pub echo: Option<Echo>,
    /// The next exchange answers for this account `(id, name)` instead.
    pub other_account: Option<(String, String)>,
    /// `expires_in` of every token issued.
    pub expires_in: Option<u64>,
    /// The callback the person's browser makes: a wrong state, no state, or no code.
    pub callback: Callback,
    /// The service gives a token outside the bearer-token set.
    pub bad_token: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Echo {
    Body,
    Header,
    Location,
    JsonEscaped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Callback {
    #[default]
    Right,
    WrongState,
    NoState,
    Error,
    /// The code of an earlier sign-in, already exchanged.
    UsedCode,
}

#[derive(Default)]
struct State {
    script: Script,
    /// The account signed in: `(id, name)`.
    account: (String, String),
    /// Codes issued: code → (challenge, redirect address, client id, used).
    codes: HashMap<String, (String, String, String, bool)>,
    /// Valid access tokens → account id.
    access: HashMap<String, String>,
    /// Valid refresh tokens → account id.
    refresh: HashMap<String, String>,
    revoked: Vec<String>,
    /// Every secret issued or seen, for the scan.
    secrets: Vec<String>,
    last_code: Option<String>,
}

/// The test sign-in service, running.
pub struct SignInService {
    pub server: Server,
    state: Arc<Mutex<State>>,
}

impl SignInService {
    /// Starts the service on an HTTPS test server whose certificate `pki` issued for `notes.test`.
    pub fn start(pki: &TestPki) -> SignInService {
        let server = Server::https(&pki.leaf(&[HOST]));
        let state = Arc::new(Mutex::new(State {
            account: ("1234567".to_owned(), "ada".to_owned()),
            ..State::default()
        }));
        let service = SignInService { server, state };
        service.routes();
        service
    }

    /// The seam's route from `notes.test:443` to the server.
    pub fn route(&self) -> runtime::Route {
        runtime::Route {
            host: HOST.to_owned(),
            port: 443,
            answers: vec![vec![self.server.address()]],
        }
    }

    pub fn script(&self, edit: impl FnOnce(&mut Script)) {
        edit(&mut self.lock().script);
    }

    /// Every token, refresh token, code and verifier the service issued or saw.
    pub fn secrets(&self) -> Vec<String> {
        self.lock().secrets.clone()
    }

    /// The tokens revoked at `/oauth/revoke`.
    pub fn revoked(&self) -> Vec<String> {
        self.lock().revoked.clone()
    }

    /// Requests that reached `path`.
    pub fn requests_to(&self, path: &str) -> Vec<Received> {
        self.server
            .received()
            .into_iter()
            .filter(|request| request.path == path)
            .collect()
    }

    /// Plays the person in the browser at `address`, the sign-in address the host opens: signs
    /// in, accepts, and sends the browser to the loopback callback, as the script says.
    pub fn browser(&self) -> impl Fn(&url::Url) + Send + Sync + 'static {
        let state = Arc::clone(&self.state);
        move |address: &url::Url| {
            let query: HashMap<String, String> = address.query_pairs().into_owned().collect();
            let redirect = query.get("redirect_uri").cloned().unwrap_or_default();
            let challenge = query.get("code_challenge").cloned().unwrap_or_default();
            let client = query.get("client_id").cloned().unwrap_or_default();
            let given_state = query.get("state").cloned().unwrap_or_default();
            let (callback, code) = {
                let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
                let code = match (state.script.callback, &state.last_code) {
                    (Callback::UsedCode, Some(used)) => used.clone(),
                    _ => {
                        let code = random("code");
                        state
                            .codes
                            .insert(code.clone(), (challenge, redirect.clone(), client, false));
                        state.secrets.push(code.clone());
                        state.last_code = Some(code.clone());
                        code
                    }
                };
                (state.script.callback, code)
            };
            let query = match callback {
                Callback::Right | Callback::UsedCode => format!("code={code}&state={given_state}"),
                Callback::WrongState => format!("code={code}&state=forged"),
                Callback::NoState => format!("code={code}"),
                Callback::Error => "error=access_denied&state=".to_owned() + &given_state,
            };
            let redirect = url::Url::parse(&redirect).expect("a redirect address");
            let port = redirect.port().expect("a loopback port");
            if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
                let _ = write!(
                    stream,
                    "GET {}?{query} HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n",
                    redirect.path()
                );
                let mut answer = String::new();
                let _ = stream.read_to_string(&mut answer);
            }
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn routes(&self) {
        let state = Arc::clone(&self.state);
        self.server.route_with("/oauth/token", move |request| {
            let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
            let form: HashMap<String, String> = url::form_urlencoded::parse(&request.body)
                .into_owned()
                .collect();
            for secret in ["code", "code_verifier", "refresh_token"] {
                if let Some(value) = form.get(secret) {
                    state.secrets.push(value.clone());
                }
            }
            match form.get("grant_type").map(String::as_str) {
                Some("authorization_code") => {
                    if state.script.token_refused {
                        return refusal("invalid_grant", "the exchange is refused");
                    }
                    let code = form.get("code").cloned().unwrap_or_default();
                    let verifier = form.get("code_verifier").cloned().unwrap_or_default();
                    let Some((challenge, redirect, client, used)) = state.codes.get(&code).cloned()
                    else {
                        return refusal("invalid_grant", "unknown code");
                    };
                    if used {
                        return refusal(
                            "invalid_grant",
                            &format!("the code {code} was already used"),
                        );
                    }
                    if s256(&verifier) != challenge
                        || form.get("redirect_uri") != Some(&redirect)
                        || form.get("client_id") != Some(&client)
                    {
                        return refusal(
                            "invalid_grant",
                            "the verifier, redirect or client does not match",
                        );
                    }
                    if form.contains_key("client_secret") {
                        return refusal("invalid_request", "a public client sends no secret");
                    }
                    state
                        .codes
                        .insert(code, (challenge, redirect, client, true));
                    let account = match state.script.other_account.take() {
                        Some(other) => {
                            state.account = other.clone();
                            other
                        }
                        None => state.account.clone(),
                    };
                    issue(&mut state, &account.0)
                }
                Some("refresh_token") => {
                    let refresh = form.get("refresh_token").cloned().unwrap_or_default();
                    let Some(account) = state.refresh.remove(&refresh) else {
                        return refusal("invalid_grant", "unknown refresh token");
                    };
                    if state.script.refresh_refused {
                        return refusal("invalid_grant", "the refresh is refused");
                    }
                    // Rotation: the old access tokens of this account stop working too
                    state.access.retain(|_, owner| *owner != account);
                    issue(&mut state, &account)
                }
                _ => refusal("unsupported_grant_type", "unknown grant"),
            }
        });
        let state = Arc::clone(&self.state);
        self.server.route_with("/oauth/revoke", move |request| {
            let mut state = state.lock().unwrap_or_else(PoisonError::into_inner);
            let form: HashMap<String, String> = url::form_urlencoded::parse(&request.body)
                .into_owned()
                .collect();
            if let Some(token) = form.get("token") {
                state.access.remove(token);
                state.refresh.remove(token);
                state.revoked.push(token.clone());
            }
            Reply::status(200)
        });
        let state = Arc::clone(&self.state);
        self.server.route_with("/me", move |request| {
            let state = state.lock().unwrap_or_else(PoisonError::into_inner);
            match bearer(request).and_then(|token| state.access.get(&token)) {
                Some(account) if !state.script.identity_refused => {
                    let name = if *account == state.account.0 {
                        state.account.1.clone()
                    } else {
                        "someone".to_owned()
                    };
                    Reply::json(&serde_json::json!({ "id": account, "name": name }).to_string())
                }
                _ => Reply::status(401),
            }
        });
        let state = Arc::clone(&self.state);
        self.server.route_with("/notes", move |request| {
            let state = state.lock().unwrap_or_else(PoisonError::into_inner);
            let public = serde_json::json!([{ "title": "Welcome", "private": false }]);
            let Some(token) = bearer(request) else {
                return Reply::json(&public.to_string());
            };
            if let Some(echo) = state.script.echo {
                let header = request
                    .header("authorization")
                    .unwrap_or_default()
                    .to_owned();
                return match echo {
                    Echo::Body => Reply::json(&serde_json::json!({ "seen": header }).to_string()),
                    Echo::Header => Reply::json("[]").header("ETag", &header),
                    Echo::Location => Reply::redirect(302, &format!("/notes?seen={token}")),
                    Echo::JsonEscaped => Reply::json(&format!(
                        r#"{{"seen":"{}"}}"#,
                        token
                            .replace('/', "\\/")
                            .replace('+', "\\u002b")
                            .replace('=', "\\u003d")
                    )),
                };
            }
            if !state.access.contains_key(&token) {
                return Reply::status(401);
            }
            if let Some(status) = state.script.notes_status {
                return Reply::status(status);
            }
            Reply::json(
                &serde_json::json!([
                    { "title": "Welcome", "private": false },
                    { "title": "Ada's own note", "private": true }
                ])
                .to_string(),
            )
        });
    }
}

/// Issues an access and a refresh token for `account`.
fn issue(state: &mut State, account: &str) -> Reply {
    let access = if state.script.bad_token {
        format!("{} with space", random("at"))
    } else {
        random("at")
    };
    let refresh = random("rt");
    state.access.insert(access.clone(), account.to_owned());
    state.refresh.insert(refresh.clone(), account.to_owned());
    state.secrets.push(access.clone());
    state.secrets.push(refresh.clone());
    let mut answer = serde_json::json!({
        "access_token": access,
        "token_type": "Bearer",
        "refresh_token": refresh,
        "scope": "notes:read"
    });
    if let Some(expires_in) = state.script.expires_in {
        answer["expires_in"] = serde_json::json!(expires_in);
    }
    Reply::json(&answer.to_string())
}

fn refusal(code: &str, description: &str) -> Reply {
    Reply::status(400).body(
        serde_json::json!({ "error": code, "error_description": description })
            .to_string()
            .into_bytes(),
    )
}

fn bearer(request: &Received) -> Option<String> {
    request
        .header("authorization")
        .and_then(|value| value.strip_prefix("Bearer "))
        .map(str::to_owned)
}

/// The base64url, unpadded, of the SHA-256 of `verifier` (RFC 7636, S256).
pub fn s256(verifier: &str) -> String {
    base64url(&Sha256::digest(verifier.as_bytes()))
}

fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut text = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .fold(0u32, |n, byte| (n << 8) | u32::from(*byte))
            << (8 * (3 - chunk.len()));
        for index in 0..=chunk.len() {
            text.push(ALPHABET[(n >> (18 - 6 * index)) as usize & 63] as char);
        }
    }
    text
}

/// A fresh random value for `kind`, in the bearer-token set, unlike any other.
fn random(kind: &str) -> String {
    use ring::rand::{SecureRandom, SystemRandom};
    let mut bytes = [0u8; 18];
    SystemRandom::new()
        .fill(&mut bytes)
        .expect("the system's random source");
    format!("{kind}_{}", base64url(&bytes))
}

/// Fails, naming where, when any of `secrets` appears in a file under any of `dirs` or in any
/// of `outputs` (FR-019's scan, research R15).
#[track_caller]
pub fn scan(dirs: &[&Path], outputs: &[(&str, &str)], secrets: &[String]) {
    let secrets: Vec<&String> = secrets.iter().filter(|secret| secret.len() >= 8).collect();
    assert!(!secrets.is_empty(), "the scan has nothing to look for");
    for dir in dirs {
        for file in files(dir) {
            let bytes = std::fs::read(&file).unwrap_or_default();
            for secret in &secrets {
                assert!(
                    memchr_find(&bytes, secret.as_bytes()).is_none(),
                    "{} holds a secret the test service issued ({secret})",
                    file.display()
                );
            }
        }
    }
    for (name, output) in outputs {
        for secret in &secrets {
            assert!(
                !output.contains(secret.as_str()),
                "{name} holds a secret ({secret})"
            );
        }
    }
}

fn files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = std::fs::read_dir(dir) else {
        if dir.is_file() {
            found.push(dir.to_owned());
        }
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(files(&path));
        } else {
            found.push(path);
        }
    }
    found
}

fn memchr_find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
