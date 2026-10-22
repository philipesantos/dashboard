//! Helpers shared by the integration tests.

// Each test file compiles this module on its own and uses only some of the helpers.
#![allow(dead_code)]

use std::path::PathBuf;

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
