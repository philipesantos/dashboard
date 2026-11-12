//! The runtime's integration tests, as one executable (`autotests = false` in `Cargo.toml`).
//! Each file in this directory is a module here, under its own name, so a test is selected as
//! `cargo test -p runtime --test it <file>::` and runs by name as `<file>::<test>`. An executable
//! links the whole runtime, about 100 MB in a debug build: one per file filled the disk.

mod common;

mod accounts;
mod actions;
mod cli;
mod compile;
mod contract;
mod contract_v1;
mod dialog_model;
mod docs_reference;
mod docs_walk;
mod flows;
mod hostile;
mod install_github;
mod install_records;
mod manifest_schema;
mod network;
mod permissions;
mod storage;
mod updates;
mod vault;

/// Cargo builds only this file, so a test file added beside it and not listed above would never
/// run: this test fails instead of letting it be skipped.
#[test]
fn every_test_file_is_a_module() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let listed = include_str!("it.rs");
    let mut missing = Vec::new();
    for entry in std::fs::read_dir(&dir).expect("the tests directory") {
        let path = entry.expect("an entry of the tests directory").path();
        if path.extension().is_none_or(|extension| extension != "rs") {
            continue;
        }
        let name = path.file_stem().expect("a file name").to_string_lossy();
        // By line, since a checkout on Windows may end them with \r\n
        let module = format!("mod {name};");
        if name != "it" && !listed.lines().any(|line| line == module) {
            missing.push(path.display().to_string());
        }
    }
    assert!(
        missing.is_empty(),
        "add `mod <name>;` to tests/it.rs for: {}",
        missing.join(", ")
    );
}
