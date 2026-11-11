//! The author docs' page on widget content, `docs/authors/reference/content.md`, agrees with the
//! host's reader and the published schema (spec FR-006, SC-003): every example on it is checked
//! as the app checks content, and against `wit/content.schema.json`.
//!
//! The page's examples are its fenced blocks whose info string starts with `json`. The words
//! after `json` say what the example is:
//!
//! - `json`: a whole document. The host draws it with no entry for the plugin's log, and the
//!   schema accepts it.
//! - `json component`: one component, checked as the only component of a document's `body`, as
//!   above.
//! - `json logs=<n>`: a whole document the host draws with `<n>` log entries, which the schema
//!   refuses: a mistake the host forgives.
//! - `json refused=<kind>`: a whole document the host does not draw, refused with `<kind>`
//!   (`bad-shape` or `over-bound`). The schema is not asked: some rules are the host's only.
//!
//! When a `logs=` or `refused=` example is followed by a `text` block, that block is what
//! `runtime run --check-content` prints for it, line for line.
//!
//! The page also names every bound of the format with its value, as `limits.md` does, and every
//! name the schema allows for a type, kind, tone or variant.

use std::path::{Path, PathBuf};

use boon::{Compiler, SchemaIndex, Schemas};
use serde_json::{Value, json};
use widget_content::parse;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("widget-content/ is in the repository")
        .to_path_buf()
}

fn page(relative: &str) -> String {
    std::fs::read_to_string(repository().join(relative))
        .unwrap_or_else(|error| panic!("{relative} is read: {error}"))
}

fn schema() -> (Schemas, SchemaIndex) {
    let path = repository().join("wit/content.schema.json");
    let mut schemas = Schemas::new();
    let index = Compiler::new()
        .compile(path.to_str().expect("a UTF-8 path"), &mut schemas)
        .unwrap_or_else(|error| panic!("the schema compiles: {error}"));
    (schemas, index)
}

/// A fenced block of a page: the line of its opening fence, counting from 1, its info string
/// and its text.
#[derive(Debug)]
struct Block {
    line: usize,
    info: String,
    text: String,
}

fn blocks(markdown: &str) -> Vec<Block> {
    let mut found = Vec::new();
    let mut open: Option<Block> = None;
    for (index, line) in markdown.lines().enumerate() {
        let trimmed = line.trim_start();
        match open.take() {
            None => {
                if let Some(info) = trimmed.strip_prefix("```") {
                    open = Some(Block {
                        line: index + 1,
                        info: info.trim().to_owned(),
                        text: String::new(),
                    });
                }
            }
            Some(block) if trimmed == "```" => found.push(block),
            Some(mut block) => {
                block.text.push_str(line);
                block.text.push('\n');
                open = Some(block);
            }
        }
    }
    assert!(open.is_none(), "a fenced block does not end");
    found
}

/// What `runtime run --check-content` prints for `document` (`runtime/src/cli/mod.rs`), for
/// text without control characters, which the CLI escapes.
fn printed(document: &Value) -> Vec<String> {
    match parse(document) {
        Ok((_, log)) => {
            let mut lines: Vec<String> = log
                .iter()
                .map(|entry| format!("[content] {entry}"))
                .collect();
            lines.push(match log.len() {
                0 => "content: drawn".to_owned(),
                n => format!("content: drawn, {n} log entries"),
            });
            lines
        }
        Err(error) => vec![format!("content: not drawn: {error}")],
    }
}

/// Checks one example, saying what is wrong with it.
fn check(
    block: &Block,
    next: Option<&Block>,
    schema: &(Schemas, SchemaIndex),
) -> Result<(), String> {
    let mut words = block.info.split_whitespace();
    words.next();
    let what = words.next();
    if words.next().is_some() {
        return Err(format!("too many words in `{}`", block.info));
    }
    let value: Value =
        serde_json::from_str(&block.text).map_err(|error| format!("not JSON: {error}"))?;
    let document = match what {
        Some("component") => json!({ "format": 1, "body": [value] }),
        _ => value,
    };
    let accepted = schema.0.validate(&document, schema.1).is_ok();
    let outcome = parse(&document);
    match what {
        None | Some("component") => {
            let (_, log) = outcome.map_err(|error| format!("not drawn: {error}"))?;
            if !log.is_empty() {
                return Err(format!("drawn with log entries: {log:?}"));
            }
            if !accepted {
                let error = schema.0.validate(&document, schema.1).unwrap_err();
                return Err(format!("the schema refuses it: {error}"));
            }
        }
        Some(other) => {
            if let Some(count) = other.strip_prefix("logs=") {
                let count: usize = count
                    .parse()
                    .map_err(|_| format!("`{other}` needs a number"))?;
                let (_, log) = outcome.map_err(|error| format!("not drawn: {error}"))?;
                if log.len() != count {
                    return Err(format!("drawn with {} log entries: {log:?}", log.len()));
                }
                if accepted {
                    return Err("the schema accepts it, so the host has nothing to forgive".into());
                }
            } else if let Some(kind) = other.strip_prefix("refused=") {
                let error = match outcome {
                    Ok(_) => return Err("the host draws it".to_owned()),
                    Err(error) => error,
                };
                let shown = error.to_string();
                if !shown.starts_with(&format!("{kind} at ")) {
                    return Err(format!("refused as {shown}, not {kind}"));
                }
            } else {
                return Err(format!("unknown example kind `{other}`"));
            }
            if let Some(next) = next.filter(|next| next.info == "text") {
                let shown: Vec<&str> = next.text.lines().collect();
                let expected = printed(&document);
                if shown != expected {
                    return Err(format!(
                        "the page shows (line {}):\n{}\nthe CLI prints:\n{}",
                        next.line,
                        shown.join("\n"),
                        expected.join("\n")
                    ));
                }
            }
        }
    }
    Ok(())
}

#[test]
fn every_example_on_the_page_is_what_it_says() {
    let schema = schema();
    let page = page("docs/authors/reference/content.md");
    let all = blocks(&page);
    let mut problems = Vec::new();
    let mut examples = 0;
    for (index, block) in all.iter().enumerate() {
        if block.info.split_whitespace().next() != Some("json") {
            continue;
        }
        examples += 1;
        if let Err(problem) = check(block, all.get(index + 1), &schema) {
            problems.push(format!("content.md:{}: {problem}", block.line));
        }
    }
    assert!(examples >= 8, "only {examples} examples were found");
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

/// A count as the reference writes it: `2,000`.
fn grouped(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// Every table row of `text` whose first cell is a constant's name in backticks, with the value
/// in its second cell.
fn bound_rows(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|line| {
            let cells: Vec<&str> = line.split('|').map(str::trim).collect();
            let name = cells.get(1)?.strip_prefix("`MAX_")?.strip_suffix('`')?;
            Some((format!("MAX_{name}"), cells.get(2)?.to_string()))
        })
        .collect()
}

#[test]
fn the_bounds_on_the_pages_are_the_hosts() {
    let bounds = [
        ("MAX_DEPTH", widget_content::MAX_DEPTH),
        ("MAX_COMPONENTS", widget_content::MAX_COMPONENTS),
        ("MAX_STRING_CHARS", widget_content::MAX_STRING_CHARS),
        ("MAX_LIST_ITEMS", widget_content::MAX_LIST_ITEMS),
        ("MAX_TABLE_COLUMNS", widget_content::MAX_TABLE_COLUMNS),
        ("MAX_TABLE_ROWS", widget_content::MAX_TABLE_ROWS),
        ("MAX_SERIES", widget_content::MAX_SERIES),
        ("MAX_POINTS", widget_content::MAX_POINTS),
        ("MAX_FIELDS", widget_content::MAX_FIELDS),
        ("MAX_OPTIONS", widget_content::MAX_OPTIONS),
        ("MAX_FIELD_ID_CHARS", widget_content::MAX_FIELD_ID_CHARS),
    ];
    let expected: Vec<(String, String)> = bounds
        .iter()
        .map(|(name, value)| (name.to_string(), grouped(*value)))
        .collect();
    for relative in [
        "docs/authors/reference/content.md",
        "docs/authors/reference/limits.md",
    ] {
        assert_eq!(bound_rows(&page(relative)), expected, "{relative}");
    }
}

/// Every name the schema allows for a type, kind, tone or variant: its `const` and `enum`
/// strings.
fn names(node: &Value, found: &mut Vec<String>) {
    match node {
        Value::Object(object) => {
            if let Some(Value::String(name)) = object.get("const") {
                found.push(name.clone());
            }
            for name in object
                .get("enum")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                found.extend(name.as_str().map(str::to_owned));
            }
            object.values().for_each(|value| names(value, found));
        }
        Value::Array(items) => items.iter().for_each(|item| names(item, found)),
        _ => {}
    }
}

#[test]
fn every_name_the_schema_allows_is_on_the_page() {
    let schema: Value = serde_json::from_str(&page("wit/content.schema.json")).expect("JSON");
    let mut all = Vec::new();
    names(&schema, &mut all);
    let page = page("docs/authors/reference/content.md");
    let missing: Vec<&String> = all
        .iter()
        .filter(|name| !page.contains(&format!("`{name}`")))
        .collect();
    assert!(missing.is_empty(), "not on content.md: {missing:?}");
}

// The checker's own tests

#[test]
fn a_wrong_example_fails_naming_its_line() {
    let schema = schema();
    let markdown = "text\n\n```json component\n{ \"type\": \"text\", \"text\": \"a\", \"color\": \"red\" }\n```\n";
    let all = blocks(markdown);
    assert_eq!(all[0].line, 3);
    let problem = check(&all[0], None, &schema).unwrap_err();
    assert!(problem.contains("log entries"), "{problem}");
    let refused = "```json refused=over-bound\n{ \"format\": 1, \"body\": [{ \"type\": \"table\", \"columns\": [\"a\"], \"rows\": [[\"a\", \"b\"]] }] }\n```\n";
    let problem = check(&blocks(refused)[0], None, &schema).unwrap_err();
    assert!(problem.contains("bad-shape"), "{problem}");
}

#[test]
fn the_output_after_an_example_must_be_what_the_cli_prints() {
    let schema = schema();
    let markdown = "```json logs=1\n{ \"format\": 1, \"body\": [{ \"type\": \"text\", \"text\": \"a\", \"tone\": \"loud\" }] }\n```\n\n```text\ncontent: drawn\n```\n";
    let all = blocks(markdown);
    let problem = check(&all[0], all.get(1), &schema).unwrap_err();
    assert!(
        problem.contains("content: drawn, 1 log entries"),
        "{problem}"
    );
}
