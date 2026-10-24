//! Feature 004: the storage cases of `specs/004-plugin-storage/data-model.md` (Hostile cases,
//! FR-017), the gate case of Phase 2: no plugin reads or changes another plugin's storage, or
//! anything outside its own store.
//!
//! Every case runs in a [`Vault`]: a runtime with a data directory and a grant record file of its
//! own. Each case is wrapped in [`Vault::contained`], which is the `host-records-untouched` case:
//! it takes a snapshot of every file of the data directory (the grant record, kept data, the
//! request log and every store) after the case's setup, and after the case compares them byte
//! for byte, except what the case changes on purpose. Then the well-behaved `update-counter`
//! updates and renders normally, on the same runtime.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use runtime::{ErrorKind, Limits, Plugin};
use serde_json::{Value, json};

use crate::common::{World, fixture, render_steps, repack, steps, stored_manifest};

const QUOTA: u64 = 64 * 1024;

/// A runtime whose data directory and grant record are its own.
pub(crate) struct Vault {
    pub(crate) world: World,
}

/// Every file of a data directory and its bytes, lock files left out: reading the grant record
/// creates its lock.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Snapshot(BTreeMap<PathBuf, Vec<u8>>);

impl Snapshot {
    fn of(dir: &Path) -> Snapshot {
        let mut files = BTreeMap::new();
        let mut pending = vec![dir.to_owned()];
        while let Some(dir) = pending.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    pending.push(path);
                } else if path.extension().is_none_or(|extension| extension != "lock") {
                    files.insert(path.clone(), std::fs::read(&path).unwrap());
                }
            }
        }
        Snapshot(files)
    }

    /// The files that differ from `self` in `after`: changed, added or removed.
    fn changes(&self, after: &Snapshot) -> Vec<PathBuf> {
        let mut changed: Vec<PathBuf> = self
            .0
            .keys()
            .chain(after.0.keys())
            .filter(|path| self.0.get(*path) != after.0.get(*path))
            .cloned()
            .collect();
        changed.sort();
        changed.dedup();
        changed
    }
}

impl Vault {
    pub(crate) fn new() -> Vault {
        Vault::with_limits(Limits::default())
    }

    pub(crate) fn with_limits(limits: Limits) -> Vault {
        Vault {
            world: World::production_with(limits, false),
        }
    }

    pub(crate) fn data(&self) -> &Path {
        self.world.data.path()
    }

    /// A path under the data directory's parent that stands for a package file's origin.
    pub(crate) fn origin(&self, name: &str) -> String {
        self.data()
            .join("packages")
            .join(format!("{name}.wasm"))
            .to_str()
            .unwrap()
            .to_owned()
    }

    /// `store-probe` as `id`, declaring required storage with `quota`.
    pub(crate) fn probe(id: &str, quota: u64) -> Vec<u8> {
        repack(
            &fixture("store-probe"),
            &stored_manifest(id, "0.1.0", true, quota),
        )
    }

    /// Approves and loads `package` from `origin`.
    pub(crate) fn plugin(&self, package: &[u8], origin: &str) -> Plugin {
        self.world.approve_at(package, origin);
        self.world.load_at(package, origin)
    }

    /// The file of the store of `id` from `origin`.
    pub(crate) fn store_file(&self, id: &str, origin: &str) -> PathBuf {
        std::fs::canonicalize(self.data())
            .unwrap()
            .join("storage")
            .join(format!("{}.sqlite", file_hash(id, origin)))
    }

    /// The files a case's own plugin changes on purpose: its store, and the kept data of its
    /// updates (feature 003).
    pub(crate) fn own(&self, id: &str, origin: &str) -> Vec<PathBuf> {
        vec![
            self.store_file(id, origin),
            self.data()
                .join("widget-data")
                .join(format!("{}.json", file_hash(id, origin))),
        ]
    }

    /// Runs `case` between two snapshots of the data directory, and fails unless the only files
    /// that changed are those in `allowed`, as `host-records-untouched` requires; then checks that
    /// the well-behaved example still works.
    pub(crate) fn contained(&self, allowed: &[PathBuf], case: impl FnOnce()) {
        let canonical = |path: &Path| {
            path.parent()
                .and_then(|parent| std::fs::canonicalize(parent).ok())
                .map_or_else(
                    || path.to_owned(),
                    |parent| parent.join(path.file_name().unwrap()),
                )
        };
        let before = Snapshot::of(self.data());
        case();
        let after = Snapshot::of(self.data());
        let allowed: Vec<PathBuf> = allowed.iter().map(|path| canonical(path)).collect();
        for changed in before.changes(&after) {
            assert!(
                allowed.contains(&canonical(&changed)),
                "the case changed {}, which is not its own",
                changed.display()
            );
        }
        self.assert_well_behaved();
    }

    /// Right after a hostile case, the example updates and renders normally on the same
    /// runtime (User Story 2, acceptance scenario 6).
    pub(crate) fn assert_well_behaved(&self) {
        let counter = self.world.approve_and_load(&fixture("update-counter"));
        let report = counter.update(r#"{"note":"after the case"}"#);
        report
            .result
            .unwrap_or_else(|error| panic!("update-counter updates: {error}"));
        let content = counter
            .render("{}")
            .result
            .unwrap_or_else(|error| panic!("update-counter renders: {error}"));
        assert!(
            content["text"].as_str().unwrap().starts_with("Updated "),
            "{content}"
        );
    }
}

/// The hex SHA-256 of `"<id>\n<origin>"`, which names a plugin's store and kept data.
fn file_hash(id: &str, origin: &str) -> String {
    <sha2::Sha256 as sha2::Digest>::digest(format!("{id}\n{origin}"))
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(crate) fn set(name: &str, value: &str) -> Value {
    json!({ "op": "set", "name": name, "value": value })
}

pub(crate) fn get(name: &str) -> Value {
    json!({ "op": "get", "name": name })
}

pub(crate) fn list(prefix: &str) -> Value {
    json!({ "op": "list", "prefix": prefix })
}

pub(crate) fn hex(text: &str) -> Value {
    json!({ "ok": { "hex": text.bytes().map(|byte| format!("{byte:02x}")).collect::<String>() } })
}

/// The answers of a `store-probe` run.
pub(crate) fn answers(content: &Value) -> Vec<Value> {
    content["answers"]
        .as_array()
        .unwrap_or_else(|| panic!("store-probe answered {content}"))
        .clone()
}

/// An update of `plugin` that makes `step_list` and returns `ok`.
pub(crate) fn update(plugin: &Plugin, step_list: &[Value]) -> Vec<Value> {
    let report = plugin.update(&steps(step_list, None));
    answers(
        &report
            .result
            .unwrap_or_else(|error| panic!("the update: {error}")),
    )
}

/// A render of `plugin` that makes `step_list`.
pub(crate) fn render(plugin: &Plugin, step_list: &[Value]) -> Vec<Value> {
    let report = plugin.render(&render_steps(step_list));
    answers(
        &report
            .result
            .unwrap_or_else(|error| panic!("the render: {error}")),
    )
}

// User Story 2: no plugin reaches another plugin's storage

#[test]
fn other_plugin() {
    let vault = Vault::new();
    let a_origin = vault.origin("store-a");
    let a = vault.plugin(&Vault::probe("local/store-a", QUOTA), &a_origin);
    update(
        &a,
        &[set("secret", "A's value"), set("notes/0", "A's note")],
    );
    let b_origin = vault.origin("store-b");
    let b = vault.plugin(&Vault::probe("local/store-b", QUOTA), &b_origin);
    vault.contained(&vault.own("local/store-b", &b_origin), || {
        let found = update(
            &b,
            &[
                get("secret"),
                get("notes/0"),
                list(""),
                list("notes/"),
                set("secret", "B's value"),
                json!({ "op": "delete", "name": "notes/0" }),
            ],
        );
        assert_eq!(found[0], json!({ "ok": null }), "B reads nothing of A's");
        assert_eq!(found[1], json!({ "ok": null }));
        assert_eq!(found[2], json!({ "ok": { "names": [], "next": null } }));
        assert_eq!(found[3], json!({ "ok": { "names": [], "next": null } }));
    });
    assert_eq!(
        render(&a, &[get("secret"), get("notes/0")]),
        [hex("A's value"), hex("A's note")]
    );
    assert_eq!(render(&b, &[get("secret")]), [hex("B's value")]);
}

#[test]
fn same_id_other_origin() {
    let vault = Vault::new();
    let first_origin = vault.origin("first");
    let package = Vault::probe("local/store-probe", QUOTA);
    let first = vault.plugin(&package, &first_origin);
    update(&first, &[set("secret", "the first one's")]);
    let first_store = vault.store_file("local/store-probe", &first_origin);
    let copy_origin = vault.origin("copy");
    vault.contained(&[], || {
        let error = vault
            .world
            .try_load_at(&package, &copy_origin)
            .err()
            .expect("a copy at another path inherits no approval");
        assert_eq!(error.kind(), ErrorKind::NotGranted, "{error}");
    });
    // Approved in its own right: its own, empty store
    let mut allowed = vault.own("local/store-probe", &copy_origin);
    allowed.push(vault.data().join("grants.json"));
    vault.contained(&allowed, || {
        let copy = vault.plugin(&package, &copy_origin);
        let found = update(&copy, &[get("secret"), list("")]);
        assert_eq!(found[0], json!({ "ok": null }));
        assert_eq!(found[1], json!({ "ok": { "names": [], "next": null } }));
    });
    assert!(first_store.is_file());
    // The reverse: another id at the first one's path gets its own store too
    let other = Vault::probe("local/other-probe", QUOTA);
    let mut allowed = vault.own("local/other-probe", &first_origin);
    allowed.push(vault.data().join("grants.json"));
    vault.contained(&allowed, || {
        let replaced = vault.plugin(&other, &first_origin);
        let found = update(&replaced, &[get("secret"), list("")]);
        assert_eq!(found[0], json!({ "ok": null }));
        assert_eq!(found[1], json!({ "ok": { "names": [], "next": null } }));
    });
    // The first plugin approved again from its path finds its data
    let again = vault.plugin(&package, &first_origin);
    assert_eq!(render(&again, &[get("secret")]), [hex("the first one's")]);
}

#[test]
fn name_injection() {
    let vault = Vault::new();
    let origin = vault.origin("injection");
    let plugin = vault.plugin(&Vault::probe("local/store-probe", QUOTA), &origin);
    let store = vault.store_file("local/store-probe", &origin);
    let names = [
        "../../../grants.json",
        "/etc/passwd",
        "..\\..\\windows",
        "a\u{0}b",
        "it's \"quoted\"",
        "line\nbreak\r\n",
        "'; DROP TABLE entries; --",
        "ATTACH DATABASE '/tmp/x' AS y",
        "a%",
        "a_c",
        "ab",
    ];
    vault.contained(&vault.own("local/store-probe", &origin), || {
        let writes: Vec<Value> = names
            .iter()
            .enumerate()
            .map(|(index, name)| set(name, &format!("value {index}")))
            .collect();
        update(&plugin, &writes);
        let reads: Vec<Value> = names.iter().map(|name| get(name)).collect();
        let expected: Vec<Value> = (0..names.len())
            .map(|index| hex(&format!("value {index}")))
            .collect();
        assert_eq!(render(&plugin, &reads), expected, "every name round-trips");
        // A prefix is bytes, never a pattern
        assert_eq!(
            render(&plugin, &[list("a%")]),
            [json!({ "ok": { "names": ["a%"], "next": null } })]
        );
        assert_eq!(
            render(&plugin, &[list("a_")]),
            [json!({ "ok": { "names": ["a_c"], "next": null } })]
        );
        let mut sorted: Vec<&str> = names.to_vec();
        sorted.sort();
        assert_eq!(
            render(&plugin, &[list("")]),
            [json!({ "ok": { "names": sorted, "next": null } })]
        );
        // Nothing but the plugin's own store in the storage directory
        let files: Vec<PathBuf> = std::fs::read_dir(store.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(files, std::slice::from_ref(&store));
    });
}

#[test]
fn storage_file() {
    let vault = Vault::new();
    let a_origin = vault.origin("store-a");
    let a = vault.plugin(&Vault::probe("local/store-a", QUOTA), &a_origin);
    update(&a, &[set("secret", "A's value")]);
    let target = vault.store_file("local/store-a", &a_origin);
    let package = fixture("storage-file");
    let origin = vault.origin("storage-file");
    vault.contained(&[], || {
        // Refused at load, before it can run, whatever its approval
        let error = vault
            .world
            .runtime
            .check(&package)
            .err()
            .expect("a plugin importing wasi:filesystem is refused");
        assert_eq!(error.kind(), ErrorKind::CapabilityRefused, "{error}");
        assert!(
            error
                .names()
                .iter()
                .any(|name| name.starts_with("wasi:filesystem/")),
            "{error}"
        );
        let error = vault
            .world
            .try_load_at(&package, &origin)
            .err()
            .expect("refused");
        assert_eq!(error.kind(), ErrorKind::CapabilityRefused, "{error}");
    });
    assert!(target.is_file());
    assert_eq!(render(&a, &[get("secret")]), [hex("A's value")]);
}

/// A store file somebody changed, or swapped for another plugin's, is damaged: the host runs
/// nothing in it, changes nothing and deletes nothing, and other stores keep working.
#[test]
fn foreign_store() {
    use runtime::storage::{Foreign, plant_foreign_store};

    let vault = Vault::new();
    let a_origin = vault.origin("store-a");
    let a = vault.plugin(&Vault::probe("local/store-a", QUOTA), &a_origin);
    update(&a, &[set("secret", "A's value")]);
    let a_store = vault.store_file("local/store-a", &a_origin);
    for kind in [
        Some(Foreign::ExtraTable),
        Some(Foreign::Trigger),
        Some(Foreign::View),
        Some(Foreign::OtherId),
        // A's store copied over B's file: it is A's, not B's
        None,
    ] {
        let b_origin = vault.origin(&format!("store-b-{kind:?}"));
        let b = vault.plugin(&Vault::probe("local/store-b", QUOTA), &b_origin);
        update(&b, &[set("mine", "B's value")]);
        let b_store = vault.store_file("local/store-b", &b_origin);
        match kind {
            Some(kind) => plant_foreign_store(&b_store, kind).unwrap(),
            None => {
                std::fs::copy(&a_store, &b_store).unwrap();
            }
        }
        // Only B's kept data may change: its failed update is recorded
        let kept = vault.own("local/store-b", &b_origin).remove(1);
        vault.contained(&[kept], || {
            let found = render(&b, &[get("mine"), list("")]);
            for answer in &found {
                assert_eq!(answer["error"], "failed:damaged", "{kind:?}: {answer}");
            }
            let report = b.update(&steps(&[set("mine", "rewritten")], None));
            let found = answers(&report.result.unwrap());
            assert_eq!(found[0]["error"], "failed:damaged", "{kind:?}");
        });
        assert!(b_store.is_file(), "{kind:?}: the host deleted the file");
        assert_eq!(render(&a, &[get("secret")]), [hex("A's value")]);
    }
}

/// White box (FR-004): every layer of the production setup, read back, and one attempt past
/// each, on an update's connection and on a render's.
#[test]
fn store_hardened() {
    let dir = tempfile::TempDir::new().unwrap();
    let file = std::fs::canonicalize(dir.path())
        .unwrap()
        .join("store.sqlite");
    let cap = 5 * QUOTA + 64 * 1024;
    for render in [false, true] {
        let report = runtime::storage::hardening_report(&file, render, cap).unwrap();
        assert_eq!(
            report.attached_limit, 0,
            "no other database can be attached"
        );
        assert_eq!(report.length_limit, 256 + 256 * 1024 + 4096);
        assert_eq!(report.sql_length_limit as usize, report.longest_statement);
        assert!(report.defensive, "defensive mode");
        assert!(!report.triggers, "triggers off");
        assert!(!report.views, "views off");
        assert!(!report.trusted_schema, "untrusted schema");
        assert_eq!(report.max_page_count as u64, cap.div_ceil(4096));
        assert_eq!(report.secure_delete, 1, "secure_delete = ON");
        assert_eq!(report.cache_size, -256);
        assert_eq!(report.journal_mode, "delete");
        for (attempt, refused) in &report.attempts {
            assert!(refused, "{attempt} was allowed (render: {render})");
        }
        assert_eq!(report.attempts.len(), if render { 10 } else { 9 });
    }
}

/// White box (FR-002): the store a plugin is linked with is the file of its own id and origin,
/// and the stand-in of storage that is off has none.
#[test]
fn store_binding() {
    let vault = Vault::new();
    let origin = vault.origin("bound");
    let plugin = vault.plugin(&Vault::probe("local/store-probe", QUOTA), &origin);
    assert_eq!(
        plugin.store_binding(),
        Some(vault.store_file("local/store-probe", &origin))
    );
    let optional = repack(
        &fixture("store-probe"),
        &stored_manifest("local/optional-probe", "0.1.0", false, QUOTA),
    );
    let optional_origin = vault.origin("optional");
    let off = vault.plugin(&optional, &optional_origin);
    assert_eq!(off.store_binding(), None, "the stand-in reaches no store");
    let hello = vault.world.approve_and_load(&fixture("hello-widget"));
    assert_eq!(hello.store_binding(), None);
}

// User Story 3: the quota bounds what a plugin keeps

/// `store-probe` as `local/store-probe`, version `version`, with required storage of `quota`.
fn quota_probe(version: &str, quota: u64) -> Vec<u8> {
    repack(
        &fixture("store-probe"),
        &stored_manifest("local/store-probe", version, true, quota),
    )
}

#[test]
fn quota_raised() {
    let vault = Vault::new();
    let origin = vault.origin("quota");
    vault.plugin(&quota_probe("0.1.0", 4096), &origin);
    // A new build at the same path that asks for more
    let larger = quota_probe("0.2.0", 8192);
    vault.contained(&[], || {
        let error = vault
            .world
            .try_load_at(&larger, &origin)
            .err()
            .expect("a larger quota needs approval");
        assert_eq!(error.kind(), ErrorKind::NeedsNewApproval, "{error}");
        assert_eq!(error.names(), ["storage"]);
        let detail = error.to_string();
        assert!(
            detail.contains("4096") && detail.contains("8192"),
            "{detail}"
        );
    });
    // Approved at a smaller quota, a version back at the earlier larger one needs the user again
    vault.plugin(&quota_probe("0.3.0", 2048), &origin);
    let back = quota_probe("0.4.0", 4096);
    vault.contained(&[], || {
        let error = vault
            .world
            .try_load_at(&back, &origin)
            .err()
            .expect("above the quota approved last");
        assert_eq!(error.kind(), ErrorKind::NeedsNewApproval, "{error}");
        assert!(error.to_string().contains("2048"), "{error}");
    });
}

#[test]
fn approval_digest() {
    let vault = Vault::new();
    let origin = vault.origin("digest");
    let package = quota_probe("0.1.0", 4096);
    let checked = vault.world.runtime.check(&package).unwrap();
    let stale = vault
        .world
        .runtime
        .check(&quota_probe("0.1.0", 2048))
        .unwrap()
        .approval_digest();
    vault.contained(&[], || {
        for expected in [None, Some(stale.as_str())] {
            let error = checked
                .approve(&vault.world.grants, &origin, expected)
                .expect_err("refused");
            assert_eq!(
                error.kind(),
                ErrorKind::ApprovalChanged,
                "{expected:?}: {error}"
            );
        }
    });
    let error = vault
        .world
        .try_load_at(&package, &origin)
        .err()
        .expect("nothing was granted");
    assert_eq!(error.kind(), ErrorKind::NotGranted, "{error}");
}

#[test]
fn quota_fill() {
    let vault = Vault::new();
    let origin = vault.origin("fill");
    let plugin = vault.plugin(&quota_probe("0.1.0", 1000), &origin);
    vault.contained(&vault.own("local/store-probe", &origin), || {
        let found = update(
            &plugin,
            &[
                json!({ "op": "set", "name": "a", "value_size": 400 }), // 465
                json!({ "op": "set", "name": "b", "value_size": 400 }), // 930
                json!({ "op": "set", "name": "c", "value_size": 10 }),  // 1005: refused
                get("c"),
                // Replacing counts the old entry as gone: 930 - 465 + 535 = 1000
                json!({ "op": "set", "name": "a", "value_size": 470 }),
                json!({ "op": "set", "name": "a", "value_size": 471 }),
                json!({ "op": "usage" }),
            ],
        );
        assert_eq!(found[2]["error"], "refused:over-quota", "{}", found[2]);
        let detail = found[2]["detail"].as_str().unwrap();
        assert!(
            detail.contains("1000") && detail.contains("1005"),
            "{detail}"
        );
        assert_eq!(
            found[3],
            json!({ "ok": null }),
            "nothing of the refused write was kept"
        );
        assert_eq!(found[4], json!({ "ok": null }));
        assert_eq!(found[5]["error"], "refused:over-quota");
        assert_eq!(found[6], json!({ "ok": { "used": 1000, "quota": 1000 } }));
    });
    assert_eq!(
        render(&plugin, &[json!({ "op": "usage" })]),
        [json!({ "ok": { "used": 1000, "quota": 1000 } })]
    );
}

#[test]
fn quota_lowered() {
    let vault = Vault::new();
    let origin = vault.origin("lowered");
    let first = vault.plugin(&quota_probe("0.1.0", 1000), &origin);
    update(
        &first,
        &[
            json!({ "op": "set", "name": "a", "value_size": 400 }),
            json!({ "op": "set", "name": "b", "value_size": 400 }),
        ],
    );

    // A new version with a smaller quota loads without a new approval
    let lowered = vault.world.load_at(&quota_probe("0.2.0", 500), &origin);
    vault.contained(&vault.own("local/store-probe", &origin), || {
        let found = update(
            &lowered,
            &[
                json!({ "op": "usage" }),
                json!({ "op": "set", "name": "c", "value": "" }),
                // Smaller, but usage stays over the new quota: refused
                json!({ "op": "set", "name": "a", "value_size": 300 }),
                json!({ "op": "delete", "name": "b" }),
                json!({ "op": "set", "name": "a", "value_size": 300 }),
                json!({ "op": "usage" }),
            ],
        );
        assert_eq!(
            found[0],
            json!({ "ok": { "used": 930, "quota": 500 } }),
            "data kept"
        );
        assert_eq!(found[1]["error"], "refused:over-quota");
        assert_eq!(found[2]["error"], "refused:over-quota");
        assert_eq!(found[3], json!({ "ok": null }), "deletes pass");
        assert_eq!(found[4], json!({ "ok": null }), "a write that fits passes");
        assert_eq!(found[5], json!({ "ok": { "used": 365, "quota": 500 } }));
    });
    // The approval keeps its quota: the file cap stays that of 1000 bytes
    let entry = vault
        .world
        .grants
        .entry(&runtime::manifest::PluginId::parse("local/store-probe").unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(entry.storage_quota_bytes, Some(1000));
}
