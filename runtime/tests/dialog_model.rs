//! The installation dialog's model (`specs/010-plugin-installation/research.md`, R6, R7; tasks
//! T017, T018): every permission and connector the manifest declares is in it, in the host's
//! words; the plugin's name and author are sanitised and bounded; the choice digest covers the
//! optionals and the warning; an update marks what changed; and the development tool shows the
//! same operations the dialog does.

use std::process::Command;

use runtime::flows::Source;
use runtime::install::dialog::{
    Change, DialogKind, ModelInput, choice_digest, model, permission_sentence,
};
use runtime::{CheckedPackage, Limits, PermissionKind, Runtime, RuntimeOptions, TestNetwork};
use serde_json::json;
use tempfile::TempDir;

use crate::common::servers::TestPki;
use crate::common::{fixture, fixture_path, repack};

fn account_echo() -> String {
    json!({
        "connector_version": 1, "id": "catalog/account-echo", "version": "1.0.0",
        "author": "Tests", "description": "A test service read as an account",
        "reserves": ["echo.test"], "destinations": { "api": "https://echo.test" },
        "credential": { "kind": "key", "required": true, "destinations": ["api"],
                        "header": "Authorization", "scheme": "Bearer" },
        "operations": { "look": { "kind": "read", "method": "GET", "destination": "api",
            "path": "/look", "parameters": {}, "response": { "format": "json", "headers": [] } } }
    })
    .to_string()
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
            catalog: vec![account_echo()],
        },
    )
    .expect("the runtime starts");
    (data, runtime)
}

fn install(package: &CheckedPackage, optional_on: &[String]) -> runtime::install::DialogModel {
    model(&ModelInput {
        kind: DialogKind::Install,
        package,
        previous: None,
        optional_on,
        remembered: &[],
    })
}

/// `plain-language`: every permission kind has the host's sentence, and every permission and
/// connector property of every install fixture is in the model.
#[test]
fn every_declaration_is_in_the_model_in_the_hosts_words() {
    for kind in PermissionKind::ALL {
        assert!(
            !permission_sentence(*kind, Some(1024)).is_empty(),
            "{kind:?}"
        );
    }
    let (_data, runtime) = runtime();
    for name in ["install-basic", "install-account"] {
        let package = runtime.check(&fixture(name)).expect("valid");
        let shown = install(&package, &[]);
        let manifest = package.manifest();
        assert_eq!(shown.asking.id, manifest.id().as_str());
        assert_eq!(shown.digest, package.approval_digest());
        for permission in manifest.permissions() {
            let line = shown
                .permissions
                .iter()
                .find(|line| line.name == permission.name)
                .unwrap_or_else(|| panic!("{name}: {} is missing", permission.name));
            assert_eq!(line.required, permission.required);
            assert!(line.sentence.ends_with('.'), "{}", line.sentence);
        }
        assert_eq!(shown.connectors.len(), manifest.connectors().len());
        for loaded in package.connectors() {
            let line = shown
                .connectors
                .iter()
                .find(|line| line.id == loaded.id())
                .unwrap_or_else(|| panic!("{name}: {} is missing", loaded.id()));
            assert_eq!(line.origin, "catalog");
            assert_eq!(line.required, loaded.required());
            let destinations: Vec<String> = loaded
                .connector()
                .destinations()
                .iter()
                .map(|origin| origin.serialization())
                .collect();
            assert_eq!(line.destinations, destinations);
            let operations: Vec<&str> = line
                .operations
                .iter()
                .map(|operation| operation.name.as_str())
                .collect();
            assert_eq!(operations, loaded.operations());
            for operation in &line.operations {
                assert_eq!(operation.review, "reviewed");
                assert!(operation.url.starts_with("https://"), "{}", operation.url);
            }
            assert!(!line.sentence.is_empty());
        }
    }
    let package = runtime.check(&fixture("install-account")).expect("valid");
    let shown = install(&package, &[]);
    let account = shown
        .connectors
        .iter()
        .find(|line| line.id == "catalog/account-echo")
        .and_then(|line| line.account.clone())
        .expect("the account is shown");
    assert!(!account.connected);
    assert!(
        account.sentence.contains("Nenhuma conta"),
        "{}",
        account.sentence
    );
}

#[test]
fn the_name_and_author_are_drawn_sanitised_and_bounded() {
    let (_data, runtime) = runtime();
    let manifest = json!({
        "manifest_version": 1,
        "id": "github.com/owner/loud",
        "version": "0.1.0",
        "author": format!("<b>Evil</b>\u{202E}gnp.exe{}", "x".repeat(80)),
        "name": format!("Vit\u{202E}rine\u{200B} <i>{}", "y".repeat(40)),
        "permissions": [{ "name": "log", "required": true }]
    });
    let package = runtime
        .check(&repack(&fixture("hello-widget"), &manifest.to_string()))
        .expect("valid");
    let shown = install(&package, &[]);
    for text in [&shown.asking.name, &shown.asking.author] {
        assert!(
            !text.contains('\u{202E}') && !text.contains('\u{200B}'),
            "{text:?}"
        );
    }
    assert!(
        shown.asking.name.chars().count() <= 40,
        "{}",
        shown.asking.name
    );
    assert!(
        shown.asking.author.chars().count() <= 60,
        "{}",
        shown.asking.author
    );
    assert!(
        shown.asking.name.starts_with("Vitrine <i>"),
        "{}",
        shown.asking.name
    );
    assert!(
        shown.asking.author.starts_with("<b>Evil</b>"),
        "{}",
        shown.asking.author
    );
    assert_eq!(shown.asking.id, "github.com/owner/loud");
}

#[test]
fn the_choice_digest_covers_the_optionals_and_the_warning() {
    let (_data, runtime) = runtime();
    let package = runtime.check(&fixture("install-account")).expect("valid");
    let off = install(&package, &[]);
    let on = install(&package, &["catalog/github".to_owned()]);
    assert!(off.warning.is_empty());
    assert_eq!(on.warning.len(), 1);
    assert!(
        matches!(&on.warning[0].source, Source::Account { connector } if connector == "catalog/account-echo")
    );
    assert_eq!(on.digest, off.digest);
    assert_ne!(on.choice, off.choice);
    assert_eq!(
        install(&package, &["catalog/github".to_owned()]).choice,
        on.choice
    );
    // The same choices with another warning shown are another choice
    assert_ne!(
        choice_digest(&on.digest, &["catalog/github".to_owned()], &[]),
        on.choice
    );
}

#[test]
fn an_update_marks_what_changed_and_lists_everything() {
    let (_data, runtime) = runtime();
    let before = runtime.check(&fixture("install-basic")).expect("valid");
    let after = runtime.check(&fixture("install-more")).expect("valid");
    let shown = model(&ModelInput {
        kind: DialogKind::Update,
        package: &after,
        previous: Some(&before),
        optional_on: &[],
        remembered: &[],
    });
    assert_eq!(shown.kind, DialogKind::Update);
    assert_eq!(shown.asking.installed_version.as_deref(), Some("0.1.0"));
    let change = |name: &str| {
        shown
            .permissions
            .iter()
            .find(|line| line.name == name)
            .and_then(|line| line.change)
    };
    assert_eq!(change("log"), Some(Change::Unchanged));
    assert_eq!(change("storage"), Some(Change::Unchanged));
    let github = shown
        .connectors
        .iter()
        .find(|line| line.id == "catalog/github")
        .expect("listed");
    assert_eq!(github.change, Some(Change::Changed));
    assert_eq!(
        github.destinations,
        vec!["https://api.github.com".to_owned()]
    );
}

/// The development tool's `permissions` shows each operation as the dialog's model does.
#[test]
fn the_development_tool_shows_the_operations_the_dialog_shows() {
    let (_data, runtime) = runtime();
    let package = runtime.check(&fixture("install-basic")).expect("valid");
    let shown = install(&package, &[]);
    let records = TempDir::new().expect("a temporary directory");
    let output = Command::new(env!("CARGO_BIN_EXE_runtime"))
        .arg("permissions")
        .arg(fixture_path("install-basic"))
        .arg("--grants")
        .arg(records.path().join("grants.json"))
        .arg("--data")
        .arg(records.path().join("data"))
        .output()
        .expect("the CLI runs");
    let text = String::from_utf8_lossy(&output.stdout);
    for connector in &shown.connectors {
        assert!(
            text.contains(&format!(
                "destinations: {}",
                connector.destinations.join(", ")
            )),
            "{text}"
        );
        for operation in &connector.operations {
            assert!(
                text.contains(&format!(
                    "{}  {} (reviewed)  {} {}",
                    operation.name, operation.kind, operation.method, operation.url
                )),
                "{text}"
            );
            let sends = if operation.sends.is_empty() {
                "nothing".to_owned()
            } else {
                operation.sends.join(", ")
            };
            assert!(text.contains(&format!("sends: {sends}")), "{text}");
        }
    }
}
