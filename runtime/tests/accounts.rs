//! Accounts end to end through the library (`specs/005-plugin-credentials/`, User Stories 1, 3,
//! 4 and 5): sign-in against the test sign-in service, approval, calls as the account, refresh,
//! disconnecting. Every test reaches only the test service, through the seam, with the mock
//! vault, and ends with the scan for leaked secrets (research R15).

mod common;

use common::servers::TestPki;
use common::signin::{self, SignInService};
use common::{World, fixture, repack};
use runtime::accounts::Connected;
use serde_json::{Value, json};

/// The development connector of `private-notes`, as shipped beside it.
const NOTES: &str = include_str!("../../plugins/examples/private-notes/connectors/notes.json");

struct Setup {
    service: SignInService,
    world: World,
}

/// The test sign-in service and a development runtime that reaches it as `https://notes.test`,
/// with `local/notes` in its connectors directory.
fn setup() -> Setup {
    let pki = TestPki::new();
    let service = SignInService::start(&pki);
    let test = runtime::TestNetwork {
        routes: vec![service.route()],
        public_for_tests: vec![service.server.address()],
        root: pki.root(),
        catalog: Vec::new(),
    };
    let world = World::seam(test, true);
    world.write_connector("notes", &serde_json::from_str(NOTES).unwrap());
    Setup { service, world }
}

impl Setup {
    fn connect(&self) -> runtime::accounts::Account {
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

    /// Fails when a secret of the service is in the data directory, the grant record or `outputs`.
    fn scan(&self, outputs: &[(&str, String)]) {
        let outputs: Vec<(&str, &str)> = outputs
            .iter()
            .map(|(name, text)| (*name, text.as_str()))
            .collect();
        signin::scan(&[self.world.data.path()], &outputs, &self.service.secrets());
    }
}

/// The titles of the notes `content` shows.
fn titles(content: &Value) -> Vec<String> {
    content["items"]
        .as_array()
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item["title"].as_str().map(str::to_owned))
                .collect()
        })
        .unwrap_or_default()
}

/// `private_notes`: the example end to end (User Story 1, quickstart step 3).
#[test]
fn private_notes() {
    let setup = setup();
    let account = setup.connect();
    assert_eq!(account.name, "ada");
    assert_eq!(account.service_id.as_deref(), Some("1234567"));
    assert_eq!(account.access_reported, ["notes:read"]);

    let package = fixture("private-notes");
    let plugin = setup.world.approve_and_load(&package);
    let update = plugin.update("{}");
    let data = update
        .result
        .as_ref()
        .unwrap_or_else(|error| panic!("update: {error}"))
        .clone();
    assert_eq!(data["notes"].as_array().unwrap().len(), 2, "{data}");
    let render = plugin.render("{}");
    let content = render.result.as_ref().unwrap().clone();
    assert!(
        titles(&content).contains(&"Ada's own note".to_owned()),
        "{content}"
    );

    // The request log: the host's own token and identity requests, and the call as the account
    let entries = setup.world.runtime.read_requests(100, None).unwrap();
    let host: Vec<&str> = entries
        .iter()
        .filter(|entry| entry.plugin == "host")
        .map(|entry| entry.operation.as_str())
        .collect();
    assert_eq!(host, ["token", "identity"], "{entries:#?}");
    let call = entries
        .iter()
        .find(|entry| entry.plugin == "local/private-notes")
        .expect("the widget's call");
    assert_eq!(call.account.as_deref(), Some("ada"));
    let notes_request = setup.service.requests_to("/notes");
    assert!(
        notes_request[0]
            .header("authorization")
            .is_some_and(|value| value.starts_with("Bearer at_"))
    );

    // Disconnecting: the kept data read as the account goes at once, the token is revoked
    let gone = setup
        .world
        .runtime
        .disconnect_account("local/notes", None)
        .unwrap()
        .expect("an account was connected");
    assert_eq!(gone.kept_discarded, ["local/private-notes"]);
    assert_eq!(gone.revoked, Some(Ok(())));
    assert_eq!(setup.service.revoked().len(), 1);
    assert!(setup.world.runtime.list_accounts().unwrap().is_empty());

    // Loaded again, the optional credential is not sent and only the public notes come
    let plugin = setup.world.load(&package);
    let render = plugin.render("{}");
    assert_eq!(
        render.result.as_ref().unwrap()["text"],
        json!("No notes yet")
    );
    let update = plugin.update("{}");
    assert_eq!(
        update.result.as_ref().unwrap()["notes"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert!(
        setup
            .service
            .requests_to("/notes")
            .last()
            .unwrap()
            .header("authorization")
            .is_none()
    );

    setup.scan(&[
        ("updates and renders", format!("{update:?} {render:?}")),
        ("the request log", format!("{entries:?}")),
    ]);
}

/// A key typed by the maintainer is sent as `Authorization: Bearer <key>`.
#[test]
fn a_key_is_sent_as_its_connector_says() {
    let setup = setup();
    let mut keyed: Value = serde_json::from_str(NOTES).unwrap();
    keyed["id"] = json!("local/keyed");
    keyed["credential"] = json!({
        "kind": "key", "required": true, "destinations": ["api"],
        "header": "Authorization", "scheme": "Bearer"
    });
    setup.world.write_connector("keyed", &keyed);
    setup
        .world
        .runtime
        .connect_key("local/keyed", "k3y-123_abc")
        .unwrap();
    let manifest = json!({
        "manifest_version": 1, "id": "local/keyed-probe", "version": "0.1.0", "author": "A",
        "permissions": [],
        "connectors": [{ "id": "local/keyed", "major": 1, "operations": ["list-notes"], "required": true, "account": true }]
    });
    let package = repack(&fixture("account-probe"), &manifest.to_string());
    let plugin = setup.world.approve_and_load(&package);
    let update = plugin.update(
        &json!({ "calls": [{ "via": "accounts", "connector": "local/keyed", "operation": "list-notes" }] }).to_string(),
    );
    assert!(update.result.is_ok(), "{update:?}");
    assert_eq!(
        setup.service.requests_to("/notes")[0].header("authorization"),
        Some("Bearer k3y-123_abc")
    );
    let text = std::fs::read_to_string(setup.world.data.path().join("accounts.json")).unwrap();
    assert!(!text.contains("k3y-123_abc"));
}

/// A widget that does not say it uses the account sends none, even while one is connected.
#[test]
fn a_widget_without_account_use_sends_no_credential() {
    let setup = setup();
    setup.connect();
    // The probe imports accounts, so it declares a second connector used with its account
    let mut other: Value = serde_json::from_str(NOTES).unwrap();
    other["id"] = json!("local/other");
    setup.world.write_connector("other", &other);
    let manifest = json!({
        "manifest_version": 1, "id": "local/plain-probe", "version": "0.1.0", "author": "A",
        "permissions": [],
        "connectors": [
            { "id": "local/notes", "major": 1, "operations": ["list-notes"], "required": true },
            { "id": "local/other", "major": 1, "operations": ["list-notes"], "required": true, "account": true }
        ]
    });
    let package = repack(&fixture("account-probe"), &manifest.to_string());
    let plugin = setup.world.approve_and_load(&package);
    let update = plugin.update(
        &json!({ "calls": [
            { "via": "connectors", "connector": "local/notes", "operation": "list-notes" },
            { "via": "accounts", "connector": "local/notes", "operation": "list-notes" }
        ] })
        .to_string(),
    );
    let answers = common::answers(update.result.as_ref().unwrap());
    assert!(answers[0].get("ok").is_some(), "{answers:?}");
    assert_eq!(answers[1]["error"], json!("not-granted"));
    let notes = setup.service.requests_to("/notes");
    assert_eq!(notes.len(), 1);
    assert!(notes[0].header("authorization").is_none());
    setup.scan(&[("the update", format!("{update:?}"))]);
}

/// Whether an account is connected to `local/notes` in a combination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Absent,
    Connected,
    NeedsConnecting,
}

/// How the widget's approval and grant of `local/notes` stand in a combination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Grant {
    Required,
    OptionalOn,
    OptionalOff,
}

/// `credential_combinations` (SC-002, `data-model.md` Combinations): the credential reaches the
/// service exactly when the connector declares it, the widget says `account: true`, its approval
/// covers that use, the connector is granted, an account is connected, and the destination is
/// one the credential is for. Every other combination sends no `Authorization`.
#[test]
fn credential_combinations() {
    let mut checked = 0;
    for credential in [None, Some(false), Some(true)] {
        let states: &[State] = if credential.is_some() {
            &[State::Absent, State::Connected, State::NeedsConnecting]
        } else {
            &[State::Absent]
        };
        for &state in states {
            checked += combinations(credential, state);
        }
    }
    // 1 + 3 + 3 worlds of 2 × 2 × 3 widgets, each calling 2 destinations
    assert_eq!(checked, 7 * 12 * 2);
}

/// Runs every widget of one world: `local/notes` with `credential` (`Some(required)`) and its
/// account in `state`. Returns how many calls it checked.
fn combinations(credential: Option<bool>, state: State) -> usize {
    let pki = TestPki::new();
    let service = SignInService::start(&pki);
    let other = common::servers::Server::https(&pki.leaf(&["other.test"]));
    other.route("/other", common::servers::Reply::json("[]"));
    let test = runtime::TestNetwork {
        routes: vec![
            service.route(),
            runtime::Route {
                host: "other.test".to_owned(),
                port: 443,
                answers: vec![vec![other.address()]],
            },
        ],
        public_for_tests: vec![service.server.address(), other.address()],
        root: pki.root(),
        catalog: Vec::new(),
    };
    let world = World::seam(test, true);
    let mut notes: Value = serde_json::from_str(NOTES).unwrap();
    notes["destinations"]["other"] = json!("https://other.test");
    notes["operations"]["list-other"] = json!({
        "kind": "read", "method": "GET", "destination": "other", "path": "/other",
        "headers": { "Accept": "application/json" }, "parameters": {},
        "response": { "format": "json", "headers": [] }
    });
    // Never called: every widget declares it, so `account-probe`'s import of `accounts` is
    // declared whatever the widget says of `local/notes`
    let mut anchor = notes.clone();
    anchor["id"] = json!("local/anchor");
    world.write_connector("anchor", &anchor);
    match credential {
        None => {
            notes.as_object_mut().unwrap().remove("credential");
        }
        Some(required) => notes["credential"]["required"] = json!(required),
    }
    let path = world.write_connector("notes", &notes);
    let origin = std::fs::canonicalize(path)
        .unwrap()
        .to_string_lossy()
        .into_owned();
    if state != State::Absent {
        let connected = world
            .runtime
            .connect_account("local/notes", Some("dashboard-dev"), &service.browser())
            .unwrap_or_else(|error| panic!("signing in: {error}"));
        let (Connected::New(account) | Connected::Again(account)) = connected;
        if state == State::NeedsConnecting {
            runtime::accounts::AccountStore::open(world.data.path())
                .mark_needs_connecting(
                    "local/notes",
                    &origin,
                    &account.account_id,
                    runtime::accounts::Reason::Refused,
                    1,
                )
                .unwrap();
        }
    }
    let mut checked = 0;
    let mut number = 0;
    for account in [false, true] {
        for uses in [false, true] {
            for grant in [Grant::Required, Grant::OptionalOn, Grant::OptionalOff] {
                number += 1;
                let manifest = |account: bool| {
                    json!({
                        "manifest_version": 1, "id": format!("local/combination-{number}"),
                        "version": "0.1.0", "author": "A", "permissions": [],
                        "connectors": [
                            { "id": "local/notes", "major": 1, "operations": ["list-notes", "list-other"],
                              "required": grant == Grant::Required, "account": account },
                            { "id": "local/anchor", "major": 1, "operations": ["list-notes"],
                              "required": true, "account": true }
                        ]
                    })
                    .to_string()
                };
                // The approval covers the use `uses` says; the widget loaded says `account`
                let approved = repack(&fixture("account-probe"), &manifest(uses));
                let loaded = repack(&fixture("account-probe"), &manifest(account));
                let what = format!(
                    "credential {credential:?}, {state:?}, account {account}, uses {uses}, {grant:?}"
                );
                let checked_package = world.runtime.check(&approved);
                let Ok(checked_package) = checked_package else {
                    // `account: true` of a connector without a credential, or a required
                    // credential without it: refused, so nothing can be sent
                    assert!(
                        (credential.is_none() && uses) || (credential == Some(true) && !uses),
                        "{what}: refused at check"
                    );
                    checked += 2;
                    continue;
                };
                let digest = checked_package.approval_digest();
                checked_package
                    .approve(&world.grants, common::TEST_ORIGIN, Some(&digest))
                    .unwrap_or_else(|error| panic!("{what}: approving: {error}"));
                if grant != Grant::Required {
                    world
                        .grants
                        .set_connector(
                            checked_package.manifest(),
                            common::TEST_ORIGIN,
                            "local/notes",
                            grant == Grant::OptionalOn,
                        )
                        .unwrap_or_else(|error| panic!("{what}: setting the grant: {error}"));
                }
                let via = if account { "accounts" } else { "connectors" };
                let notes_before = service.requests_to("/notes").len();
                let other_before = other.received().len();
                if let Ok(plugin) = world.try_load_at(&loaded, common::TEST_ORIGIN) {
                    let config = json!({ "calls": [
                        { "via": via, "connector": "local/notes", "operation": "list-notes" },
                        { "via": via, "connector": "local/notes", "operation": "list-other" }
                    ] });
                    let report = plugin.update(&config.to_string());
                    assert!(report.result.is_ok(), "{what}: {report:?}");
                }
                let expected = credential.is_some()
                    && account
                    && uses
                    && grant != Grant::OptionalOff
                    && state == State::Connected;
                let to_notes: Vec<bool> = service.requests_to("/notes")[notes_before..]
                    .iter()
                    .map(|request| request.header("authorization").is_some())
                    .collect();
                let to_other: Vec<bool> = other.received()[other_before..]
                    .iter()
                    .map(|request| request.header("authorization").is_some())
                    .collect();
                if expected {
                    assert_eq!(to_notes, [true], "{what}: to its destination");
                } else {
                    assert!(to_notes.iter().all(|sent| !sent), "{what}: {to_notes:?}");
                }
                assert!(
                    to_other.iter().all(|sent| !sent),
                    "{what}: to another destination"
                );
                checked += 2;
            }
        }
    }
    if state != State::Absent {
        signin::scan(&[world.data.path()], &[], &service.secrets());
    }
    checked
}

/// `credential_rules` (`data-model.md`, Other tests named): a redirect to a host the credential
/// is for keeps it; the whole header value echoed is caught; a `401` body and a
/// `subscription-required` answer's body never reach the widget; `account: true` of a connector
/// without a credential is refused at load; narrowed access runs without a new approval.
#[test]
fn credential_rules() {
    let setup = setup();
    let mut notes: Value = serde_json::from_str(NOTES).unwrap();
    for (name, path) in [
        ("jump", "/jump"),
        ("private", "/private"),
        ("paid", "/paid"),
    ] {
        notes["operations"][name] = json!({
            "kind": "read", "method": "GET", "destination": "api", "path": path,
            "headers": { "Accept": "application/json" }, "parameters": {},
            "response": { "format": "json", "headers": [] }
        });
    }
    notes["credential"]["subscription_required"] = json!([402]);
    notes["credential"]["account"]["access"] =
        json!([{ "name": "notes", "level": "read" }, { "name": "tags", "level": "read" }]);
    let path = setup.world.write_connector("notes", &notes);
    let server = &setup.service.server;
    server.route(
        "/jump",
        common::servers::Reply::redirect(302, "https://notes.test/notes"),
    );
    server.route(
        "/private",
        common::servers::Reply::status(401).body(b"the body of the 401".to_vec()),
    );
    server.route(
        "/paid",
        common::servers::Reply::status(402).body(b"the body of the 402".to_vec()),
    );
    setup.connect();
    let manifest = json!({
        "manifest_version": 1, "id": "local/rules-probe", "version": "0.1.0", "author": "A",
        "permissions": [],
        "connectors": [{ "id": "local/notes", "major": 1,
            "operations": ["jump", "private", "paid", "list-notes"], "required": true, "account": true }]
    });
    let package = repack(&fixture("account-probe"), &manifest.to_string());
    let plugin = setup.world.approve_and_load(&package);
    let call = |operation: &str| {
        json!({ "calls": [{ "via": "accounts", "connector": "local/notes", "operation": operation }] })
            .to_string()
    };

    // A redirect to the credential's own host keeps it
    let jumped = plugin.update(&call("jump"));
    assert!(
        common::answers(jumped.result.as_ref().unwrap())[0]
            .get("ok")
            .is_some(),
        "{jumped:?}"
    );
    let followed = setup.service.requests_to("/notes");
    assert!(followed.last().unwrap().header("authorization").is_some());

    // A 401 body and a 402 body never reach the widget
    let refused = plugin.update(&call("private"));
    let answers = common::answers(refused.result.as_ref().unwrap());
    assert_eq!(
        answers[0]["error"],
        json!("account-not-connected"),
        "{answers:?}"
    );
    let paid = plugin.update(&call("paid"));
    let answers = common::answers(paid.result.as_ref().unwrap());
    assert_eq!(
        answers[0]["error"],
        json!("subscription-required"),
        "{answers:?}"
    );
    let shown = format!("{refused:?} {paid:?}");
    assert!(!shown.contains("the body of the"), "{shown}");

    // The whole header value echoed is caught
    setup
        .service
        .script(|script| script.echo = Some(signin::Echo::Body));
    let echoed = plugin.update(&call("list-notes"));
    let answers = common::answers(echoed.result.as_ref().unwrap());
    assert_eq!(
        answers[0]["error"],
        json!("failed:invalid-response"),
        "{answers:?}"
    );
    setup.service.script(|script| script.echo = None);

    // Narrowed access: loads without a new approval, and the call carries the credential
    notes["credential"]["account"]["access"] = json!([{ "name": "notes", "level": "read" }]);
    std::fs::write(&path, notes.to_string()).unwrap();
    let plugin = setup.world.load(&package);
    let narrowed = plugin.update(&call("list-notes"));
    assert!(
        common::answers(narrowed.result.as_ref().unwrap())[0]
            .get("ok")
            .is_some(),
        "{narrowed:?}"
    );
    assert!(
        setup
            .service
            .requests_to("/notes")
            .last()
            .unwrap()
            .header("authorization")
            .is_some()
    );

    // `account: true` of a connector without a credential, and a required credential without it
    let mut plain = notes.clone();
    plain["id"] = json!("local/plain");
    plain.as_object_mut().unwrap().remove("credential");
    setup.world.write_connector("plain", &plain);
    let undeclared = json!({
        "manifest_version": 1, "id": "local/undeclared-probe", "version": "0.1.0", "author": "A",
        "permissions": [],
        "connectors": [{ "id": "local/plain", "major": 1, "operations": ["list-notes"], "required": true, "account": true }]
    });
    let error = match setup
        .world
        .runtime
        .check(&repack(&fixture("account-probe"), &undeclared.to_string()))
    {
        Ok(_) => panic!("account use of a connector without a credential was accepted"),
        Err(error) => error,
    };
    assert_eq!(error.kind(), runtime::ErrorKind::InvalidManifest, "{error}");
    notes["credential"]["required"] = json!(true);
    std::fs::write(&path, notes.to_string()).unwrap();
    // A second connector used with its account, so the probe's import of `accounts` is declared
    let mut anchor: Value = serde_json::from_str(NOTES).unwrap();
    anchor["id"] = json!("local/anchor");
    setup.world.write_connector("anchor", &anchor);
    let mut unused = manifest.clone();
    unused["connectors"][0]["account"] = json!(false);
    unused["connectors"]
        .as_array_mut()
        .unwrap()
        .push(json!({ "id": "local/anchor", "major": 1, "operations": ["list-notes"], "required": true, "account": true }));
    let error = match setup
        .world
        .runtime
        .check(&repack(&fixture("account-probe"), &unused.to_string()))
    {
        Ok(_) => panic!("a required credential without account use was accepted"),
        Err(error) => error,
    };
    assert_eq!(
        error.kind(),
        runtime::ErrorKind::AccountUseNotDeclared,
        "{error}"
    );

    setup.scan(&[(
        "the updates",
        format!("{jumped:?} {refused:?} {paid:?} {echoed:?} {narrowed:?}"),
    )]);
}
