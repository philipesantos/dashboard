//! The frame and dialog criteria cover the threat model (SC-005, T031, contracts/
//! host-surfaces-criteria.md): every criterion has its four fields and a known tester, every quoted
//! row exists, and every Phase 3 row on boundary B4 is answered or decided elsewhere.

use crate::common::root;
use design_gen::criteria::{coverage, parse};

fn read(path: &str) -> String {
    std::fs::read_to_string(root().join(path)).expect(path)
}

#[test]
fn the_criteria_cover_every_phase_3_row_on_b4() {
    let criteria = parse(&read("docs/design/host-surfaces.md"))
        .unwrap_or_else(|problems| panic!("{}", problems.join("\n")));
    let problems = coverage(&criteria, &read("docs/threat-model.md"));
    assert!(problems.is_empty(), "{}", problems.join("\n"));
    assert!(
        criteria.criteria.len() >= 15,
        "only {} criteria",
        criteria.criteria.len()
    );
}

const ROW: &str = "Password field or drawing over the frame (fake login)";

fn file(answers: &str, tested_by: &str) -> String {
    format!(
        "# Host surfaces\n\n## Chrome\n### HS-01 The chrome stays uncovered\n- **Observe**: bounds.\n- **Expect**: no overlap.\n- **Answers**: V\n{answers}- **Tested by**: {tested_by}\n"
    )
}

const MODEL: &str = "| Threat | Attacker | Asset | Boundary | Mitigation | Test from |\n| --- | --- | --- | --- | --- | --- |\n| Password field or drawing over the frame (fake login) | T1 | A2 | B4 | V | Phase 3 |\n| Widget opens a window or dialog (phishing) | T1 | A2 | B4 | V | Phase 3 |\n";

#[test]
fn a_missing_field_fails() {
    let text =
        file(&format!("  - {ROW}\n"), "host window").replace("- **Expect**: no overlap.\n", "");
    let problems = parse(&text).expect_err("no Expect");
    assert!(
        problems.iter().any(|p| p.contains("HS-01 has no Expect")),
        "{problems:?}"
    );
}

#[test]
fn a_misquoted_row_fails() {
    let criteria =
        parse(&file("  - Password field (fake login)\n", "host window")).expect("parses");
    let problems = coverage(&criteria, MODEL);
    assert!(
        problems
            .iter()
            .any(|p| p.contains("Password field (fake login)")),
        "{problems:?}"
    );
}

#[test]
fn an_uncovered_row_fails_with_its_title() {
    let criteria = parse(&file(&format!("  - {ROW}\n"), "host window")).expect("parses");
    let problems = coverage(&criteria, MODEL);
    assert!(
        problems
            .iter()
            .any(|p| p.contains("Widget opens a window or dialog (phishing)")),
        "{problems:?}"
    );
    let covered = format!(
        "{}\n## Decided elsewhere\n- Widget opens a window or dialog (phishing)\n  Owner: widget kit.\n",
        file(&format!("  - {ROW}\n"), "host window")
    );
    assert!(coverage(&parse(&covered).expect("parses"), MODEL).is_empty());
}

#[test]
fn a_criterion_without_a_tester_fails() {
    let text =
        file(&format!("  - {ROW}\n"), "host window").replace("- **Tested by**: host window\n", "");
    let problems = parse(&text).expect_err("no Tested by line");
    assert!(
        problems
            .iter()
            .any(|p| p.contains("HS-01 has no Tested by")),
        "{problems:?}"
    );
}

#[test]
fn an_unknown_tester_fails() {
    let problems = parse(&file(&format!("  - {ROW}\n"), "someone")).expect_err("unknown tester");
    assert!(
        problems.iter().any(|p| p.contains("someone")),
        "{problems:?}"
    );
    let problems = parse(&file(&format!("  - {ROW}\n"), "")).expect_err("no tester");
    assert!(
        problems.iter().any(|p| p.contains("has no Tested by")),
        "{problems:?}"
    );
}
