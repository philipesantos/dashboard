//! What plugin storage costs (`specs/004-plugin-storage/research.md`, R16): the first call of a
//! run, one `get`, one `set`, an update that writes 100 values and commits them, and a render
//! that reads 100, each the median of `--iterations` runs (default 100); then the file a store
//! takes when filled to its quota with values of 32 B, 1,000 B and 4 KiB.
//!
//! Per-call costs are differences of medians between runs that differ only in their storage
//! calls, so no timing code enters the host. Every call re-reads a real grant record file, as
//! in production. Run with `cargo run -p dashboard-bench --release --bin storage_cost`, after
//! `cargo xtask plugins`, which builds the `store-probe` fixture this measures.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use runtime::grants::GrantStore;
use runtime::{Limits, Plugin, Runtime, RuntimeOptions, package};
use serde_json::{Value, json};

const ORIGIN: &str = "storage-cost";

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let value_of = |option: &str| {
        args.iter()
            .position(|arg| arg == option)
            .and_then(|at| args.get(at + 1))
            .map(|value| value.parse::<u64>().expect("a whole number"))
    };
    let iterations = value_of("--iterations").unwrap_or(100).max(1) as usize;
    // The quota of the stores filled at the end; the host cap unless a test asks for less
    let fill_quota = value_of("--quota").unwrap_or(Limits::default().max_storage_quota_bytes);

    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("target")
        .join("plugins")
        .join("store-probe.wasm");
    let probe = std::fs::read(&fixture)
        .unwrap_or_else(|error| panic!("{}: {error}; run cargo xtask plugins", fixture.display()));
    let data = tempfile::TempDir::new().expect("a temporary data directory");
    let runtime = Runtime::new(Limits::default(), RuntimeOptions::new(data.path()))
        .expect("the runtime starts");
    let grants = Arc::new(GrantStore::open(data.path().join("grants.json")));

    let plugin = load(&runtime, &grants, &probe, "local/storage-cost", 64 * 1024);
    // The warm-up: 100 values of 32 B, which the reads below find
    update(&plugin, &[set_many("k{i}", 32, 100)], "ok");

    let render = |steps: &[Value]| {
        let config = json!({ "render_steps": steps }).to_string();
        median(iterations, || {
            plugin.render(&config).result.expect("the render");
        })
    };
    let none = render(&[]);
    let one_get = render(&[json!({ "op": "get", "name": "k0" })]);
    let hundred_gets = render(&[json!({ "op": "get", "name": "k{i}", "repeat": 100 })]);
    let writes = |count: u64, end: &str| {
        let config = json!({ "steps": [set_many("w{i}", 32, count)], "end": end }).to_string();
        median(iterations, || {
            let _ = plugin.update(&config);
        })
    };
    let one_set = writes(1, "error");
    let hundred_sets = writes(100, "error");
    let no_storage = writes(0, "ok");
    let hundred_committed = writes(100, "ok");

    let micros = |duration: Duration| duration.as_micros();
    println!("storage_open_us: {}", micros(one_get.saturating_sub(none)));
    println!(
        "storage_get_us: {}",
        micros(hundred_gets.saturating_sub(one_get)) / 99
    );
    println!(
        "storage_set_us: {}",
        micros(hundred_sets.saturating_sub(one_set)) / 99
    );
    // A rollback costs about what a commit does (both end the journal), so the commit is
    // measured with everything else an update with storage does: open, writes and commit
    println!(
        "storage_update_100_us: {}",
        micros(hundred_committed.saturating_sub(no_storage))
    );
    println!("render_reading_100_us: {}", micros(hundred_gets));

    for value_size in [32u64, 1000, 4096] {
        let (used, file) = fill(&probe, value_size, fill_quota);
        println!("file_bytes_at_quota: {value_size} {used} {file}");
    }
}

/// Fills a fresh store of `quota` with values of `value_size` bytes, as many per update as the
/// limits allow, until a write is refused as over quota. Returns its usage and its file's size.
fn fill(probe: &[u8], value_size: u64, quota: u64) -> (u64, u64) {
    let data = tempfile::TempDir::new().expect("a temporary data directory");
    let runtime = Runtime::new(Limits::default(), RuntimeOptions::new(data.path()))
        .expect("the runtime starts");
    let grants = GrantStore::open(data.path().join("grants.json"));
    let plugin = load(&runtime, &grants, probe, "local/storage-fill", quota);
    let per_update = ((1024 * 1024) / (value_size + 16)).clamp(1, 999);
    for batch in 0.. {
        let content = update(
            &plugin,
            &[set_many(&format!("b{batch}-{{i}}"), value_size, per_update)],
            "ok",
        );
        if !content["first_error"].is_null() {
            assert_eq!(content["first_error"]["error"], "refused:over-quota");
            break;
        }
    }
    let usage = plugin
        .render(&json!({ "render_steps": [{ "op": "usage" }] }).to_string())
        .result
        .expect("the usage");
    let used = usage["answers"][0]["ok"]["used"]
        .as_u64()
        .expect("the usage");
    // The fill's data directory holds this one store
    let file = std::fs::read_dir(data.path().join("storage"))
        .expect("the storage directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "sqlite")
        })
        .map_or(0, |path| {
            std::fs::metadata(path).map_or(0, |metadata| metadata.len())
        });
    (used, file)
}

/// `store-probe` as `id`, declaring storage with `quota`, approved with its digest and loaded.
fn load(runtime: &Runtime, grants: &GrantStore, probe: &[u8], id: &str, quota: u64) -> Plugin {
    let manifest = json!({
        "manifest_version": 1, "id": id, "version": "0.1.0", "author": "Dashboard contributors",
        "permissions": [{ "name": "storage", "required": true, "quota_bytes": quota }],
        "login_providers": []
    });
    // The fixture is built bare: each run packs it with the manifest it needs
    let package = package::pack(probe, manifest.to_string().as_bytes()).expect("the manifest");
    let checked = runtime.check(&package).expect("the package checks");
    checked
        .approve(grants, ORIGIN, Some(&checked.approval_digest()))
        .expect("approved");
    runtime
        .load(
            &package,
            ORIGIN,
            Arc::new(GrantStore::open(grants.path().unwrap())),
        )
        .expect("the package loads")
}

fn set_many(name: &str, value_size: u64, repeat: u64) -> Value {
    json!({ "op": "set", "name": name, "value_size": value_size, "repeat": repeat })
}

fn update(plugin: &Plugin, steps: &[Value], end: &str) -> Value {
    let config = json!({ "steps": steps, "end": end }).to_string();
    plugin.update(&config).result.expect("the update")
}

/// The median time of `iterations` calls of `run`.
fn median(iterations: usize, mut run: impl FnMut()) -> Duration {
    let mut times: Vec<Duration> = (0..iterations)
        .map(|_| {
            let started = Instant::now();
            run();
            started.elapsed()
        })
        .collect();
    times.sort();
    times[times.len() / 2]
}
