//! The compile process's own side (`specs/010-plugin-installation/research.md`, R2): entered when
//! the executable's only argument is `--compile-package`, before anything else of the app or the
//! CLI starts.
//!
//! It reads one package from standard input, compiles it with the same engine configuration as
//! the host, and writes the engine's fingerprint and then the compiled form to standard output.
//! It opens no file, makes no request and reads no record: the package is the only input.

use std::io::{Read, Write};
use std::process::ExitCode;

use wasmtime::Engine;

use super::{EXIT_REFUSED, fingerprint};
use crate::{COMPONENT_PREAMBLE, Limits, engine_config};

/// Runs the compile process and returns its exit code: 0 with the form written, or
/// [`EXIT_REFUSED`] with the reason on standard error.
pub fn child_main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            // The parent keeps at most 64 KiB of this, for the app's log, never for the page
            let _ = writeln!(std::io::stderr(), "{reason}");
            ExitCode::from(EXIT_REFUSED)
        }
    }
}

fn run() -> Result<(), String> {
    let engine = Engine::new(&engine_config()).map_err(|error| format!("{error:#}"))?;
    let mut out = std::io::stdout().lock();
    out.write_all(&fingerprint(&engine).to_le_bytes())
        .map_err(|error| error.to_string())?;

    let max = Limits::default().max_package_bytes;
    let mut package = Vec::new();
    std::io::stdin()
        .lock()
        .take(max as u64 + 1)
        .read_to_end(&mut package)
        .map_err(|error| error.to_string())?;
    if package.len() > max {
        return Err(format!(
            "the package is larger than the limit of {max} bytes"
        ));
    }
    // A binary component only: precompile_component would also take the text format in a
    // build with Wasmtime's `wat` feature, as the runtime's tests are
    if !package.starts_with(&COMPONENT_PREAMBLE) {
        return Err("not a WebAssembly component".to_owned());
    }
    let compiled = engine
        .precompile_component(&package)
        .map_err(|error| format!("{error:#}"))?;
    out.write_all(&compiled)
        .map_err(|error| error.to_string())?;
    out.flush().map_err(|error| error.to_string())
}
