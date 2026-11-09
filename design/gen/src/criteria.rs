//! The frame and dialog criteria of `docs/design/host-surfaces.md`, and their coverage of the
//! threat model (specs/007-host-visual-design/contracts/host-surfaces-criteria.md, research R10).
//!
//! A criterion is a `### HS-NN Title` heading followed by four fields: `- **Observe**:`,
//! `- **Expect**:`, `- **Answers**:` (the constitution rules, then one threat row per indented
//! `  - ` line, quoted exactly) and `- **Tested by**:`. Under `## Decided elsewhere`, each `- `
//! line is a threat row's exact title, and the indented lines below it say who owns it.

use crate::model::Criterion;

/// The later Phase 3 features a criterion can be tested by.
pub const TESTED_BY: &[&str] = &[
    "host window",
    "widget kit",
    "host dialogs",
    "declared actions",
    "installation",
    "dashboard grid",
];

/// The criteria file, read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Criteria {
    pub criteria: Vec<Criterion>,
    /// Each criterion's quoted threat rows, by id.
    pub rows: Vec<(String, Vec<String>)>,
    /// The rows owned by other features.
    pub elsewhere: Vec<String>,
}

/// A threat-model row: its title, boundaries and the phase its test starts in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreatRow {
    pub title: String,
    pub boundary: String,
    pub test_from: String,
}

/// The rows of the threat model's table of threats (six cells each).
pub fn threat_rows(threat_model: &str) -> Vec<ThreatRow> {
    threat_model
        .lines()
        .filter(|line| line.starts_with("| ") && !line.starts_with("| ---"))
        .filter_map(|line| {
            let cells: Vec<&str> = line
                .trim()
                .trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect();
            (cells.len() == 6 && cells[0] != "Threat").then(|| ThreatRow {
                title: cells[0].to_owned(),
                boundary: cells[3].to_owned(),
                test_from: cells[5].to_owned(),
            })
        })
        .collect()
}

fn field<'a>(line: &'a str, name: &str) -> Option<&'a str> {
    line.strip_prefix(&format!("- **{name}**:")).map(str::trim)
}

/// Reads the criteria file. Problems are returned as messages with their line.
pub fn parse(text: &str) -> Result<Criteria, Vec<String>> {
    let mut problems = Vec::new();
    let mut criteria: Vec<Criterion> = Vec::new();
    let mut rows: Vec<(String, Vec<String>)> = Vec::new();
    let mut elsewhere = Vec::new();
    let mut in_elsewhere = false;
    // The field a continuation line belongs to
    let mut current: Option<&'static str> = None;
    for (index, raw) in text.lines().enumerate() {
        let number = index + 1;
        let line = raw.trim_end_matches('\r');
        if let Some(title) = line.strip_prefix("## ") {
            in_elsewhere = title.trim() == "Decided elsewhere";
            current = None;
            continue;
        }
        if in_elsewhere {
            if let Some(row) = line.strip_prefix("- ") {
                elsewhere.push(row.trim().to_owned());
            }
            continue;
        }
        if let Some(heading) = line.strip_prefix("### ") {
            let (id, title) = heading.split_once(' ').unwrap_or((heading, ""));
            let well_formed = id.len() == 5
                && id.starts_with("HS-")
                && id[3..].bytes().all(|b| b.is_ascii_digit());
            if !well_formed {
                problems.push(format!(
                    "host-surfaces.md:{number}: {id:?} is not an HS-NN id"
                ));
            }
            if criteria.iter().any(|c| c.id == id) {
                problems.push(format!("host-surfaces.md:{number}: {id} appears twice"));
            }
            criteria.push(Criterion {
                id: id.to_owned(),
                title: title.trim().to_owned(),
                observe: String::new(),
                expect: String::new(),
                answers: String::new(),
                tested_by: String::new(),
                line: number,
            });
            rows.push((id.to_owned(), Vec::new()));
            current = None;
            continue;
        }
        let Some(criterion) = criteria.last_mut() else {
            continue;
        };
        let fields: [(&'static str, &mut String); 4] = [
            ("Observe", &mut criterion.observe),
            ("Expect", &mut criterion.expect),
            ("Answers", &mut criterion.answers),
            ("Tested by", &mut criterion.tested_by),
        ];
        let mut matched = false;
        for (name, slot) in fields {
            if let Some(value) = field(line, name) {
                *slot = value.to_owned();
                current = Some(name);
                matched = true;
            }
        }
        if matched {
            continue;
        }
        if current == Some("Answers")
            && let Some(row) = line.strip_prefix("  - ")
        {
            rows.last_mut()
                .expect("pushed with the criterion")
                .1
                .push(row.trim().to_owned());
            continue;
        }
        if let (Some(name), true) = (current, line.starts_with("  ") && !line.trim().is_empty()) {
            let slot = match name {
                "Observe" => &mut criterion.observe,
                "Expect" => &mut criterion.expect,
                "Answers" => &mut criterion.answers,
                _ => &mut criterion.tested_by,
            };
            slot.push(' ');
            slot.push_str(line.trim());
        }
    }
    for (criterion, (_, quoted)) in criteria.iter().zip(&rows) {
        for (name, value) in [
            ("Observe", &criterion.observe),
            ("Expect", &criterion.expect),
            ("Answers", &criterion.answers),
            ("Tested by", &criterion.tested_by),
        ] {
            if value.is_empty() {
                problems.push(format!(
                    "host-surfaces.md:{}: {} has no {name}",
                    criterion.line, criterion.id
                ));
            }
        }
        if quoted.is_empty() {
            problems.push(format!(
                "host-surfaces.md:{}: {} answers no threat row",
                criterion.line, criterion.id
            ));
        }
        if !criterion.tested_by.is_empty()
            && !TESTED_BY.contains(&criterion.tested_by.trim_end_matches('.'))
        {
            problems.push(format!(
                "host-surfaces.md:{}: {} is tested by {:?}, which is not one of {TESTED_BY:?}",
                criterion.line, criterion.id, criterion.tested_by
            ));
        }
    }
    if problems.is_empty() {
        Ok(Criteria {
            criteria,
            rows,
            elsewhere,
        })
    } else {
        Err(problems)
    }
}

/// Checks the criteria against the threat model: every quoted row exists, and every Phase 3 row
/// on boundary B4 is answered or decided elsewhere (SC-005).
pub fn coverage(criteria: &Criteria, threat_model: &str) -> Vec<String> {
    let rows = threat_rows(threat_model);
    let titles: Vec<&str> = rows.iter().map(|row| row.title.as_str()).collect();
    let mut problems = Vec::new();
    for (id, quoted) in &criteria.rows {
        for row in quoted {
            if !titles.contains(&row.as_str()) {
                problems.push(format!(
                    "{id} quotes a row the threat model does not have: {row:?}"
                ));
            }
        }
    }
    for row in &criteria.elsewhere {
        if !titles.contains(&row.as_str()) {
            problems.push(format!(
                "Decided elsewhere quotes a row the threat model does not have: {row:?}"
            ));
        }
    }
    for row in rows
        .iter()
        .filter(|row| row.boundary.contains("B4") && row.test_from.contains("Phase 3"))
    {
        let answered = criteria
            .rows
            .iter()
            .any(|(_, quoted)| quoted.contains(&row.title));
        if !answered && !criteria.elsewhere.contains(&row.title) {
            problems.push(format!(
                "no criterion answers, and nothing decides elsewhere: {:?}",
                row.title
            ));
        }
    }
    problems
}
