//! Plugin storage at the API level (`specs/004-plugin-storage/`): a widget remembers its own
//! data across runs and versions (User Story 1), writes are all or nothing (User Story 4), and
//! the tests the data model names beside the hostile cases.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use common::{
    TEST_ORIGIN, World, fixture, render_steps, repack, steps, store_probe, stored_manifest,
};
use runtime::grants::GrantStore;
use runtime::{ErrorKind, Limits, Runtime, RuntimeOptions};
use serde_json::{Value, json};
use tempfile::TempDir;

const PROBE: &str = "local/store-probe";
const QUOTA: u64 = 64 * 1024;

fn probe(required: bool) -> Vec<u8> {
    store_probe(&stored_manifest(PROBE, "0.1.0", required, QUOTA))
}

fn set(name: &str, value: &str) -> Value {
    json!({ "op": "set", "name": name, "value": value })
}

fn get(name: &str) -> Value {
    json!({ "op": "get", "name": name })
}

/// The answers of a `store-probe` update or render.
fn answers(content: &Value) -> Vec<Value> {
    content["answers"]
        .as_array()
        .unwrap_or_else(|| panic!("store-probe answered {content}"))
        .clone()
}

fn hex(text: &str) -> Value {
    json!({ "ok": { "hex": text.bytes().map(|byte| format!("{byte:02x}")).collect::<String>() } })
}

/// Runs an update of `plugin` that makes `steps`, and returns its answers.
fn update(plugin: &runtime::Plugin, step_list: &[Value]) -> Vec<Value> {
    let report = plugin.update(&steps(step_list, None));
    answers(
        &report
            .result
            .unwrap_or_else(|error| panic!("the update: {error}")),
    )
}

/// Runs a render of `plugin` that makes `steps`, and returns its answers.
fn render(plugin: &runtime::Plugin, step_list: &[Value]) -> Vec<Value> {
    let report = plugin.render(&render_steps(step_list));
    answers(
        &report
            .result
            .unwrap_or_else(|error| panic!("the render: {error}")),
    )
}

#[test]
fn update_counter_remembers_its_values_across_runs_of_the_host() {
    let world = World::production(false);
    let counter = world.approve_and_load(&fixture("update-counter"));
    assert!(counter.update(r#"{"note":"first"}"#).result.is_ok());
    let report = counter.update(r#"{"note":"second"}"#);
    assert_eq!(report.result.unwrap(), json!({ "count": 2 }));
    let storage = report.storage.expect("the update reports its storage");
    assert!(storage.writes_kept > 0);
    assert_eq!(storage.quota_bytes, 65536);
    let content = counter.render("{}").result.unwrap();
    assert_eq!(content["text"], "Updated 2 times");
    assert_eq!(content["items"], json!(["second", "first"]));
    // A new run of the host, on the same data directory and grant record
    let runtime = Runtime::new(Limits::default(), RuntimeOptions::new(world.data.path())).unwrap();
    let again = runtime
        .load(
            &fixture("update-counter"),
            TEST_ORIGIN,
            Arc::new(GrantStore::open(world.data.path().join("grants.json"))),
        )
        .unwrap();
    let content = again.render("{}").result.unwrap();
    assert_eq!(content["text"], "Updated 2 times");
    assert_eq!(content["items"], json!(["second", "first"]));
}

#[test]
fn a_new_version_from_the_same_origin_reads_what_the_earlier_one_wrote() {
    let world = World::production(false);
    let first = world.approve_and_load(&probe(true));
    update(&first, &[set("kept", "by 0.1.0")]);
    let rendered = first.render(&render_steps(&[])).result.unwrap();
    assert!(!rendered["render_config"]["data"].is_null(), "kept data");
    let newer = repack(
        &fixture("store-probe"),
        &stored_manifest(PROBE, "0.2.0", true, QUOTA),
    );
    // The same quota: no new approval
    let second = world.load(&newer);
    let report = second.render(&render_steps(&[get("kept")]));
    let content = report.result.unwrap();
    assert_eq!(answers(&content), [hex("by 0.1.0")]);
    // Kept data belongs to a version; storage to the plugin
    assert!(content["render_config"]["data"].is_null(), "{content}");
}

#[test]
fn reads_writes_and_deletes_answer_as_the_contract_says() {
    let world = World::production(false);
    let plugin = world.approve_and_load(&probe(true));
    let found = update(
        &plugin,
        &[
            get("never written"),
            set("empty", ""),
            set("twice", "first"),
            set("twice", "last"),
            json!({ "op": "delete", "name": "absent" }),
            json!({ "op": "list", "prefix": "" }),
            json!({ "op": "usage" }),
        ],
    );
    assert_eq!(found[0], json!({ "ok": null }));
    assert_eq!(found[1], json!({ "ok": null }));
    assert_eq!(
        found[4],
        json!({ "ok": null }),
        "deleting an absent name succeeds"
    );
    assert_eq!(
        found[5],
        json!({ "ok": { "names": ["empty", "twice"], "next": null } }),
        "list sees the update's own writes"
    );
    // "empty" counts the entry cost alone; "twice" its last value
    let usage = (5 + 64) + (5 + 4 + 64);
    assert_eq!(found[6], json!({ "ok": { "used": usage, "quota": QUOTA } }));
    let read = render(&plugin, &[get("empty"), get("twice"), get("never written")]);
    assert_eq!(read, [hex(""), hex("last"), json!({ "ok": null })]);
}

#[test]
fn list_pages_through_a_prefix_in_byte_order() {
    let world = World::production(false);
    let plugin = world.approve_and_load(&probe(true));
    update(
        &plugin,
        &[
            json!({ "op": "set", "name": "notes/{i}", "value": "", "repeat": 250 }),
            set("notesX", ""),
            set("other", ""),
        ],
    );
    let mut names: Vec<String> = Vec::new();
    let mut after: Option<String> = None;
    let mut pages = 0;
    loop {
        let mut step = json!({ "op": "list", "prefix": "notes/" });
        if let Some(after) = &after {
            step["after"] = json!(after);
        }
        let page = render(&plugin, &[step]).remove(0);
        let page = &page["ok"];
        let page_names: Vec<String> = serde_json::from_value(page["names"].clone()).unwrap();
        assert!(page_names.len() <= 100);
        names.extend(page_names);
        pages += 1;
        match page["next"].as_str() {
            Some(next) => after = Some(next.to_owned()),
            None => break,
        }
    }
    assert_eq!(pages, 3);
    let mut expected: Vec<String> = (0..250).map(|index| format!("notes/{index}")).collect();
    expected.sort();
    assert_eq!(names, expected, "byte order, nothing outside the prefix");
}

/// A data directory where `storage/` cannot be created, because a file has that name.
fn data_without_storage() -> TempDir {
    let data = TempDir::new().unwrap();
    std::fs::write(data.path().join("storage"), b"not a directory").unwrap();
    data
}

#[test]
fn storage_unavailable_at_load() {
    let data = data_without_storage();
    let runtime = Runtime::new(Limits::default(), RuntimeOptions::new(data.path())).unwrap();
    let grants = Arc::new(GrantStore::open(data.path().join("grants.json")));
    // Required: refused, naming the cause
    let required = probe(true);
    common::approve(&grants, &required, TEST_ORIGIN);
    let error = runtime
        .load(&required, TEST_ORIGIN, Arc::clone(&grants))
        .err()
        .expect("refused");
    assert_eq!(error.kind(), ErrorKind::StorageFailed, "{error}");
    assert!(error.to_string().contains("storage"), "{error}");
    // Optional and on: linked, and every call fails as unavailable, never as if it worked
    let optional = probe(false);
    common::approve(&grants, &optional, TEST_ORIGIN);
    runtime
        .check(&optional)
        .unwrap()
        .set_optional(&grants, TEST_ORIGIN, "storage", true)
        .unwrap();
    let plugin = runtime.load(&optional, TEST_ORIGIN, grants).unwrap();
    let report = plugin.update(&steps(&[set("a", "b"), get("a")], None));
    let found = answers(&report.result.unwrap());
    for answer in found {
        assert_eq!(answer["error"], "failed:unavailable", "{answer}");
    }
}

/// A directory under the workspace's `target/`, reached by a relative path from the package
/// directory, where Cargo runs the tests.
struct RelativeDir {
    relative: PathBuf,
    absolute: PathBuf,
}

impl RelativeDir {
    fn new(name: &str) -> RelativeDir {
        let relative = Path::new("..")
            .join("target")
            .join("storage-tests")
            .join(format!("{name}-{}", std::process::id()));
        let absolute = Path::new(env!("CARGO_MANIFEST_DIR")).join(&relative);
        let _ = std::fs::remove_dir_all(&absolute);
        std::fs::create_dir_all(&absolute).unwrap();
        RelativeDir { relative, absolute }
    }
}

impl Drop for RelativeDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.absolute);
    }
}

#[test]
fn data_dir_is_absolute() {
    assert_eq!(
        std::env::current_dir().unwrap().canonicalize().unwrap(),
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .canonicalize()
            .unwrap(),
        "Cargo runs the tests from the package directory"
    );
    let dir = RelativeDir::new("relative");
    let mut data_dirs = vec![(dir.relative.clone(), dir.absolute.clone())];
    // A name that SQLite would read as a URI if it ever reached it relative
    #[cfg(unix)]
    {
        let uri = dir.absolute.join("file:x");
        data_dirs.push((uri.clone(), uri));
    }
    for (given, absolute) in data_dirs {
        let runtime = Runtime::new(Limits::default(), RuntimeOptions::new(&given)).unwrap();
        let grants = Arc::new(GrantStore::open(dir.absolute.join("grants.json")));
        let package = probe(true);
        common::approve(&grants, &package, TEST_ORIGIN);
        let plugin = runtime.load(&package, TEST_ORIGIN, grants).unwrap();
        update(&plugin, &[set("a", "b")]);
        let stores: Vec<PathBuf> = std::fs::read_dir(absolute.join("storage"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect();
        assert_eq!(stores.len(), 1, "{stores:?}");
        assert!(
            stores[0]
                .extension()
                .is_some_and(|extension| extension == "sqlite"),
            "{stores:?}"
        );
        assert_eq!(render(&plugin, &[get("a")]), [hex("b")]);
    }
}

#[test]
fn bundled_sqlite() {
    runtime::storage::engine_check().expect("the linked SQLite is 3.54.0 with defensive mode");
}
