//! Helpers shared by the test files: where the repository is, the committed DESIGN.md and the
//! vendored Radix palette.

#![allow(dead_code)]

use design_gen::radix::Palette;
use std::path::PathBuf;

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("the repository")
}

pub fn design_md() -> String {
    std::fs::read_to_string(root().join("DESIGN.md")).expect("DESIGN.md at the repository root")
}

pub fn palette() -> Palette {
    design_gen::radix::read_vendored(&root().join("design")).expect("the vendored Radix files")
}

/// `text` with `from` replaced once by `to`, failing when `from` is not in it, so a test never
/// passes because DESIGN.md moved on.
pub fn mutate(text: &str, from: &str, to: &str) -> String {
    assert!(
        text.contains(from),
        "DESIGN.md no longer contains {from:?}: update this test"
    );
    text.replacen(from, to, 1)
}

/// The problems the checker reports for `text`, as lines, failing when it accepts it.
pub fn refusals(text: &str) -> Vec<String> {
    match design_gen::check_text(text, &palette()) {
        Ok(_) => panic!("the checker accepted a DESIGN.md it must refuse"),
        Err(problems) => problems.iter().map(ToString::to_string).collect(),
    }
}

/// Asserts one refusal names `needle` and a line of DESIGN.md.
pub fn refused_naming(text: &str, needle: &str) {
    let problems = refusals(text);
    assert!(
        problems
            .iter()
            .any(|problem| problem.contains(needle) && problem.starts_with("DESIGN.md:")),
        "no refusal names {needle:?}:\n{}",
        problems.join("\n")
    );
}
