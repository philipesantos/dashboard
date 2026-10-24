//! Plugin permissions at the API level: the acceptance scenarios of
//! `specs/002-plugin-permissions/spec.md`, one section per user story.

mod common;

use std::sync::Arc;

use common::{TEST_ORIGIN, approve, fixture, manifest};
use runtime::grants::GrantStore;
use runtime::package::pack;
use runtime::{ErrorKind, Limits, LogLevel, LogMessage, Plugin, Runtime, RuntimeError};
use serde_json::json;

fn runtime() -> Runtime {
    Runtime::new(Limits::default(), common::options())
        .expect("the runtime starts with the default limits")
}

/// The `probe` fixture packed with a manifest for `local/probe` at `version`.
fn probe(version: &str, permissions: &[(&str, bool)]) -> Vec<u8> {
    pack(
        &fixture("probe"),
        manifest("local/probe", version, permissions).as_bytes(),
    )
    .expect("the manifest is valid")
}

/// A manifest section on its own, encoded as `pack` encodes one, to append to a package.
fn manifest_section(manifest: &str) -> Vec<u8> {
    const EMPTY_COMPONENT: [u8; 8] = [0x00, 0x61, 0x73, 0x6d, 0x0d, 0x00, 0x01, 0x00];
    let packed = pack(&EMPTY_COMPONENT, manifest.as_bytes()).expect("the manifest is valid");
    packed[EMPTY_COMPONENT.len()..].to_vec()
}

fn load(
    runtime: &Runtime,
    package: &[u8],
    grants: &Arc<GrantStore>,
) -> Result<Plugin, RuntimeError> {
    runtime.load(package, TEST_ORIGIN, Arc::clone(grants))
}

fn refusal(result: Result<Plugin, RuntimeError>) -> RuntimeError {
    match result {
        Ok(_) => panic!("the plugin was loaded, but it must be refused"),
        Err(error) => error,
    }
}

// User Story 1: a plugin gets exactly the capabilities declared and granted

#[test]
fn us1_the_record_shows_what_the_plugin_requests_and_what_is_granted() {
    let runtime = runtime();
    let grants = Arc::new(GrantStore::in_memory());
    let package = probe("1.0.0", &[("log", false)]);

    let checked = runtime
        .check(&package)
        .expect("the package passes its checks");
    let requested: Vec<(&str, bool)> = checked
        .manifest()
        .permissions()
        .iter()
        .map(|permission| (permission.name.as_str(), permission.required))
        .collect();
    assert_eq!(requested, [("log", false)]);
    assert_eq!(grants.entry(checked.manifest().id()), Ok(None));

    approve(&grants, &package, TEST_ORIGIN);
    let entry = grants
        .entry(checked.manifest().id())
        .expect("readable")
        .expect("approved");
    assert!(
        !entry.is_granted("log"),
        "an optional permission starts off"
    );
}

#[test]
fn us1_an_approved_plugin_runs_with_log() {
    let runtime = runtime();
    let grants = Arc::new(GrantStore::in_memory());
    let package = probe("1.0.0", &[("log", true)]);
    approve(&grants, &package, TEST_ORIGIN);

    let report = load(&runtime, &package, &grants)
        .expect("the approved plugin loads")
        .render("{}");
    assert_eq!(report.result.expect("it renders"), json!({"log": "ok"}));
    assert_eq!(
        report.logs,
        [LogMessage {
            level: LogLevel::Info,
            message: "probe".to_owned()
        }]
    );
}

#[test]
fn us1_an_import_the_manifest_does_not_declare_is_refused() {
    let runtime = runtime();
    let grants = Arc::new(GrantStore::in_memory());
    let package = probe("1.0.0", &[]);
    approve(&grants, &package, TEST_ORIGIN);

    let error = refusal(load(&runtime, &package, &grants));
    assert_eq!(error.kind(), ErrorKind::NotDeclared);
    assert_eq!(error.names(), ["log"]);
}

#[test]
fn us1_a_required_permission_not_approved_is_refused() {
    let runtime = runtime();
    let grants = Arc::new(GrantStore::in_memory());
    let error = refusal(load(&runtime, &probe("1.0.0", &[("log", true)]), &grants));
    assert_eq!(error.kind(), ErrorKind::NotGranted);
    assert_eq!(error.names(), ["log"]);
}

#[test]
fn us1_a_package_without_one_manifest_is_refused() {
    let runtime = runtime();
    let grants = Arc::new(GrantStore::in_memory());

    let bare = fixture("probe");
    assert_eq!(
        refusal(load(&runtime, &bare, &grants)).kind(),
        ErrorKind::NoManifest
    );

    let mut twice = probe("1.0.0", &[("log", true)]);
    twice.extend_from_slice(&manifest_section(&manifest("local/probe", "1.0.0", &[])));
    assert_eq!(
        refusal(load(&runtime, &twice, &grants)).kind(),
        ErrorKind::ManifestMismatch
    );
}

#[test]
fn us1_approve_grants_every_required_permission_at_once() {
    // Only `log` exists in this feature, so scenario 6 holds by the shape of the API: approve
    // takes the manifest and grants all of its required permissions; nothing takes a subset.
    let grants = GrantStore::in_memory();
    let package = probe("1.0.0", &[("log", true)]);
    let checked = runtime().check(&package).expect("checked");
    let approval = grants
        .approve(checked.manifest(), TEST_ORIGIN, "sha256:00")
        .expect("approved");
    assert_eq!(approval.required, ["log"]);
}

// User Story 3: updates never widen access silently

/// The `quiet` fixture, which imports nothing, packed as a version of `local/probe`.
fn quiet_as_probe(version: &str, permissions: &[(&str, bool)]) -> Vec<u8> {
    pack(
        &fixture("quiet"),
        manifest("local/probe", version, permissions).as_bytes(),
    )
    .expect("the manifest is valid")
}

fn probe_entry(grants: &GrantStore) -> Option<runtime::grants::Entry> {
    let id = runtime::manifest::PluginId::parse("local/probe").expect("a valid id");
    grants.entry(&id).expect("readable")
}

#[test]
fn us3_a_new_version_with_the_same_permissions_runs_without_approval() {
    let runtime = runtime();
    let grants = Arc::new(GrantStore::in_memory());
    approve(&grants, &probe("1.0.0", &[("log", true)]), TEST_ORIGIN);

    // Another component, another digest, the same id from the same origin
    let update = quiet_as_probe("1.1.0", &[("log", true)]);
    let plugin = load(&runtime, &update, &grants).expect("the update runs");
    assert_eq!(
        plugin.render("{}").result.expect("it renders"),
        json!({"quiet": true})
    );
}

#[test]
fn us3_a_new_version_that_requires_more_waits_for_approval() {
    let runtime = runtime();
    let grants = Arc::new(GrantStore::in_memory());
    approve(&grants, &probe("1.0.0", &[]), TEST_ORIGIN);
    let before = probe_entry(&grants);

    let update = probe("1.1.0", &[("log", true)]);
    let error = refusal(load(&runtime, &update, &grants));
    assert_eq!(error.kind(), ErrorKind::NeedsNewApproval);
    assert_eq!(error.names(), ["log"]);
    assert_eq!(probe_entry(&grants), before, "the approval is unchanged");

    approve(&grants, &update, TEST_ORIGIN);
    let plugin = load(&runtime, &update, &grants).expect("approved, it runs");
    assert_eq!(plugin.render("{}").result.unwrap(), json!({"log": "ok"}));
}

#[test]
fn us3_a_permission_no_longer_declared_stops_being_granted() {
    let runtime = runtime();
    let grants = Arc::new(GrantStore::in_memory());
    approve(&grants, &probe("1.0.0", &[("log", true)]), TEST_ORIGIN);

    load(&runtime, &quiet_as_probe("1.1.0", &[]), &grants).expect("it runs");
    let entry = probe_entry(&grants).expect("still approved");
    assert!(!entry.is_granted("log"), "log left the record at load");

    let error = refusal(load(&runtime, &probe("1.2.0", &[("log", true)]), &grants));
    assert_eq!(error.kind(), ErrorKind::NeedsNewApproval);
}

#[test]
fn us3_a_required_permission_cannot_be_turned_off_alone() {
    let grants = GrantStore::in_memory();
    let package = probe("1.0.0", &[("log", true)]);
    approve(&grants, &package, TEST_ORIGIN);
    let checked = runtime().check(&package).expect("checked");
    let error = grants
        .set_optional(checked.manifest(), TEST_ORIGIN, "log", false)
        .expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::RequiredTogether);
    assert!(probe_entry(&grants).expect("approved").is_granted("log"));
}

#[test]
fn us3_a_removed_approval_refuses_the_plugin() {
    let runtime = runtime();
    let grants = Arc::new(GrantStore::in_memory());
    let package = probe("1.0.0", &[("log", true)]);
    approve(&grants, &package, TEST_ORIGIN);
    load(&runtime, &package, &grants).expect("approved, it runs");

    let id = runtime::manifest::PluginId::parse("local/probe").unwrap();
    assert_eq!(grants.remove(&id), Ok(true));
    let error = refusal(load(&runtime, &package, &grants));
    assert_eq!(error.kind(), ErrorKind::NotGranted);
}

#[test]
fn us3_the_same_version_number_with_other_contents_is_a_new_version() {
    let runtime = runtime();
    let grants = Arc::new(GrantStore::in_memory());
    approve(&grants, &probe("1.0.0", &[]), TEST_ORIGIN);

    // Same number, other contents: judged on its permissions like any new version
    let same_number = quiet_as_probe("1.0.0", &[]);
    load(&runtime, &same_number, &grants).expect("nothing new to approve");
    let requires_more = probe("1.0.0", &[("log", true)]);
    assert_eq!(
        refusal(load(&runtime, &requires_more, &grants)).kind(),
        ErrorKind::NeedsNewApproval
    );
}

// User Story 4: optional permissions let a plugin run with less, and every grant change applies
// from the plugin's next call, while it stays loaded

/// The probe's manifest, parsed, to change its grants through the store.
fn probe_manifest(version: &str, permissions: &[(&str, bool)]) -> runtime::manifest::Manifest {
    runtime::manifest::Manifest::parse(manifest("local/probe", version, permissions).as_bytes())
        .expect("valid")
}

/// Renders and returns what the probe says `log` answered, or the call's error kind, with the
/// number of log messages kept.
fn probe_says(plugin: &Plugin) -> (Result<String, ErrorKind>, usize) {
    let report = plugin.render("{}");
    let said = report
        .result
        .map(|content| content["log"].as_str().unwrap_or_default().to_owned())
        .map_err(|error| error.kind());
    (said, report.logs.len())
}

#[test]
fn us4_an_optional_permission_starts_off_and_changes_from_the_next_call() {
    let runtime = runtime();
    let grants = Arc::new(GrantStore::in_memory());
    let package = probe("1.0.0", &[("log", false)]);
    approve(&grants, &package, TEST_ORIGIN);
    let optional = probe_manifest("1.0.0", &[("log", false)]);

    // Loaded once, and kept loaded
    let plugin = load(&runtime, &package, &grants).expect("it runs without log");
    assert_eq!(probe_says(&plugin), (Ok("not-granted".to_owned()), 0));

    grants
        .set_optional(&optional, TEST_ORIGIN, "log", true)
        .expect("on");
    assert_eq!(probe_says(&plugin), (Ok("ok".to_owned()), 1));

    grants
        .set_optional(&optional, TEST_ORIGIN, "log", false)
        .expect("off");
    assert_eq!(probe_says(&plugin), (Ok("not-granted".to_owned()), 0));
}

#[test]
fn us4_removing_the_approval_stops_a_loaded_plugin_on_its_next_call() {
    let runtime = runtime();
    let grants = Arc::new(GrantStore::in_memory());
    let package = probe("1.0.0", &[("log", true)]);
    approve(&grants, &package, TEST_ORIGIN);
    let plugin = load(&runtime, &package, &grants).expect("approved, it runs");
    assert_eq!(probe_says(&plugin), (Ok("ok".to_owned()), 1));

    let id = runtime::manifest::PluginId::parse("local/probe").unwrap();
    grants.remove(&id).expect("removed");
    assert_eq!(probe_says(&plugin), (Err(ErrorKind::NotGranted), 0));
}

#[test]
fn us4_approving_another_file_stops_a_loaded_plugin_on_its_next_call() {
    let runtime = runtime();
    let grants = Arc::new(GrantStore::in_memory());
    let package = probe("1.0.0", &[("log", true)]);
    approve(&grants, &package, TEST_ORIGIN);
    let plugin = load(&runtime, &package, &grants).expect("approved, it runs");

    approve(&grants, &package, "test://elsewhere");
    assert_eq!(probe_says(&plugin), (Err(ErrorKind::NotGranted), 0));
}

#[test]
fn us4_a_new_version_that_adds_an_optional_permission_runs_with_it_off() {
    let runtime = runtime();
    let grants = Arc::new(GrantStore::in_memory());
    approve(&grants, &quiet_as_probe("1.0.0", &[]), TEST_ORIGIN);

    let update = probe("1.1.0", &[("log", false)]);
    let plugin = load(&runtime, &update, &grants).expect("no new approval needed");
    assert_eq!(probe_says(&plugin), (Ok("not-granted".to_owned()), 0));
}

#[test]
fn us4_an_optional_permission_that_becomes_required_needs_approval() {
    let runtime = runtime();
    let grants = Arc::new(GrantStore::in_memory());
    approve(&grants, &probe("1.0.0", &[("log", false)]), TEST_ORIGIN);

    let error = refusal(load(&runtime, &probe("1.1.0", &[("log", true)]), &grants));
    assert_eq!(error.kind(), ErrorKind::NeedsNewApproval);
    assert_eq!(error.names(), ["log"]);
}

// Feature 003, User Story 4: changes never widen network access silently. Local connectors on
// a loopback test service, through the real network module in development mode.

mod connected {
    use common_servers::{Reply, Server};
    use runtime::ErrorKind;
    use runtime::manifest::Manifest;
    use serde_json::{Value, json};

    use super::common::servers as common_servers;
    use super::common::{TEST_ORIGIN, World, answers, calls, connected_manifest, probe};

    fn echo(server: &Server, edit: impl FnOnce(&mut Value)) -> Value {
        let mut connector = json!({
            "connector_version": 1, "id": "local/echo", "version": "1.0.0", "author": "A",
            "description": "Echo",
            "destinations": { "api": server.http_origin() },
            "operations": {
                "get": { "kind": "read", "method": "GET", "destination": "api", "path": "/",
                         "response": { "format": "json" } },
                "more": { "kind": "read", "method": "GET", "destination": "api", "path": "/more",
                          "response": { "format": "json" } }
            }
        });
        edit(&mut connector);
        connector
    }

    fn server() -> Server {
        let server = Server::http();
        server.route("/", Reply::json(r#"{"n":1}"#));
        server.route("/more", Reply::json(r#"{"n":2}"#));
        server
    }

    fn manifest(operations: &[&str], required: bool) -> String {
        connected_manifest(
            "local/net-probe",
            &[("local/echo", 1, operations, required)],
        )
    }

    fn get(plugin: &runtime::Plugin) -> Value {
        answers(
            &plugin
                .update(&calls(&[("local/echo", "get", json!([]))]))
                .result
                .unwrap(),
        )
        .remove(0)
    }

    fn load_error(world: &World, package: &[u8]) -> runtime::RuntimeError {
        match world
            .runtime
            .load(package, TEST_ORIGIN, std::sync::Arc::clone(&world.grants))
        {
            Ok(_) => panic!("loaded, but it must be refused"),
            Err(error) => error,
        }
    }

    #[test]
    fn a_connector_that_only_adds_operations_the_widget_does_not_call_runs_without_approval() {
        let server = server();
        let world = World::production(true);
        world.write_connector("echo", &echo(&server, |_| {}));
        let package = probe(&manifest(&["get"], true));
        world.approve(&package);
        world.write_connector(
            "echo",
            &echo(&server, |c| {
                c["version"] = json!("1.1.0");
                c["operations"]["new"] = c["operations"]["get"].clone();
            }),
        );
        let plugin = world.load(&package);
        assert_eq!(get(&plugin)["status"], 200);
    }

    #[test]
    fn a_change_to_what_the_widget_calls_needs_approval_and_leaves_the_grants() {
        let server = server();
        let world = World::production(true);
        let package = probe(&manifest(&["get"], true));
        type Change = (fn(&mut Value), &'static str);
        let changes: [Change; 4] = [
            (
                |c| c["destinations"]["more"] = json!("http://localhost:9"),
                "destination added",
            ),
            (
                |c| c["operations"]["get"]["path"] = json!("/more"),
                "operation changed: get",
            ),
            (
                |c| c["operations"]["get"]["query"] = json!({ "x": "1" }),
                "operation changed: get",
            ),
            (
                |c| c["operations"]["get"]["kind"] = json!("change"),
                "operation changed: get",
            ),
        ];
        for (change, named) in changes {
            world.write_connector("echo", &echo(&server, |_| {}));
            world.approve(&package);
            let entry = world
                .grants
                .entry(&runtime::manifest::PluginId::parse("local/net-probe").unwrap())
                .unwrap();
            world.write_connector("echo", &echo(&server, change));
            let error = load_error(&world, &package);
            assert_eq!(error.kind(), ErrorKind::NeedsNewApproval, "{error}");
            assert!(error.to_string().contains(named), "{error}");
            assert_eq!(
                world
                    .grants
                    .entry(&runtime::manifest::PluginId::parse("local/net-probe").unwrap())
                    .unwrap(),
                entry,
                "the grants changed"
            );
        }
        // A version of the widget that calls a new operation
        world.write_connector("echo", &echo(&server, |_| {}));
        world.approve(&package);
        let newer = probe(&manifest(&["get", "more"], true));
        let error = load_error(&world, &newer);
        assert_eq!(error.kind(), ErrorKind::NeedsNewApproval);
        assert!(
            error.to_string().contains("operation newly declared: more"),
            "{error}"
        );
        assert!(server.received().is_empty());
    }

    #[test]
    fn an_optional_connector_starts_off_and_its_changes_apply_from_the_next_call() {
        let server = server();
        let world = World::production(true);
        world.write_connector("echo", &echo(&server, |_| {}));
        let text = manifest(&["get"], false);
        let package = probe(&text);
        let plugin = world.approve_and_load(&package);
        assert_eq!(get(&plugin)["error"], "not-granted");
        assert!(server.received().is_empty());

        let manifest = Manifest::parse(text.as_bytes()).unwrap();
        world
            .grants
            .set_connector(&manifest, TEST_ORIGIN, "local/echo", true)
            .unwrap();
        assert_eq!(get(&plugin)["status"], 200, "on from the next update");
        assert!(!plugin.render("{}").result.unwrap()["data"].is_null());

        world
            .grants
            .set_connector(&manifest, TEST_ORIGIN, "local/echo", false)
            .unwrap();
        assert!(
            plugin.render("{}").result.unwrap()["data"].is_null(),
            "kept data discarded"
        );
        assert_eq!(get(&plugin)["error"], "not-granted");

        world
            .grants
            .set_connector(&manifest, TEST_ORIGIN, "local/echo", true)
            .unwrap();
        assert_eq!(get(&plugin)["status"], 200);
        world.grants.remove(manifest.id()).unwrap();
        assert_eq!(
            plugin
                .update(&calls(&[("local/echo", "get", json!([]))]))
                .result
                .unwrap_err()
                .kind(),
            ErrorKind::NotGranted
        );
        assert_eq!(
            plugin.render("{}").result.unwrap_err().kind(),
            ErrorKind::NotGranted
        );
        // Approved again: the data of before is gone
        let plugin = world.approve_and_load(&package);
        world
            .grants
            .set_connector(&manifest, TEST_ORIGIN, "local/echo", true)
            .unwrap();
        assert!(
            plugin.render("{}").result.unwrap()["data"].is_null(),
            "old data came back"
        );
    }

    #[test]
    fn a_connector_that_is_missing_or_of_another_version_is_unavailable() {
        let server = server();
        let world = World::production(true);
        let package = probe(&manifest(&["get"], true));
        // Not found at all
        let error = world.runtime.check(&package).err().expect("refused");
        assert_eq!(error.kind(), ErrorKind::ConnectorUnavailable, "{error}");
        assert!(error.to_string().contains("local/echo"), "{error}");
        // Of another major
        world.write_connector("echo", &echo(&server, |c| c["version"] = json!("2.0.0")));
        let error = world.runtime.check(&package).err().expect("refused");
        assert_eq!(error.kind(), ErrorKind::ConnectorUnavailable);
        assert!(error.to_string().contains("major version 1"), "{error}");
        // Without an operation the widget declares
        world.write_connector(
            "echo",
            &echo(&server, |c| {
                c["operations"].as_object_mut().unwrap().remove("get");
            }),
        );
        let error = world.runtime.check(&package).err().expect("refused");
        assert_eq!(error.kind(), ErrorKind::ConnectorUnavailable);
        assert!(error.to_string().contains("`get`"), "{error}");
        // Deleted after approval
        let path = world.write_connector("echo", &echo(&server, |_| {}));
        world.approve(&package);
        std::fs::remove_file(path).unwrap();
        assert_eq!(
            load_error(&world, &package).kind(),
            ErrorKind::ConnectorUnavailable
        );
    }
}

// Feature 004: storage as a permission (`specs/004-plugin-storage/data-model.md`, Combinations).
// One test per row of the table; the hostile cases of `runtime/tests/hostile/storage.rs` cover
// several rows in depth, as each test says.
mod storage {
    use std::sync::Arc;

    use runtime::grants::GrantStore;
    use runtime::manifest::PluginId;
    use runtime::{ErrorKind, Limits, Runtime, RuntimeOptions};
    use serde_json::{Value, json};
    use tempfile::TempDir;

    use crate::common::{
        TEST_ORIGIN, World, fixture, manifest, render_steps, repack, steps, stored_manifest,
    };

    const ID: &str = "local/store-probe";

    fn probe(required: bool, quota: u64) -> Vec<u8> {
        repack(
            &fixture("store-probe"),
            &stored_manifest(ID, "0.1.0", required, quota),
        )
    }

    fn first_answer(content: &Value) -> Value {
        content["answers"][0].clone()
    }

    fn set_step() -> Value {
        json!({ "op": "set", "name": "a", "value": "b" })
    }

    fn get_step() -> Value {
        json!({ "op": "get", "name": "a" })
    }

    /// Row 1, and the test `storage_needs_its_declaration`: a component that imports storage
    /// without its manifest declaring it.
    #[test]
    fn storage_needs_its_declaration() {
        let world = World::production(false);
        let package = repack(&fixture("store-probe"), &manifest(ID, "0.1.0", &[]));
        let error = world
            .runtime
            .check(&package)
            .err()
            .expect("an undeclared import is refused");
        assert_eq!(error.kind(), ErrorKind::NotDeclared, "{error}");
        assert_eq!(error.names(), ["storage"]);
    }

    /// Row 2, and the test `required_storage_needs_approval`.
    #[test]
    fn required_storage_needs_approval() {
        let world = World::production(false);
        let error = world
            .try_load_at(&probe(true, 4096), TEST_ORIGIN)
            .err()
            .expect("not approved");
        assert_eq!(error.kind(), ErrorKind::NotGranted, "{error}");
        assert_eq!(error.names(), ["storage"]);
    }

    /// Rows 3 and 4: approved and required, render reads and update writes its own store
    /// (the hostile case `write-in-render` covers a write from render in depth).
    #[test]
    fn approved_storage_reads_in_render_and_writes_in_update() {
        let world = World::production(false);
        let plugin = world.approve_and_load(&probe(true, 4096));
        let report = plugin.update(&steps(&[set_step()], None));
        assert_eq!(first_answer(&report.result.unwrap()), json!({ "ok": null }));
        let content = plugin
            .render(&render_steps(&[get_step(), set_step()]))
            .result
            .unwrap();
        assert_eq!(content["answers"][0], json!({ "ok": { "hex": "62" } }));
        assert_eq!(content["answers"][1]["error"], "refused:render-only-reads");
    }

    /// Row 5: approved at a smaller quota than declared (the hostile case `quota-raised`).
    #[test]
    fn a_quota_above_the_approval_needs_a_new_one() {
        let world = World::production(false);
        world.approve(&probe(true, 4096));
        let larger = repack(
            &fixture("store-probe"),
            &stored_manifest(ID, "0.2.0", true, 8192),
        );
        let error = world
            .try_load_at(&larger, TEST_ORIGIN)
            .err()
            .expect("refused");
        assert_eq!(error.kind(), ErrorKind::NeedsNewApproval, "{error}");
    }

    /// Row 6: optional and off, the stand-in (the hostile case `optional-off`).
    #[test]
    fn optional_storage_that_is_off_answers_not_granted() {
        let world = World::production(false);
        let plugin = world.approve_and_load(&probe(false, 4096));
        let report = plugin.update(&steps(&[set_step(), get_step()], None));
        let content = report.result.unwrap();
        assert_eq!(content["answers"][0]["error"], "not-granted");
        assert_eq!(content["answers"][1]["error"], "not-granted");
        assert!(
            !world.data.path().join("storage").exists(),
            "nothing was created"
        );
    }

    /// Row 7, and the test `turning_storage_on_checks_the_quota`.
    #[test]
    fn turning_storage_on_checks_the_quota() {
        let world = World::production(false);
        world.approve(&probe(false, 4096));
        let larger = repack(
            &fixture("store-probe"),
            &stored_manifest(ID, "0.2.0", false, 8192),
        );
        let checked = world.runtime.check(&larger).unwrap();
        let error = checked
            .set_optional(&world.grants, TEST_ORIGIN, "storage", true)
            .expect_err("above the approved quota");
        assert_eq!(error.kind(), ErrorKind::NeedsNewApproval, "{error}");
        assert!(error.to_string().contains("4096") && error.to_string().contains("8192"));
    }

    /// Row 8: optional and on, as required.
    #[test]
    fn optional_storage_that_is_on_works_as_required() {
        let world = World::production(false);
        let package = probe(false, 4096);
        world.approve(&package);
        world
            .runtime
            .check(&package)
            .unwrap()
            .set_optional(&world.grants, TEST_ORIGIN, "storage", true)
            .unwrap();
        let plugin = world.load(&package);
        let report = plugin.update(&steps(&[set_step()], None));
        assert_eq!(first_answer(&report.result.unwrap()), json!({ "ok": null }));
        let content = plugin.render(&render_steps(&[get_step()])).result.unwrap();
        assert_eq!(first_answer(&content), json!({ "ok": { "hex": "62" } }));
    }

    /// Row 9: approved from another origin (the hostile case `same-id-other-origin`).
    #[test]
    fn an_approval_from_another_origin_grants_nothing() {
        let world = World::production(false);
        let package = probe(true, 4096);
        world.approve_at(&package, "/elsewhere/store-probe.wasm");
        let error = world
            .try_load_at(&package, TEST_ORIGIN)
            .err()
            .expect("refused");
        assert_eq!(error.kind(), ErrorKind::NotGranted, "{error}");
    }

    /// The test `approval_records_the_quota_shown`: re-approving at a smaller quota lowers the
    /// approved quota.
    #[test]
    fn approval_records_the_quota_shown() {
        let world = World::production(false);
        world.approve(&probe(true, 8192));
        world.approve(&repack(
            &fixture("store-probe"),
            &stored_manifest(ID, "0.2.0", true, 4096),
        ));
        let entry = world
            .grants
            .entry(&PluginId::parse(ID).unwrap())
            .unwrap()
            .unwrap();
        assert_eq!(entry.storage_quota_bytes, Some(4096));
    }

    /// A later host whose cap is below a quota already approved: the plugin is refused, naming
    /// the field and both bounds, and its data is kept (spec edge case).
    #[test]
    fn a_host_cap_below_an_approved_quota_refuses_the_plugin_and_keeps_its_data() {
        let data = TempDir::new().unwrap();
        let grants = Arc::new(GrantStore::open(data.path().join("grants.json")));
        let package = probe(true, 1024 * 1024);
        let runtime = Runtime::new(Limits::default(), RuntimeOptions::new(data.path())).unwrap();
        crate::common::approve(&grants, &package, TEST_ORIGIN);
        let plugin = runtime
            .load(&package, TEST_ORIGIN, Arc::clone(&grants))
            .unwrap();
        assert!(plugin.update(&steps(&[set_step()], None)).result.is_ok());
        let lower = Limits {
            max_storage_quota_bytes: 512 * 1024,
            ..Limits::default()
        };
        let runtime = Runtime::new(lower, RuntimeOptions::new(data.path())).unwrap();
        let error = runtime
            .load(&package, TEST_ORIGIN, Arc::clone(&grants))
            .err()
            .expect("over this host's cap");
        assert_eq!(error.kind(), ErrorKind::InvalidManifest, "{error}");
        let detail = error.to_string();
        assert!(detail.contains("permissions[0].quota_bytes"), "{detail}");
        assert!(
            detail.contains("64") && detail.contains("524288"),
            "{detail}"
        );
        // Back on a host with the default cap, the data is there
        let runtime = Runtime::new(Limits::default(), RuntimeOptions::new(data.path())).unwrap();
        let plugin = runtime.load(&package, TEST_ORIGIN, grants).unwrap();
        let content = plugin.render(&render_steps(&[get_step()])).result.unwrap();
        assert_eq!(first_answer(&content), json!({ "ok": { "hex": "62" } }));
    }
}
