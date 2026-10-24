//! Helpers shared by the integration tests.

// Each test file compiles this module on its own and uses only some of the helpers.
#![allow(dead_code)]

pub mod servers;

use std::path::PathBuf;
use std::sync::Arc;

use runtime::RuntimeOptions;
use runtime::grants::GrantStore;
use runtime::manifest::Manifest;
use tempfile::TempDir;

/// The path of the plugin component `target/plugins/<name>.wasm`, built by
/// `cargo xtask plugins`.
///
/// A missing fixture panics: it must fail the test, never skip it, or a hostile case that was
/// never built would let a broken rule pass (Principle VIII).
pub fn fixture_path(name: &str) -> PathBuf {
    let path = [env!("CARGO_MANIFEST_DIR"), "..", "target", "plugins"]
        .iter()
        .collect::<PathBuf>()
        .join(format!("{name}.wasm"));
    if !path.is_file() {
        panic!(
            "missing fixture {}: run cargo xtask plugins",
            path.display()
        );
    }
    path
}

/// The bytes of the plugin component `target/plugins/<name>.wasm`; see [`fixture_path`].
pub fn fixture(name: &str) -> Vec<u8> {
    let path = fixture_path(name);
    std::fs::read(&path)
        .unwrap_or_else(|error| panic!("reading fixture {}: {error}", path.display()))
}

/// The origin every test approves fixtures from and loads them with.
pub const TEST_ORIGIN: &str = "test://fixtures";

/// A manifest for `id` and `version`, requesting `permissions` as (name, required) pairs.
pub fn manifest(id: &str, version: &str, permissions: &[(&str, bool)]) -> String {
    let permissions: Vec<serde_json::Value> = permissions
        .iter()
        .map(|(name, required)| serde_json::json!({ "name": name, "required": required }))
        .collect();
    serde_json::json!({
        "manifest_version": 1,
        "id": id,
        "version": version,
        "author": "Dashboard contributors",
        "permissions": permissions,
        "login_providers": []
    })
    .to_string()
}

/// `package` with its top-level manifest section, if it has one, replaced by `manifest`.
pub fn repack(package: &[u8], manifest: &str) -> Vec<u8> {
    runtime::package::pack(&without_manifest(package), manifest.as_bytes())
        .unwrap_or_else(|error| panic!("packing {manifest}: {error}"))
}

/// `package` without its top-level manifest section.
pub fn without_manifest(package: &[u8]) -> Vec<u8> {
    use wasmparser::{Parser, Payload};

    let mut depth = 0;
    for payload in Parser::new(0).parse_all(package) {
        match payload.expect("the package is well formed") {
            Payload::Version { .. } => depth += 1,
            Payload::End(_) => depth -= 1,
            Payload::CustomSection(section)
                if depth == 1 && section.name() == runtime::package::MANIFEST_SECTION =>
            {
                // The range covers the name and the data; the section id and the size, in
                // LEB128, come before it
                let range = section.range();
                let (start, end) = (range.start as usize, range.end as usize);
                let mut size_bytes = 1;
                while (end - start) >> (7 * size_bytes) > 0 {
                    size_bytes += 1;
                }
                let section_start = start - size_bytes - 1;
                assert_eq!(package[section_start], 0, "a custom section starts there");
                let mut bare = package[..section_start].to_vec();
                bare.extend_from_slice(&package[end..]);
                return bare;
            }
            _ => {}
        }
    }
    package.to_vec()
}

/// An in-memory grant store where `package`'s required permissions are approved from
/// [`TEST_ORIGIN`].
pub fn approved(package: &[u8]) -> Arc<GrantStore> {
    let store = GrantStore::in_memory();
    approve(&store, package, TEST_ORIGIN);
    Arc::new(store)
}

/// Approves `package`'s required permissions in `store`, from `origin`; a plugin that declares
/// storage with the approval digest of what it declares, as `approve --expect` does.
pub fn approve(store: &GrantStore, package: &[u8], origin: &str) {
    let manifest = runtime::package::manifest_section(package)
        .and_then(Manifest::parse)
        .unwrap_or_else(|error| panic!("the package's manifest: {error}"));
    let digest = runtime::package::digest(package);
    let approved = if manifest.storage().is_some() {
        let expected = runtime::grants::approval_digest(&digest, &manifest, &[]);
        store.approve_connected(&manifest, origin, &digest, &[], &expected)
    } else {
        store.approve(&manifest, origin, &digest)
    };
    approved.unwrap_or_else(|error| panic!("approving {}: {error}", manifest.id()));
}

/// A temporary directory, removed when dropped, and the path of a grant record inside it that
/// does not exist yet.
pub fn temp_grants() -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("a temporary directory");
    let path = dir.path().join("grants.json");
    (dir, path)
}

/// `package` with `payload` appended as one more manifest section, unchecked, as a hostile
/// author could write it.
pub fn append_manifest_section(package: &[u8], payload: &[u8]) -> Vec<u8> {
    fn leb128(mut value: usize, out: &mut Vec<u8>) {
        loop {
            let byte = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                out.push(byte);
                return;
            }
            out.push(byte | 0x80);
        }
    }
    let name = runtime::package::MANIFEST_SECTION.as_bytes();
    let mut contents = Vec::new();
    leb128(name.len(), &mut contents);
    contents.extend_from_slice(name);
    contents.extend_from_slice(payload);
    let mut appended = package.to_vec();
    appended.push(0);
    leb128(contents.len(), &mut appended);
    appended.extend_from_slice(&contents);
    appended
}

/// Runtime options for tests whose plugins declare no connector: development mode off, and a
/// data directory of this test binary's own, which such plugins never write to.
pub fn options() -> RuntimeOptions {
    static DATA: std::sync::OnceLock<TempDir> = std::sync::OnceLock::new();
    let data = DATA.get_or_init(|| TempDir::new().expect("a temporary data directory"));
    RuntimeOptions::new(data.path())
}

/// A manifest for `id` that declares `connectors` as (id, major, operations, required).
pub fn connected_manifest(id: &str, connectors: &[(&str, u16, &[&str], bool)]) -> String {
    let connectors: Vec<serde_json::Value> = connectors
        .iter()
        .map(|(id, major, operations, required)| {
            serde_json::json!({ "id": id, "major": major, "operations": operations, "required": required })
        })
        .collect();
    serde_json::json!({
        "manifest_version": 1,
        "id": id,
        "version": "0.1.0",
        "author": "Dashboard contributors",
        "permissions": [],
        "login_providers": [],
        "connectors": connectors
    })
    .to_string()
}

/// A manifest for `id` and `version` that declares storage, required or optional, with
/// `quota_bytes` (feature 004).
pub fn stored_manifest(id: &str, version: &str, required: bool, quota_bytes: u64) -> String {
    serde_json::json!({
        "manifest_version": 1,
        "id": id,
        "version": version,
        "author": "Dashboard contributors",
        "permissions": [{ "name": "storage", "required": required, "quota_bytes": quota_bytes }],
        "login_providers": []
    })
    .to_string()
}

/// `store-probe` packed with `manifest`.
pub fn store_probe(manifest: &str) -> Vec<u8> {
    repack(&fixture("store-probe"), manifest)
}

/// A `store-probe` update configuration that makes `steps`, each a step object of the probe
/// (`plugins/fixtures/store-probe/src/lib.rs`), and ends as `end` says (`"ok"` when `None`).
pub fn steps(steps: &[serde_json::Value], end: Option<&str>) -> String {
    serde_json::json!({ "steps": steps, "end": end.unwrap_or("ok") }).to_string()
}

/// A `store-probe` render configuration that makes `steps` in render.
pub fn render_steps(steps: &[serde_json::Value]) -> String {
    serde_json::json!({ "render_steps": steps }).to_string()
}

/// `net-probe` packed with `manifest`.
pub fn probe(manifest: &str) -> Vec<u8> {
    repack(&fixture("net-probe"), manifest)
}

/// A runtime with a data directory of its own, a grant record on disk beside it, and the
/// development connectors directory `<data>/connectors/`.
pub struct World {
    pub data: TempDir,
    pub runtime: runtime::Runtime,
    pub grants: Arc<GrantStore>,
}

impl World {
    /// Requests go through the test seam `test`; development mode as given.
    pub fn seam(test: runtime::TestNetwork, development: bool) -> World {
        World::seam_with(runtime::Limits::default(), test, development)
    }

    pub fn seam_with(
        limits: runtime::Limits,
        test: runtime::TestNetwork,
        development: bool,
    ) -> World {
        let data = TempDir::new().expect("a temporary data directory");
        let options = RuntimeOptions::new(data.path()).development(development);
        let runtime =
            runtime::Runtime::with_test_network(limits, options, test).expect("the runtime starts");
        World::with(data, runtime)
    }

    /// Requests go to the real network (local connectors on loopback); development mode as given.
    pub fn production(development: bool) -> World {
        let data = TempDir::new().expect("a temporary data directory");
        let options = RuntimeOptions::new(data.path()).development(development);
        let runtime =
            runtime::Runtime::new(runtime::Limits::default(), options).expect("the runtime starts");
        World::with(data, runtime)
    }

    fn with(data: TempDir, runtime: runtime::Runtime) -> World {
        let grants = Arc::new(GrantStore::open(data.path().join("grants.json")));
        World {
            data,
            runtime,
            grants,
        }
    }

    /// Writes `connector` as the local connector `<data>/connectors/<name>.json`.
    pub fn write_connector(&self, name: &str, connector: &serde_json::Value) -> PathBuf {
        let dir = self.data.path().join("connectors");
        std::fs::create_dir_all(&dir).expect("the connectors directory");
        let path = dir.join(format!("{name}.json"));
        std::fs::write(&path, connector.to_string()).expect("the connector is written");
        path
    }

    /// Approves `package` from [`TEST_ORIGIN`] with the approval digest of what it is now.
    pub fn approve(&self, package: &[u8]) {
        let checked = self
            .runtime
            .check(package)
            .unwrap_or_else(|error| panic!("the package checks: {error}"));
        let digest = checked.approval_digest();
        checked
            .approve(&self.grants, TEST_ORIGIN, Some(&digest))
            .unwrap_or_else(|error| panic!("approving: {error}"));
    }

    pub fn load(&self, package: &[u8]) -> runtime::Plugin {
        self.runtime
            .load(package, TEST_ORIGIN, Arc::clone(&self.grants))
            .unwrap_or_else(|error| panic!("the package loads: {error}"))
    }

    /// Approves and loads `package`.
    pub fn approve_and_load(&self, package: &[u8]) -> runtime::Plugin {
        self.approve(package);
        self.load(package)
    }
}

/// The answers of a `net-probe` update or render.
pub fn answers(content: &serde_json::Value) -> Vec<serde_json::Value> {
    content["answers"]
        .as_array()
        .unwrap_or_else(|| panic!("net-probe answered {content}"))
        .clone()
}

/// A `net-probe` configuration that makes `calls`, each `(connector, operation, arguments)`.
pub fn calls(calls: &[(&str, &str, serde_json::Value)]) -> String {
    let calls: Vec<serde_json::Value> = calls
        .iter()
        .map(|(connector, operation, arguments)| {
            serde_json::json!({ "connector": connector, "operation": operation, "arguments": arguments })
        })
        .collect();
    serde_json::json!({ "calls": calls }).to_string()
}
