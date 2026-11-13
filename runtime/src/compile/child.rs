//! The compile process's own side (`specs/010-plugin-installation/research.md`, R2): entered when
//! the executable's only argument is `--compile-package`, before anything else of the app or the
//! CLI starts.
//!
//! It builds its engine and its thread pool, confines itself (research R13: on Linux, Landlock
//! before the pool exists, so its threads inherit it, and seccomp filters on every thread after),
//! and only then reads one package from standard input, compiles it with the
//! same engine configuration as the host, and writes the engine's fingerprint and then the
//! compiled form to standard output. Once confined it cannot open a file, make a request or reach
//! another process: the package is its only input.

use std::io::{Read, Write};
use std::process::ExitCode;

use wasmtime::Engine;

use super::{EXIT_MEMORY, EXIT_PANICKED, EXIT_REFUSED, fingerprint};
use crate::{COMPONENT_PREAMBLE, Limits, engine_config};

/// Why the child ended without a compiled form.
enum Refusal {
    /// The system refused memory or a thread: [`EXIT_MEMORY`].
    Memory(String),
    /// Anything else, the package's own faults among them: [`EXIT_REFUSED`].
    Other(String),
}

impl From<String> for Refusal {
    fn from(reason: String) -> Refusal {
        Refusal::Other(reason)
    }
}

/// Runs the compile process and returns its exit code: 0 with the form written, or
/// [`EXIT_REFUSED`] or [`EXIT_MEMORY`] with the reason on standard error.
pub fn child_main() -> ExitCode {
    // A panic ends the process with a code of its own, not with the abort the release build's
    // `panic = "abort"` would give: an abort then means only an allocation that failed, which the
    // parent reads as the memory limit without reading what the child wrote
    std::panic::set_hook(Box::new(|info| {
        let _ = writeln!(std::io::stderr(), "{info}");
        std::process::exit(i32::from(EXIT_PANICKED));
    }));
    let (code, reason) = match run() {
        Ok(()) => return ExitCode::SUCCESS,
        Err(Refusal::Memory(reason)) => (EXIT_MEMORY, reason),
        Err(Refusal::Other(reason)) => (EXIT_REFUSED, reason),
    };
    // The parent keeps at most 64 KiB of this, for the app's log, never for the page
    let _ = writeln!(std::io::stderr(), "{reason}");
    ExitCode::from(code)
}

/// A refusal from the engine or the compiler: [`Refusal::Memory`] when the system refused an
/// allocation somewhere in its chain, which only the system's own error can say.
fn engine_refusal(error: &wasmtime::Error) -> Refusal {
    let reason = format!("{error:#}");
    let memory = error.chain().any(|cause| {
        cause
            .downcast_ref::<std::io::Error>()
            .is_some_and(|io| io.kind() == std::io::ErrorKind::OutOfMemory || is_enomem(io))
    });
    if memory {
        Refusal::Memory(reason)
    } else {
        Refusal::Other(reason)
    }
}

fn is_enomem(error: &std::io::Error) -> bool {
    #[cfg(unix)]
    return error.raw_os_error() == Some(libc::ENOMEM);
    #[cfg(not(unix))]
    {
        let _ = error;
        false
    }
}

fn run() -> Result<(), Refusal> {
    let engine = Engine::new(&engine_config()).map_err(|error| engine_refusal(&error))?;
    let mut out = std::io::stdout().lock();
    out.write_all(&fingerprint(&engine).to_le_bytes())
        .map_err(|error| error.to_string())?;
    // What binds the threads the process starts from now on, then the pool Cranelift compiles
    // on, of the size the parent chose in RAYON_NUM_THREADS, then what binds every thread: once
    // confined, no thread can be started
    confine_before_threads()?;
    // Only the system can refuse the pool, for lack of memory or of threads
    rayon::ThreadPoolBuilder::new()
        .build_global()
        .map_err(|error| Refusal::Memory(format!("the compile threads: {error}")))?;
    confine_after_threads()?;
    #[cfg(feature = "test-network")]
    if let Some(attempt) = std::env::args()
        .nth(2)
        .and_then(|arg| arg.strip_prefix(super::TRY_ESCAPE).map(str::to_owned))
    {
        try_escape(&attempt);
    }

    let max = Limits::default().max_package_bytes;
    let mut package = Vec::new();
    std::io::stdin()
        .lock()
        .take(max as u64 + 1)
        .read_to_end(&mut package)
        .map_err(|error| error.to_string())?;
    if package.len() > max {
        return Err(Refusal::Other(format!(
            "the package is larger than the limit of {max} bytes"
        )));
    }
    // A binary component only: precompile_component would also take the text format in a
    // build with Wasmtime's `wat` feature, as the runtime's tests are
    if !package.starts_with(&COMPONENT_PREAMBLE) {
        return Err(Refusal::Other("not a WebAssembly component".to_owned()));
    }
    let compiled = engine
        .precompile_component(&package)
        .map_err(|error| engine_refusal(&error))?;
    out.write_all(&compiled)
        .map_err(|error| error.to_string())?;
    out.flush()
        .map_err(|error| Refusal::Other(error.to_string()))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn refused(error: String) -> String {
    format!("the compile process could not confine itself: {error}")
}

/// The confinement that binds the threads started afterwards (research R13): Landlock on Linux.
#[cfg(target_os = "linux")]
fn confine_before_threads() -> Result<(), String> {
    super::confine_linux::before_threads().map_err(refused)
}

/// Elsewhere, everything is confined after the threads exist, or before the process starts.
#[cfg(not(target_os = "linux"))]
fn confine_before_threads() -> Result<(), String> {
    Ok(())
}

/// The confinement of every thread, before the process reads anything from the parent: on Linux,
/// the seccomp filters.
#[cfg(target_os = "linux")]
fn confine_after_threads() -> Result<(), String> {
    super::confine_linux::after_threads().map_err(refused)
}

/// On macOS: the memory watchdog, then the sandbox.
#[cfg(target_os = "macos")]
fn confine_after_threads() -> Result<(), String> {
    let memory = std::env::var(super::MEMORY_VARIABLE)
        .ok()
        .and_then(|value| value.parse::<u64>().ok());
    super::confine_macos::confine(memory).map_err(refused)
}

/// On Windows the parent created the process already confined, in an AppContainer.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn confine_after_threads() -> Result<(), String> {
    Ok(())
}

/// The runtime's tests only (`compile-confined`): once confined, makes one attempt, named by
/// the hook's argument, and if it worked says so on its error stream and ends at once, before any
/// other call the confinement might refuse:
///
/// - `file`: opens a file of the user's, `Cargo.toml` in the current folder (the runtime's own);
/// - `file-on-a-compile-thread`: the same, from one of the threads that existed before
///   confinement, which Linux's Landlock binds only when it came first;
/// - `net:<port>`: connects to `port` on loopback, where the test listens;
/// - `burn-cpu`: not an escape but a package that costs more CPU than any limit: it computes for
///   30 seconds of wall time, so the CPU limit's test depends neither on how fast a system
///   compiles nor on when Windows checks a job's time, which it does only now and then.
#[cfg(feature = "test-network")]
#[allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    reason = "a test of the confinement, not a request: it must fail"
)]
fn try_escape(attempt: &str) {
    let worked = match attempt {
        "file" => open_users_file(),
        "burn-cpu" => {
            let started = std::time::Instant::now();
            let mut state = 1u64;
            while started.elapsed() < std::time::Duration::from_secs(30) {
                state = std::hint::black_box(
                    state
                        .wrapping_mul(6_364_136_223_846_793_005)
                        .wrapping_add(1),
                );
            }
            false
        }
        "file-on-a-compile-thread" => {
            let (send, receive) = std::sync::mpsc::channel();
            rayon::spawn(move || {
                let _ = send.send(open_users_file());
            });
            receive.recv().unwrap_or(false)
        }
        _ => attempt
            .strip_prefix("net:")
            .and_then(|port| port.parse::<u16>().ok())
            .is_some_and(|port| {
                let address = std::net::SocketAddr::from(([127, 0, 0, 1], port));
                std::net::TcpStream::connect_timeout(&address, std::time::Duration::from_secs(2))
                    .is_ok()
            }),
    };
    if worked {
        let message = format!("escaped: {attempt}\n");
        let _ = std::io::stderr().write_all(message.as_bytes());
        std::process::exit(3);
    }
}

/// Opens `Cargo.toml` in the current folder with the system's own call, so nothing else runs
/// before the answer.
#[cfg(feature = "test-network")]
fn open_users_file() -> bool {
    #[cfg(unix)]
    {
        // SAFETY: a constant, nul-terminated path; the descriptor is not used again
        let fd = unsafe { libc::open(c"Cargo.toml".as_ptr(), libc::O_RDONLY) };
        fd >= 0
    }
    #[cfg(not(unix))]
    {
        std::fs::File::open("Cargo.toml").is_ok()
    }
}
