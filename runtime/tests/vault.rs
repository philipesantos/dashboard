//! The development vault on the system's keychain (`specs/005-plugin-credentials/`, User Story
//! 4, research R5): Credential Manager on Windows, the login keychain on macOS, the Secret Service
//! on Linux, which CI runs in its own session bus. Each test uses entries of a vault id of its
//! own, under a lock for the whole binary, and deletes them before it ends.
//!
//! `keychain_locked_call` runs on the mock store instead: a keychain that refuses access cannot
//! be set up on every system.

mod common;

use std::sync::Mutex;
use std::time::Duration;

use common::servers::TestPki;
use common::signin::SignInService;
use common::{World, answers, fixture, repack};
use runtime::{Limits, TestEntry, TestKeychain, VaultFailure};
use serde_json::{Value, json};

/// The keychain is the user's: one test at a time touches it.
static KEYCHAIN: Mutex<()> = Mutex::new(());

/// What each operation waits for the keychain, as the development tool does.
const BOUND: Duration = Duration::from_secs(120);

/// A vault id no other run uses.
fn vault_id() -> String {
    format!(
        "test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )
}

/// Deletes `user`'s entry when dropped, so a failing test leaves nothing behind.
struct Cleanup<'a> {
    keychain: &'a TestKeychain,
    users: Vec<String>,
}

impl Drop for Cleanup<'_> {
    fn drop(&mut self) {
        for user in &self.users {
            let _ = self.keychain.delete(user, BOUND);
        }
    }
}

/// `vault_round_trip`: write, read, overwrite, delete and no entry, on the system's keychain;
/// a document of 2,560 bytes is kept, and one byte more refused before the keychain sees it.
#[test]
fn vault_round_trip() {
    let _lock = KEYCHAIN
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let limits = Limits::default();
    let keychain = TestKeychain::system(&limits);
    let id = vault_id();
    let user = TestKeychain::user(&id, "local/notes", "/tests/notes.json");
    let big = TestKeychain::user(&id, "local/big", "/tests/big.json");
    let _cleanup = Cleanup {
        keychain: &keychain,
        users: vec![user.clone(), big.clone()],
    };

    assert_eq!(keychain.read(&user, BOUND), Ok(None));
    let account = TestEntry::Account {
        access: "at_round-trip.1".to_owned(),
        refresh: Some("rt_round-trip.1".to_owned()),
    };
    keychain.write(&user, &account, BOUND).unwrap();
    assert_eq!(keychain.read(&user, BOUND), Ok(Some(account)));
    let replaced = TestEntry::Account {
        access: "at_round-trip.2".to_owned(),
        refresh: None,
    };
    keychain.write(&user, &replaced, BOUND).unwrap();
    assert_eq!(keychain.read(&user, BOUND), Ok(Some(replaced)));
    assert_eq!(keychain.delete(&user, BOUND), Ok(true));
    assert_eq!(keychain.read(&user, BOUND), Ok(None));
    assert_eq!(keychain.delete(&user, BOUND), Ok(false));

    // `{"key":"…"}` is 10 bytes around the key
    let fits = TestEntry::Key("k".repeat(limits.max_secret_bytes - 10));
    keychain.write(&big, &fits, BOUND).unwrap();
    assert_eq!(keychain.read(&big, BOUND), Ok(Some(fits)));
    let over = TestEntry::Key("k".repeat(limits.max_secret_bytes - 9));
    let error = keychain.write(&big, &over, BOUND).unwrap_err();
    assert!(error.contains("2561 bytes"), "{error}");
    assert!(!error.contains("kkkk"), "{error}");
    // The refused document changed nothing
    assert!(
        matches!(keychain.read(&big, BOUND), Ok(Some(TestEntry::Key(key))) if key.len() == limits.max_secret_bytes - 10)
    );
}

/// `keychain_locked_call`: the keychain refusing access during a call answers
/// `failed(unavailable)`, naming the keychain, and nothing is sent, whether the credential is
/// required or optional.
#[test]
fn keychain_locked_call() {
    for required in [true, false] {
        let pki = TestPki::new();
        let service = SignInService::start(&pki);
        let test = runtime::TestNetwork {
            routes: vec![service.route()],
            public_for_tests: vec![service.server.address()],
            root: pki.root(),
            catalog: Vec::new(),
        };
        let world = World::seam(test, true);
        let mut notes: Value = serde_json::from_str(include_str!(
            "../../plugins/examples/private-notes/connectors/notes.json"
        ))
        .unwrap();
        notes["credential"]["required"] = json!(required);
        let path = world.write_connector("notes", &notes);
        let origin = std::fs::canonicalize(path)
            .unwrap()
            .to_string_lossy()
            .into_owned();
        world
            .runtime
            .connect_account("local/notes", Some("dashboard-dev"), &service.browser())
            .unwrap();
        let manifest = json!({
            "manifest_version": 1, "id": "local/locked-probe", "version": "0.1.0", "author": "A",
            "permissions": [],
            "connectors": [{ "id": "local/notes", "major": 1, "operations": ["list-notes"], "required": true, "account": true }]
        });
        let plugin =
            world.approve_and_load(&repack(&fixture("account-probe"), &manifest.to_string()));
        world
            .runtime
            .test_vault_fail_next("local/notes", &origin, VaultFailure::NoStorageAccess);
        let before = service.requests_to("/notes").len();
        let report = plugin.update(
            &json!({ "calls": [{ "via": "accounts", "connector": "local/notes", "operation": "list-notes" }] })
                .to_string(),
        );
        let answers = answers(report.result.as_ref().unwrap());
        assert_eq!(
            answers[0]["error"],
            json!("failed:unavailable"),
            "required {required}: {answers:?}"
        );
        assert!(
            answers[0]["detail"]
                .as_str()
                .is_some_and(|detail| detail.contains("keychain")),
            "{answers:?}"
        );
        assert_eq!(
            service.requests_to("/notes").len(),
            before,
            "required {required}"
        );
    }
}
