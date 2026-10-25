//! Connecting an account: the authorization code flow with PKCE on a loopback callback, and keys
//! typed by the maintainer (`specs/005-plugin-credentials/research.md`, R2; `data-model.md`,
//! Sign-in). Also what a token answer holds, for the sign-in and for refreshing a token.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::thread;
use std::time::Instant;

use serde::Deserialize;
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::accounts::{AccountKind, AccountStore, Client, Connected, NewAccount};
use crate::calls::{self, now_ms};
use crate::connector::{Connector, CredentialKind};
use crate::error::{ErrorKind, RuntimeError};
use crate::limits::Limits;
use crate::network::{self, RequestCredential, Session, is_bearer_token};
use crate::requests::RequestLog;
use crate::text;
use crate::vault::{Secret, Stored, Vault, entry_user};

/// What connecting needs from the runtime.
pub(crate) struct Context<'a> {
    pub(crate) accounts: AccountStore,
    pub(crate) vault: &'a Arc<Vault>,
    pub(crate) session: Session,
    pub(crate) log: RequestLog,
    pub(crate) limits: &'a Limits,
}

/// Connects an account to `connector` from `origin` (`data-model.md`, Sign-in, steps 1–8):
/// the client registration, the listener, the sign-in address opened with `open`, the callback,
/// the code exchanged, the identity learned, the comparison with the account connected, then the
/// vault and the record. Nothing is written before the last step.
pub(crate) fn connect(
    context: &Context<'_>,
    connector: &Connector,
    origin: &str,
    client: Option<&str>,
    open: &(dyn Fn(&url::Url) + Sync),
) -> Result<Connected, RuntimeError> {
    let id = connector.id();
    let failed = |why: String| RuntimeError::new(ErrorKind::SignInFailed, format!("{id}: {why}"));
    // Step 1
    let declared = connector.credential().ok_or_else(|| {
        RuntimeError::new(ErrorKind::InvalidConnector, format!("{id} declares no credential"))
    })?;
    let Some(account) = declared.account() else {
        return Err(RuntimeError::new(
            ErrorKind::InvalidConnector,
            format!("{id} takes a key, which is typed, not signed in to"),
        ));
    };
    let client = match client {
        Some(client) => client.to_owned(),
        None => context
            .accounts
            .get(id, origin)?
            .and_then(|account| account.client)
            .map(|client| client.client_id)
            .ok_or_else(|| {
                RuntimeError::new(
                    ErrorKind::NoClientConfigured,
                    format!(
                        "{id} needs the client id of an app registered at its service; give it with --client <id>"
                    ),
                )
            })?,
    };
    if client.is_empty() || client.chars().count() > 100 || client.chars().any(|c| !c.is_ascii_graphic()) {
        return Err(RuntimeError::new(
            ErrorKind::NoClientConfigured,
            "a client id is 1 to 100 visible ASCII characters",
        ));
    }
    // Step 2
    let callback = network::Callback::bind()
        .map_err(|error| failed(format!("the loopback listener cannot start: {error}")))?;
    let redirect_uri = callback.redirect_uri();
    let verifier = random_token()?;
    let state = random_token()?;
    let challenge = s256(&verifier);
    // Step 3
    let mut address = account.authorize().clone();
    {
        let mut query = address.query_pairs_mut();
        query
            .append_pair("response_type", "code")
            .append_pair("client_id", &client)
            .append_pair("redirect_uri", &redirect_uri);
        if !account.scopes().is_empty() {
            query.append_pair("scope", &account.scopes().join(" "));
        }
        query
            .append_pair("state", &state)
            .append_pair("code_challenge", &challenge)
            .append_pair("code_challenge_method", "S256");
    }
    let deadline = Instant::now() + context.limits.sign_in_time_limit;
    let (max_head, read_time) = (
        context.limits.max_callback_head_bytes,
        context.limits.callback_read_time,
    );
    // The listener waits on its own thread, so a browser that answers at once finds it there
    let waiting = thread::spawn(move || callback.wait(deadline, max_head, read_time));
    open(&address);
    // Step 4
    let query = waiting
        .join()
        .map_err(|_| failed("the loopback listener stopped".to_owned()))?
        .map_err(|error| failed(format!("the loopback listener failed: {error}")))?
        .ok_or_else(|| {
            failed(format!(
                "no answer from the browser within {} min",
                context.limits.sign_in_time_limit.as_secs() / 60
            ))
        })?;
    let pairs: BTreeMap<String, Zeroizing<String>> = url::form_urlencoded::parse(query.as_bytes())
        .map(|(name, value)| (name.into_owned(), Zeroizing::new(value.into_owned())))
        .collect();
    if let Some(error) = pairs.get("error") {
        return Err(failed(format!(
            "the service answered {}",
            text::untrusted(error, 100)
        )));
    }
    if pairs.get("state").map(|given| given.as_str()) != Some(state.as_str()) {
        return Err(failed(
            "the browser came back with a wrong or missing state; nothing was connected".to_owned(),
        ));
    }
    let Some(code) = pairs.get("code").filter(|code| !code.is_empty()) else {
        return Err(failed("the browser came back without a code".to_owned()));
    };
    // Step 5
    let started = now_ms();
    let answer = context.session.host_post(
        account.token(),
        &[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", &redirect_uri),
            ("client_id", &client),
            ("code_verifier", &verifier),
        ],
        Instant::now() + context.limits.call_time_limit,
        context.limits.max_sign_in_response_bytes,
    );
    let _ = context
        .log
        .append(calls::host_entry("", id, "token", account.token(), started, &answer));
    let answer = answer.map_err(|failure| failed(failure.detail))?;
    let tokens = tokens(answer.status, &answer.body, &[code, &verifier]).map_err(failed)?;
    // Step 6
    let (name, service_id) = identity(context, connector, &tokens.access)?;
    // Steps 7 and 8
    let now = now_ms();
    let new = NewAccount {
        connector: id.to_owned(),
        origin: origin.to_owned(),
        kind: AccountKind::Account,
        flow: Some("authorization-code".to_owned()),
        client: Some(Client {
            client_id: client.clone(),
            from: "option".to_owned(),
        }),
        name,
        service_id,
        access_asked: declared.access().to_vec(),
        access_reported: tokens.scopes.clone(),
        now_ms: now,
        expires_at_ms: tokens
            .expires_in_s
            .map(|seconds| now.saturating_add(seconds.saturating_mul(1000))),
    };
    let stored = Stored::Account {
        access: tokens.access.clone(),
        refresh: tokens.refresh.clone(),
    };
    let connected = context.accounts.connect(new, |_| {
        let vault_id = context.accounts.vault_id()?;
        context
            .vault
            .write(&entry_user(&vault_id, id, origin), &stored)
            .within(context.limits.vault_wait_tool)
            .map_err(|error| RuntimeError::new(ErrorKind::KeychainUnavailable, error.to_string()))
    });
    if let Err(error) = &connected
        && error.kind() == ErrorKind::AnotherAccountConnected
    {
        // The token just obtained goes back to the service when it takes it, then is dropped
        revoke(context, connector, &client, &tokens.access);
    }
    connected
}

/// The account's name and stable id, from the identity operation with the new credential, or
/// `account` and none when the connector declares none (research R3).
fn identity(
    context: &Context<'_>,
    connector: &Connector,
    access: &Secret,
) -> Result<(String, Option<String>), RuntimeError> {
    let id = connector.id();
    let failed = |why: String| RuntimeError::new(ErrorKind::SignInFailed, format!("{id}: {why}"));
    let declared = connector.credential().expect("a connector with an account");
    let Some(identity) = declared.account().and_then(|account| account.identity()) else {
        return Ok(("account".to_owned(), None));
    };
    let operation = connector
        .operation(identity.operation())
        .expect("a checked identity names an operation");
    let mut request = network::build_request(connector, operation, &BTreeMap::new(), context.limits)
        .map_err(|network::Refusal(why)| failed(why))?;
    request.credential = Some(RequestCredential::new(declared, access).map_err(|why| failed(why.to_owned()))?);
    let mut budget = context.limits.max_response_body_bytes;
    let exchange = context.session.send(
        connector,
        operation,
        request,
        Instant::now() + context.limits.call_time_limit,
        &mut budget,
    );
    let response = exchange
        .result
        .map_err(|failure| failed(format!("the identity operation failed: {}", failure.detail)))?;
    if response.status != 200 {
        return Err(failed(format!(
            "the identity operation answered {}",
            response.status
        )));
    }
    let document: serde_json::Value = serde_json::from_slice(&response.body)
        .map_err(|_| failed("the identity operation answered something other than JSON".to_owned()))?;
    let text_at = |pointer: &str| {
        document.pointer(pointer).and_then(|value| match value {
            serde_json::Value::String(text) => Some(text.clone()),
            serde_json::Value::Number(number) => Some(number.to_string()),
            _ => None,
        })
    };
    let name = text_at(identity.name())
        .ok_or_else(|| failed(format!("the identity has no name at {}", identity.name())))?;
    let service_id = text_at(identity.id())
        .ok_or_else(|| failed(format!("the identity has no id at {}", identity.id())))?;
    Ok((
        text::untrusted(&name, context.limits.max_account_name_chars * 4)
            .chars()
            .take(context.limits.max_account_name_chars)
            .collect(),
        Some(service_id.chars().take(context.limits.max_account_name_chars).collect()),
    ))
}

/// Asks the service to revoke `token` when the connector declares where (RFC 7009); whether it
/// did. A key, or a connector without `revoke`, revokes nothing.
pub(crate) fn revoke(context: &Context<'_>, connector: &Connector, client: &str, token: &Secret) -> Option<Result<(), String>> {
    let revoke = connector.credential()?.account()?.revoke()?;
    let started = now_ms();
    let answer = context.session.host_post(
        revoke,
        &[("token", token.expose()), ("client_id", client)],
        Instant::now() + context.limits.call_time_limit,
        context.limits.max_sign_in_response_bytes,
    );
    let _ = context
        .log
        .append(calls::host_entry("", connector.id(), "revoke", revoke, started, &answer));
    Some(match answer {
        Ok(answer) if (200..300).contains(&answer.status) => Ok(()),
        Ok(answer) => Err(format!("the service answered {}", answer.status)),
        Err(failure) => Err(failure.detail),
    })
}

/// Checks a typed key (FR-010): 1 to `max_bytes` bytes of the bearer-token set.
pub(crate) fn check_key(key: &str, max_bytes: usize) -> Result<(), RuntimeError> {
    if key.is_empty() || key.len() > max_bytes || !is_bearer_token(key) {
        return Err(RuntimeError::new(
            ErrorKind::KeyRefused,
            format!(
                "a key is 1 to {max_bytes} bytes of letters, digits and -._~+/=; this one is not (its {} bytes are not shown)",
                key.len()
            ),
        ));
    }
    Ok(())
}

/// Connects a key the maintainer typed to `connector` from `origin`.
pub(crate) fn connect_key(
    context: &Context<'_>,
    connector: &Connector,
    origin: &str,
    key: &str,
) -> Result<Connected, RuntimeError> {
    let id = connector.id();
    let declared = connector.credential().ok_or_else(|| {
        RuntimeError::new(ErrorKind::InvalidConnector, format!("{id} declares no credential"))
    })?;
    if declared.kind() != CredentialKind::Key {
        return Err(RuntimeError::new(
            ErrorKind::InvalidConnector,
            format!("{id} connects an account by signing in, not with a key"),
        ));
    }
    check_key(key, context.limits.max_key_bytes)?;
    let new = NewAccount {
        connector: id.to_owned(),
        origin: origin.to_owned(),
        kind: AccountKind::Key,
        flow: None,
        client: None,
        name: "key".to_owned(),
        service_id: None,
        access_asked: declared.access().to_vec(),
        access_reported: Vec::new(),
        now_ms: now_ms(),
        expires_at_ms: None,
    };
    let stored = Stored::Key {
        key: Secret::new(key.to_owned()),
    };
    context.accounts.connect(new, |_| {
        let vault_id = context.accounts.vault_id()?;
        context
            .vault
            .write(&entry_user(&vault_id, id, origin), &stored)
            .within(context.limits.vault_wait_tool)
            .map_err(|error| RuntimeError::new(ErrorKind::KeychainUnavailable, error.to_string()))
    })
}

/// 32 random bytes, base64url without padding: a PKCE verifier or a state (RFC 7636, §4.1).
fn random_token() -> Result<String, RuntimeError> {
    use ring::rand::{SecureRandom, SystemRandom};
    let mut bytes = Zeroizing::new([0u8; 32]);
    SystemRandom::new().fill(bytes.as_mut()).map_err(|_| {
        RuntimeError::new(ErrorKind::SignInFailed, "the system's random source is unavailable")
    })?;
    Ok(base64url(bytes.as_ref()))
}

/// The base64url, unpadded, of the SHA-256 of `verifier` (RFC 7636, S256).
pub(crate) fn s256(verifier: &str) -> String {
    base64url(&Sha256::digest(verifier.as_bytes()))
}

/// Base64 with the URL alphabet and no padding (RFC 4648, §5).
fn base64url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .fold(0u32, |n, byte| (n << 8) | u32::from(*byte))
            << (8 * (3 - chunk.len()));
        for index in 0..=chunk.len() {
            text.push(char::from(ALPHABET[(n >> (18 - 6 * index)) as usize & 63]));
        }
    }
    text
}

/// What a token answer gave: the tokens, when the access token expires, and the scopes the
/// service names.
#[derive(Debug)]
pub(crate) struct Tokens {
    pub(crate) access: Secret,
    pub(crate) refresh: Option<Secret>,
    pub(crate) expires_in_s: Option<u64>,
    pub(crate) scopes: Vec<String>,
}

/// The token answer as written (RFC 6749, §5.1 and §5.2). Every string moves into a [`Secret`]
/// or is checked at once; the parser's own errors are never shown, since they quote the value
/// they could not read.
#[derive(Deserialize)]
struct RawTokens {
    #[serde(default)]
    access_token: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: Option<u64>,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    error_description: Option<String>,
}

/// Longest error code or description of a service shown, in bytes, before it is cut.
const MAX_SERVICE_ERROR_BYTES: usize = 200;

/// Reads a token answer of `status` with `body`. A refusal names the service's error code and
/// description, escaped and cut, with any of `sent` (the code, verifier or refresh token the
/// request carried) taken out, so no secret of the exchange is ever shown.
pub(crate) fn tokens(status: u16, body: &[u8], sent: &[&str]) -> Result<Tokens, String> {
    let Ok(raw) = serde_json::from_slice::<RawTokens>(body) else {
        return Err(format!(
            "the service answered with status {status} and something that is not a token answer"
        ));
    };
    let RawTokens {
        access_token,
        token_type,
        refresh_token,
        expires_in,
        scope,
        error,
        error_description,
    } = raw;
    let access = access_token.map(Secret::new);
    let refresh = refresh_token.map(Secret::new);
    if let Some(code) = error {
        let mut shown = shown(&code, sent);
        if let Some(description) = error_description {
            shown.push_str(": ");
            shown.push_str(&self::shown(&description, sent));
        }
        return Err(format!("the service refused it ({shown})"));
    }
    let Some(access) = access.filter(|_| (200..300).contains(&status)) else {
        return Err(format!(
            "the service answered with status {status} and no access token"
        ));
    };
    if !token_type
        .as_deref()
        .is_some_and(|kind| kind.eq_ignore_ascii_case("bearer"))
    {
        return Err("the service gave a token that is not a bearer token".to_owned());
    }
    if !is_bearer_token(access.expose())
        || refresh
            .as_ref()
            .is_some_and(|refresh| !is_bearer_token(refresh.expose()))
    {
        return Err(
            "the service gave a token with characters outside letters, digits and -._~+/=, which the host does not keep"
                .to_owned(),
        );
    }
    Ok(Tokens {
        access,
        refresh,
        expires_in_s: expires_in,
        scopes: scope
            .map(|scope| scope.split_whitespace().map(str::to_owned).collect())
            .unwrap_or_default(),
    })
}

/// A service's text, without any secret the request carried, escaped and cut.
fn shown(text: &str, sent: &[&str]) -> String {
    let mut text = text.to_owned();
    for secret in sent.iter().filter(|secret| !secret.is_empty()) {
        text = text.replace(secret, "[secret]");
    }
    text::untrusted(&text, MAX_SERVICE_ERROR_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// pkce: the challenge of RFC 7636 appendix B's verifier.
    #[test]
    fn the_challenge_is_that_of_rfc_7636_appendix_b() {
        assert_eq!(
            s256("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
        assert_eq!(base64url(&[0xfb, 0xff]), "-_8");
        let verifier = random_token().unwrap();
        assert_eq!(verifier.len(), 43);
        assert!(is_bearer_token(&verifier));
        assert_ne!(verifier, random_token().unwrap());
    }

    /// key_checks: empty, too long and outside the bearer-token set are refused.
    #[test]
    fn a_key_is_checked_before_it_reaches_the_vault() {
        assert!(check_key("k3y-ok.~+/=", 1024).is_ok());
        assert!(check_key(&"a".repeat(1024), 1024).is_ok());
        for key in [String::new(), "a".repeat(1025), "has space".to_owned(), "é".to_owned(), "a\nb".to_owned()] {
            let error = check_key(&key, 1024).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::KeyRefused);
            assert!(key.is_empty() || !error.detail().contains(&key), "{error}");
        }
    }

    #[test]
    fn a_token_answer_gives_its_tokens_expiry_and_scopes() {
        let tokens = tokens(
            200,
            br#"{"access_token":"at-1","token_type":"Bearer","refresh_token":"rt-1","expires_in":3600,"scope":"notes:read  x"}"#,
            &[],
        )
        .unwrap();
        assert_eq!(tokens.access.expose(), "at-1");
        assert_eq!(tokens.refresh.as_ref().map(Secret::expose), Some("rt-1"));
        assert_eq!(tokens.expires_in_s, Some(3600));
        assert_eq!(tokens.scopes, ["notes:read", "x"]);
        let bare = super::tokens(200, br#"{"access_token":"a","token_type":"bearer"}"#, &[]).unwrap();
        assert!(bare.refresh.is_none() && bare.expires_in_s.is_none() && bare.scopes.is_empty());
    }

    #[test]
    fn a_refusal_names_the_service_error_without_any_secret() {
        let error = tokens(
            400,
            br#"{"error":"invalid_grant","error_description":"code c0de-123 is not valid\n\u001b[2Jerror: forged"}"#,
            &["c0de-123", "verifier-xyz"],
        )
        .unwrap_err();
        assert!(error.contains("invalid_grant"), "{error}");
        assert!(!error.contains("c0de-123") && error.contains("[secret]"), "{error}");
        assert!(!error.contains('\n') && !error.contains('\u{1b}'), "{error:?}");
    }

    #[test]
    fn anything_else_is_refused_with_fixed_text() {
        for (status, body) in [
            (200, &br#"{"access_token":"has space","token_type":"bearer"}"#[..]),
            (200, br#"{"access_token":"a","token_type":"bearer","refresh_token":"r\"x"}"#),
            (200, br#"{"access_token":"a","token_type":"mac"}"#),
            (200, br#"{"access_token":"a"}"#),
            (500, br#"{"access_token":"a","token_type":"bearer"}"#),
            (200, br#"{"token_type":"bearer"}"#),
            (200, b"<html>gho_leaked</html>"),
            (200, br#"{"access_token": 7}"#),
        ] {
            let error = tokens(status, body, &[]).unwrap_err();
            assert!(!error.contains("gho_leaked") && !error.contains("has space"), "{error}");
        }
    }
}
