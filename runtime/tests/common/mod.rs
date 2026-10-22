//! Helpers shared by the integration tests.

use std::io::ErrorKind;
use std::path::PathBuf;

/// Reads the plugin component `target/plugins/<name>.wasm`, built by `cargo xtask plugins`.
///
/// A missing fixture panics: it must fail the test, never skip it, or a hostile case that was
/// never built would let a broken rule pass (Principle VIII).
pub fn fixture(name: &str) -> Vec<u8> {
    let path: PathBuf = [env!("CARGO_MANIFEST_DIR"), "..", "target", "plugins"]
        .iter()
        .collect::<PathBuf>()
        .join(format!("{name}.wasm"));
    match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            panic!("missing fixture {}: run cargo xtask plugins", path.display())
        }
        Err(error) => panic!("reading fixture {}: {error}", path.display()),
    }
}
