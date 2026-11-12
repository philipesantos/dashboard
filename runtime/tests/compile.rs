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
    let runtime =
        Runtime::new(Limits::default(), RuntimeOptions::new(data.path())).expect("the runtime starts");
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
