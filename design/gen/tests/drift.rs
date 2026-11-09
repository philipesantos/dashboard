//! The committed generated files are what DESIGN.md produces (FR-004, T013).

use crate::common::{design_md, mutate, palette, root};

#[test]
fn the_committed_files_are_what_design_md_produces() {
    if let Err(differences) = design_gen::generate::compare(&root()) {
        panic!(
            "generated files are stale; run `cargo xtask design`:\n{}",
            differences.join("\n")
        );
    }
}

#[test]
fn a_changed_value_is_named_with_its_file() {
    let changed = mutate(&design_md(), "fontSize: 14px", "fontSize: 15px");
    let design = design_gen::check_text(&changed, &palette()).expect("still valid");
    let generated = design_gen::generate::generate(&design, &palette());
    let differences = design_gen::generate::differences(&root(), &generated);
    assert!(
        differences.iter().any(|line| line.contains("design/tokens/src/generated.rs") && line.contains("body-md")),
        "{differences:?}"
    );
}

#[test]
fn generation_is_deterministic_and_lf() {
    let design = design_gen::check_text(&design_md(), &palette()).expect("valid");
    let first = design_gen::generate::generate(&design, &palette());
    let second = design_gen::generate::generate(&design, &palette());
    assert_eq!(first, second);
    for (path, text) in &first.files {
        assert!(!text.contains('\r'), "{path} has a carriage return");
        assert!(text.ends_with('\n'), "{path} does not end with a newline");
    }
}
