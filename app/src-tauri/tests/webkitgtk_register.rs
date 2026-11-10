//! The WebKitGTK register (specs/008-tauri-shell/contracts/webkitgtk-register.md).
//!
//! Every change made only because Linux uses the system WebKitGTK carries a
//! `WEBKITGTK-ONLY(<id>)` marker and an entry in `docs/webkitgtk-register.toml`, so the move to
//! the embedded engine can remove each one. This test fails when the markers and the entries
//! disagree, and when a line in `app/` or `.github/` names the engine with no marker nearby.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;
use std::process::Command;

const REGISTER: &str = "docs/webkitgtk-register.toml";
const MARKER: &str = "WEBKITGTK-ONLY(";
/// This file holds the tripwire's patterns, so the tripwire skips it.
const THIS_TEST: &str = "app/src-tauri/tests/webkitgtk_register.rs";

/// The plain signs of a WebKitGTK-only change, matched ignoring case.
const TRIPWIRE: &[&str] = &[
    "webkit",
    "target_os = \"linux\"",
    "useragent",
    "navigator.platform",
];
/// Names of the register's own bookkeeping, which mention the engine without being a change for
/// it; removed from a line before the tripwire looks at it.
const BOOKKEEPING: &[&str] = &["webkitgtk-register", "webkitgtk_register", "webkitgtk-only"];
/// How many lines above a matching line a marker may sit.
const MARKER_REACH: usize = 3;

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The files git tracks, apart from specs/ and the register.
fn tracked() -> Vec<String> {
    let output = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(repo())
        .output()
        .expect("git ls-files");
    assert!(output.status.success(), "git ls-files failed");
    String::from_utf8(output.stdout)
        .expect("utf-8 paths")
        .split('\0')
        .filter(|path| !path.is_empty() && !path.starts_with("specs/") && *path != REGISTER)
        .map(str::to_string)
        .collect()
}

/// The ids of the markers on one line.
fn markers(line: &str) -> Vec<String> {
    line.match_indices(MARKER)
        .filter_map(|(start, _)| {
            let rest = &line[start + MARKER.len()..];
            let end = rest.find(')')?;
            let id = &rest[..end];
            let valid = !id.is_empty()
                && id
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
            valid.then(|| id.to_string())
        })
        .collect()
}

fn in_tripwire_scope(path: &str) -> bool {
    (path.starts_with("app/") || path.starts_with(".github/"))
        && !path.starts_with("app/src-tauri/gen/")
        && path != THIS_TEST
}

#[test]
fn the_register_matches_the_markers() {
    let mut failures = Vec::new();

    let register: toml::Table = fs::read_to_string(repo().join(REGISTER))
        .unwrap_or_else(|error| panic!("{REGISTER}: {error}"))
        .parse()
        .unwrap_or_else(|error| panic!("{REGISTER}: {error}"));
    let mut entries: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for entry in register
        .get("entry")
        .and_then(|e| e.as_array())
        .into_iter()
        .flatten()
    {
        let text = |key: &str| {
            entry
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .trim()
        };
        let id = text("id").to_string();
        if id.is_empty()
            || !id
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            failures.push(format!("{REGISTER}: entry \"{id}\" is not kebab-case"));
        }
        if text("reason").is_empty() {
            failures.push(format!("{REGISTER}: entry \"{id}\" has no reason"));
        }
        if text("removal").is_empty() {
            failures.push(format!("{REGISTER}: entry \"{id}\" has no removal"));
        }
        let files: Vec<String> = entry
            .get("files")
            .and_then(|f| f.as_array())
            .into_iter()
            .flatten()
            .filter_map(|f| f.as_str().map(str::to_string))
            .collect();
        if files.is_empty() {
            failures.push(format!("{REGISTER}: entry \"{id}\" names no file"));
        }
        if entries.insert(id.clone(), files).is_some() {
            failures.push(format!("{REGISTER}: entry \"{id}\" is listed twice"));
        }
    }

    // Every marker, by file
    let mut found: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for path in tracked() {
        let Ok(text) = fs::read_to_string(repo().join(&path)) else {
            continue;
        };
        let lines: Vec<&str> = text.lines().collect();
        for (index, line) in lines.iter().enumerate() {
            for id in markers(line) {
                found.entry(id).or_default().insert(path.clone());
            }
            if in_tripwire_scope(&path) {
                let mut lower = line.to_lowercase();
                for name in BOOKKEEPING {
                    lower = lower.replace(name, "");
                }
                if let Some(sign) = TRIPWIRE.iter().find(|sign| lower.contains(*sign)) {
                    let from = index.saturating_sub(MARKER_REACH);
                    if !lines[from..=index]
                        .iter()
                        .any(|near| !markers(near).is_empty())
                    {
                        failures.push(format!(
                            "{path}:{}: names the engine (\"{sign}\") with no WEBKITGTK-ONLY marker",
                            index + 1
                        ));
                    }
                }
            }
        }
    }

    for (id, paths) in &found {
        for path in paths {
            if !entries.get(id).is_some_and(|files| files.contains(path)) {
                failures.push(format!(
                    "{REGISTER}: marker \"{id}\" in {path} has no entry"
                ));
            }
        }
    }
    for (id, files) in &entries {
        for file in files {
            if !repo().join(file).exists() {
                failures.push(format!(
                    "{REGISTER}: entry \"{id}\" names {file}, which does not exist"
                ));
            } else if !found.get(id).is_some_and(|paths| paths.contains(file)) {
                failures.push(format!(
                    "{REGISTER}: entry \"{id}\" has no marker in {file}"
                ));
            }
        }
    }

    assert!(failures.is_empty(), "\n{}\n", failures.join("\n"));
}
