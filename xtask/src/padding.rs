//! Generated WebAssembly code that is costly to compile: the `compile-heavy` install fixture
//! (`specs/010-plugin-installation/research.md`, R11) and the `compile_cost` measurement
//! (`bench/src/bin/compile_cost.rs`, which includes this file) use the same shapes, so the fixture
//! costs what the measurement measured.
//!
//! Each function returns the text of one core module, `(module …)`, which a caller wraps in a
//! component or adds to one.

use std::fmt::Write;

/// About 10 bytes of code: `local.get a`, `local.get b`, `i32.add`, `local.set c`, with local
/// indices below 16,384.
fn add_op(wat: &mut String, i: usize, locals: usize) {
    let (a, b, c) = ((i * 7 + 1) % locals, (i * 13 + 5) % locals, i % locals);
    writeln!(wat, "local.get {a} local.get {b} i32.add local.set {c}")
        .expect("writing to a String does not fail");
}

/// Many functions of about 1.2 KiB each, like a large ordinary program, with about
/// `code_bytes` of code in all.
pub fn many(code_bytes: usize) -> String {
    const OPS: usize = 120;
    const LOCALS: usize = 16;
    // Indices below 128 take 1 byte, so each operation here is 7 bytes
    let functions = code_bytes / (OPS * 7);
    let mut wat = String::from("(module\n");
    for _ in 0..functions {
        wat.push_str("(func (param i32) (result i32)");
        wat.push_str(&" (local i32)".repeat(LOCALS));
        wat.push('\n');
        for i in 0..OPS {
            add_op(&mut wat, i, LOCALS + 1);
        }
        wat.push_str("local.get 0)\n");
    }
    wat.push(')');
    wat
}

/// One function, as large as validation allows, with thousands of locals in use at once, which
/// strains register allocation.
#[allow(dead_code, reason = "used by the compile_cost measurement, not by the fixture")]
pub fn huge(code_bytes: usize) -> String {
    const LOCALS: usize = 4_000;
    // wasmparser refuses function bodies over 7,654,321 bytes
    let ops = code_bytes.min(7_500_000) / 10;
    let mut wat = String::from("(module\n(func (param i32) (result i32)");
    wat.push_str(&" (local i32)".repeat(LOCALS));
    wat.push('\n');
    for i in 0..ops {
        add_op(&mut wat, i, LOCALS + 1);
    }
    wat.push_str("local.get 0))");
    wat
}
