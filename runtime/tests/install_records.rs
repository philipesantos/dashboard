//! The installed record and the grant record's installation rules
//! (`specs/010-plugin-installation/data-model.md`; tasks T013, T014): `installed.json` round-trips,
//! refuses anything this host would not have written and writes under its lock; the grant record
//! reads versions 1 to 4 and writes version 5 with `account_sources`; an installation's grant is
//! built fresh, never keeping an optional an earlier entry left on (`stale-optional`).

use std::collections::BTreeMap;

use runtime::grants::{AccountState, ConnectorState, GrantStore, approval_digest};
use runtime::install::{Installed, InstalledStore};
use runtime::manifest::{Manifest, PluginId};
use runtime::{ErrorKind, Limits, Runtime, RuntimeOptions};
use serde_json::json;
use tempfile::TempDir;

use crate::common::fixture;

const DIGEST_A: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const DIGEST_B: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn installed(repository_id: u64, version: &str) -> Installed {
    Installed {
        repository_id,
        tag: format!("v{version}"),
        version: version.to_owned(),
        package_digest: DIGEST_A.to_owned(),
        compiled_digest: DIGEST_B.to_owned(),
        installed_at: "2026-11-12T20:00:00Z".to_owned(),
    }
}

#[test]
fn the_installed_record_round_trips() {
    let dir = TempDir::new().expect("a temporary directory");
    let store = InstalledStore::open(dir.path().join("host").join("installed.json"));
    assert!(store.read().expect("a missing file is empty").is_empty());
    store
        .insert("github.com/owner/one", installed(1, "1.0.0"))
        .expect("written");
    store
        .insert("github.com/owner/two", installed(2, "0.3.1"))
        .expect("written");
    let plugins = store.read().expect("read");
    assert_eq!(plugins.len(), 2);
    let one = &plugins["github.com/owner/one"];
    assert_eq!(one, &installed(1, "1.0.0"));
    assert_eq!(one.origin(), "github:1");
    assert_eq!(one.directory(), DIGEST_A.trim_start_matches("sha256:"));
    let written: serde_json::Value =
        serde_json::from_slice(&std::fs::read(store.path()).expect("the file")).expect("JSON");
    assert_eq!(written["record_version"], 1);
    assert!(store.remove("github.com/owner/one").expect("removed"));
    assert!(!store.remove("github.com/owner/one").expect("already gone"));
    assert_eq!(store.read().expect("read").len(), 1);
}

#[test]
fn an_installed_record_this_host_would_not_write_is_an_error_never_empty() {
    let dir = TempDir::new().expect("a temporary directory");
    let path = dir.path().join("installed.json");
    let store = InstalledStore::open(&path);
    let entry = json!({
        "repository_id": 1, "tag": "v1.0.0", "version": "1.0.0",
        "package_digest": DIGEST_A, "compiled_digest": DIGEST_B,
        "installed_at": "2026-11-12T20:00:00Z"
    });
    let mut unknown_field = entry.clone();
    unknown_field["extra"] = json!(true);
    let mut bad_digest = entry.clone();
    bad_digest["compiled_digest"] = json!("sha256:XYZ");
    let cases = [
        json!({ "record_version": 2, "plugins": {} }),
        json!({ "record_version": 1, "plugins": { "github.com/owner/one": unknown_field } }),
        json!({ "record_version": 1, "plugins": { "local/one": entry } }),
        json!({ "record_version": 1, "plugins": { "github.com/owner/one": bad_digest } }),
        json!({ "record_version": 1, "plugins": {}, "more": 1 }),
    ];
    for case in cases {
        std::fs::write(&path, case.to_string()).expect("written");
        let error = store.read().expect_err("refused");
        assert_eq!(error.kind(), ErrorKind::NotGranted, "{case}: {error}");
    }
    std::fs::write(&path, b"not json").expect("written");
    assert!(store.read().is_err());
    // A local id is never recorded
    std::fs::remove_file(&path).expect("removed");
    assert!(store.insert("local/one", installed(1, "1.0.0")).is_err());
}

/// A version 4 grant record, as feature 005 wrote it.
fn version_4_record(origin: &str) -> serde_json::Value {
    json!({
        "record_version": 4,
        "plugins": {
            "github.com/owner/one": {
                "origin": origin,
                "granted": ["log"],
                "approved_version": "1.0.0",
                "approved_digest": DIGEST_A,
                "connectors": {}
            }
        }
    })
}

#[test]
fn the_grant_record_reads_version_4_and_writes_version_5() {
    let dir = TempDir::new().expect("a temporary directory");
    let path = dir.path().join("grants.json");
    std::fs::write(&path, version_4_record("github:1").to_string()).expect("written");
    let store = GrantStore::open(&path);
    let id = PluginId::parse("github.com/owner/one").expect("an id");
    let entry = store.entry(&id).expect("read").expect("present");
    assert!(entry.account_sources.is_empty());
    assert!(store.remove(&id).expect("removed"));
    let written: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).expect("the file")).expect("JSON");
    assert_eq!(written["record_version"], 5);

    // A version 4 record cannot carry what version 5 brought
    let mut record = version_4_record("github:1");
    record["plugins"]["github.com/owner/one"]["account_sources"] = json!(["catalog/account-echo"]);
    std::fs::write(&path, record.to_string()).expect("written");
    assert!(GrantStore::open(&path).entry(&id).is_err());
}

/// `install-basic`, checked by a runtime whose catalog is the built-in one.
fn install_basic() -> (TempDir, runtime::CheckedPackage) {
    let data = TempDir::new().expect("a temporary directory");
    let runtime =
        Runtime::new(Limits::default(), RuntimeOptions::new(data.path())).expect("the runtime");
    let checked = runtime
        .check(&fixture("install-basic"))
        .expect("install-basic is valid");
    (data, checked)
}

#[test]
fn an_installation_grants_only_the_required_and_what_the_dialog_turned_on() {
    let (_data, checked) = install_basic();
    let grants = GrantStore::in_memory();
    let origin = "github:1";
    let digest = checked.approval_digest();
    checked
        .install(&grants, origin, &digest, &[])
        .expect("installed");
    let entry = grants
        .entry(checked.manifest().id())
        .expect("read")
        .expect("present");
    assert_eq!(entry.granted, vec!["log".to_owned(), "storage".to_owned()]);
    assert!(!entry.is_connector_granted("catalog/github"));
    assert_eq!(entry.storage_quota_bytes, Some(65536));

    checked
        .install(&grants, origin, &digest, &["catalog/github".to_owned()])
        .expect("installed");
    let entry = grants
        .entry(checked.manifest().id())
        .expect("read")
        .expect("present");
    assert!(entry.is_connector_granted("catalog/github"));
}

/// `stale-optional`: an entry an interrupted installation left, with the optional connector on,
/// does not turn it on in the next installation, which shows it off.
#[test]
fn an_optional_left_on_by_an_earlier_entry_stays_off() {
    let (_data, checked) = install_basic();
    let grants = GrantStore::in_memory();
    let origin = "github:1";
    let digest = checked.approval_digest();
    checked
        .approve(&grants, origin, Some(&digest))
        .expect("approved");
    checked
        .set_connector(&grants, origin, "catalog/github", true)
        .expect("turned on");
    checked
        .install(&grants, origin, &digest, &[])
        .expect("installed");
    let entry = grants
        .entry(checked.manifest().id())
        .expect("read")
        .expect("present");
    assert!(!entry.is_connector_granted("catalog/github"));
}

#[test]
fn an_installation_refuses_a_stale_digest_or_a_choice_it_does_not_declare() {
    let (_data, checked) = install_basic();
    let grants = GrantStore::in_memory();
    let error = checked
        .install(&grants, "github:1", "sha256:00", &[])
        .expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::ApprovalChanged);
    let digest = checked.approval_digest();
    for name in ["storage", "log", "catalog/elsewhere", "telepathy"] {
        let error = checked
            .install(&grants, "github:1", &digest, &[name.to_owned()])
            .expect_err("refused");
        assert_eq!(error.kind(), ErrorKind::NotDeclared, "{name}: {error}");
    }
    assert!(
        grants
            .entry(checked.manifest().id())
            .expect("read")
            .is_none()
    );
}

/// A manifest with storage and an account connector, and the states an approval records.
fn account_and_storage(
    version: &str,
    account: bool,
    other: bool,
) -> (Manifest, Vec<ConnectorState>) {
    let mut connectors = Vec::new();
    let mut states = Vec::new();
    if account {
        connectors.push(json!({ "id": "catalog/account-echo", "major": 1,
            "operations": ["look"], "required": true, "account": true }));
        states.push(ConnectorState {
            id: "catalog/account-echo".to_owned(),
            origin: "catalog".to_owned(),
            version: "1.0.0".to_owned(),
            required: true,
            destinations: vec!["https://echo.test".to_owned()],
            operations: BTreeMap::from([("look".to_owned(), "sha256:01".to_owned())]),
            account: Some(AccountState {
                uses: true,
                required: true,
                credential: "sha256:02".to_owned(),
                access: Vec::new(),
            }),
        });
    }
    if other {
        connectors.push(json!({ "id": "catalog/elsewhere", "major": 1,
            "operations": ["send"], "required": true }));
        states.push(ConnectorState {
            id: "catalog/elsewhere".to_owned(),
            origin: "catalog".to_owned(),
            version: "1.0.0".to_owned(),
            required: true,
            destinations: vec!["https://elsewhere.test".to_owned()],
            operations: BTreeMap::from([("send".to_owned(), "sha256:03".to_owned())]),
            account: None,
        });
    }
    let manifest = json!({
        "manifest_version": 1,
        "id": "github.com/owner/one",
        "version": version,
        "author": "Tests",
        "permissions": [{ "name": "storage", "required": true, "quota_bytes": 4096 }],
        "connectors": connectors
    });
    let manifest = Manifest::parse(manifest.to_string().as_bytes()).expect("a valid manifest");
    (manifest, states)
}

#[test]
fn an_account_held_with_storage_is_remembered_after_it_goes() {
    let grants = GrantStore::in_memory();
    let (v1, states) = account_and_storage("1.0.0", true, false);
    let expected = approval_digest(DIGEST_A, &v1, &states);
    grants
        .install(&v1, "github:1", DIGEST_A, &states, &expected, &[])
        .expect("installed");
    let id = v1.id().clone();
    let entry = grants.entry(&id).expect("read").expect("present");
    assert_eq!(
        entry.account_sources,
        vec!["catalog/account-echo".to_owned()]
    );

    // Version 2 drops the account and adds another destination: the store still holds what
    // was read as the account
    let (v2, states) = account_and_storage("2.0.0", false, true);
    let expected = approval_digest(DIGEST_B, &v2, &states);
    grants
        .approve_connected(&v2, "github:1", DIGEST_B, &states, &expected)
        .expect("approved");
    let entry = grants.entry(&id).expect("read").expect("present");
    assert_eq!(
        entry.account_sources,
        vec!["catalog/account-echo".to_owned()]
    );

    // From another origin it is another store: nothing is remembered
    grants
        .approve_connected(&v2, "github:2", DIGEST_B, &states, &expected)
        .expect("approved");
    let entry = grants.entry(&id).expect("read").expect("present");
    assert!(entry.account_sources.is_empty());
}
