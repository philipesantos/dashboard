//! The compile process (`specs/010-plugin-installation/research.md`, R2, R3; tasks T007, T037):
//! a package compiled by a child process, the CLI binary started with `--compile-package`, loads
//! through `Runtime::load_compiled` and renders; the child refuses what is not a binary component;
//! a child of another engine is never trusted; and a compiled form or a package that changed
//! after it was recorded never runs.

use std::sync::Arc;

use runtime::compile::{CompileLimits, Compiler};
use runtime::grants::GrantStore;
use runtime::manifest::Manifest;
use runtime::{ErrorKind, Limits, Runtime, RuntimeOptions, package};
use tempfile::TempDir;

use crate::common::fixture;

/// The CLI binary, which enters the compile process on `--compile-package` as the app does.
fn compiler() -> Compiler {
    Compiler::with_executable(env!("CARGO_BIN_EXE_runtime"), CompileLimits::default())
}

fn runtime() -> (TempDir, Runtime) {
    let data = TempDir::new().expect("a temporary directory");
    let runtime = Runtime::new(Limits::default(), RuntimeOptions::new(data.path()))
        .expect("the runtime starts");
    (data, runtime)
}

/// `hello-widget` with its required `log` approved from `origin`.
fn approved(package: &[u8], origin: &str) -> Arc<GrantStore> {
    let grants = GrantStore::in_memory();
    let manifest = Manifest::parse(package::manifest_section(package).expect("a manifest"))
        .expect("a valid manifest");
    grants
        .approve(&manifest, origin, &package::digest(package))
        .expect("approved");
    Arc::new(grants)
}

const ORIGIN: &str = "github:1";

#[test]
fn a_form_the_child_compiled_loads_and_renders() {
    let package = fixture("hello-widget");
    let compiled = compiler().compile(&package).expect("the child compiles it");
    let (_data, runtime) = runtime();
    let plugin = runtime
        .load_compiled(
            &package,
            &package::digest(&package),
            &compiled,
            &package::digest(&compiled),
            ORIGIN,
            approved(&package, ORIGIN),
        )
        .expect("the compiled form loads");
    let content = plugin.render("{}").result.expect("it renders");
    assert!(content.to_string().contains("Hello, world!"), "{content}");
}

#[test]
fn the_child_refuses_what_is_not_a_binary_component() {
    // The runtime's test builds enable Wasmtime's text format; the child still refuses it
    for bytes in [&b"(component)"[..], b"not wasm at all", b"\0asm\x01\0\0\0"] {
        let error = compiler().compile(bytes).expect_err("refused");
        assert_eq!(error.kind(), ErrorKind::CompileFailed, "{error}");
    }
}

#[test]
fn a_child_of_another_engine_is_never_trusted_or_retried() {
    let package = fixture("hello-widget");
    let error = compiler()
        .with_expected_fingerprint(0)
        .compile(&package)
        .expect_err("another engine's form is refused");
    assert_eq!(error.kind(), ErrorKind::CompileFailed);
    assert!(error.to_string().contains("restart"), "{error}");
}

#[test]
fn a_package_over_the_cap_is_refused_before_any_child_starts() {
    let package = vec![0u8; Limits::default().max_package_bytes + 1];
    let error = compiler().compile(&package).expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::InvalidPackage);
}

/// `compiled-tampered`: one flipped byte, and the recorded digest no longer matches: the form is
/// refused before `deserialize` sees it, as `compile-failed`, which tells the installer to
/// compile again (once).
#[test]
fn a_tampered_compiled_form_is_never_deserialized() {
    let package = fixture("hello-widget");
    let compiled = compiler().compile(&package).expect("compiled");
    let digest = package::digest(&compiled);
    let mut tampered = compiled.clone();
    let middle = tampered.len() / 2;
    tampered[middle] ^= 0x01;
    let (_data, runtime) = runtime();
    let error = runtime
        .load_compiled(
            &package,
            &package::digest(&package),
            &tampered,
            &digest,
            ORIGIN,
            approved(&package, ORIGIN),
        )
        .err()
        .expect("refused");
    assert_eq!(error.kind(), ErrorKind::CompileFailed);
}

/// A compiled form the record names but that is not one Wasmtime accepts (garbage with a
/// matching digest): refused as `compile-failed`, never run.
#[test]
fn a_form_wasmtime_refuses_is_compile_failed() {
    let package = fixture("hello-widget");
    let garbage = b"not a compiled form".to_vec();
    let (_data, runtime) = runtime();
    let error = runtime
        .load_compiled(
            &package,
            &package::digest(&package),
            &garbage,
            &package::digest(&garbage),
            ORIGIN,
            approved(&package, ORIGIN),
        )
        .err()
        .expect("refused");
    assert_eq!(error.kind(), ErrorKind::CompileFailed);
}

/// `package-tampered`: the package no longer matches its recorded digest: `package-changed`,
/// and nothing is compiled again from it.
#[test]
fn a_tampered_package_is_package_changed() {
    let package = fixture("hello-widget");
    let compiled = compiler().compile(&package).expect("compiled");
    let recorded = package::digest(&package);
    let mut tampered = package.clone();
    let last = tampered.len() - 1;
    tampered[last] ^= 0x01;
    let (_data, runtime) = runtime();
    let error = runtime
        .load_compiled(
            &tampered,
            &recorded,
            &compiled,
            &package::digest(&compiled),
            ORIGIN,
            approved(&package, ORIGIN),
        )
        .err()
        .expect("refused");
    assert_eq!(error.kind(), ErrorKind::PackageChanged);
}

// US4: the compile process's limits and confinement (task T037)

fn limited(limits: CompileLimits) -> Compiler {
    Compiler::with_executable(env!("CARGO_BIN_EXE_runtime"), limits)
}

/// One thread, so the CPU time is the compile's own and the limits are reached in a known order.
fn one_thread() -> CompileLimits {
    CompileLimits {
        threads: 1,
        ..CompileLimits::default()
    }
}

/// `compile-slow`: the parent's wall-clock kill.
#[test]
fn a_compile_past_its_wall_time_is_refused_naming_it() {
    let error = limited(CompileLimits {
        wall_time: std::time::Duration::from_millis(200),
        ..one_thread()
    })
    .compile(&fixture("compile-heavy"))
    .expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::CompileTimeLimit, "{error}");
}

/// `compile-cpu`: the system's CPU time limit (`RLIMIT_CPU`, in whole seconds; the job's on
/// Windows, which counts user time only and where `compile-heavy` takes less than a second of it).
#[test]
fn a_compile_past_its_cpu_time_is_refused_naming_it() {
    let cpu_time = if cfg!(windows) {
        std::time::Duration::from_millis(250)
    } else {
        std::time::Duration::from_secs(1)
    };
    let error = limited(CompileLimits {
        cpu_time,
        ..one_thread()
    })
    .compile(&fixture("compile-heavy"))
    .expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::CompileTimeLimit, "{error}");
    assert!(error.to_string().contains("CPU"), "{error}");
}

/// `compile-memory`: the system's memory limit (`RLIMIT_DATA` on Linux; the job's on Windows; the
/// child's own watchdog on macOS). The watchdog reads the peak every 50 ms, so on macOS the limit
/// is one `compile-heavy` passes mid-compile, not near its end (its peak is about 104 MiB); on
/// Windows the job counts committed memory, which stays under 96 MiB.
#[test]
fn a_compile_past_its_memory_is_refused_naming_it() {
    let mebibytes = if cfg!(target_os = "linux") { 96 } else { 48 };
    let error = limited(CompileLimits {
        memory_bytes: mebibytes * 1024 * 1024,
        ..one_thread()
    })
    .compile(&fixture("compile-heavy"))
    .expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::CompileMemoryLimit, "{error}");
}

/// `compile-output`: a compiled form over its cap is not read on.
#[test]
fn a_compiled_form_over_its_cap_is_refused() {
    let error = limited(CompileLimits {
        max_compiled_bytes: 1024,
        ..CompileLimits::default()
    })
    .compile(&fixture("hello-widget"))
    .expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::CompileFailed, "{error}");
    assert!(
        error.to_string().contains("larger than the limit"),
        "{error}"
    );
}

/// `compile-confined` (FR-029): once confined, the child can open no file of the user's, from its
/// main thread or from a compile thread that existed before, and reach no port on loopback, on
/// each system (Landlock and seccomp; the macOS sandbox; an AppContainer). Each attempt is a run of
/// its own, which ends as soon as it worked, so no later refusal can hide it.
#[test]
#[allow(
    clippy::disallowed_types,
    clippy::disallowed_methods,
    reason = "the port the confined child must not reach"
)]
fn the_compile_process_can_open_no_file_and_reach_no_network() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a loopback port");
    let port = listener.local_addr().expect("its address").port();
    for attempt in [
        "file".to_owned(),
        "file-on-a-compile-thread".to_owned(),
        format!("net:{port}"),
    ] {
        let outcome = compiler()
            .trying_to_escape(&attempt)
            .compile(&fixture("hello-widget"));
        if let Err(error) = &outcome {
            assert!(!error.to_string().contains("escaped"), "{attempt}: {error}");
        }
        // Confined, the attempt fails and the compile goes on, or ends with the process
        assert!(
            outcome
                .as_ref()
                .map_or(true, |compiled| !compiled.is_empty()),
            "{attempt}"
        );
    }
    // Confined as it is, a compile still works
    assert!(compiler().compile(&fixture("hello-widget")).is_ok());
}

/// T042: `compile-heavy`, a package just under the cap, through the real compile process under
/// the default limits. CI runs it in a step of its own, after the tests, and prints its time and
/// the peak memory of this process's children (the compile process alone there), so research R2
/// can record them per system and a limit can be moved before a legitimate package nears it.
#[test]
#[ignore = "a measurement CI runs on its own: cargo test ... -- --ignored compile::measure_"]
fn measure_compile_heavy_under_the_default_limits() {
    let limits = CompileLimits::default();
    let started = std::time::Instant::now();
    let compiled = limited(limits.clone())
        .compile(&fixture("compile-heavy"))
        .expect("a legitimate package compiles under the default limits");
    let elapsed = started.elapsed();
    #[cfg(unix)]
    let peak = {
        // SAFETY: getrusage fills the zeroed struct it is given
        let usage = unsafe {
            let mut usage: libc::rusage = std::mem::zeroed();
            libc::getrusage(libc::RUSAGE_CHILDREN, &mut usage);
            usage
        };
        // Kilobytes on Linux, bytes on macOS
        let bytes = if cfg!(target_os = "macos") {
            usage.ru_maxrss as u64
        } else {
            usage.ru_maxrss as u64 * 1024
        };
        format!("{} MiB resident at most", bytes / (1024 * 1024))
    };
    #[cfg(not(unix))]
    let peak = "not measured here (the job's accounting is not read)".to_owned();
    println!(
        "compile-heavy: {} ms with {} threads, {peak}; the form is {} MiB (limits: {} s wall, {} s CPU, {} MiB)",
        elapsed.as_millis(),
        limits.threads,
        compiled.len() / (1024 * 1024),
        limits.wall_time.as_secs(),
        limits.cpu_time.as_secs(),
        limits.memory_bytes / (1024 * 1024)
    );
    // Half of a limit is the line the research sets for moving it
    assert!(
        elapsed < limits.wall_time / 2,
        "{elapsed:?} is past half the wall-clock limit"
    );
}
