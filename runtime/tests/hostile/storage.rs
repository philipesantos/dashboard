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
