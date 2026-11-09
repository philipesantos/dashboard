//! `design-gen`'s integration tests, as one executable (`autotests = false` in `Cargo.toml`), as
//! the runtime's are. Each file in this directory is a module here, under its own name, so a test
//! is selected as `cargo test -p design-gen --test it <file>::`.

mod common;

mod checker;
mod drift;
mod dtcg;
mod host_surfaces;
mod hue_rule;
mod lint;
mod math;
mod pins;
mod preview;
mod radix_reader;

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
