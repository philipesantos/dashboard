//! How long loading a large package takes. Compiling happens at load, outside every per-call
//! limit and bounded only by the package cap (`docs/threat-model.md`), so this measures what a
//! package near that cap costs.
//!
//! Run with `cargo run -p dashboard-bench --release --bin compile_cost -- <shape> [<MiB of code>]`,
//! where the shape is `many` (many functions of about 1.2 KiB, like a large ordinary program) or
//! `huge` (one function, as large as validation allows, with thousands of locals in use at
//! once, which strains register allocation). It prints the package size, the load time and the
//! outcome; the package has no `renderer` export, so a load that compiles ends in
//! `contract-mismatch`, after the compilation this measures. The package carries a manifest
//! and is approved, so nothing before the compile refuses it.

use std::sync::Arc;
use std::time::Instant;

use runtime::grants::GrantStore;
use runtime::manifest::Manifest;
use runtime::package;
use runtime::{Limits, Runtime, RuntimeOptions};

const MANIFEST: &str = r#"{"manifest_version":1,"id":"local/compile-cost","version":"0.1.0","author":"Dashboard contributors","permissions":[],"login_providers":[]}"#;
const ORIGIN: &str = "compile-cost";

const MIB: usize = 1024 * 1024;

// The generator the `compile-heavy` install fixture uses too, so both cost the same
#[path = "../../../xtask/src/padding.rs"]
mod padding;

/// Peak memory of this process since [`reset_peak_memory`], where the system reports it.
fn peak_memory() -> String {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|status| {
            status
                .lines()
                .find(|line| line.starts_with("VmHWM:"))
                .map(|line| line["VmHWM:".len()..].trim().to_owned())
        })
        .unwrap_or_else(|| "not reported on this system".to_owned())
}

/// Starts the peak over, so it counts the load alone, not building the package (Linux only).
fn reset_peak_memory() {
    let _ = std::fs::write("/proc/self/clear_refs", "5");
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mib: f64 = args
        .get(1)
        .map_or(9.5, |value| value.parse().expect("MiB is a number"));
    let code_bytes = (mib * MIB as f64) as usize;
    let wat = match args.first().map(String::as_str) {
        Some("many") => format!("(component {})", padding::many(code_bytes)),
        Some("huge") => format!("(component {})", padding::huge(code_bytes)),
        _ => panic!("usage: compile_cost many|huge [<MiB of code>]"),
    };
    let component = wat::parse_str(&wat).expect("the generated text is valid");
    drop(wat);
    let package = package::pack(&component, MANIFEST.as_bytes()).expect("the manifest is valid");
    drop(component);
    let grants = GrantStore::in_memory();
    let manifest = Manifest::parse(MANIFEST.as_bytes()).expect("the manifest is valid");
    grants
        .approve(&manifest, ORIGIN, &package::digest(&package))
        .expect("approved");

    // Loading these packages writes nothing: they declare no connector
    let options = RuntimeOptions::new(std::env::temp_dir().join("dashboard-compile-cost"));
    let runtime = Runtime::new(Limits::default(), options).expect("the runtime starts");
    reset_peak_memory();
    let start = Instant::now();
    let outcome = match runtime.load(&package, ORIGIN, Arc::new(grants)) {
        Ok(_) => "loaded".to_owned(),
        Err(error) => error.kind().as_str().to_owned(),
    };
    let elapsed = start.elapsed();

    println!("shape: {}", args[0]);
    println!("package_mib: {:.2}", package.len() as f64 / MIB as f64);
    println!("load_ms: {:.0}", elapsed.as_secs_f64() * 1000.0);
    println!("outcome: {outcome}");
    println!("peak_memory_during_load: {}", peak_memory());
}
