//! How long loading a large package takes. Compiling happens at load, outside every per-call
//! limit and bounded only by the package cap (`docs/threat-model.md`), so this measures what a
//! package near that cap costs.
//!
//! Run with `cargo run -p runtime --release --example compile_cost -- <shape> [<MiB of code>]`,
//! where the shape is `many` (many functions of about 1.2 KiB, like a large ordinary program) or
//! `huge` (one function, as large as validation allows, with thousands of locals in use at
//! once, which strains register allocation). It prints the package size, the load time and the
//! outcome; the package has no `renderer` export, so a load that compiles ends in
//! `contract-mismatch`, after the compilation this measures.

use std::fmt::Write;
use std::time::Instant;

use runtime::{Limits, Runtime};

const MIB: usize = 1024 * 1024;

/// About 10 bytes of code: `local.get a`, `local.get b`, `i32.add`, `local.set c`, with local
/// indices below 16,384.
fn add_op(wat: &mut String, i: usize, locals: usize) {
    let (a, b, c) = ((i * 7 + 1) % locals, (i * 13 + 5) % locals, i % locals);
    writeln!(wat, "local.get {a} local.get {b} i32.add local.set {c}").unwrap();
}

fn many(code_bytes: usize) -> String {
    const OPS: usize = 120;
    const LOCALS: usize = 16;
    // Indices below 128 take 1 byte, so each operation here is 7 bytes
    let functions = code_bytes / (OPS * 7);
    let mut wat = String::from("(component (core module\n");
    for _ in 0..functions {
        wat.push_str("(func (param i32) (result i32)");
        wat.push_str(&" (local i32)".repeat(LOCALS));
        wat.push('\n');
        for i in 0..OPS {
            add_op(&mut wat, i, LOCALS + 1);
        }
        wat.push_str("local.get 0)\n");
    }
    wat.push_str("))");
    wat
}

fn huge(code_bytes: usize) -> String {
    const LOCALS: usize = 4_000;
    // wasmparser refuses function bodies over 7,654,321 bytes
    let ops = code_bytes.min(7_500_000) / 10;
    let mut wat = String::from("(component (core module\n(func (param i32) (result i32)");
    wat.push_str(&" (local i32)".repeat(LOCALS));
    wat.push('\n');
    for i in 0..ops {
        add_op(&mut wat, i, LOCALS + 1);
    }
    wat.push_str("local.get 0)))");
    wat
}

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
        Some("many") => many(code_bytes),
        Some("huge") => huge(code_bytes),
        _ => panic!("usage: compile_cost many|huge [<MiB of code>]"),
    };
    let package = wat::parse_str(&wat).expect("the generated text is valid");
    drop(wat);

    let runtime = Runtime::new(Limits::default()).expect("the runtime starts");
    reset_peak_memory();
    let start = Instant::now();
    let outcome = match runtime.load(&package) {
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
