//! The hostile-plugin suite, the security gate of Phases 1 and 2 (Principle VIII).
//!
//! Each case must be refused or stopped with the error kind that names its rule, and a
//! well-behaved plugin must render normally right after it, on the same runtime. The cases of
//! feature 001 come first (`specs/001-plugin-runtime/data-model.md`, Hostile-plugin case), then
//! those of feature 002, one per rule of its FR-016 (`specs/002-plugin-permissions/data-model.md`,
//! Hostile-plugin cases).

mod common;

use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use common::{TEST_ORIGIN, append_manifest_section, approve, approved, fixture, manifest};
use runtime::grants::{Entry, GrantStore};
use runtime::manifest::PluginId;
use runtime::package::pack;
use runtime::{ErrorKind, Limits, Plugin, RenderReport, Runtime, RuntimeError};
use serde_json::json;
use tempfile::TempDir;

/// A render still running after this means a limit failed: the test fails instead of hanging.
const WATCHDOG: Duration = Duration::from_secs(30);

fn runtime() -> Runtime {
    Runtime::new(Limits::default()).expect("the runtime starts with the default limits")
}

/// Loads a fixture with its required permissions approved.
fn load(runtime: &Runtime, name: &str) -> Arc<Plugin> {
    let bytes = fixture(name);
    let plugin = runtime
        .load(&bytes, TEST_ORIGIN, approved(&bytes))
        .unwrap_or_else(|error| panic!("{name} should load: {error}"));
    Arc::new(plugin)
}

/// Loads a fixture approved by nobody, which must be refused.
fn load_error(runtime: &Runtime, name: &str) -> RuntimeError {
    let grants = Arc::new(GrantStore::in_memory());
    match runtime.load(&fixture(name), TEST_ORIGIN, grants) {
        Ok(_) => panic!("{name} was loaded, but it must be refused"),
        Err(error) => error,
    }
}

/// Renders on another thread, so a plugin that is never stopped fails the test.
fn render(plugin: &Arc<Plugin>, config: &str) -> RenderReport {
    let (sender, receiver) = mpsc::channel();
    let plugin = Arc::clone(plugin);
    let config = config.to_owned();
    thread::spawn(move || {
        let _ = sender.send(plugin.render(&config));
    });
    match receiver.recv_timeout(WATCHDOG) {
        Ok(report) => report,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            panic!("the plugin was not stopped within {WATCHDOG:?}")
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => panic!("the render thread panicked"),
    }
}

fn render_error(report: &RenderReport) -> &RuntimeError {
    match &report.result {
        Ok(content) => panic!(
            "the plugin rendered {} bytes of content, but it must be stopped",
            content.to_string().len()
        ),
        Err(error) => error,
    }
}

/// Acceptance scenario 4: right after a hostile case, a well-behaved plugin renders normally.
fn assert_host_still_works(runtime: &Runtime) {
    let report = render(&load(runtime, "hello-widget"), "{}");
    assert_eq!(
        report
            .result
            .expect("hello-widget renders after a hostile case"),
        json!({"title": "Hello", "text": "Hello, world!"})
    );
}

#[test]
fn infinite_loop_runs_out_of_cpu_budget_and_keeps_its_log() {
    // The default fuel, with a time limit only a broken budget could reach: on a slow or busy
    // machine, the default 1 s could otherwise run out first. Fuel stops at the same point on
    // every system (R2).
    let runtime = Runtime::new(Limits {
        time_limit: Duration::from_secs(20),
        ..Limits::default()
    })
    .unwrap();
    let report = render(&load(&runtime, "infinite-loop"), "{}");

    assert_eq!(render_error(&report).kind(), ErrorKind::CpuBudgetExceeded);
    let messages: Vec<&str> = report.logs.iter().map(|log| log.message.as_str()).collect();
    assert_eq!(messages, ["looping"]);
    assert_host_still_works(&runtime);
}

#[test]
fn infinite_loop_with_unlimited_fuel_hits_the_time_limit_on_time() {
    let limits = Limits {
        fuel: u64::MAX / 2,
        time_limit: Duration::from_millis(500),
        ..Limits::default()
    };
    let runtime = Runtime::new(limits.clone()).unwrap();
    let plugin = load(&runtime, "infinite-loop");

    let start = Instant::now();
    let report = render(&plugin, "{}");
    let elapsed = start.elapsed();

    assert_eq!(render_error(&report).kind(), ErrorKind::TimeLimitExceeded);
    // SC-002: stopped no later than one second after the limit, and never before it
    assert!(
        elapsed >= limits.time_limit,
        "stopped after {elapsed:?}, before the limit"
    );
    assert!(
        elapsed <= limits.time_limit + Duration::from_secs(1),
        "stopped after {elapsed:?}, more than 1 s past the limit"
    );
    assert_host_still_works(&runtime);
}

#[test]
fn huge_allocation_hits_the_memory_cap() {
    let runtime = runtime();
    let report = render(&load(&runtime, "huge-alloc"), "{}");

    assert_eq!(render_error(&report).kind(), ErrorKind::MemoryLimitExceeded);
    assert_host_still_works(&runtime);
}

#[test]
fn deep_recursion_crashes_the_plugin_only() {
    let runtime = runtime();
    let report = render(&load(&runtime, "deep-recursion"), "{}");

    assert_eq!(render_error(&report).kind(), ErrorKind::PluginCrashed);
    assert_host_still_works(&runtime);
}

#[test]
fn reading_a_file_is_refused_at_load() {
    let runtime = runtime();
    let error = load_error(&runtime, "read-file");

    assert_eq!(error.kind(), ErrorKind::CapabilityRefused);
    assert!(
        error
            .names()
            .iter()
            .any(|import| import.starts_with("wasi:filesystem/")),
        "the refused imports should name the filesystem: {:?}",
        error.names()
    );
    assert_host_still_works(&runtime);
}

#[test]
fn opening_a_socket_is_refused_at_load() {
    let runtime = runtime();
    let error = load_error(&runtime, "open-socket");

    assert_eq!(error.kind(), ErrorKind::CapabilityRefused);
    assert!(
        error
            .names()
            .iter()
            .any(|import| import.starts_with("wasi:sockets/")),
        "the refused imports should name the sockets: {:?}",
        error.names()
    );
    assert_host_still_works(&runtime);
}

#[test]
fn malformed_output_is_rejected() {
    let runtime = runtime();
    let report = render(&load(&runtime, "malformed-output"), "{}");

    assert_eq!(render_error(&report).kind(), ErrorKind::InvalidOutput);
    assert_host_still_works(&runtime);
}

#[test]
fn oversized_output_is_rejected() {
    let runtime = runtime();
    let report = render(&load(&runtime, "oversized-output"), "{}");

    assert_eq!(render_error(&report).kind(), ErrorKind::OutputTooLarge);
    assert_host_still_works(&runtime);
}

#[test]
fn log_flood_keeps_100_messages_and_truncates_the_long_one() {
    let runtime = runtime();
    let report = render(&load(&runtime, "log-flood"), "{}");

    assert_eq!(
        report.result.as_ref().expect("log-flood renders"),
        &json!({"ok": true})
    );
    assert_eq!(report.logs.len(), 100);
    assert_eq!(report.dropped_logs, 900);
    let kept = report.logs[0]
        .message
        .strip_suffix(" [truncated]")
        .expect("the 4 KiB message is marked as truncated");
    // Cut at 1 KiB, moved back to the boundary of a 3-byte character
    assert!(
        kept.len() <= 1024 && kept.len() > 1024 - 3,
        "{} bytes kept",
        kept.len()
    );
    assert!(kept.chars().all(|c| c == '€'));
    assert_eq!(report.logs[1].message, "message 1");
    assert_host_still_works(&runtime);
}

#[test]
fn no_state_survives_between_calls_or_plugins() {
    let runtime = runtime();
    let plugin = load(&runtime, "state-leak");
    for _ in 0..2 {
        assert_eq!(render(&plugin, "{}").result.unwrap(), json!({"calls": 1}));
    }

    let first = load(&runtime, "state-leak");
    let second = load(&runtime, "state-leak");
    for plugin in [&first, &second, &first, &second] {
        assert_eq!(render(plugin, "{}").result.unwrap(), json!({"calls": 1}));
    }
    assert_host_still_works(&runtime);
}

#[test]
fn reading_outside_its_memory_crashes_the_plugin() {
    let runtime = runtime();
    let report = render(&load(&runtime, "out-of-bounds"), "{}");

    let error = render_error(&report);
    assert_eq!(error.kind(), ErrorKind::PluginCrashed);
    assert!(
        error.detail().contains("out of bounds memory access"),
        "the detail should name the trap: {}",
        error.detail()
    );
    assert_host_still_works(&runtime);
}

/// Characters that could make plugin text pass for the host's output: control characters and
/// bidirectional-text controls.
fn is_unsafe(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{061C}' | '\u{200E}' | '\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}'
        )
}

#[test]
fn plugin_text_cannot_imitate_the_host() {
    let runtime = runtime();
    let report = render(&load(&runtime, "log-spoof"), "{}");

    let error = render_error(&report);
    assert_eq!(error.kind(), ErrorKind::PluginError);
    let texts = report
        .logs
        .iter()
        .map(|log| log.message.as_str())
        .chain([error.detail()]);
    for text in texts {
        assert!(!text.chars().any(is_unsafe), "not escaped: {text:?}");
    }
    assert_eq!(
        report.logs[0].message,
        r"ok\n[runtime] 0 log messages dropped"
    );
    assert_host_still_works(&runtime);
}

// Feature 002: the permission cases. Each is a setup, which approves what the case needs, and an
// act, which loads and renders and must be refused or contained; `grant_record_untouched` runs
// every act again against a file-backed record that must not change.

/// The origin of the second file in `same-id-other-origin`.
const OTHER_ORIGIN: &str = "test://elsewhere";

struct PermissionCase {
    name: &'static str,
    setup: fn(&GrantStore),
    act: fn(&Runtime, &Arc<GrantStore>),
}

/// The `probe` fixture packed with a manifest for `local/probe`.
fn probe(version: &str, permissions: &[(&str, bool)]) -> Vec<u8> {
    pack(
        &fixture("probe"),
        manifest("local/probe", version, permissions).as_bytes(),
    )
    .expect("the manifest is valid")
}

fn load_from(
    runtime: &Runtime,
    package: &[u8],
    origin: &str,
    grants: &Arc<GrantStore>,
) -> Result<Arc<Plugin>, RuntimeError> {
    runtime
        .load(package, origin, Arc::clone(grants))
        .map(Arc::new)
}

/// Asserts that loading `package` from the test origin is refused with `kind`.
fn assert_refused(
    runtime: &Runtime,
    package: &[u8],
    grants: &Arc<GrantStore>,
    kind: ErrorKind,
) -> RuntimeError {
    match load_from(runtime, package, TEST_ORIGIN, grants) {
        Ok(_) => panic!("loaded, but it must be refused with {kind}"),
        Err(error) => {
            assert_eq!(error.kind(), kind, "{error}");
            error
        }
    }
}

fn probe_entry(grants: &GrantStore) -> Option<Entry> {
    let id = PluginId::parse("local/probe").expect("a valid id");
    grants.entry(&id).expect("the record is readable")
}

fn no_setup(_: &GrantStore) {}

/// FR-001: a package without a manifest.
const NO_MANIFEST: PermissionCase = PermissionCase {
    name: "no-manifest",
    setup: no_setup,
    act: |runtime, grants| {
        assert_refused(runtime, &fixture("probe"), grants, ErrorKind::NoManifest);
    },
};

/// FR-003: manifests that break the schema, refused naming the field.
const INVALID_MANIFEST: PermissionCase = PermissionCase {
    name: "invalid-manifest",
    setup: no_setup,
    act: |runtime, grants| {
        let unknown_field = manifest("local/probe", "1.0.0", &[("log", true)]).replacen(
            '{',
            r#"{"homepage":"https://example.com","#,
            1,
        );
        let uppercase_id = manifest("local/Probe", "1.0.0", &[("log", true)]);
        for (payload, field) in [(unknown_field, "homepage"), (uppercase_id, "id")] {
            let package = append_manifest_section(&fixture("probe"), payload.as_bytes());
            let error = assert_refused(runtime, &package, grants, ErrorKind::InvalidManifest);
            assert!(error.detail().contains(field), "{error}");
        }
    },
};

/// FR-004: two manifests, so a tool and the host could read different ones.
const TWO_MANIFESTS: PermissionCase = PermissionCase {
    name: "two-manifests",
    setup: no_setup,
    act: |runtime, grants| {
        let second = manifest("local/probe", "1.0.0", &[("log", true)]);
        let package = append_manifest_section(&probe("1.0.0", &[]), second.as_bytes());
        assert_refused(runtime, &package, grants, ErrorKind::ManifestMismatch);
    },
};

/// FR-004: the only manifest inside a nested module, where a tool could miss it.
const NESTED_MANIFEST: PermissionCase = PermissionCase {
    name: "nested-manifest",
    setup: no_setup,
    act: |runtime, grants| {
        let payload = manifest("local/probe", "1.0.0", &[])
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        let package = wat::parse_str(format!(
            r#"(component (core module (@custom "dashboard-manifest" "{payload}")))"#
        ))
        .expect("valid WAT");
        assert_refused(runtime, &package, grants, ErrorKind::ManifestMismatch);
    },
};

/// FR-008: a permission this host does not know.
const UNKNOWN_PERMISSION: PermissionCase = PermissionCase {
    name: "unknown-permission",
    setup: no_setup,
    act: |runtime, grants| {
        let payload = manifest("local/quiet", "1.0.0", &[("telepathy", true)]);
        let package = append_manifest_section(&fixture("quiet"), payload.as_bytes());
        let error = assert_refused(runtime, &package, grants, ErrorKind::UnknownPermission);
        assert_eq!(error.names(), ["telepathy"]);
    },
};

/// FR-006: an import the manifest does not declare, even with an approval.
const UNDECLARED_CAPABILITY: PermissionCase = PermissionCase {
    name: "undeclared-capability",
    setup: |grants| approve(grants, &probe("1.0.0", &[]), TEST_ORIGIN),
    act: |runtime, grants| {
        let error = assert_refused(
            runtime,
            &probe("1.0.0", &[]),
            grants,
            ErrorKind::NotDeclared,
        );
        assert_eq!(error.names(), ["log"]);
    },
};

/// FR-007: a required permission nobody granted.
const UNGRANTED_CAPABILITY: PermissionCase = PermissionCase {
    name: "ungranted-capability",
    setup: no_setup,
    act: |runtime, grants| {
        let package = probe("1.0.0", &[("log", true)]);
        let error = assert_refused(runtime, &package, grants, ErrorKind::NotGranted);
        assert_eq!(error.names(), ["log"]);
    },
};

/// FR-011: a new version that quietly makes an optional permission required.
const ESCALATING_UPDATE: PermissionCase = PermissionCase {
    name: "escalating-update",
    setup: |grants| approve(grants, &probe("1.0.0", &[("log", false)]), TEST_ORIGIN),
    act: |runtime, grants| {
        let first = load_from(
            runtime,
            &probe("1.0.0", &[("log", false)]),
            TEST_ORIGIN,
            grants,
        )
        .expect("the approved version loads");
        assert_eq!(
            render(&first, "{}").result.expect("it renders"),
            json!({"log": "not-granted"})
        );

        let before = probe_entry(grants);
        let update = probe("1.1.0", &[("log", true)]);
        let error = assert_refused(runtime, &update, grants, ErrorKind::NeedsNewApproval);
        assert_eq!(error.names(), ["log"]);
        assert_eq!(probe_entry(grants), before, "the approval is unchanged");
    },
};

/// FR-019: an optional capability that is off gives the plugin nothing.
const OPTIONAL_OFF: PermissionCase = PermissionCase {
    name: "optional-off",
    setup: |grants| approve(grants, &probe("1.0.0", &[("log", false)]), TEST_ORIGIN),
    act: |runtime, grants| {
        let plugin = load_from(
            runtime,
            &probe("1.0.0", &[("log", false)]),
            TEST_ORIGIN,
            grants,
        )
        .expect("an optional permission that is off does not stop the plugin");
        let report = render(&plugin, "{}");
        assert_eq!(
            report.result.expect("it renders"),
            json!({"log": "not-granted"})
        );
        assert!(
            report.logs.is_empty(),
            "the stand-in wrote {:?}",
            report.logs
        );
        assert_eq!(report.dropped_logs, 0);
    },
};

/// FR-020: another file that claims an approved plugin's id, byte for byte the same.
const SAME_ID_OTHER_ORIGIN: PermissionCase = PermissionCase {
    name: "same-id-other-origin",
    setup: |grants| approve(grants, &probe("1.0.0", &[("log", true)]), TEST_ORIGIN),
    act: |runtime, grants| {
        let before = probe_entry(grants);
        match load_from(
            runtime,
            &probe("1.0.0", &[("log", true)]),
            OTHER_ORIGIN,
            grants,
        ) {
            Ok(_) => panic!("a package from another origin got the approval's grants"),
            Err(error) => {
                assert_eq!(error.kind(), ErrorKind::NotGranted, "{error}");
                assert!(error.detail().contains(TEST_ORIGIN), "{error}");
            }
        }
        assert_eq!(probe_entry(grants), before, "the approval is unchanged");
    },
};

/// The probe with `log` granted, for `grant_record_untouched`: a plugin that uses a granted
/// capability, several times.
const PROBE: PermissionCase = PermissionCase {
    name: "probe",
    setup: |grants| approve(grants, &probe("1.0.0", &[("log", true)]), TEST_ORIGIN),
    act: |runtime, grants| {
        let plugin = load_from(
            runtime,
            &probe("1.0.0", &[("log", true)]),
            TEST_ORIGIN,
            grants,
        )
        .expect("the approved probe loads");
        for _ in 0..3 {
            assert_eq!(
                render(&plugin, "{}").result.expect("it renders"),
                json!({"log": "ok"})
            );
        }
    },
};

const PERMISSION_CASES: [&PermissionCase; 11] = [
    &NO_MANIFEST,
    &INVALID_MANIFEST,
    &TWO_MANIFESTS,
    &NESTED_MANIFEST,
    &UNKNOWN_PERMISSION,
    &UNDECLARED_CAPABILITY,
    &UNGRANTED_CAPABILITY,
    &ESCALATING_UPDATE,
    &OPTIONAL_OFF,
    &SAME_ID_OTHER_ORIGIN,
    &PROBE,
];

/// Runs a case against an in-memory record, then a well-behaved plugin on the same runtime.
fn run_case(case: &PermissionCase) {
    let runtime = runtime();
    let grants = Arc::new(GrantStore::in_memory());
    (case.setup)(&grants);
    (case.act)(&runtime, &grants);
    assert_host_still_works(&runtime);
}

#[test]
fn a_package_without_a_manifest_is_refused() {
    run_case(&NO_MANIFEST);
}

#[test]
fn a_manifest_that_breaks_the_schema_is_refused_naming_the_field() {
    run_case(&INVALID_MANIFEST);
}

#[test]
fn a_package_with_two_manifests_is_refused() {
    run_case(&TWO_MANIFESTS);
}

#[test]
fn a_manifest_in_a_nested_module_is_refused() {
    run_case(&NESTED_MANIFEST);
}

#[test]
fn an_unknown_permission_is_refused_naming_it() {
    run_case(&UNKNOWN_PERMISSION);
}

#[test]
fn an_undeclared_import_is_refused_even_when_approved() {
    run_case(&UNDECLARED_CAPABILITY);
}

#[test]
fn a_required_permission_nobody_granted_is_refused() {
    run_case(&UNGRANTED_CAPABILITY);
}

#[test]
fn an_update_that_requires_more_is_refused_and_changes_nothing() {
    run_case(&ESCALATING_UPDATE);
}

#[test]
fn an_optional_capability_that_is_off_gives_nothing() {
    run_case(&OPTIONAL_OFF);
}

#[test]
fn another_file_with_an_approved_id_gets_none_of_its_grants() {
    run_case(&SAME_ID_OTHER_ORIGIN);
}

/// FR-009, FR-010: no plugin, well-behaved or hostile, changes the grant record by running.
/// Each case runs against a record in a file; after its loads and renders, the file holds the
/// same bytes and the store the same generation as after its setup.
#[test]
fn grant_record_untouched() {
    let runtime = runtime();
    for case in PERMISSION_CASES {
        let dir = TempDir::new().expect("a temporary directory");
        let path = dir.path().join("grants.json");
        let grants = Arc::new(GrantStore::open(&path));
        (case.setup)(&grants);
        let bytes = std::fs::read(&path).ok();
        let generation = grants.generation();

        (case.act)(&runtime, &grants);

        assert_eq!(
            std::fs::read(&path).ok(),
            bytes,
            "{}: the record changed",
            case.name
        );
        assert_eq!(
            grants.generation(),
            generation,
            "{}: the store changed",
            case.name
        );
    }
    assert_host_still_works(&runtime);
}
