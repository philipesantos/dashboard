//! Plugin contract v0: loading packages and rendering the example plugin (User Stories 1 and 3).

mod common;

use common::fixture;
use runtime::{ErrorKind, Limits, LogLevel, LogMessage, Runtime};
use serde_json::json;

fn runtime() -> Runtime {
    Runtime::new(Limits::default()).expect("the runtime starts with the default limits")
}

/// Loads `bytes` and returns the kind and detail of the error, failing if loading succeeds.
fn load_error(runtime: &Runtime, bytes: &[u8]) -> (ErrorKind, String) {
    match runtime.load(bytes) {
        Ok(_) => panic!("the package was loaded, but it should have been refused"),
        Err(error) => (error.kind(), error.detail().to_owned()),
    }
}

#[test]
fn hello_widget_renders_its_greeting_and_one_log() {
    let runtime = runtime();
    let plugin = runtime
        .load(&fixture("hello-widget"))
        .expect("hello-widget loads");

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
    let plugin = runtime
        .load(&fixture("hello-widget"))
        .expect("hello-widget loads");

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
    let plugin = runtime
        .load(&fixture("hello-widget"))
        .expect("hello-widget loads");

    for config in ["{bad", r#"{"name":5}"#] {
        let report = plugin.render(config);
        let error = report.result.expect_err("the configuration is refused");
        assert_eq!(error.kind(), ErrorKind::PluginError, "config {config}");
        assert!(!error.detail().is_empty());
    }
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
    let runtime = Runtime::new(Limits {
        max_package_bytes: bytes.len() - 1,
        ..Limits::default()
    })
    .unwrap();
    assert_eq!(load_error(&runtime, &bytes).0, ErrorKind::InvalidPackage);
}

#[test]
fn a_component_without_the_renderer_export_does_not_match_the_contract() {
    let component = wat::parse_str("(component)").unwrap();
    assert_eq!(
        load_error(&runtime(), &component).0,
        ErrorKind::ContractMismatch
    );
}

#[test]
fn a_component_built_for_another_contract_version_does_not_match() {
    // A plugin built for 0.2.0 imports that version's log interface, with its functions.
    let component = wat::parse_str(
        r#"(component
             (import "dashboard:widget/log@0.2.0" (instance
               (export "write" (func (param "message" string))))))"#,
    )
    .unwrap();
    let (kind, detail) = load_error(&runtime(), &component);
    assert_eq!(kind, ErrorKind::ContractMismatch);
    assert!(
        detail.contains("dashboard:widget/log@0.2.0"),
        "the detail should name the import: {detail}"
    );
}

#[test]
fn a_component_using_a_turned_off_proposal_is_refused() {
    let component = wat::parse_str("(component (core module (memory i64 1)))").unwrap();
    assert_eq!(
        load_error(&runtime(), &component).0,
        ErrorKind::InvalidPackage
    );
}
