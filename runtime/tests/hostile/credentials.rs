//! Feature 005: the credential cases of `specs/005-plugin-credentials/data-model.md` (Hostile
//! cases, FR-019), the gate case of Phase 2 still open until this feature: an attempt to obtain
//! the token must be blocked.
//!
//! Every case runs in a [`Signed`]: the test sign-in service at `https://notes.test`, reached
//! through the seam, and a development runtime with the mock vault and `local/notes` in its
//! connectors directory. Each case ends with the scan for the service's secrets in the data
//! directory, the grant record and whatever the case printed or received (research R15), then
//! the well-behaved `private-notes` updates and renders normally, on the same runtime.

use std::path::PathBuf;

use runtime::accounts::{Account, Connected};
use runtime::{ErrorKind, UpdateReport, VaultFailure};
use serde_json::{Value, json};

use crate::common::servers::{Reply, Server, TestPki};
use crate::common::signin::{self, Echo, SignInService};
use crate::common::{World, answers, fixture, repack};

/// The development connector of `private-notes`, as shipped beside it.
const NOTES: &str = include_str!("../../../plugins/examples/private-notes/connectors/notes.json");

/// The test sign-in service and a development runtime that reaches it, with two more HTTPS
/// servers: `https://other.test`, and `bad.test`, whose certificate is for another name.
pub(crate) struct Signed {
    pub(crate) service: SignInService,
    pub(crate) world: World,
    pub(crate) other: Server,
    pub(crate) bad: Server,
    /// The seam, for another runtime that reaches the same servers.
    pub(crate) test: runtime::TestNetwork,
}

impl Signed {
    pub(crate) fn new() -> Signed {
        Signed::with_connector(notes())
    }

    /// With `connector` as `local/notes` instead of the one shipped.
    pub(crate) fn with_connector(connector: Value) -> Signed {
        // A sign-in that goes wrong ends the test in seconds, not in ten minutes
        let limits = runtime::Limits {
            sign_in_time_limit: std::time::Duration::from_secs(5),
            ..runtime::Limits::default()
        };
        Signed::with(connector, limits)
    }

    pub(crate) fn with(connector: Value, limits: runtime::Limits) -> Signed {
        let pki = TestPki::new();
        let service = SignInService::start(&pki);
        let other = Server::https(&pki.leaf(&["other.test"]));
        let bad = Server::https(&pki.leaf(&["wrong.test"]));
        let route = |host: &str, server: &Server| runtime::Route {
            host: host.to_owned(),
            port: 443,
            answers: vec![vec![server.address()]],
        };
        let test = runtime::TestNetwork {
            routes: vec![
                service.route(),
                route("other.test", &other),
                route("bad.test", &bad),
            ],
            public_for_tests: vec![service.server.address(), other.address(), bad.address()],
            root: pki.root(),
            catalog: Vec::new(),
        };
        let world = World::seam_with(limits, test.clone(), true);
        world.write_connector("notes", &connector);
        Signed {
            service,
            world,
            other,
            bad,
            test,
        }
    }

    /// The canonical path of `local/notes`'s file: the origin of its account.
    pub(crate) fn notes_origin(&self) -> String {
        let path: PathBuf = self.world.data.path().join("connectors").join("notes.json");
        std::fs::canonicalize(path)
            .unwrap()
            .to_string_lossy()
            .into_owned()
    }

    /// Signs in to `local/notes` as the test service's account.
    pub(crate) fn connect(&self) -> Account {
        match self
            .world
            .runtime
            .connect_account(
                "local/notes",
                Some("dashboard-dev"),
                &self.service.browser(),
            )
            .unwrap_or_else(|error| panic!("signing in: {error}"))
        {
            Connected::New(account) | Connected::Again(account) => account,
        }
    }

    /// The access token the service issued last.
    pub(crate) fn token(&self) -> String {
        self.service
            .secrets()
            .into_iter()
            .rev()
            .find(|secret| secret.starts_with("at_"))
            .expect("the service issued a token")
    }

    /// Approves and loads `package`, runs its update with `config`, and returns the report.
    pub(crate) fn update(&self, package: &[u8], config: &Value) -> UpdateReport {
        let plugin = self.world.approve_and_load(package);
        plugin.update(&config.to_string())
    }

    /// Fails when a secret of the service is in the data directory, the grant record, the
    /// request log or `outputs`.
    #[track_caller]
    pub(crate) fn scan(&self, outputs: &[(&str, String)]) {
        let log = format!("{:?}", self.world.runtime.read_requests(1000, None));
        let mut outputs: Vec<(&str, &str)> = outputs
            .iter()
            .map(|(name, text)| (*name, text.as_str()))
            .collect();
        outputs.push(("the request log", &log));
        signin::scan(&[self.world.data.path()], &outputs, &self.service.secrets());
    }

    /// The well-behaved `private-notes` updates and renders normally, on the same runtime.
    #[track_caller]
    pub(crate) fn assert_well_behaved(&self) {
        let package = fixture("private-notes");
        let plugin = self.world.approve_and_load(&package);
        let update = plugin.update("{}");
        let data = update
            .result
            .as_ref()
            .unwrap_or_else(|error| panic!("private-notes updates after a hostile case: {error}"));
        assert!(data["notes"].is_array(), "{data}");
        let render = plugin.render("{}");
        assert!(
            render.result.is_ok(),
            "private-notes renders after a hostile case: {render:?}"
        );
    }
}

/// `local/notes` as shipped.
pub(crate) fn notes() -> Value {
    serde_json::from_str(NOTES).unwrap()
}

/// `account-probe` packed as `id`, declaring `connectors`.
pub(crate) fn probe(id: &str, connectors: Value) -> Vec<u8> {
    let manifest = json!({
        "manifest_version": 1, "id": id, "version": "0.1.0", "author": "Dashboard contributors",
        "permissions": [], "connectors": connectors
    });
    repack(&fixture("account-probe"), &manifest.to_string())
}

/// `account-probe` using `local/notes` with its account, for `operations`.
pub(crate) fn notes_probe(operations: &[&str]) -> Vec<u8> {
    probe(
        "local/account-probe",
        json!([{ "id": "local/notes", "major": 1, "operations": operations, "required": true, "account": true }]),
    )
}

/// The probe's configuration for `calls`, each `(via, connector, operation)`.
pub(crate) fn calls(calls: &[(&str, &str, &str)]) -> Value {
    let calls: Vec<Value> = calls
        .iter()
        .map(|(via, connector, operation)| {
            json!({ "via": via, "connector": connector, "operation": operation })
        })
        .collect();
    json!({ "calls": calls })
}

/// The probe's answers in `report`.
#[track_caller]
pub(crate) fn probe_answers(report: &UpdateReport) -> Vec<Value> {
    answers(
        report
            .result
            .as_ref()
            .unwrap_or_else(|error| panic!("the probe's update: {error}")),
    )
}

/// `token-hunt`: a plugin that looks everywhere for the token finds nothing. The WASI build,
/// which would read the keychain's files and the host records, is refused before it runs; the
/// probe's `dump` shows everything a widget is given, and holds no secret.
#[test]
fn token_hunt() {
    let signed = Signed::new();
    signed.connect();
    let hunt = fixture("token-hunt");
    let error = match signed.world.runtime.load(
        &hunt,
        crate::common::TEST_ORIGIN,
        signed.world.grants.clone(),
    ) {
        Ok(_) => panic!("token-hunt was loaded, but it must be refused"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), ErrorKind::CapabilityRefused, "{error}");
    assert!(
        error
            .names()
            .iter()
            .any(|import| import.starts_with("wasi:")),
        "{:?}",
        error.names()
    );

    let probe = notes_probe(&["list-notes"]);
    // A call as the account first, so the credential was in the host's memory during the run
    let called = signed.update(&probe, &calls(&[("accounts", "local/notes", "list-notes")]));
    assert!(probe_answers(&called)[0].get("ok").is_some(), "{called:?}");
    let records: Vec<String> = ["accounts.json", "grants.json"]
        .iter()
        .map(|name| {
            signed
                .world
                .data
                .path()
                .join(name)
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    let dumped = signed.update(
        &probe,
        &json!({ "dump": true, "paths": records, "calls": [] }),
    );
    let dump = dumped.result.as_ref().unwrap().to_string();
    signed.scan(&[
        ("the error", format!("{error:?}")),
        ("the call", format!("{called:?}")),
        ("the dump", dump),
        (
            "the render",
            format!("{:?}", signed.world.load(&probe).render("{}")),
        ),
    ]);
    signed.assert_well_behaved();
}

/// The echo cases: the destination sends the credential back, in its body, in a declared
/// response header, in a redirect's `Location`, or in JSON with `/`, `+` and `=` escaped. Each
/// answers `failed(invalid-response)`, and nothing of the response reaches the widget.
#[test]
fn echo_body_header_location_and_json_escaped() {
    let mut connector = notes();
    // The header is declared, so only the echo check can hold it back
    connector["operations"]["list-notes"]["response"]["headers"] = json!(["ETag"]);
    for echo in [Echo::Body, Echo::Header, Echo::Location, Echo::JsonEscaped] {
        let signed = Signed::with_connector(connector.clone());
        signed.connect();
        signed.service.script(|script| script.echo = Some(echo));
        let report = signed.update(
            &notes_probe(&["list-notes"]),
            &calls(&[("accounts", "local/notes", "list-notes")]),
        );
        let answers = probe_answers(&report);
        assert_eq!(
            answers[0]["error"],
            json!("failed:invalid-response"),
            "{echo:?}: {answers:?}"
        );
        assert!(answers[0].get("ok").is_none(), "{echo:?}: {answers:?}");
        // The redirect's target is not logged
        let entries = signed.world.runtime.read_requests(100, None).unwrap();
        assert!(
            entries.iter().all(|entry| entry
                .path
                .as_deref()
                .is_none_or(|path| !path.contains("seen"))),
            "{echo:?}: {entries:?}"
        );
        signed.scan(&[("the update", format!("{report:?}"))]);
        signed.service.script(|script| script.echo = None);
        signed.assert_well_behaved();
    }
}

/// `widget-connect`: while nothing is connected, every call a widget can make answers
/// `account-not-connected` or `not-granted`, and none starts a sign-in.
#[test]
fn widget_connect() {
    let mut required = notes();
    required["credential"]["required"] = json!(true);
    let signed = Signed::with_connector(required);
    let report = signed.update(
        &notes_probe(&["list-notes", "get-me"]),
        &calls(&[
            ("accounts", "local/notes", "list-notes"),
            ("accounts", "local/notes", "get-me"),
            ("connectors", "local/notes", "list-notes"),
            ("connectors", "local/notes", "get-me"),
        ]),
    );
    let answers = probe_answers(&report);
    let errors: Vec<&Value> = answers.iter().map(|answer| &answer["error"]).collect();
    assert_eq!(
        errors,
        [
            "account-not-connected",
            "account-not-connected",
            "not-granted",
            "not-granted"
        ],
        "{answers:?}"
    );
    // Nothing reached the service, the sign-in address least of all, and nothing was connected
    assert!(signed.service.server.received().is_empty());
    assert!(signed.world.runtime.list_accounts().unwrap().is_empty());
    let entries = signed.world.runtime.read_requests(100, None).unwrap();
    assert!(
        entries.iter().all(|entry| entry.plugin != "host"),
        "{entries:?}"
    );
}

/// `copied-widget-id`: a file elsewhere claiming the approved probe's id is not granted, and no
/// request carries the credential.
#[test]
fn copied_widget_id() {
    let signed = Signed::new();
    signed.connect();
    let probe = notes_probe(&["list-notes"]);
    signed.world.approve(&probe);
    let before = signed.service.requests_to("/notes").len();
    match signed
        .world
        .try_load_at(&probe, "/elsewhere/account-probe.wasm")
    {
        Ok(plugin) => {
            let report =
                plugin.update(&calls(&[("accounts", "local/notes", "list-notes")]).to_string());
            let answers = probe_answers(&report);
            assert_eq!(answers[0]["error"], json!("not-granted"), "{answers:?}");
        }
        Err(error) => assert!(
            matches!(
                error.kind(),
                ErrorKind::NotGranted | ErrorKind::NeedsNewApproval
            ),
            "{error}"
        ),
    }
    let notes = signed.service.requests_to("/notes");
    assert!(
        notes[before..]
            .iter()
            .all(|request| request.header("authorization").is_none()),
        "{notes:?}"
    );
    signed.scan(&[]);
    signed.assert_well_behaved();
}

/// `secret_free_errors`: the keychain answering `BadEncoding(<token bytes>)`, a stored document
/// that does not parse and a token answer that does not parse give errors that hold no secret,
/// and so do the log and the widget's output.
#[test]
fn secret_free_errors() {
    let signed = Signed::new();
    signed.connect();
    let origin = signed.notes_origin();
    let probe = notes_probe(&["list-notes"]);
    let call = calls(&[("accounts", "local/notes", "list-notes")]);

    // The keychain answers with the token's own bytes in its error
    let token = signed.token();
    signed.world.runtime.test_vault_fail_next(
        "local/notes",
        &origin,
        VaultFailure::BadEncoding(token.clone().into_bytes()),
    );
    let bad_encoding = signed.update(&probe, &call);
    assert!(
        probe_answers(&bad_encoding)[0].get("error").is_some(),
        "{bad_encoding:?}"
    );

    // A stored document that is not JSON, holding a secret
    signed.connect();
    let token = signed.token();
    signed.world.runtime.test_vault_write_raw(
        "local/notes",
        &origin,
        format!("{{\"access\":\"{token}\" broken").into_bytes(),
    );
    let bad_document = signed.update(&probe, &call);
    assert!(
        probe_answers(&bad_document)[0].get("error").is_some(),
        "{bad_document:?}"
    );

    // A token answer cut short, after the tokens in it
    signed.service.script(|script| script.token_cut = true);
    let cut = signed.world.runtime.connect_account(
        "local/notes",
        Some("dashboard-dev"),
        &signed.service.browser(),
    );
    let error = match cut {
        Ok(_) => panic!("a token answer that does not parse connected an account"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), ErrorKind::SignInFailed, "{error}");
    signed.service.script(|script| script.token_cut = false);

    signed.scan(&[
        (
            "the update with a keychain error",
            format!("{bad_encoding:?}"),
        ),
        (
            "the update with a broken document",
            format!("{bad_document:?}"),
        ),
        ("the sign-in error", format!("{error} {error:?}")),
    ]);
    signed.connect();
    signed.assert_well_behaved();
}

/// `local/notes` with a second destination, `https://other.test`, which the credential is not
/// for, and the operation `list-other` on it.
fn notes_and_other() -> Value {
    let mut connector = notes();
    connector["destinations"]["other"] = json!("https://other.test");
    connector["operations"]["list-other"] = json!({
        "kind": "read", "method": "GET", "destination": "other", "path": "/other",
        "headers": { "Accept": "application/json" }, "parameters": {},
        "response": { "format": "json", "headers": [] }
    });
    connector
}

/// Every request `server` received carried no `Authorization`.
#[track_caller]
fn assert_no_credential(server: &Server) {
    let received = server.received();
    assert!(
        received
            .iter()
            .all(|request| request.header("authorization").is_none()),
        "{received:?}"
    );
}

/// `other-destination`: a call to the connector's destination the credential is not for is sent
/// without it, while one to its own destination carries it.
#[test]
fn other_destination() {
    let signed = Signed::with_connector(notes_and_other());
    signed.other.route("/other", Reply::json("[]"));
    signed.connect();
    let report = signed.update(
        &notes_probe(&["list-notes", "list-other"]),
        &calls(&[
            ("accounts", "local/notes", "list-notes"),
            ("accounts", "local/notes", "list-other"),
        ]),
    );
    let answers = probe_answers(&report);
    assert!(
        answers.iter().all(|answer| answer.get("ok").is_some()),
        "{answers:?}"
    );
    let token = signed.token();
    assert_eq!(
        signed.service.requests_to("/notes")[0].header("authorization"),
        Some(format!("Bearer {token}").as_str())
    );
    assert_eq!(signed.other.received().len(), 1);
    assert_no_credential(&signed.other);
    signed.scan(&[("the update", format!("{report:?}"))]);
    signed.assert_well_behaved();
}

/// `redirect-other-host`: a request that carries the credential, redirected to another host of
/// the connector, is followed without it.
#[test]
fn redirect_other_host() {
    let mut connector = notes_and_other();
    connector["operations"]["jump"] = json!({
        "kind": "read", "method": "GET", "destination": "api", "path": "/jump",
        "headers": { "Accept": "application/json" }, "parameters": {},
        "response": { "format": "json", "headers": [] }
    });
    let signed = Signed::with_connector(connector);
    signed
        .service
        .server
        .route("/jump", Reply::redirect(302, "https://other.test/landing"));
    signed.other.route("/landing", Reply::json("[]"));
    signed.connect();
    let report = signed.update(
        &notes_probe(&["jump"]),
        &calls(&[("accounts", "local/notes", "jump")]),
    );
    let answers = probe_answers(&report);
    assert!(answers[0].get("ok").is_some(), "{answers:?}");
    assert!(
        signed.service.requests_to("/jump")[0]
            .header("authorization")
            .is_some()
    );
    assert_eq!(signed.other.received().len(), 1);
    assert_no_credential(&signed.other);
    signed.scan(&[("the update", format!("{report:?}"))]);
    signed.assert_well_behaved();
}

/// `header-smuggling`: values with line breaks, `Authorization:` text, the scheme or the header's
/// name are refused by their type or sent as data: no header is added, and none is replaced.
#[test]
fn header_smuggling() {
    let mut connector = notes();
    connector["operations"]["find"] = json!({
        "kind": "read", "method": "GET", "destination": "api", "path": "/notes",
        "query": { "q": "{q}" },
        "headers": { "Accept": "application/json" },
        "parameters": { "q": { "type": "text", "max_length": 200, "required": true } },
        "response": { "format": "json", "headers": [] }
    });
    let signed = Signed::with_connector(connector);
    signed.connect();
    let values = [
        "x\r\nAuthorization: Bearer evil",
        "x\nX-Evil: 1",
        "Authorization: Bearer evil",
        "Bearer",
        "Authorization",
        "{credential}",
        "Bearer {token}",
    ];
    let config: Vec<Value> = values
        .iter()
        .map(|value| {
            json!({ "via": "accounts", "connector": "local/notes", "operation": "find",
                    "arguments": [["q", value]] })
        })
        .collect();
    let report = signed.update(&notes_probe(&["find"]), &json!({ "calls": config }));
    let answers = probe_answers(&report);
    let token = signed.token();
    let sent = signed.service.requests_to("/notes");
    for (value, answer) in values.iter().zip(&answers) {
        assert!(
            answer.get("ok").is_some() || answer["error"] == json!("refused:value-outside-type"),
            "{value:?}: {answer}"
        );
    }
    for request in &sent {
        let credentials: Vec<&(String, String)> = request
            .headers
            .iter()
            .filter(|(name, _)| name.eq_ignore_ascii_case("authorization"))
            .collect();
        assert_eq!(credentials.len(), 1, "{request:?}");
        assert_eq!(credentials[0].1, format!("Bearer {token}"), "{request:?}");
        assert!(request.header("x-evil").is_none(), "{request:?}");
        let q: Vec<String> =
            url::form_urlencoded::parse(request.query.as_deref().unwrap_or_default().as_bytes())
                .filter(|(name, _)| name == "q")
                .map(|(_, value)| value.into_owned())
                .collect();
        assert!(
            q.len() == 1 && values.contains(&q[0].as_str()),
            "sent as data, unchanged: {request:?}"
        );
    }
    signed.scan(&[("the update", format!("{report:?}"))]);
    signed.assert_well_behaved();
}

/// The connectors of `plugins/hostile/connectors/` refused at load, each with its kind, before
/// anything is sent: `fixed-auth-header`, `plain-http-credential`, `plain-http-beside`,
/// `token-host-elsewhere` and `identity-elsewhere`.
#[test]
fn hostile_connectors_are_refused_at_load() {
    let signed = Signed::new();
    for (name, kind) in [
        ("fixed-auth-header", ErrorKind::InvalidConnector),
        ("plain-http-key", ErrorKind::CredentialNeedsHttps),
        ("plain-http-beside", ErrorKind::CredentialNeedsHttps),
        ("token-host-elsewhere", ErrorKind::InvalidConnector),
        ("identity-elsewhere", ErrorKind::InvalidConnector),
    ] {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("../plugins/hostile/connectors/{name}.json"));
        let connector: Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        signed.world.write_connector(name, &connector);
        let package = probe(
            "local/hostile-probe",
            json!([{ "id": format!("local/{name}"), "major": 1, "operations": ["list-notes"], "required": true, "account": true }]),
        );
        let error = match signed.world.runtime.check(&package) {
            Ok(_) => panic!("{name} was accepted"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), kind, "{name}: {error}");
        let error = signed
            .world
            .runtime
            .connect_key(&format!("local/{name}"), "k3y")
            .expect_err(name);
        assert_eq!(error.kind(), kind, "{name}: {error}");
    }
    assert!(signed.service.server.received().is_empty());
    assert!(signed.other.received().is_empty());
    signed.assert_well_behaved();
}

/// `bad-certificate`: a destination whose certificate is not valid for it gets nothing, the
/// credential least of all.
#[test]
fn bad_certificate() {
    let signed = Signed::new();
    let mut keyed = notes();
    keyed["id"] = json!("local/bad");
    keyed["destinations"]["api"] = json!("https://bad.test");
    keyed["credential"] = json!({
        "kind": "key", "required": true, "destinations": ["api"],
        "header": "Authorization", "scheme": "Bearer"
    });
    signed.world.write_connector("bad", &keyed);
    signed
        .world
        .runtime
        .connect_key("local/bad", "k3y-bad-cert")
        .unwrap();
    let report = signed.update(
        &probe(
            "local/bad-probe",
            json!([{ "id": "local/bad", "major": 1, "operations": ["list-notes"], "required": true, "account": true }]),
        ),
        &calls(&[("accounts", "local/bad", "list-notes")]),
    );
    let answers = probe_answers(&report);
    assert_eq!(
        answers[0]["error"],
        json!("failed:unavailable"),
        "{answers:?}"
    );
    assert!(signed.bad.received().is_empty());
    assert!(!format!("{report:?}").contains("k3y-bad-cert"));
    signed.assert_well_behaved();
}

/// `wrong-interface`: an account connector called through `connectors`, and a plain one
/// through `accounts`, are both `not-granted`; nothing is sent.
#[test]
fn wrong_interface() {
    let signed = Signed::new();
    signed.connect();
    let mut plain = notes();
    plain["id"] = json!("local/plain");
    plain.as_object_mut().unwrap().remove("credential");
    signed.world.write_connector("plain", &plain);
    let package = probe(
        "local/wrong-probe",
        json!([
            { "id": "local/notes", "major": 1, "operations": ["list-notes"], "required": true, "account": true },
            { "id": "local/plain", "major": 1, "operations": ["list-notes"], "required": true }
        ]),
    );
    let before = signed.service.requests_to("/notes").len();
    let report = signed.update(
        &package,
        &calls(&[
            ("connectors", "local/notes", "list-notes"),
            ("accounts", "local/plain", "list-notes"),
        ]),
    );
    let answers = probe_answers(&report);
    assert_eq!(answers[0]["error"], json!("not-granted"), "{answers:?}");
    assert_eq!(answers[1]["error"], json!("not-granted"), "{answers:?}");
    assert_eq!(signed.service.requests_to("/notes").len(), before);
    signed.scan(&[("the update", format!("{report:?}"))]);
    signed.assert_well_behaved();
}

/// `no-account-use`: a widget without `account: true` sends no credential, even while an account
/// is connected.
#[test]
fn no_account_use() {
    let signed = Signed::new();
    signed.connect();
    let mut other = notes();
    other["id"] = json!("local/other");
    signed.world.write_connector("other", &other);
    let package = probe(
        "local/plain-probe",
        json!([
            { "id": "local/notes", "major": 1, "operations": ["list-notes"], "required": true },
            { "id": "local/other", "major": 1, "operations": ["list-notes"], "required": true, "account": true }
        ]),
    );
    let report = signed.update(
        &package,
        &calls(&[("connectors", "local/notes", "list-notes")]),
    );
    assert!(probe_answers(&report)[0].get("ok").is_some(), "{report:?}");
    // The sign-in's own identity call carried it; the widget's call did not
    let notes = signed.service.requests_to("/notes");
    assert_eq!(notes.len(), 1);
    assert!(notes[0].header("authorization").is_none(), "{notes:?}");
    signed.scan(&[("the update", format!("{report:?}"))]);
    signed.assert_well_behaved();
}

/// `optional-off`: an account connector that is optional and turned off is `not-granted`, and
/// nothing is sent.
#[test]
fn optional_off() {
    let signed = Signed::new();
    signed.connect();
    let package = probe(
        "local/optional-probe",
        json!([{ "id": "local/notes", "major": 1, "operations": ["list-notes"], "required": false, "account": true }]),
    );
    signed.world.approve(&package);
    let checked = signed.world.runtime.check(&package).unwrap();
    signed
        .world
        .grants
        .set_connector(
            checked.manifest(),
            crate::common::TEST_ORIGIN,
            "local/notes",
            false,
        )
        .unwrap();
    let before = signed.service.server.received().len();
    let report = signed
        .world
        .load(&package)
        .update(&calls(&[("accounts", "local/notes", "list-notes")]).to_string());
    assert_eq!(
        probe_answers(&report)[0]["error"],
        json!("not-granted"),
        "{report:?}"
    );
    assert_eq!(signed.service.server.received().len(), before);
    signed.assert_well_behaved();
}

/// `copied-connector-id`: a connector file elsewhere that claims the id of one with an account
/// has no account at its origin.
#[test]
fn copied_connector_id() {
    let signed = Signed::new();
    signed.connect();
    let elsewhere = tempfile::TempDir::new().unwrap();
    std::fs::write(elsewhere.path().join("notes.json"), notes().to_string()).unwrap();
    let other = runtime::Runtime::with_test_network(
        runtime::Limits::default(),
        runtime::RuntimeOptions::new(signed.world.data.path())
            .development(true)
            .connectors_dir(elsewhere.path()),
        signed.test.clone(),
    )
    .unwrap();
    let package = fixture("private-notes");
    let checked = other.check(&package).unwrap();
    let digest = checked.approval_digest();
    checked
        .approve(
            &signed.world.grants,
            crate::common::TEST_ORIGIN,
            Some(&digest),
        )
        .unwrap();
    let plugin = other
        .load(
            &package,
            crate::common::TEST_ORIGIN,
            signed.world.grants.clone(),
        )
        .unwrap();
    let before = signed.service.requests_to("/notes").len();
    let update = plugin.update("{}");
    assert!(update.result.is_ok(), "{update:?}");
    let notes = signed.service.requests_to("/notes");
    assert!(notes.len() > before);
    assert!(
        notes[before..]
            .iter()
            .all(|request| request.header("authorization").is_none()),
        "{notes:?}"
    );
    signed.scan(&[("the update", format!("{update:?}"))]);
    signed.assert_well_behaved();
}

/// The only account connected, as the record has it.
#[track_caller]
fn the_account(signed: &Signed) -> Account {
    let mut accounts = signed.world.runtime.list_accounts().unwrap();
    assert_eq!(accounts.len(), 1, "{accounts:?}");
    accounts.remove(0)
}

/// The earlier account is still connected and still works: a call carries its token.
#[track_caller]
fn assert_kept(signed: &Signed, earlier: &Account, token: &str) {
    let account = the_account(signed);
    assert_eq!(account.account_id, earlier.account_id);
    assert_eq!(account.name, earlier.name);
    assert!(account.needs_connecting.is_none(), "{account:?}");
    let report = signed.update(
        &notes_probe(&["list-notes"]),
        &calls(&[("accounts", "local/notes", "list-notes")]),
    );
    assert!(probe_answers(&report)[0].get("ok").is_some(), "{report:?}");
    assert_eq!(
        signed
            .service
            .requests_to("/notes")
            .last()
            .unwrap()
            .header("authorization"),
        Some(format!("Bearer {token}").as_str())
    );
}

/// Signs in again with the service scripted as `script` says; the sign-in must fail as `kind`.
#[track_caller]
fn failed_sign_in(
    signed: &Signed,
    script: impl FnOnce(&mut signin::Script),
    kind: ErrorKind,
) -> runtime::RuntimeError {
    signed.service.script(script);
    let error = match signed.world.runtime.connect_account(
        "local/notes",
        Some("dashboard-dev"),
        &signed.service.browser(),
    ) {
        Ok(connected) => panic!("the sign-in connected {connected:?}"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), kind, "{error}");
    signed
        .service
        .script(|script| *script = signin::Script::default());
    error
}

/// `forged-callback`: a callback with a wrong state, none, or an error ends the sign-in;
/// nothing is connected and the earlier account is kept.
#[test]
fn forged_callback() {
    let signed = Signed::new();
    let earlier = signed.connect();
    let token = signed.token();
    let mut errors = Vec::new();
    for callback in [
        signin::Callback::WrongState,
        signin::Callback::NoState,
        signin::Callback::Error,
    ] {
        let exchanges = signed.service.requests_to("/oauth/token").len();
        errors.push(failed_sign_in(
            &signed,
            |script| script.callback = callback,
            ErrorKind::SignInFailed,
        ));
        // Nothing was exchanged for the forged callback's code
        assert_eq!(
            signed.service.requests_to("/oauth/token").len(),
            exchanges,
            "{callback:?}"
        );
    }
    assert_kept(&signed, &earlier, &token);
    signed.scan(&[("the errors", format!("{errors:?}"))]);
    signed.assert_well_behaved();
}

/// `replayed-code`: a callback with a code already exchanged connects nothing.
#[test]
fn replayed_code() {
    let signed = Signed::new();
    let earlier = signed.connect();
    let token = signed.token();
    let error = failed_sign_in(
        &signed,
        |script| script.callback = signin::Callback::UsedCode,
        ErrorKind::SignInFailed,
    );
    assert_kept(&signed, &earlier, &token);
    signed.scan(&[("the error", format!("{error} {error:?}"))]);
    signed.assert_well_behaved();
}

/// `late-callback`: a callback after the sign-in's time limit finds the listener closed.
#[test]
fn late_callback() {
    let limits = runtime::Limits {
        sign_in_time_limit: std::time::Duration::from_secs(1),
        ..runtime::Limits::default()
    };
    let signed = Signed::with(notes(), limits);
    let browser = std::sync::Arc::new(signed.service.browser());
    let late = std::sync::Mutex::new(None);
    let error = signed
        .world
        .runtime
        .connect_account(
            "local/notes",
            Some("dashboard-dev"),
            &|address: &url::Url| {
                let browser = std::sync::Arc::clone(&browser);
                let address = address.clone();
                *late.lock().unwrap() = Some(std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(1500));
                    browser(&address);
                }));
            },
        )
        .expect_err("a sign-in past its time limit");
    assert_eq!(error.kind(), ErrorKind::SignInFailed, "{error}");
    late.lock().unwrap().take().unwrap().join().unwrap();
    // The person's browser came too late: no code was exchanged, nothing connected
    assert!(signed.service.requests_to("/oauth/token").is_empty());
    assert!(signed.world.runtime.list_accounts().unwrap().is_empty());
    signed.assert_well_behaved();
}

/// `another-account`: a sign-in that returns another account fails as
/// `another-account-connected`; its token is revoked and dropped, and the earlier account kept.
#[test]
fn another_account() {
    let signed = Signed::new();
    let earlier = signed.connect();
    let token = signed.token();
    let error = failed_sign_in(
        &signed,
        |script| script.other_account = Some(("7654321".to_owned(), "bob".to_owned())),
        ErrorKind::AnotherAccountConnected,
    );
    let other_token = signed.token();
    assert_ne!(other_token, token);
    let revoked = signed.service.revoked();
    assert!(
        !revoked.is_empty(),
        "the other account's token was not revoked"
    );
    assert!(!revoked.contains(&token), "{revoked:?}");
    assert_kept(&signed, &earlier, &token);
    signed.scan(&[("the error", format!("{error} {error:?}"))]);
    signed.assert_well_behaved();
}

/// `revoked-token`: the service answers `401` and refuses the identity call too: one refresh,
/// one resend, one identity call, then `account-not-connected` and the account marked; no
/// further request.
#[test]
fn revoked_token() {
    let signed = Signed::new();
    signed.connect();
    signed.service.script(|script| {
        script.notes_status = Some(401);
        script.identity_refused = true;
    });
    let before = signed.service.server.received().len();
    let probe = notes_probe(&["list-notes"]);
    let call = calls(&[("accounts", "local/notes", "list-notes")]);
    let report = signed.update(&probe, &call);
    assert_eq!(
        probe_answers(&report)[0]["error"],
        json!("account-not-connected"),
        "{report:?}"
    );
    let paths: Vec<String> = signed.service.server.received()[before..]
        .iter()
        .map(|request| request.path.clone())
        .collect();
    assert_eq!(paths, ["/notes", "/oauth/token", "/notes", "/me"]);
    let account = the_account(&signed);
    assert_eq!(
        account.needs_connecting.map(|needs| needs.reason),
        Some(runtime::accounts::Reason::Refused)
    );
    // Marked: the next run answers at once, and nothing more reaches the service
    let after = signed.service.server.received().len();
    let again = signed.update(&probe, &call);
    assert_eq!(
        probe_answers(&again)[0]["error"],
        json!("account-not-connected"),
        "{again:?}"
    );
    assert_eq!(signed.service.server.received().len(), after);
    signed.scan(&[("the updates", format!("{report:?} {again:?}"))]);
    signed
        .service
        .script(|script| *script = signin::Script::default());
    signed.assert_well_behaved();
}

/// `foreign-401`: one operation answers `401` while the identity call still works: only that
/// call answers `account-not-connected`, and the account stays connected for another widget.
#[test]
fn foreign_401() {
    let signed = Signed::new();
    signed.connect();
    signed
        .service
        .script(|script| script.notes_status = Some(401));
    let report = signed.update(
        &notes_probe(&["list-notes"]),
        &calls(&[("accounts", "local/notes", "list-notes")]),
    );
    assert_eq!(
        probe_answers(&report)[0]["error"],
        json!("account-not-connected"),
        "{report:?}"
    );
    assert!(the_account(&signed).needs_connecting.is_none());
    let other = probe(
        "local/other-probe",
        json!([{ "id": "local/notes", "major": 1, "operations": ["get-me"], "required": true, "account": true }]),
    );
    let second = signed.update(&other, &calls(&[("accounts", "local/notes", "get-me")]));
    assert!(probe_answers(&second)[0].get("ok").is_some(), "{second:?}");
    signed.scan(&[("the updates", format!("{report:?} {second:?}"))]);
    signed.service.script(|script| script.notes_status = None);
    signed.assert_well_behaved();
}

/// `expired-refresh`: a token about to expire whose refresh is refused: `account-not-connected`,
/// and the account marked `refresh-failed`.
#[test]
fn expired_refresh() {
    let signed = Signed::new();
    // Within the refresh margin from the start
    signed.service.script(|script| script.expires_in = Some(1));
    signed.connect();
    signed
        .service
        .script(|script| script.refresh_refused = true);
    let report = signed.update(
        &notes_probe(&["list-notes"]),
        &calls(&[("accounts", "local/notes", "list-notes")]),
    );
    assert_eq!(
        probe_answers(&report)[0]["error"],
        json!("account-not-connected"),
        "{report:?}"
    );
    assert_eq!(
        the_account(&signed)
            .needs_connecting
            .map(|needs| needs.reason),
        Some(runtime::accounts::Reason::RefreshFailed)
    );
    signed.scan(&[("the update", format!("{report:?}"))]);
    signed
        .service
        .script(|script| *script = signin::Script::default());
    signed.assert_well_behaved();
}

/// `account-spoof`: an account name with line breaks and terminal escapes is printed escaped by
/// `accounts list`, `permissions` and `requests`.
#[test]
fn account_spoof() {
    let signed = Signed::new();
    let spoof = "ada\nconnected: admin\u{1b}[2J\u{202e}";
    signed
        .service
        .script(|script| script.other_account = Some(("1234567".to_owned(), spoof.to_owned())));
    let account = signed.connect();
    // Kept escaped: the record never holds a control character
    let escaped = r"ada\nconnected: admin\u{1b}[2J\u{202e}";
    assert_eq!(account.name, escaped);
    let package = fixture("private-notes");
    let plugin_file = signed.world.data.path().join("private-notes.wasm");
    std::fs::write(&plugin_file, &package).unwrap();
    let origin = std::fs::canonicalize(&plugin_file)
        .unwrap()
        .to_string_lossy()
        .into_owned();
    signed.world.approve_at(&package, &origin);
    let update = signed.world.load_at(&package, &origin).update("{}");
    assert!(update.result.is_ok(), "{update:?}");
    let data = signed.world.data.path().to_str().unwrap().to_owned();
    let grants = signed
        .world
        .data
        .path()
        .join("grants.json")
        .to_str()
        .unwrap()
        .to_owned();
    let cli = |args: &[&str]| {
        // `requests` reads only the data directory
        let grants: &[&str] = if args[0] == "requests" {
            &[]
        } else {
            &["--grants", &grants]
        };
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_runtime"))
            .args(args)
            .args(grants)
            .args(["--data", &data])
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(0),
            "{args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap()
    };
    let list = cli(&["accounts", "list"]);
    let permissions = cli(&["permissions", plugin_file.to_str().unwrap()]);
    let requests = cli(&["requests"]);
    for (name, text) in [
        ("accounts list", &list),
        ("permissions", &permissions),
        ("requests", &requests),
    ] {
        assert!(text.contains(escaped), "{name}: {text}");
        assert!(
            !text.contains('\u{1b}')
                && !text.contains('\u{202e}')
                && !text.contains("\nconnected: admin"),
            "{name}: {text}"
        );
    }
    signed.scan(&[
        ("accounts list", list),
        ("permissions", permissions),
        ("requests", requests),
    ]);
    signed.assert_well_behaved();
}
