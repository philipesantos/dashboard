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
    Runtime::new(Limits::default()).expect("the runtime starts with the default limits")
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
