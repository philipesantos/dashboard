//! Plugin contracts: loading packages and rendering the example plugin (feature 001, User
//! Stories 1 and 3).

mod common;

use std::sync::Arc;

use common::{TEST_ORIGIN, approved, fixture, manifest, repack};
use runtime::grants::GrantStore;
use runtime::package::pack;
use runtime::{Contract, ErrorKind, Limits, LogLevel, LogMessage, Plugin, Runtime};
use serde_json::json;

fn runtime() -> Runtime {
    Runtime::new(Limits::default(), common::options())
        .expect("the runtime starts with the default limits")
}

/// Loads `bytes` with its required permissions approved.
fn load(runtime: &Runtime, bytes: &[u8]) -> Plugin {
    runtime
        .load(bytes, TEST_ORIGIN, approved(bytes))
        .unwrap_or_else(|error| panic!("the package should load: {error}"))
}

/// Loads `bytes`, approved by nobody, and returns the kind and detail of the error, failing if
/// loading succeeds.
fn load_error(runtime: &Runtime, bytes: &[u8]) -> (ErrorKind, String) {
    match runtime.load(bytes, TEST_ORIGIN, Arc::new(GrantStore::in_memory())) {
        Ok(_) => panic!("the package was loaded, but it should have been refused"),
        Err(error) => (error.kind(), error.detail().to_owned()),
    }
}

/// A WAT component packed with a manifest that requests nothing, so the checks after the
/// manifest decide it.
fn packed(wat: &str) -> Vec<u8> {
    let component = wat::parse_str(wat).unwrap();
    pack(&component, manifest("local/test", "0.1.0", &[]).as_bytes()).unwrap()
}

/// SC-006, research R11: a plugin built from the frozen 1.0.0 contract loads, links and renders
/// exactly as before on the 1.1.0 host, its imports matched by compatible version and its
/// renderer found by the name it exports.
#[test]
fn a_contract_1_0_0_plugin_runs_on_the_1_1_0_host() {
    let runtime = runtime();
    let plugin = load(&runtime, &fixture("hello-widget"));
    assert_eq!(plugin.contract(), Contract::V1);
    assert_eq!(
        plugin.contract_version().version,
        semver::Version::new(1, 0, 0)
    );
    assert_eq!(plugin.contract_version().display(), "v1 (1.0.0)");
    let report = plugin.render(r#"{"name":"Ada"}"#);
    assert_eq!(
        report.result.expect("hello-widget renders"),
        json!({"title": "Hello", "text": "Hello, Ada!"})
    );
    assert_eq!(report.logs.len(), 1);
}

/// Feature 004, SC-007: plugins built from the frozen 1.0.0 and 1.1.0 contracts load, link and
/// run as before on the 1.2.0 host.
#[test]
fn contract_1_0_0_and_1_1_0_plugins_run_on_the_1_2_0_host() {
    let world = common::World::production(false);
    let hello = world.approve_and_load(&fixture("hello-widget"));
    assert_eq!(hello.contract_version().display(), "v1 (1.0.0)");
    assert!(hello.render(r#"{"name":"Ada"}"#).result.is_ok());
    let releases = world.approve_and_load(&fixture("github-releases"));
    assert_eq!(releases.contract_version().display(), "v1 (1.1.0)");
    let content = releases
        .render(r#"{"owner":"rust-lang","repo":"rust"}"#)
        .result
        .expect("github-releases renders before its first update");
    assert_eq!(content["text"], "No data yet");
}

#[test]
fn hello_widget_renders_its_greeting_and_one_log() {
    let runtime = runtime();
    let plugin = load(&runtime, &fixture("hello-widget"));
    assert_eq!(plugin.contract(), Contract::V1);

    let report = plugin.render("{}");

    assert_eq!(
        report.result.expect("hello-widget renders"),
        json!({"title": "Hello", "text": "Hello, world!"})
    );
    assert_eq!(
        report.logs,
        [LogMessage {
            level: LogLevel::Info,
            message: "rendering hello-widget".to_owned(),
        }]
    );
    assert_eq!(report.dropped_logs, 0);
}

#[test]
fn hello_widget_greets_the_configured_name() {
    let runtime = runtime();
    let plugin = load(&runtime, &fixture("hello-widget"));

    for (config, text) in [
        (r#"{"name":"Ada"}"#, "Hello, Ada!"),
        (r#"{"name":"Grace"}"#, "Hello, Grace!"),
        ("{}", "Hello, world!"),
    ] {
        assert_eq!(
            plugin.render(config).result.expect("hello-widget renders"),
            json!({"title": "Hello", "text": text}),
            "config {config}"
        );
    }
}

#[test]
fn hello_widget_explains_a_configuration_it_cannot_use() {
    let runtime = runtime();
    let plugin = load(&runtime, &fixture("hello-widget"));

    for config in ["{bad", r#"{"name":5}"#] {
        let report = plugin.render(config);
        let error = report.result.expect_err("the configuration is refused");
        assert_eq!(error.kind(), ErrorKind::PluginError, "config {config}");
        assert!(!error.detail().is_empty());
    }
}

#[test]
fn a_contract_v0_plugin_still_renders_after_approval() {
    // FR-013, User Story 5 scenario 3
    let runtime = runtime();
    let plugin = load(&runtime, &fixture("hello-v0"));
    assert_eq!(plugin.contract(), Contract::V0);

    let report = plugin.render(r#"{"name":"Ada"}"#);
    assert_eq!(
        report.result.expect("hello-v0 renders"),
        json!({"title": "Hello", "text": "Hello, Ada!"})
    );
    assert_eq!(
        report.logs,
        [LogMessage {
            level: LogLevel::Info,
            message: "rendering hello-v0".to_owned(),
        }]
    );
}

#[test]
fn a_contract_v0_plugin_cannot_have_optional_permissions() {
    let optional = repack(
        &fixture("hello-v0"),
        &manifest("local/hello-v0", "0.1.0", &[("log", false)]),
    );
    let (kind, detail) = load_error(&runtime(), &optional);
    assert_eq!(kind, ErrorKind::OptionalNeedsV1);
    assert!(
        detail.contains("log"),
        "the detail should name log: {detail}"
    );
}

#[test]
fn empty_bytes_are_not_a_package() {
    assert_eq!(load_error(&runtime(), b"").0, ErrorKind::InvalidPackage);
}

#[test]
fn arbitrary_bytes_are_not_a_package() {
    assert_eq!(
        load_error(&runtime(), b"not wasm").0,
        ErrorKind::InvalidPackage
    );
}

#[test]
fn a_core_module_is_not_a_component() {
    let module = wat::parse_str("(module)").unwrap();
    assert_eq!(load_error(&runtime(), &module).0, ErrorKind::InvalidPackage);
}

#[test]
fn a_package_over_the_size_limit_is_refused_before_anything_else() {
    let bytes = fixture("hello-widget");
    let runtime = Runtime::new(
        Limits {
            max_package_bytes: bytes.len() - 1,
            ..Limits::default()
        },
        common::options(),
    )
    .unwrap();
    assert_eq!(load_error(&runtime, &bytes).0, ErrorKind::InvalidPackage);
}

#[test]
fn a_component_without_the_renderer_export_does_not_match_the_contract() {
    let component = packed("(component)");
    assert_eq!(
        load_error(&runtime(), &component).0,
        ErrorKind::ContractMismatch
    );
}

#[test]
fn a_component_built_for_another_contract_version_does_not_match() {
    // A plugin built for 0.2.0 imports that version's log interface, with its functions.
    let component = packed(
        r#"(component
             (import "dashboard:widget/log@0.2.0" (instance
               (export "write" (func (param "message" string))))))"#,
    );
    let (kind, detail) = load_error(&runtime(), &component);
    assert_eq!(kind, ErrorKind::ContractMismatch);
    assert!(
        detail.contains("dashboard:widget/log@0.2.0"),
        "the detail should name the import: {detail}"
    );
}

#[test]
fn a_component_using_a_turned_off_proposal_is_refused() {
    let component = packed("(component (core module (memory i64 1)))");
    assert_eq!(
        load_error(&runtime(), &component).0,
        ErrorKind::InvalidPackage
    );
}

// Feature 003: contract 1.1.0 and older minors (FR-016), and local connectors (FR-018)

/// A WAT component exporting `exports`, packed with `manifest`.
fn component_with(exports: &[&str], manifest: &str) -> Vec<u8> {
    let mut wat = String::from("(component");
    for (index, export) in exports.iter().enumerate() {
        wat.push_str(&format!(
            " (instance $e{index}) (export \"{export}\" (instance $e{index}))"
        ));
    }
    wat.push(')');
    pack(&wat::parse_str(&wat).unwrap(), manifest.as_bytes()).unwrap()
}

#[test]
fn a_component_built_for_a_newer_minor_is_a_contract_mismatch_naming_both_versions() {
    let package = component_with(
        &["dashboard:widget/renderer@1.4.0"],
        &manifest("local/newer", "0.1.0", &[]),
    );
    let (kind, detail) = load_error(&runtime(), &package);
    assert_eq!(kind, ErrorKind::ContractMismatch);
    assert!(
        detail.contains("1.4.0") && detail.contains("1.3.0"),
        "{detail}"
    );
}

#[test]
fn a_1_0_0_component_that_declares_a_connector_is_contract_too_old() {
    let package = component_with(
        &["dashboard:widget/renderer@1.0.0"],
        &common::connected_manifest(
            "local/old",
            &[("catalog/github", 1, &["list-releases"], true)],
        ),
    );
    let (kind, detail) = load_error(&runtime(), &package);
    assert_eq!(kind, ErrorKind::ContractTooOld);
    assert!(detail.contains("1.1.0"), "{detail}");
}

#[test]
fn a_local_connector_on_this_machine_runs_only_in_development_mode() {
    use common::servers::{Reply, Server};

    let server = Server::http();
    server.route("/", Reply::json(r#"{"here":true}"#));
    let connector = json!({
        "connector_version": 1, "id": "local/here", "version": "0.1.0", "author": "You",
        "description": "This machine", "destinations": { "api": server.http_origin() },
        "operations": { "get": { "kind": "read", "method": "GET", "destination": "api",
            "path": "/", "response": { "format": "json" } } }
    });
    let package = common::probe(&common::connected_manifest(
        "local/net-probe",
        &[("local/here", 0, &["get"], true)],
    ));

    let development = common::World::production(true);
    development.write_connector("here", &connector);
    let plugin = development.approve_and_load(&package);
    let report = plugin.update(&common::calls(&[("local/here", "get", json!([]))]));
    assert_eq!(common::answers(&report.result.unwrap())[0]["status"], 200);

    let production = common::World::production(false);
    production.write_connector("here", &connector);
    let error = production.runtime.check(&package).err().expect("refused");
    assert_eq!(error.kind(), ErrorKind::DestinationNotVerified, "{error}");

    // Even in development: the local network and the catalog's hosts
    for (destination, kind) in [
        ("http://192.168.1.10:8765", ErrorKind::InvalidConnector),
        ("https://api.github.com", ErrorKind::DestinationReserved),
    ] {
        let mut other = connector.clone();
        other["destinations"]["api"] = json!(destination);
        development.write_connector("here", &other);
        let error = development.runtime.check(&package).err().expect("refused");
        assert_eq!(error.kind(), kind, "{destination}: {error}");
    }
}

// Feature 004: contract 1.2.0 (FR-015)

/// A plugin built for 1.1.0 whose manifest declares storage is refused, naming 1.2.0.
#[test]
fn a_1_1_0_plugin_that_declares_storage_is_contract_too_old() {
    let package = repack(
        &fixture("net-probe"),
        &common::stored_manifest("local/net-probe", "0.1.0", true, 65536),
    );
    let (kind, detail) = load_error(&runtime(), &package);
    assert_eq!(kind, ErrorKind::ContractTooOld, "{detail}");
    assert!(
        detail.contains("1.2.0") && detail.contains("1.1.0"),
        "{detail}"
    );
}

/// Every plugin of the repository built for 1.0.0 or 1.1.0 still runs on the 1.2.0 host
/// (SC-007): the fixtures `probe` and `quiet` as well as the examples.
#[test]
fn every_older_minor_fixture_still_renders() {
    let runtime = runtime();
    for (name, manifest) in [
        ("probe", manifest("local/probe", "0.1.0", &[("log", true)])),
        ("quiet", manifest("local/quiet", "0.1.0", &[])),
    ] {
        let package = repack(&fixture(name), &manifest);
        let plugin = load(&runtime, &package);
        assert_eq!(plugin.contract_version().display(), "v1 (1.0.0)", "{name}");
        assert!(plugin.render("{}").result.is_ok(), "{name}");
    }
}

// Feature 005: contract 1.3.0 (FR-016)

/// The development connector of `private-notes`, as shipped beside it.
const NOTES: &str = include_str!("../../plugins/examples/private-notes/connectors/notes.json");

/// A development runtime that reaches the test sign-in service as `https://notes.test`, with
/// `connector` as `local/notes`.
fn notes_world(connector: &serde_json::Value) -> (common::signin::SignInService, common::World) {
    let pki = common::servers::TestPki::new();
    let service = common::signin::SignInService::start(&pki);
    let world = common::World::seam(
        runtime::TestNetwork {
            routes: vec![service.route()],
            public_for_tests: vec![service.server.address()],
            root: pki.root(),
            catalog: Vec::new(),
        },
        true,
    );
    world.write_connector("notes", connector);
    (service, world)
}

/// Plugins built from the frozen 1.0.0, 1.1.0 and 1.2.0 contracts run as before on the 1.3.0
/// host (SC-007).
#[test]
fn contract_1_0_0_to_1_2_0_plugins_run_on_the_1_3_0_host() {
    let world = common::World::production(false);
    for (name, version, config) in [
        ("hello-widget", "1.0.0", "{}"),
        (
            "github-releases",
            "1.1.0",
            r#"{"owner":"rust-lang","repo":"rust"}"#,
        ),
        ("update-counter", "1.2.0", "{}"),
    ] {
        let plugin = world.approve_and_load(&fixture(name));
        assert_eq!(
            plugin.contract_version().display(),
            format!("v1 ({version})"),
            "{name}"
        );
        let report = plugin.render(config);
        assert!(report.result.is_ok(), "{name}: {report:?}");
    }
    let counter = world.load(&fixture("update-counter"));
    assert!(counter.update("{}").result.is_ok());
}

/// A plugin built for 1.2.0 whose manifest uses a connector's account is refused, naming 1.3.0.
#[test]
fn a_1_2_0_plugin_that_uses_an_account_is_contract_too_old() {
    let (_service, world) = notes_world(&serde_json::from_str(NOTES).unwrap());
    let manifest = json!({
        "manifest_version": 1, "id": "local/store-probe", "version": "0.1.0", "author": "A",
        "permissions": [],
        "connectors": [{ "id": "local/notes", "major": 1, "operations": ["list-notes"], "required": true, "account": true }]
    });
    let package = repack(&fixture("store-probe"), &manifest.to_string());
    let error = world.runtime.check(&package).err().expect("refused");
    assert_eq!(error.kind(), ErrorKind::ContractTooOld, "{error}");
    assert!(
        error.detail().contains("1.3.0") && error.detail().contains("1.2.0"),
        "{error}"
    );
}

/// `connectors_type_only` (research R9): a widget whose connectors are all used with their
/// account imports `connectors` only for the types `accounts` shares; it loads, `connectors` is
/// the stand-in, and its calls work through `accounts`.
#[test]
fn connectors_type_only() {
    let (_service, world) = notes_world(&serde_json::from_str(NOTES).unwrap());
    let manifest = json!({
        "manifest_version": 1, "id": "local/account-probe", "version": "0.1.0", "author": "A",
        "permissions": [],
        "connectors": [{ "id": "local/notes", "major": 1, "operations": ["list-notes"], "required": true, "account": true }]
    });
    let plugin = world.approve_and_load(&repack(&fixture("account-probe"), &manifest.to_string()));
    assert_eq!(plugin.contract_version().display(), "v1 (1.3.0)");
    assert_eq!(plugin.connector_table(), None, "connectors is the stand-in");
    let report = plugin.update(
        &json!({ "calls": [
            { "via": "accounts", "connector": "local/notes", "operation": "list-notes" },
            { "via": "connectors", "connector": "local/notes", "operation": "list-notes" }
        ] })
        .to_string(),
    );
    let answers = common::answers(report.result.as_ref().unwrap());
    assert!(answers[0].get("ok").is_some(), "{answers:?}");
    assert_eq!(answers[1]["error"], json!("not-granted"), "{answers:?}");
}

/// `subscription-required` answers only widgets of 1.3.0, which call through `accounts`: a
/// widget built for an older minor calls a connector with an optional credential without it,
/// and sees the service's answer as before.
#[test]
fn subscription_required_answers_only_1_3_0_widgets() {
    let mut notes: serde_json::Value = serde_json::from_str(NOTES).unwrap();
    notes["credential"]["subscription_required"] = json!([402]);
    let (service, world) = notes_world(&notes);
    service.script(|script| script.notes_status = Some(402));
    world
        .runtime
        .connect_account("local/notes", Some("dashboard-dev"), &service.browser())
        .unwrap();
    // 1.1.0: through `connectors`, without the credential, so the service's public answer
    let old = world.approve_and_load(&common::probe(&common::connected_manifest(
        "local/net-probe",
        &[("local/notes", 1, &["list-notes"], true)],
    )));
    assert_eq!(old.contract_version().display(), "v1 (1.1.0)");
    let report = old.update(&common::calls(&[("local/notes", "list-notes", json!([]))]));
    let answers = common::answers(report.result.as_ref().unwrap());
    assert_eq!(answers[0]["status"], 200, "{answers:?}");
    assert!(
        service.requests_to("/notes")[0]
            .header("authorization")
            .is_none()
    );
    // 1.3.0, as the account: the service refuses for lack of a plan
    let manifest = json!({
        "manifest_version": 1, "id": "local/account-probe", "version": "0.1.0", "author": "A",
        "permissions": [],
        "connectors": [{ "id": "local/notes", "major": 1, "operations": ["list-notes"], "required": true, "account": true }]
    });
    let new = world.approve_and_load(&repack(&fixture("account-probe"), &manifest.to_string()));
    let report = new.update(
        &json!({ "calls": [{ "via": "accounts", "connector": "local/notes", "operation": "list-notes" }] })
            .to_string(),
    );
    let answers = common::answers(report.result.as_ref().unwrap());
    assert_eq!(
        answers[0]["error"],
        json!("subscription-required"),
        "{answers:?}"
    );
}
