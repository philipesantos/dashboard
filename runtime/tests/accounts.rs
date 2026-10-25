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
