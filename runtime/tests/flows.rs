//! The dangerous combination rule (`specs/010-plugin-installation/research.md`, R7; tasks T015,
//! T016): judged on the grants a widget would hold after a change and on the account sources its
//! store may still hold, a flow is found exactly when a source sits beside another destination.

use runtime::flows::{Flow, Source, judge};
use runtime::{CheckedPackage, Limits, Runtime, RuntimeOptions, TestNetwork};
use serde_json::{Value, json};
use tempfile::TempDir;

use crate::common::servers::TestPki;
use crate::common::{fixture, repack};

/// `catalog/account-echo`: a catalog connector whose credential is for `https://echo.test`.
fn account_echo() -> Value {
    json!({
        "connector_version": 1,
        "id": "catalog/account-echo",
        "version": "1.0.0",
        "author": "Tests",
        "description": "A test service read as an account",
        "reserves": ["echo.test"],
        "destinations": { "api": "https://echo.test" },
        "credential": {
            "kind": "key", "required": true, "destinations": ["api"],
            "header": "Authorization", "scheme": "Bearer"
        },
        "operations": {
            "look": {
                "kind": "read", "method": "GET", "destination": "api", "path": "/look",
                "parameters": {}, "response": { "format": "json", "headers": [] }
            }
        }
    })
}

/// `catalog/shout`, which names its destination in capitals.
fn shout() -> Value {
    json!({
        "connector_version": 1,
        "id": "catalog/shout",
        "version": "1.0.0",
        "author": "Tests",
        "description": "A test service named loudly",
        "reserves": ["shout.test"],
        "destinations": { "api": "https://SHOUT.Test" },
        "operations": {
            "send": {
                "kind": "read", "method": "GET", "destination": "api", "path": "/send",
                "parameters": {}, "response": { "format": "json", "headers": [] }
            }
        }
    })
}

fn runtime() -> (TempDir, Runtime) {
    let data = TempDir::new().expect("a temporary directory");
    let runtime = Runtime::with_test_network(
        Limits::default(),
        RuntimeOptions::new(data.path()),
        TestNetwork {
            routes: Vec::new(),
            public_for_tests: Vec::new(),
            root: TestPki::new().root(),
            catalog: vec![account_echo().to_string(), shout().to_string()],
        },
    )
    .expect("the runtime starts");
    (data, runtime)
}

fn checked(runtime: &Runtime, package: &[u8]) -> CheckedPackage {
    runtime.check(package).expect("a valid package")
}

fn account(connector: &str, destinations: &[&str]) -> Flow {
    Flow {
        source: Source::Account {
            connector: connector.to_owned(),
        },
        destinations: destinations.iter().map(|d| (*d).to_owned()).collect(),
    }
}

/// `ask-little-then-more`: every subset of `install-account`'s optionals. Its required account
/// connector alone is no flow, since its only destination is its credential's; the optional
/// GitHub connector turned on beside it is one.
#[test]
fn a_flow_exactly_when_an_account_sits_beside_another_destination() {
    let (_data, runtime) = runtime();
    let package = checked(&runtime, &fixture("install-account"));
    let required = |id: &str| id == "catalog/account-echo";
    assert_eq!(judge(&package, &required, &[]), Vec::new());
    let with_github = |id: &str| id == "catalog/account-echo" || id == "catalog/github";
    assert_eq!(
        judge(&package, &with_github, &[]),
        vec![account("catalog/account-echo", &["https://api.github.com"])]
    );
}

#[test]
fn no_source_no_warning() {
    let (_data, runtime) = runtime();
    let package = checked(&runtime, &fixture("install-basic"));
    for granted in [false, true] {
        assert_eq!(judge(&package, &|_| granted, &[]), Vec::new());
    }
}

/// `account-then-elsewhere`: the account connector is gone, but its data may sit in the store,
/// so a destination added later is a flow from it.
#[test]
fn a_remembered_account_is_still_a_source() {
    let (_data, runtime) = runtime();
    let package = checked(&runtime, &fixture("install-basic"));
    let remembered = vec!["catalog/account-echo".to_owned()];
    assert_eq!(judge(&package, &|_| false, &remembered), Vec::new());
    assert_eq!(
        judge(&package, &|_| true, &remembered),
        vec![account("catalog/account-echo", &["https://api.github.com"])]
    );
}

/// `flow-not-hidden`: a destination spelled in capitals is found in its normal form, a trailing
/// dot never loads at all, and an account connector declared required beside an optional
/// destination is judged with that destination as soon as it is on.
#[test]
fn a_flow_cannot_be_hidden_by_spelling_or_by_order() {
    let (_data, runtime) = runtime();
    let manifest = json!({
        "manifest_version": 1,
        "id": "github.com/owner/loud",
        "version": "0.1.0",
        "author": "Tests",
        "permissions": [{ "name": "log", "required": true }],
        "connectors": [
            { "id": "catalog/account-echo", "major": 1, "operations": ["look"],
              "required": true, "account": true },
            { "id": "catalog/shout", "major": 1, "operations": ["send"], "required": false }
        ]
    });
    let package = checked(
        &runtime,
        &repack(&fixture("account-probe"), &manifest.to_string()),
    );
    let on = |id: &str| id == "catalog/account-echo" || id == "catalog/shout";
    let flows = judge(&package, &on, &[]);
    assert_eq!(
        flows,
        vec![account("catalog/account-echo", &["https://shout.test"])]
    );
    assert_eq!(
        judge(&package, &|id| id == "catalog/account-echo", &[]),
        Vec::new()
    );

    let mut dotted = shout();
    dotted["destinations"]["api"] = json!("https://shout.test.");
    let refused = Runtime::with_test_network(
        Limits::default(),
        RuntimeOptions::new(TempDir::new().expect("a directory").path()),
        TestNetwork {
            routes: Vec::new(),
            public_for_tests: Vec::new(),
            root: TestPki::new().root(),
            catalog: vec![dotted.to_string()],
        },
    );
    assert!(
        refused.is_err_and(|error| error.to_string().contains("ends with a dot")),
        "a trailing dot loaded"
    );
}
