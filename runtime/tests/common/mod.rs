//! Helpers shared by the integration tests.

// Each test file compiles this module on its own and uses only some of the helpers.
#![allow(dead_code)]

use std::path::PathBuf;
use std::sync::Arc;

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

/// Approves `package`'s required permissions in `store`, from `origin`.
pub fn approve(store: &GrantStore, package: &[u8], origin: &str) {
    let manifest = runtime::package::manifest_section(package)
        .and_then(Manifest::parse)
        .unwrap_or_else(|error| panic!("the package's manifest: {error}"));
    store
        .approve(&manifest, origin, &runtime::package::digest(package))
        .unwrap_or_else(|error| panic!("approving {}: {error}", manifest.id()));
}

/// A temporary directory, removed when dropped, and the path of a grant record inside it that
/// does not exist yet.
pub fn temp_grants() -> (TempDir, PathBuf) {
    let dir = TempDir::new().expect("a temporary directory");
    let path = dir.path().join("grants.json");
    (dir, path)
}
