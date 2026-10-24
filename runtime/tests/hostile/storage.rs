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
            // An update that cannot use its store fails, naming why
            let report = b.update(&steps(&[set("mine", "rewritten")], None));
            let error = report.result.expect_err("the update fails");
            assert_eq!(error.kind(), ErrorKind::StorageFailed, "{kind:?}: {error}");
            assert!(error.to_string().contains("damaged"), "{kind:?}: {error}");
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

/// An update of `plugin` that makes `step_list`, ends as `end` says, and returns what it
/// reported, error or content.
fn update_report(plugin: &Plugin, step_list: &[Value], end: Option<&str>) -> runtime::UpdateReport {
    plugin.update(&steps(step_list, end))
}

/// Fills a fresh store of `quota` with values of `value_size` bytes, as many per update as the
/// limits allow, until a write is refused as over quota: the quota, never the file cap, is what
/// stops it, and the file stays under its cap. Returns the file size.
fn fill(vault: &Vault, value_size: u64, quota: u64) -> u64 {
    let id = format!("local/fill-{value_size}");
    let origin = vault.origin(&format!("fill-{value_size}"));
    let plugin = vault.plugin(&Vault::probe(&id, quota), &origin);
    let store = vault.store_file(&id, &origin);
    let cap = 5 * quota + 64 * 1024;
    let per_update = ((1024 * 1024) / (value_size + 16)).clamp(1, 999);
    for batch in 0.. {
        let report = update_report(
            &plugin,
            &[json!({
                "op": "set",
                "name": format!("b{batch}-{{i}}"),
                "value_size": value_size,
                "repeat": per_update,
            })],
            None,
        );
        let content = report
            .result
            .unwrap_or_else(|error| panic!("{value_size} B: {error}"));
        let size = std::fs::metadata(&store).unwrap().len();
        assert!(
            size <= cap,
            "{value_size} B: the file is {size} bytes, over {cap}"
        );
        if let Some(first) = content["first_error"].as_object() {
            assert_eq!(
                first["error"], "refused:over-quota",
                "{value_size} B: the quota must come first, not {first:?}"
            );
            assert_eq!(content["last_error"]["error"], "refused:over-quota");
            break;
        }
        assert!(batch < 10_000, "{value_size} B: the quota never came");
    }
    let usage = render(&plugin, &[json!({ "op": "usage" })]).remove(0);
    let used = usage["ok"]["used"].as_u64().unwrap();
    assert!(
        used <= quota,
        "{value_size} B: usage {used} over the quota {quota}"
    );
    std::fs::metadata(&store).unwrap().len()
}

#[test]
fn many_entries() {
    let vault = Vault::new();
    vault.contained(&[], || {});
    // Every entry counts its cost: thousands of empty values fill a quota of 64 B per entry
    let origin = vault.origin("empty");
    let plugin = vault.plugin(&Vault::probe("local/empty-values", 64 * 1024), &origin);
    let content = update_report(
        &plugin,
        &[
            json!({ "op": "set", "name": "{i}", "value": "", "repeat": 999 }),
            json!({ "op": "usage" }),
        ],
        None,
    )
    .result
    .unwrap();
    // 999 names of 1 to 3 digits and 64 B each, until the quota refuses
    assert_eq!(content["first_error"]["error"], "refused:over-quota");
    let used = render(&plugin, &[json!({ "op": "usage" })]).remove(0)["ok"]["used"]
        .as_u64()
        .unwrap();
    assert!(used <= 64 * 1024 && used > 60 * 1024, "{used}");
    // Values from 32 B to 4 KiB, 1,000 B among them, the worst measured (research R6), up to
    // the host cap
    for value_size in [32, 500, 1000, 1500, 4096] {
        let size = fill(&vault, value_size, 8 * 1024 * 1024);
        eprintln!("many-entries: {value_size} B values fill 8 MiB of quota in a {size}-byte file");
    }
    vault.assert_well_behaved();
}

#[test]
fn churn() {
    let vault = Vault::new();
    let origin = vault.origin("churn");
    let quota = 64 * 1024;
    let plugin = vault.plugin(&Vault::probe("local/store-probe", quota), &origin);
    let store = vault.store_file("local/store-probe", &origin);
    let cap = 5 * quota + 64 * 1024;
    let sizes = [32, 500, 1000, 1500, 4096];
    for round in 0..200usize {
        let size = sizes[round % sizes.len()];
        // Delete what the last round wrote, then fill the quota again with another size
        let count = (quota / (size + 70)).saturating_sub(1).max(1);
        let report = update_report(
            &plugin,
            &[
                json!({ "op": "delete", "name": "v{i}", "repeat": 64 }),
                json!({ "op": "set", "name": "v{i}", "value_size": size, "repeat": count }),
            ],
            None,
        );
        let content = report.result.unwrap();
        assert!(
            content["first_error"].is_null()
                || content["first_error"]["error"] == "refused:over-quota",
            "round {round}: {}",
            content["first_error"]
        );
        let file = std::fs::metadata(&store).unwrap().len();
        assert!(
            file <= cap,
            "round {round}: the file is {file} bytes, over {cap}"
        );
    }
    vault.assert_well_behaved();
}

#[test]
fn file_cap_hit() {
    let limits = Limits {
        storage_file_cap_factor: 1,
        storage_file_cap_extra_bytes: 0,
        ..Limits::default()
    };
    let vault = Vault::with_limits(limits);
    let origin = vault.origin("file-cap");
    let plugin = vault.plugin(&Vault::probe("local/store-probe", QUOTA), &origin);
    update(&plugin, &[set("kept", "before")]);
    let kept = vault.own("local/store-probe", &origin).remove(1);
    for end in ["ok", "panic"] {
        vault.contained(std::slice::from_ref(&kept), || {
            let report = update_report(
                &plugin,
                &[
                    json!({ "op": "set", "name": "v{i}", "value_size": 900, "repeat": 70 }),
                    get("kept"),
                ],
                Some(end),
            );
            let error = report.result.expect_err("the update fails");
            assert_eq!(error.kind(), ErrorKind::StorageFailed, "{end}: {error}");
            assert!(
                error
                    .to_string()
                    .contains("nothing this update wrote was kept"),
                "{error}"
            );
        });
        assert_eq!(
            render(&plugin, &[get("kept"), get("v0")]),
            [hex("before"), json!({ "ok": null })]
        );
    }
    // The update's failure names the first failure, the file reaching its size; the answers
    // the plugin got (`failed(over-limit)`, then `failed(run-aborted)`) are checked in
    // runtime/src/storage/mod.rs, a_write_that_reaches_the_file_cap_ends_the_run
    let report = update_report(
        &plugin,
        &[json!({ "op": "set", "name": "w{i}", "value_size": 900, "repeat": 70 })],
        Some("error"),
    );
    let message = report.result.err().unwrap().to_string();
    assert!(message.starts_with("storage-failed: "), "{message}");
    assert!(message.contains("size limit"), "{message}");
}

#[test]
fn unwritable_store() {
    let vault = Vault::new();
    let origin = vault.origin("unwritable");
    let plugin = vault.plugin(&Vault::probe("local/store-probe", QUOTA), &origin);
    update(&plugin, &[set("kept", "before")]);
    let store = vault.store_file("local/store-probe", &origin);
    let read_only = |on: bool| {
        let mut permissions = std::fs::metadata(&store).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(on);
        std::fs::set_permissions(&store, permissions).unwrap();
    };
    read_only(true);
    let kept = vault.own("local/store-probe", &origin).remove(1);
    vault.contained(&[kept], || {
        let report = update_report(&plugin, &[set("new", "value"), get("kept")], None);
        let error = report.result.expect_err("the update fails");
        assert_eq!(error.kind(), ErrorKind::StorageFailed, "{error}");
        assert!(error.to_string().contains("unavailable"), "{error}");
    });
    read_only(false);
    assert_eq!(
        render(&plugin, &[get("kept"), get("new")]),
        [hex("before"), json!({ "ok": null })]
    );
}

#[test]
fn over_limits() {
    let vault = Vault::new();
    let origin = vault.origin("limits");
    let plugin = vault.plugin(&Vault::probe("local/store-probe", 8 * 1024 * 1024), &origin);
    let name = "n".repeat(256);
    vault.contained(&vault.own("local/store-probe", &origin), || {
        let found = update(
            &plugin,
            &[
                json!({ "op": "set", "name": name, "value_size": 256 * 1024 }),
                json!({ "op": "set", "name": "n".repeat(257), "value": "" }),
                json!({ "op": "set", "name": "v", "value_size": 256 * 1024 + 1 }),
                json!({ "op": "list", "prefix": "p".repeat(257) }),
                json!({ "op": "list", "prefix": "", "after": "a".repeat(257) }),
                json!({ "op": "get", "name": "" }),
            ],
        );
        assert_eq!(found[0], json!({ "ok": null }), "the largest entry is kept");
        assert_eq!(found[1]["error"], "refused:invalid-name");
        assert_eq!(found[2]["error"], "refused:over-limit");
        assert!(found[2]["detail"].as_str().unwrap().contains("262144"));
        assert_eq!(found[3]["error"], "refused:invalid-name");
        assert_eq!(found[4]["error"], "refused:invalid-name");
        assert_eq!(found[5]["error"], "refused:invalid-name");
        // Call 1,001 of a run, whatever it is
        let content = update_report(
            &plugin,
            &[json!({ "op": "get", "name": "x", "repeat": 1001 })],
            None,
        )
        .result
        .unwrap();
        assert_eq!(content["errors"], 1);
        assert_eq!(content["last_error"]["error"], "refused:over-limit");
        assert!(
            content["last_error"]["detail"]
                .as_str()
                .unwrap()
                .contains("1000"),
            "{}",
            content["last_error"]
        );
        // More than 1 MiB of names and values in one update
        let content = update_report(
            &plugin,
            &[json!({ "op": "set", "name": "big{i}", "value_size": 256 * 1024, "repeat": 4 })],
            None,
        )
        .result
        .unwrap();
        assert_eq!(content["errors"], 1);
        assert_eq!(content["first_error"]["error"], "refused:over-limit");
    });
    assert_eq!(
        render(&plugin, &[json!({ "op": "get", "name": "big3" })]),
        [json!({ "ok": null })]
    );
}

// User Story 4: writes are all or nothing, and render only reads

#[test]
fn write_in_render() {
    let vault = Vault::new();
    let origin = vault.origin("render");
    let plugin = vault.plugin(&Vault::probe("local/store-probe", QUOTA), &origin);
    update(&plugin, &[set("a", "1")]);
    vault.contained(&[], || {
        let found = render(
            &plugin,
            &[
                set("a", "2"),
                json!({ "op": "delete", "name": "a" }),
                get("a"),
            ],
        );
        assert_eq!(found[0]["error"], "refused:render-only-reads");
        assert_eq!(found[1]["error"], "refused:render-only-reads");
        assert_eq!(found[2], hex("1"));
    });
}

#[test]
fn crash_after_writes() {
    let vault = Vault::new();
    let origin = vault.origin("crash");
    let plugin = vault.plugin(&Vault::probe("local/store-probe", QUOTA), &origin);
    let first = update(&plugin, &[set("a", "before")]);
    let kept = vault.own("local/store-probe", &origin).remove(1);
    let writes = [set("a", "during"), set("b", "new")];
    let cases: [(&str, Option<&str>, Option<Value>, ErrorKind); 6] = [
        ("panic", Some("panic"), None, ErrorKind::PluginCrashed),
        (
            "out of fuel",
            Some("fuel"),
            None,
            ErrorKind::CpuBudgetExceeded,
        ),
        (
            "out of memory",
            Some("memory"),
            None,
            ErrorKind::MemoryLimitExceeded,
        ),
        ("an error", Some("error"), None, ErrorKind::PluginError),
        (
            "output that is not JSON",
            None,
            Some(json!("not-json")),
            ErrorKind::InvalidOutput,
        ),
        (
            "output over 256 KiB",
            None,
            Some(json!(256 * 1024 + 1)),
            ErrorKind::OutputTooLarge,
        ),
    ];
    for (case, end, output, kind) in cases {
        vault.contained(std::slice::from_ref(&kept), || {
            let mut config = json!({ "steps": writes, "end": end.unwrap_or("ok") });
            if let Some(output) = &output {
                config["output"] = output.clone();
            }
            let report = plugin.update(&config.to_string());
            let error = report.result.expect_err(case);
            assert_eq!(error.kind(), kind, "{case}: {error}");
        });
        let content = plugin
            .render(&render_steps(&[get("a"), get("b")]))
            .result
            .unwrap();
        assert_eq!(
            answers(&content),
            [hex("before"), json!({ "ok": null })],
            "{case}"
        );
        // The previous kept data stays
        assert_eq!(
            content["render_config"]["data"]["answers"],
            json!(first),
            "{case}"
        );
    }
    // The time limit, with the CPU budget out of the way
    let vault = Vault::with_limits(Limits {
        fuel: u64::MAX,
        update_time_limit: std::time::Duration::from_secs(2),
        ..Limits::default()
    });
    let origin = vault.origin("crash");
    let plugin = vault.plugin(&Vault::probe("local/store-probe", QUOTA), &origin);
    update(&plugin, &[set("a", "before")]);
    let kept = vault.own("local/store-probe", &origin).remove(1);
    vault.contained(&[kept], || {
        let report = plugin.update(&steps(&writes, Some("loop")));
        let error = report.result.expect_err("the time limit");
        assert_eq!(error.kind(), ErrorKind::TimeLimitExceeded, "{error}");
    });
    assert_eq!(
        render(&plugin, &[get("a"), get("b")]),
        [hex("before"), json!({ "ok": null })]
    );
}

#[test]
fn optional_off() {
    let vault = Vault::new();
    let origin = vault.origin("optional");
    let package = repack(
        &fixture("store-probe"),
        &stored_manifest("local/store-probe", "0.1.0", false, QUOTA),
    );
    let plugin = vault.plugin(&package, &origin);
    let checked = vault.world.runtime.check(&package).unwrap();
    let turn = |on: bool| {
        checked
            .set_optional(&vault.world.grants, &origin, "storage", on)
            .unwrap();
    };
    let store = vault.store_file("local/store-probe", &origin);
    // Off from the start: nothing is created
    let kept = vault.own("local/store-probe", &origin).remove(1);
    vault.contained(std::slice::from_ref(&kept), || {
        let found = update(
            &plugin,
            &[set("a", "1"), get("a"), list(""), json!({ "op": "usage" })],
        );
        for answer in &found {
            assert_eq!(answer["error"], "not-granted", "{answer}");
        }
        assert!(!store.exists(), "the stand-in created a store");
    });
    // On, a write; off again, every call not granted and the store untouched
    turn(true);
    update(&plugin, &[set("a", "kept while off")]);
    turn(false);
    vault.contained(std::slice::from_ref(&kept), || {
        let found = update(&plugin, &[get("a"), set("a", "overwritten")]);
        assert_eq!(found[0]["error"], "not-granted");
        assert_eq!(found[1]["error"], "not-granted");
        assert_eq!(render(&plugin, &[get("a")])[0]["error"], "not-granted");
    });
    // On again: the data is there
    turn(true);
    assert_eq!(render(&plugin, &[get("a")]), [hex("kept while off")]);
}

/// Loop iterations of `store-probe`'s `spin` that take about `wanted` on this machine,
/// measured with a render that only spins.
fn calibrate_spin(plugin: &Plugin, wanted: std::time::Duration) -> u64 {
    let probe = 200_000_000u64;
    let started = std::time::Instant::now();
    plugin
        .render(&render_steps(&[
            json!({ "op": "spin", "iterations": probe }),
        ]))
        .result
        .expect("a render that spins");
    let took = started.elapsed().as_secs_f64().max(0.001);
    (probe as f64 * wanted.as_secs_f64() / took) as u64
}

/// The limits of the mid-run cases: no CPU budget, and time limits long enough for a test to
/// change the grants while the probe spins.
fn spinning_limits() -> Limits {
    Limits {
        fuel: u64::MAX,
        time_limit: std::time::Duration::from_secs(10),
        update_time_limit: std::time::Duration::from_secs(30),
        ..Limits::default()
    }
}

/// Waits until `path` exists, failing the test instead of hanging.
fn wait_for(path: &Path) {
    let started = std::time::Instant::now();
    while !path.exists() {
        assert!(
            started.elapsed() < std::time::Duration::from_secs(20),
            "{} never appeared",
            path.display()
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[test]
fn taken_away_mid_update() {
    use runtime::grants::GrantStore;
    use runtime::manifest::PluginId;

    for (variant, extra_call) in [
        ("turned off", false),
        ("approval removed", false),
        ("turned off, then one more call", true),
    ] {
        let vault = Vault::with_limits(spinning_limits());
        let origin = vault.origin("taken");
        let package = repack(
            &fixture("store-probe"),
            &stored_manifest("local/store-probe", "0.1.0", false, QUOTA),
        );
        let plugin = vault.plugin(&package, &origin);
        let checked = vault.world.runtime.check(&package).unwrap();
        checked
            .set_optional(&vault.world.grants, &origin, "storage", true)
            .unwrap();
        update(&plugin, &[set("a", "before")]);
        let spin = calibrate_spin(&plugin, std::time::Duration::from_secs(3));
        let journal = vault
            .store_file("local/store-probe", &origin)
            .with_extension("sqlite-journal");
        let record = vault.data().join("grants.json");
        let mut step_list = vec![
            set("a", "during"),
            json!({ "op": "spin", "iterations": spin }),
        ];
        if extra_call {
            step_list.push(get("a"));
        }
        let error = std::thread::scope(|scope| {
            let running = scope.spawn(|| plugin.update(&steps(&step_list, None)));
            // The update has written once it holds a journal: change the grants then, from
            // another grant store on the same record, as another process would
            wait_for(&journal);
            let other = GrantStore::open(&record);
            if variant == "approval removed" {
                other
                    .remove(&PluginId::parse("local/store-probe").unwrap())
                    .unwrap();
            } else {
                checked
                    .set_optional(&other, &origin, "storage", false)
                    .unwrap();
            }
            running.join().unwrap().result.expect_err(variant)
        });
        assert_eq!(error.kind(), ErrorKind::NotGranted, "{variant}: {error}");
        assert!(error.to_string().contains("storage"), "{variant}: {error}");
        // Nothing of the update was kept
        if variant == "approval removed" {
            vault.world.approve_at(&package, &origin);
        }
        checked
            .set_optional(&vault.world.grants, &origin, "storage", true)
            .unwrap();
        assert_eq!(render(&plugin, &[get("a")]), [hex("before")], "{variant}");
    }
}

#[test]
fn taken_away_mid_render() {
    use runtime::grants::GrantStore;

    let vault = Vault::with_limits(spinning_limits());
    let origin = vault.origin("taken");
    let package = repack(
        &fixture("store-probe"),
        &stored_manifest("local/store-probe", "0.1.0", false, QUOTA),
    );
    let plugin = vault.plugin(&package, &origin);
    let checked = vault.world.runtime.check(&package).unwrap();
    checked
        .set_optional(&vault.world.grants, &origin, "storage", true)
        .unwrap();
    update(&plugin, &[set("a", "1")]);
    let spin = calibrate_spin(&plugin, std::time::Duration::from_secs(3));
    let record = vault.data().join("grants.json");
    let found = std::thread::scope(|scope| {
        let running = scope.spawn(|| {
            render(
                &plugin,
                &[
                    get("a"),
                    json!({ "op": "spin", "iterations": spin }),
                    get("a"),
                ],
            )
        });
        // Render leaves no file to watch: the change comes 1 s into a render of about 3 s
        std::thread::sleep(std::time::Duration::from_secs(1));
        checked
            .set_optional(&GrantStore::open(&record), &origin, "storage", false)
            .unwrap();
        running.join().unwrap()
    });
    assert_eq!(found[0], hex("1"), "the first read, before the change");
    assert_eq!(
        found[1]["error"], "not-granted",
        "the read after the change"
    );
}

/// The child half of `killed-mid-update`: run by [`killed_mid_update`] in a process of its own,
/// which the parent kills while this update holds its transaction. Ignored when the suite runs.
#[test]
#[ignore = "run by killed_mid_update in a child process, which it kills"]
fn killed_mid_update_child_process() {
    use std::sync::Arc;

    let data = std::env::var("STORE_DATA").expect("run through killed_mid_update");
    let package = std::fs::read(std::env::var("STORE_PACKAGE").unwrap()).unwrap();
    let origin = std::env::var("STORE_ORIGIN").unwrap();
    let limits = Limits {
        update_time_limit: std::time::Duration::from_secs(120),
        ..spinning_limits()
    };
    let runtime = runtime::Runtime::new(limits, runtime::RuntimeOptions::new(&data)).unwrap();
    let grants = Arc::new(runtime::grants::GrantStore::open(
        Path::new(&data).join("grants.json"),
    ));
    let plugin = runtime.load(&package, &origin, grants).unwrap();
    // About 900 KiB, past the 256 KiB page cache, then a spin the parent never lets end
    let report = plugin.update(&steps(
        &[
            json!({ "op": "set", "name": "v{i}", "value_size": 4096, "repeat": 225 }),
            json!({ "op": "spin", "iterations": u64::MAX }),
        ],
        None,
    ));
    panic!("the update should have been killed: {:?}", report.result);
}

/// `killed-mid-update` (FR-010, research R8): the process is killed after the update spilled
/// pages to the store; the next reader rolls the hot journal back and reads the state from
/// before the update, never reporting the store damaged.
#[test]
fn killed_mid_update() {
    let vault = Vault::new();
    let origin = vault.origin("killed");
    let package = Vault::probe("local/store-probe", 2 * 1024 * 1024);
    let plugin = vault.plugin(&package, &origin);
    update(&plugin, &[set("kept", "before")]);
    let store = vault.store_file("local/store-probe", &origin);
    let journal = store.with_extension("sqlite-journal");
    let size_before = std::fs::metadata(&store).unwrap().len();
    let package_file = vault.data().join("killed.wasm");
    std::fs::write(&package_file, &package).unwrap();
    let mut child = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "storage::killed_mid_update_child_process",
            "--ignored",
            "--nocapture",
        ])
        .env("STORE_DATA", vault.data())
        .env("STORE_PACKAGE", &package_file)
        .env("STORE_ORIGIN", &origin)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("the child test starts");
    // Killed once the update wrote pages past the cache into the store file itself
    let started = std::time::Instant::now();
    while std::fs::metadata(&store).unwrap().len() < size_before + 300 * 1024 {
        assert!(
            child.try_wait().unwrap().is_none(),
            "the child ended before it was killed"
        );
        assert!(
            started.elapsed() < std::time::Duration::from_secs(60),
            "the update never spilled pages to the store"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    child.kill().unwrap();
    child.wait().unwrap();
    assert!(journal.is_file(), "the killed update left its journal");
    // The store and its journal change on purpose: the journal is rolled back
    let mut own = vault.own("local/store-probe", &origin);
    own.push(journal.clone());
    vault.contained(&own, || {
        let found = render(&plugin, &[get("kept"), get("v0"), list("v")]);
        assert_eq!(found[0], hex("before"), "the state from before the update");
        assert_eq!(found[1], json!({ "ok": null }));
        assert_eq!(found[2], json!({ "ok": { "names": [], "next": null } }));
        assert!(!journal.exists(), "the journal was rolled back");
    });
}
