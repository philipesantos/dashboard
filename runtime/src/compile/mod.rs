//! Compiling an installed package in a separate process (`specs/010-plugin-installation/
//! research.md`, R1, R2, R3): Cranelift's time and memory are not bounded by any of the runtime's
//! per-call limits, so a package the app did not build is compiled by a child process, the app's
//! own executable started with `--compile-package`, and the app only loads the result, through
//! [`Runtime::load_compiled`](crate::Runtime::load_compiled), after checking its recorded digest.
//!
//! The protocol is the child's standard streams: the parent writes the package to its input; the
//! child writes 8 bytes, its engine's fingerprint (little-endian), then the compiled form, to its
//! output, and any reason it refused to its error stream. A fingerprint other than the parent's
//! means the executable on disk is not the running one (the app was updated while it ran): the
//! form would not load, so it is refused at once, never retried.

mod child;
#[cfg(target_os = "linux")]
mod confine_linux;

use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use wasmtime::Engine;

pub use child::child_main;

use crate::{ErrorKind, Limits, RuntimeError, engine_config, text};

/// The only argument that starts the compile process.
pub const COMPILE_ARGUMENT: &str = "--compile-package";

/// The runtime's tests only: after this argument, the child tries to escape its confinement
/// (`compile-confined`, task T037).
#[cfg(feature = "test-network")]
pub const TRY_ESCAPE: &str = "--try-escape";

/// Whether this process was started as the compile process: its only argument is
/// [`COMPILE_ARGUMENT`] (and, in the runtime's tests, [`TRY_ESCAPE`] after it). The app and the
/// CLI check it before anything else starts.
pub fn is_compile_invocation() -> bool {
    let mut args = std::env::args_os().skip(1);
    if args.next().is_none_or(|arg| arg != COMPILE_ARGUMENT) {
        return false;
    }
    match args.next() {
        None => true,
        #[cfg(feature = "test-network")]
        Some(arg) if arg == TRY_ESCAPE => args.next().is_none(),
        Some(_) => false,
    }
}

/// The child refused the package; the reason is on its error stream.
const EXIT_REFUSED: u8 = 2;

/// How often the parent looks at the child while it runs.
const POLL: Duration = Duration::from_millis(10);

/// The limits of one compile (`data-model.md`, Compile limits). Tests lower them to break each
/// one; no environment variable or setting changes them in a release build.
#[derive(Debug, Clone)]
pub struct CompileLimits {
    /// The parent kills the child after this long.
    pub wall_time: Duration,
    /// The CPU time of all its threads together: `RLIMIT_CPU` on Unix, whole seconds.
    pub cpu_time: Duration,
    /// Its memory: `RLIMIT_DATA` on Linux, which counts what an allocator takes from the system.
    pub memory_bytes: u64,
    /// The rayon threads Cranelift may use: half the cores, at least one.
    pub threads: usize,
    /// The parent stops reading, and kills the child, past this many bytes of compiled form.
    pub max_compiled_bytes: usize,
    /// The child's error stream kept for the app's log, never shown to the page.
    pub max_stderr_bytes: usize,
}

impl Default for CompileLimits {
    fn default() -> Self {
        let cores = thread::available_parallelism().map_or(2, |cores| cores.get());
        CompileLimits {
            wall_time: Duration::from_secs(20),
            cpu_time: Duration::from_secs(60),
            memory_bytes: 1536 * 1024 * 1024,
            threads: (cores / 2).max(1),
            max_compiled_bytes: 64 * 1024 * 1024,
            max_stderr_bytes: 64 * 1024,
        }
    }
}

/// The parent's side: starts the compile process for one package at a time and reads its form.
#[derive(Debug, Clone)]
pub struct Compiler {
    executable: PathBuf,
    limits: CompileLimits,
    /// The fingerprint the child's engine must have: this process's own.
    expected: u64,
    /// The runtime's tests only: the child tries to escape once confined.
    #[cfg(feature = "test-network")]
    try_escape: bool,
}

impl Compiler {
    /// A compiler that starts this very executable: `/proc/self/exe` on Linux, which names the
    /// running file even if the one on disk was replaced, or `current_exe` elsewhere.
    pub fn for_this_executable(limits: CompileLimits) -> Result<Compiler, RuntimeError> {
        let executable = if cfg!(target_os = "linux") {
            PathBuf::from("/proc/self/exe")
        } else {
            std::env::current_exe().map_err(|error| {
                RuntimeError::new(
                    ErrorKind::CompileFailed,
                    format!("the app cannot find its own executable: {error}"),
                )
            })?
        };
        Ok(Compiler::with_executable(executable, limits))
    }

    /// A compiler that starts `executable`, which must enter [`child_main`] on
    /// [`COMPILE_ARGUMENT`]: the app's and the CLI's do.
    pub fn with_executable(executable: impl Into<PathBuf>, limits: CompileLimits) -> Compiler {
        Compiler {
            executable: executable.into(),
            limits,
            expected: own_fingerprint(),
            #[cfg(feature = "test-network")]
            try_escape: false,
        }
    }

    /// For tests: the fingerprint the child must have, as if this process ran another engine.
    #[doc(hidden)]
    pub fn with_expected_fingerprint(mut self, fingerprint: u64) -> Compiler {
        self.expected = fingerprint;
        self
    }

    /// The runtime's tests only: the child tries to open a file and a socket once confined.
    #[cfg(feature = "test-network")]
    #[doc(hidden)]
    pub fn trying_to_escape(mut self) -> Compiler {
        self.try_escape = true;
        self
    }

    /// The executable this compiler starts.
    pub fn executable(&self) -> &Path {
        &self.executable
    }

    /// Compiles `package` in a child process and returns its compiled form, which only
    /// [`Runtime::load_compiled`](crate::Runtime::load_compiled) may load, after its digest is
    /// recorded. Every refusal names why; no child is left behind.
    pub fn compile(&self, package: &[u8]) -> Result<Vec<u8>, RuntimeError> {
        self.compile_until(package, &AtomicBool::new(false))
    }

    /// As [`compile`](Self::compile), stopped as soon as `cancel` is set: the child is killed
    /// and waited for, and the compile is `compile-failed`. The app sets it when the window
    /// closes or the app quits while an installation compiles (FR-023).
    pub fn compile_until(
        &self,
        package: &[u8],
        cancel: &AtomicBool,
    ) -> Result<Vec<u8>, RuntimeError> {
        let max = Limits::default().max_package_bytes;
        if package.len() > max {
            return Err(RuntimeError::new(
                ErrorKind::InvalidPackage,
                format!("the package is larger than the limit of {max} bytes"),
            ));
        }
        let mut child = self.spawn()?;
        let outcome = self.run(&mut child, package, cancel);
        // Whatever happened, the child is gone before this returns
        let _ = child.kill();
        let _ = child.wait();
        outcome
    }

    fn spawn(&self) -> Result<Child, RuntimeError> {
        let mut command = Command::new(&self.executable);
        command.arg(COMPILE_ARGUMENT);
        #[cfg(feature = "test-network")]
        if self.try_escape {
            command.arg(TRY_ESCAPE);
        }
        command
            .env_clear()
            .env("RAYON_NUM_THREADS", self.limits.threads.to_string())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Windows needs SystemRoot to start a process at all; an AppImage finds its libraries
        // through LD_LIBRARY_PATH
        for kept in ["SystemRoot", "LD_LIBRARY_PATH"] {
            if let Some(value) = std::env::var_os(kept) {
                command.env(kept, value);
            }
        }
        #[cfg(unix)]
        self.limit_before_exec(&mut command);
        command.spawn().map_err(|error| {
            RuntimeError::new(
                ErrorKind::CompileFailed,
                format!("the compile process could not start: {error}"),
            )
        })
    }

    /// The limits a Unix child gets before it runs anything: its CPU time, its memory (Linux),
    /// a below-normal priority, and, on Linux, death when its parent dies.
    #[cfg(unix)]
    fn limit_before_exec(&self, command: &mut Command) {
        use std::os::unix::process::CommandExt;
        let cpu = self.limits.cpu_time.as_secs().max(1) as libc::rlim_t;
        let memory = self.limits.memory_bytes as libc::rlim_t;
        // SAFETY: the closure runs between fork and exec, and calls only async-signal-safe
        // functions with constant or copied arguments
        unsafe {
            command.pre_exec(move || {
                let soft_then_kill = libc::rlimit {
                    rlim_cur: cpu,
                    rlim_max: cpu + 1,
                };
                if libc::setrlimit(libc::RLIMIT_CPU, &soft_then_kill) != 0 {
                    return Err(std::io::Error::last_os_error());
                }
                #[cfg(target_os = "linux")]
                {
                    let data = libc::rlimit {
                        rlim_cur: memory,
                        rlim_max: memory,
                    };
                    if libc::setrlimit(libc::RLIMIT_DATA, &data) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                    if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) != 0 {
                        return Err(std::io::Error::last_os_error());
                    }
                }
                #[cfg(not(target_os = "linux"))]
                let _ = memory;
                libc::setpriority(libc::PRIO_PROCESS, 0, 10);
                Ok(())
            });
        }
    }

    fn run(
        &self,
        child: &mut Child,
        package: &[u8],
        cancel: &AtomicBool,
    ) -> Result<Vec<u8>, RuntimeError> {
        let failed = |reason: String| RuntimeError::new(ErrorKind::CompileFailed, reason);
        let over_output = Arc::new(AtomicBool::new(false));

        let mut stdin = child.stdin.take().expect("stdin is piped");
        let input = package.to_vec();
        let writer = thread::spawn(move || {
            // A child that stops reading closes the pipe; what it says is in its exit
            let _ = stdin.write_all(&input);
        });

        let mut stdout = child.stdout.take().expect("stdout is piped");
        let max_output = self.limits.max_compiled_bytes as u64 + 8;
        let over = Arc::clone(&over_output);
        let reader = thread::spawn(move || {
            let mut output = Vec::new();
            let read = (&mut stdout).take(max_output + 1).read_to_end(&mut output);
            if output.len() as u64 > max_output {
                over.store(true, Ordering::SeqCst);
            }
            read.map(|_| output)
        });

        let mut stderr = child.stderr.take().expect("stderr is piped");
        let max_stderr = self.limits.max_stderr_bytes as u64;
        let errors = thread::spawn(move || {
            let mut text = Vec::new();
            let _ = (&mut stderr).take(max_stderr).read_to_end(&mut text);
            // Drain the rest, so the child never blocks on a full pipe
            let _ = std::io::copy(&mut stderr, &mut std::io::sink());
            text
        });

        let deadline = Instant::now() + self.limits.wall_time;
        let status = loop {
            if cancel.load(Ordering::SeqCst) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(failed("the compile was cancelled".to_owned()));
            }
            if over_output.load(Ordering::SeqCst) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(failed(format!(
                    "the compiled form is larger than the limit of {} bytes",
                    self.limits.max_compiled_bytes
                )));
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(RuntimeError::new(
                        ErrorKind::CompileTimeLimit,
                        format!(
                            "compiling took longer than the limit of {} s",
                            self.limits.wall_time.as_secs_f64()
                        ),
                    ));
                }
                Ok(None) => thread::sleep(POLL),
                Err(error) => return Err(failed(format!("the compile process: {error}"))),
            }
        };
        let _ = writer.join();
        let output = reader
            .join()
            .map_err(|_| failed("reading the compiled form failed".to_owned()))?
            .map_err(|error| failed(format!("reading the compiled form: {error}")))?;
        let stderr = errors.join().unwrap_or_default();
        if over_output.load(Ordering::SeqCst) {
            return Err(failed(format!(
                "the compiled form is larger than the limit of {} bytes",
                self.limits.max_compiled_bytes
            )));
        }

        if output.len() >= 8 {
            let fingerprint = u64::from_le_bytes(output[..8].try_into().expect("8 bytes"));
            if fingerprint != self.expected {
                return Err(failed(
                    "the compile process runs another version of the app than this one: the app was updated while it ran; restart it"
                        .to_owned(),
                ));
            }
        }
        if !status.success() {
            let said = String::from_utf8_lossy(&stderr);
            if self.ran_out_of_time(&status) {
                return Err(RuntimeError::new(
                    ErrorKind::CompileTimeLimit,
                    format!(
                        "compiling used more than the CPU time limit of {} s",
                        self.limits.cpu_time.as_secs()
                    ),
                ));
            }
            if ran_out_of_memory(&said) {
                return Err(RuntimeError::new(
                    ErrorKind::CompileMemoryLimit,
                    format!(
                        "compiling needed more than the memory limit of {} bytes",
                        self.limits.memory_bytes
                    ),
                ));
            }
            let reason = text::untrusted(&said, 1024);
            return Err(failed(if reason.trim().is_empty() {
                format!("the compile process stopped ({status})")
            } else {
                reason.trim().to_owned()
            }));
        }
        if output.len() <= 8 {
            return Err(failed(
                "the compile process wrote no compiled form".to_owned(),
            ));
        }
        Ok(output[8..].to_vec())
    }
}

impl Compiler {
    /// Whether the child ended on its CPU time limit: `SIGXCPU` on Unix, which the kernel sends
    /// at the soft limit (`SIGKILL` follows a second later, at the hard one).
    fn ran_out_of_time(&self, status: &std::process::ExitStatus) -> bool {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;
            matches!(status.signal(), Some(libc::SIGXCPU))
        }
        #[cfg(not(unix))]
        {
            let _ = status;
            false
        }
    }
}

/// Whether the child said an allocation failed, which its memory limit causes: the standard
/// library's message, or a thread it could not start.
fn ran_out_of_memory(said: &str) -> bool {
    said.contains("memory allocation of")
        || said.contains("Cannot allocate memory")
        || said.contains("failed to spawn thread")
        || said.contains("could not allocate")
}

/// The fingerprint of an engine: what Wasmtime compares before it accepts a compiled form. Two
/// processes of one executable always agree; a child started from a replaced file does not.
fn fingerprint(engine: &Engine) -> u64 {
    let mut hasher = DefaultHasher::new();
    engine.precompile_compatibility_hash().hash(&mut hasher);
    hasher.finish()
}

/// This process's own fingerprint, for the engine the host loads compiled forms with.
fn own_fingerprint() -> u64 {
    Engine::new(&engine_config())
        .map(|engine| fingerprint(&engine))
        .unwrap_or_default()
}
