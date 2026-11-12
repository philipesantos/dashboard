//! The compile process's own side (`specs/010-plugin-installation/research.md`, R2): entered when
//! the executable's only argument is `--compile-package`, before anything else of the app or the
//! CLI starts.
//!
//! It builds its engine and its thread pool, confines itself (research R13: on Linux, Landlock and
//! a seccomp filter), and only then reads one package from standard input, compiles it with the
//! same engine configuration as the host, and writes the engine's fingerprint and then the
//! compiled form to standard output. Once confined it cannot open a file, make a request or reach
//! another process: the package is its only input.

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
    // The pool Cranelift compiles on, of the size the parent chose in RAYON_NUM_THREADS, built
    // now: once confined, no thread can be started
    rayon::ThreadPoolBuilder::new()
        .build_global()
        .map_err(|error| format!("the compile threads: {error}"))?;
    confine()?;
    #[cfg(feature = "test-network")]
    if std::env::args_os()
        .nth(2)
        .is_some_and(|arg| arg == super::TRY_ESCAPE)
    {
        try_escape();
    }

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

/// Confines this process before it reads anything from the parent (research R13).
fn confine() -> Result<(), String> {
    #[cfg(target_os = "linux")]
    return super::confine_linux::confine()
        .map_err(|error| format!("the compile process could not confine itself: {error}"));
    #[cfg(not(target_os = "linux"))]
    Ok(())
}

/// The runtime's tests only (`compile-confined`): once confined, tries to open a file and a
/// socket, and says so on its error stream and exits 3 if either worked. Confined, the first try
/// ends the process.
#[cfg(feature = "test-network")]
fn try_escape() {
    let file = std::fs::File::open("/etc/hostname").is_ok()
        || std::fs::File::open("C:\\Windows\\win.ini").is_ok();
    #[cfg(unix)]
    // SAFETY: socket(2) with constant arguments; the descriptor is closed if it was made
    let socket = unsafe {
        let fd = libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0);
        if fd >= 0 {
            libc::close(fd);
        }
        fd >= 0
    };
    #[cfg(not(unix))]
    let socket = false;
    if file || socket {
        let _ = writeln!(std::io::stderr(), "escaped: file {file}, socket {socket}");
        std::process::exit(3);
    }
}
