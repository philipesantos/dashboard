//! The drift test (`specs/006-contract-v1-docs/research.md`, R7, and
//! `specs/006-contract-v1-docs/contracts/docs-format.md`): the reference for authors under
//! `docs/authors/reference/` names every item of the contract, the manifest, connector and
//! widget content formats, the host's errors, the limits and the CLI, and nothing they do not
//! have. Each entry
//! carries a marker `<!-- ref: <kind> <id> -->`; the expected set comes from the sources
//! themselves, so a new item fails the build until the reference has it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use regex::Regex;
use serde_json::Value;
use wit_parser::{Resolve, TypeDefKind};

const KINDS: [&str; 7] = [
    "wit",
    "error",
    "manifest",
    "connector",
    "content",
    "limit",
    "cli",
];

/// What the reference must hold: each item by (kind, id), with its default for a limit.
type Expected = BTreeMap<(String, String), Option<String>>;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("runtime/ is in the repository")
        .to_path_buf()
}

fn add(expected: &mut Expected, kind: &str, id: impl Into<String>) {
    expected.insert((kind.to_owned(), id.into()), None);
}

/// Every world, interface, function, named type, and case or field of the package in `resolve`;
/// a definition of another kind fails, naming it, rather than pass unlisted.
fn wit_items(
    resolve: &Resolve,
    package: wit_parser::PackageId,
    expected: &mut Expected,
) -> Vec<String> {
    let mut unclassified = Vec::new();
    let package = &resolve.packages[package];
    for name in package.worlds.keys() {
        add(expected, "wit", name.clone());
    }
    for (name, &interface) in &package.interfaces {
        add(expected, "wit", name.clone());
        let interface = &resolve.interfaces[interface];
        for function in interface.functions.keys() {
            add(expected, "wit", format!("{name}/{function}"));
        }
        for (type_name, &id) in &interface.types {
            let path = format!("{name}/{type_name}");
            let parts: Vec<String> = match &resolve.types[id].kind {
                TypeDefKind::Record(record) => record
                    .fields
                    .iter()
                    .map(|field| field.name.clone())
                    .collect(),
                TypeDefKind::Variant(variant) => {
                    variant.cases.iter().map(|case| case.name.clone()).collect()
                }
                TypeDefKind::Enum(enumeration) => enumeration
                    .cases
                    .iter()
                    .map(|case| case.name.clone())
                    .collect(),
                TypeDefKind::Flags(flags) => {
                    flags.flags.iter().map(|flag| flag.name.clone()).collect()
                }
                // A name for another type: an entry of its own, with nothing inside
                TypeDefKind::Type(_)
                | TypeDefKind::List(_)
                | TypeDefKind::Option(_)
                | TypeDefKind::Result(_)
                | TypeDefKind::Tuple(_)
                | TypeDefKind::Map(..)
                | TypeDefKind::FixedLengthList(..) => Vec::new(),
                other @ (TypeDefKind::Resource
                | TypeDefKind::Handle(_)
                | TypeDefKind::Future(_)
                | TypeDefKind::Stream(_)
                | TypeDefKind::Unknown) => {
                    unclassified.push(format!("{path} is a {}", other.as_str()));
                    continue;
                }
            };
            add(expected, "wit", path.clone());
            for part in parts {
                add(expected, "wit", format!("{path}/{part}"));
            }
        }
    }
    unclassified
}

/// Every field path of a JSON Schema: `/` between names, `[]` for the items of an array and `*`
/// for the values of a map, with local `$ref`s followed. A keyword that adds fields the walk
/// does not know fails, naming it.
fn schema_fields(schema: &Value, kind: &str, expected: &mut Expected) -> Vec<String> {
    fn walk(
        node: &Value,
        path: &str,
        root: &Value,
        seen: &BTreeSet<String>,
        found: &mut BTreeSet<String>,
        unclassified: &mut Vec<String>,
    ) {
        let Some(node) = node.as_object() else {
            return;
        };
        if let Some(reference) = node.get("$ref").and_then(Value::as_str) {
            let Some(name) = reference.strip_prefix("#/$defs/") else {
                unclassified.push(format!("{path}: $ref {reference} is not local"));
                return;
            };
            if !seen.contains(name) {
                let mut seen = seen.clone();
                seen.insert(name.to_owned());
                walk(&root["$defs"][name], path, root, &seen, found, unclassified);
            }
        }
        for keyword in ["patternProperties", "prefixItems", "dependentSchemas"] {
            if node.contains_key(keyword) {
                unclassified.push(format!("{path}: {keyword}"));
            }
        }
        for keyword in ["oneOf", "anyOf", "allOf"] {
            for alternative in node
                .get(keyword)
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
            {
                walk(alternative, path, root, seen, found, unclassified);
            }
        }
        for keyword in ["then", "else"] {
            if let Some(branch) = node.get(keyword) {
                walk(branch, path, root, seen, found, unclassified);
            }
        }
        let join = |name: &str| {
            if path.is_empty() {
                name.to_owned()
            } else {
                format!("{path}/{name}")
            }
        };
        if let Some(properties) = node.get("properties").and_then(Value::as_object) {
            for (name, property) in properties {
                let field = join(name);
                found.insert(field.clone());
                walk(property, &field, root, seen, found, unclassified);
            }
        }
        if let Some(items) = node.get("items") {
            walk(items, &format!("{path}[]"), root, seen, found, unclassified);
        }
        if let Some(values) = node
            .get("additionalProperties")
            .filter(|values| values.is_object())
        {
            walk(values, &join("*"), root, seen, found, unclassified);
        }
    }
    let mut found = BTreeSet::new();
    let mut unclassified = Vec::new();
    walk(
        schema,
        "",
        schema,
        &BTreeSet::new(),
        &mut found,
        &mut unclassified,
    );
    for field in found {
        add(expected, kind, field);
    }
    unclassified
}

/// A limit's default as the reference writes it: `10 MiB`, `64 KiB` or `2560 bytes` for sizes,
/// `1 s`, `500 ms` or `10 min` for times, `30 per minute` for rates, `500,000,000` for counts.
fn shown(name: &str, value: Shown) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    match value {
        Shown::Time(time) => {
            let seconds = time.as_secs();
            if time.subsec_nanos() == 0 && seconds >= 60 && seconds % 60 == 0 {
                format!("{} min", seconds / 60)
            } else if time.subsec_nanos() == 0 {
                format!("{seconds} s")
            } else {
                format!("{} ms", time.as_millis())
            }
        }
        Shown::Number(n) if name.contains("bytes") => {
            if n >= MIB && n % MIB == 0 {
                format!("{} MiB", n / MIB)
            } else if n >= KIB && n % KIB == 0 {
                format!("{} KiB", n / KIB)
            } else {
                format!("{n} bytes")
            }
        }
        Shown::Number(n) if name.contains("per_minute") => format!("{n} per minute"),
        Shown::Number(n) => {
            let digits = n.to_string();
            let mut grouped = String::new();
            for (index, digit) in digits.chars().enumerate() {
                if index > 0 && (digits.len() - index) % 3 == 0 {
                    grouped.push(',');
                }
                grouped.push(digit);
            }
            grouped
        }
    }
}

enum Shown {
    Number(u64),
    Time(Duration),
}

impl From<usize> for Shown {
    fn from(value: usize) -> Shown {
        Shown::Number(value as u64)
    }
}

impl From<u64> for Shown {
    fn from(value: u64) -> Shown {
        Shown::Number(value)
    }
}

impl From<u32> for Shown {
    fn from(value: u32) -> Shown {
        Shown::Number(u64::from(value))
    }
}

impl From<Duration> for Shown {
    fn from(value: Duration) -> Shown {
        Shown::Time(value)
    }
}

/// Every limit with its default. The destructuring names each field, so a new limit fails to
/// compile until it is listed here, and then fails the test until the reference has it.
fn limits(expected: &mut Expected) {
    let runtime::Limits {
        max_package_bytes,
        fuel,
        time_limit,
        max_memory_bytes,
        max_output_bytes,
        max_log_messages,
        max_log_message_bytes,
        update_time_limit,
        call_time_limit,
        max_calls_per_run,
        max_arguments,
        max_argument_name_chars,
        max_request_address_bytes,
        max_request_body_bytes,
        max_response_body_bytes,
        max_update_response_bytes,
        max_response_header_bytes,
        max_redirects,
        max_update_output_bytes,
        calls_per_minute_per_widget,
        calls_per_minute_per_host,
        max_request_log_bytes,
        max_logged_value_chars,
        max_logged_name_chars,
        max_storage_quota_bytes,
        storage_entry_cost_bytes,
        max_storage_name_bytes,
        max_storage_value_bytes,
        max_storage_calls_per_run,
        max_storage_written_bytes_per_update,
        storage_list_page,
        storage_file_cap_factor,
        storage_file_cap_extra_bytes,
        max_storage_busy_wait,
        sign_in_time_limit,
        max_callback_head_bytes,
        callback_read_time,
        max_sign_in_response_bytes,
        max_secret_bytes,
        max_key_bytes,
        max_account_name_chars,
        vault_wait_tool,
        refresh_margin,
        refreshes_per_call,
    } = runtime::Limits::default();
    let all: [(&str, Shown); 44] = [
        ("max_package_bytes", max_package_bytes.into()),
        ("fuel", fuel.into()),
        ("time_limit", time_limit.into()),
        ("max_memory_bytes", max_memory_bytes.into()),
        ("max_output_bytes", max_output_bytes.into()),
        ("max_log_messages", max_log_messages.into()),
        ("max_log_message_bytes", max_log_message_bytes.into()),
        ("update_time_limit", update_time_limit.into()),
        ("call_time_limit", call_time_limit.into()),
        ("max_calls_per_run", max_calls_per_run.into()),
        ("max_arguments", max_arguments.into()),
        ("max_argument_name_chars", max_argument_name_chars.into()),
        (
            "max_request_address_bytes",
            max_request_address_bytes.into(),
        ),
        ("max_request_body_bytes", max_request_body_bytes.into()),
        ("max_response_body_bytes", max_response_body_bytes.into()),
        (
            "max_update_response_bytes",
            max_update_response_bytes.into(),
        ),
        (
            "max_response_header_bytes",
            max_response_header_bytes.into(),
        ),
        ("max_redirects", max_redirects.into()),
        ("max_update_output_bytes", max_update_output_bytes.into()),
        (
            "calls_per_minute_per_widget",
            calls_per_minute_per_widget.into(),
        ),
        (
            "calls_per_minute_per_host",
            calls_per_minute_per_host.into(),
        ),
        ("max_request_log_bytes", max_request_log_bytes.into()),
        ("max_logged_value_chars", max_logged_value_chars.into()),
        ("max_logged_name_chars", max_logged_name_chars.into()),
        ("max_storage_quota_bytes", max_storage_quota_bytes.into()),
        ("storage_entry_cost_bytes", storage_entry_cost_bytes.into()),
        ("max_storage_name_bytes", max_storage_name_bytes.into()),
        ("max_storage_value_bytes", max_storage_value_bytes.into()),
        (
            "max_storage_calls_per_run",
            max_storage_calls_per_run.into(),
        ),
        (
            "max_storage_written_bytes_per_update",
            max_storage_written_bytes_per_update.into(),
        ),
        ("storage_list_page", storage_list_page.into()),
        ("storage_file_cap_factor", storage_file_cap_factor.into()),
        (
            "storage_file_cap_extra_bytes",
            storage_file_cap_extra_bytes.into(),
        ),
        ("max_storage_busy_wait", max_storage_busy_wait.into()),
        ("sign_in_time_limit", sign_in_time_limit.into()),
        ("max_callback_head_bytes", max_callback_head_bytes.into()),
        ("callback_read_time", callback_read_time.into()),
        (
            "max_sign_in_response_bytes",
            max_sign_in_response_bytes.into(),
        ),
        ("max_secret_bytes", max_secret_bytes.into()),
        ("max_key_bytes", max_key_bytes.into()),
        ("max_account_name_chars", max_account_name_chars.into()),
        ("vault_wait_tool", vault_wait_tool.into()),
        ("refresh_margin", refresh_margin.into()),
        ("refreshes_per_call", refreshes_per_call.into()),
    ];
    for (name, value) in all {
        let default = shown(name, value);
        expected.insert(("limit".to_owned(), name.to_owned()), Some(default));
    }
}

/// Every command and option of the CLI's usage but `bench`, which is for the project's own
/// measurements; a line the parser cannot read fails, naming it.
fn cli_items(usage: &str, expected: &mut Expected) -> Vec<String> {
    let option = Regex::new(r"--[a-z][a-z-]*").expect("a valid pattern");
    let mut unclassified = Vec::new();
    for line in usage.lines() {
        let line = line.trim_start();
        let line = line.strip_prefix("usage: ").unwrap_or(line);
        let Some(rest) = line.strip_prefix("runtime ") else {
            unclassified.push(format!("usage line {line:?}"));
            continue;
        };
        let command: Vec<&str> = rest
            .split_whitespace()
            .take_while(|word| word.chars().all(|c| c.is_ascii_lowercase() || c == '-'))
            .collect();
        if command.is_empty() {
            unclassified.push(format!("usage line {line:?} names no command"));
            continue;
        }
        let command = command.join(" ");
        if command == "bench" {
            continue;
        }
        add(expected, "cli", command.clone());
        for found in option.find_iter(rest) {
            add(expected, "cli", format!("{command} {}", found.as_str()));
        }
    }
    unclassified
}

/// Compares the markers of `pages` (each a name and its text) with `expected`, naming every
/// difference: a missing item, a marker no source has, a marker twice, an unknown kind, a
/// limit's default that differs from the host's.
fn check(expected: &Expected, pages: &[(String, String)]) -> Vec<String> {
    let marker = Regex::new(r"<!-- ref: (\S+) (.+?) -->").expect("a valid pattern");
    let mut problems = Vec::new();
    let mut seen: BTreeMap<(String, String), String> = BTreeMap::new();
    for (page, text) in pages {
        for (index, line) in text.lines().enumerate() {
            let place = format!("{page}:{}", index + 1);
            for found in marker.captures_iter(line) {
                let (kind, id) = (found[1].to_owned(), found[2].trim().to_owned());
                if !KINDS.contains(&kind.as_str()) {
                    problems.push(format!("{place}: unknown kind {kind:?}"));
                    continue;
                }
                let key = (kind.clone(), id.clone());
                if let Some(first) = seen.insert(key.clone(), place.clone()) {
                    problems.push(format!(
                        "{place}: {kind} {id} is marked twice (first at {first})"
                    ));
                    continue;
                }
                match expected.get(&key) {
                    None => problems.push(format!("{place}: {kind} {id} does not exist")),
                    Some(Some(default)) => {
                        let cells: Vec<&str> = line.split('|').map(str::trim).collect();
                        let written = cells.get(2).copied().unwrap_or("");
                        if written != default {
                            problems.push(format!(
                                "{place}: limit {id} shows {written:?}, the host's default is {default:?}"
                            ));
                        }
                    }
                    Some(None) => {}
                }
            }
        }
    }
    for key in expected.keys() {
        if !seen.contains_key(key) {
            problems.push(format!("{} {} is missing from the reference", key.0, key.1));
        }
    }
    problems
}

/// A link from a page into `specs/` that a reader would need to follow: one not on a line
/// starting with `Background:` (FR-015, SC-007).
fn spec_links(pages: &[(String, String)]) -> Vec<String> {
    let link = Regex::new(r"\]\([^)]*specs/").expect("a valid pattern");
    let mut problems = Vec::new();
    for (page, text) in pages {
        for (index, line) in text.lines().enumerate() {
            if link.is_match(line) && !line.trim_start().starts_with("Background:") {
                problems.push(format!("{page}:{}: links into specs/", index + 1));
            }
        }
    }
    problems
}

fn read_pages(dir: &Path) -> Vec<(String, String)> {
    fn walk(dir: &Path, root: &Path, found: &mut Vec<(String, String)>) {
        for entry in std::fs::read_dir(dir).expect("the folder is read") {
            let path = entry.expect("an entry").path();
            if path.is_dir() {
                walk(&path, root, found);
            } else if path.extension().is_some_and(|extension| extension == "md") {
                let name = path
                    .strip_prefix(root)
                    .expect("under the root")
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = std::fs::read_to_string(&path).expect("the page is read");
                found.push((name, text));
            }
        }
    }
    let mut found = Vec::new();
    walk(dir, dir, &mut found);
    found.sort();
    found
}

fn json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("the schema is read"))
        .expect("the schema is JSON")
}

#[test]
fn every_item_is_in_the_reference() {
    let root = repository();
    let mut expected = Expected::new();
    let mut unclassified = Vec::new();
    let mut resolve = Resolve::new();
    let (package, _) = resolve
        .push_dir(root.join("wit").join("v1"))
        .unwrap_or_else(|error| panic!("wit/v1 parses: {error:#}"));
    unclassified.extend(wit_items(&resolve, package, &mut expected));
    unclassified.extend(schema_fields(
        &json(&root.join("wit/manifest.schema.json")),
        "manifest",
        &mut expected,
    ));
    unclassified.extend(schema_fields(
        &json(&root.join("wit/connector.schema.json")),
        "connector",
        &mut expected,
    ));
    unclassified.extend(schema_fields(
        &json(&root.join("wit/content.schema.json")),
        "content",
        &mut expected,
    ));
    for kind in runtime::ErrorKind::ALL {
        add(&mut expected, "error", kind.as_str());
    }
    limits(&mut expected);
    unclassified.extend(cli_items(runtime::cli::USAGE, &mut expected));
    assert!(
        unclassified.is_empty(),
        "the drift test cannot classify:\n{}",
        unclassified.join("\n")
    );
    let pages = read_pages(&root.join("docs/authors/reference"));
    let problems = check(&expected, &pages);
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn no_page_needs_a_spec() {
    let problems = spec_links(&read_pages(&repository().join("docs/authors")));
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

// The checker's own tests, on sources and pages in memory

fn page(text: &str) -> Vec<(String, String)> {
    vec![("test.md".to_owned(), text.to_owned())]
}

#[test]
fn a_missing_or_unknown_or_repeated_marker_fails_naming_it() {
    let mut expected = Expected::new();
    add(&mut expected, "error", "not-granted");
    add(&mut expected, "error", "plugin-error");
    let problems = check(
        &expected,
        &page(
            "<!-- ref: error not-granted -->\n<!-- ref: error no-such-error -->\n<!-- ref: error not-granted -->\n",
        ),
    );
    let all = problems.join("\n");
    assert!(all.contains("error plugin-error is missing"), "{all}");
    assert!(all.contains("error no-such-error does not exist"), "{all}");
    assert!(all.contains("marked twice"), "{all}");
    assert!(
        check(&expected, &page("<!-- ref: errors not-granted -->"))
            .join("\n")
            .contains("unknown kind \"errors\""),
    );
}

#[test]
fn a_limit_whose_default_differs_fails_naming_both_values() {
    let mut expected = Expected::new();
    expected.insert(
        ("limit".to_owned(), "max_package_bytes".to_owned()),
        Some("10 MiB".to_owned()),
    );
    let right = "| `max_package_bytes` | 10 MiB | packages | `invalid-package` <!-- ref: limit max_package_bytes --> |";
    assert!(check(&expected, &page(right)).is_empty());
    let wrong = right.replace("10 MiB", "8 MiB");
    let problems = check(&expected, &page(&wrong)).join("\n");
    assert!(problems.contains("\"8 MiB\""), "{problems}");
    assert!(problems.contains("\"10 MiB\""), "{problems}");
}

#[test]
fn defaults_are_shown_in_the_reference_units() {
    assert_eq!(
        shown("max_package_bytes", (10 * 1024 * 1024_usize).into()),
        "10 MiB"
    );
    assert_eq!(
        shown("max_output_bytes", (64 * 1024_usize).into()),
        "64 KiB"
    );
    assert_eq!(shown("max_secret_bytes", 2560_usize.into()), "2560 bytes");
    assert_eq!(shown("fuel", 500_000_000_u64.into()), "500,000,000");
    assert_eq!(
        shown("max_storage_calls_per_run", 1000_usize.into()),
        "1,000"
    );
    assert_eq!(shown("max_redirects", 5_usize.into()), "5");
    assert_eq!(
        shown("calls_per_minute_per_host", 60_u32.into()),
        "60 per minute"
    );
    assert_eq!(shown("time_limit", Duration::from_secs(1).into()), "1 s");
    assert_eq!(shown("x", Duration::from_millis(500).into()), "500 ms");
    assert_eq!(
        shown("sign_in_time_limit", Duration::from_secs(600).into()),
        "10 min"
    );
}

#[test]
fn a_wit_definition_the_checker_cannot_classify_fails_naming_it() {
    let mut resolve = Resolve::new();
    let package = resolve
        .push_str(
            "test.wit",
            "package test:drift@1.0.0;\ninterface files { resource handle; enum mode { read, write } open: func() -> mode; }",
        )
        .expect("the test WIT parses");
    let mut expected = Expected::new();
    let unclassified = wit_items(&resolve, package, &mut expected);
    assert_eq!(unclassified, ["files/handle is a resource"]);
    for id in [
        "files",
        "files/open",
        "files/mode",
        "files/mode/read",
        "files/mode/write",
    ] {
        assert!(
            expected.contains_key(&("wit".to_owned(), id.to_owned())),
            "{id}"
        );
    }
}

#[test]
fn schema_paths_follow_items_maps_and_refs() {
    let schema: Value = serde_json::json!({
        "properties": {
            "list": { "type": "array", "items": { "properties": { "name": {} } } },
            "map": { "additionalProperties": { "$ref": "#/$defs/entry" } }
        },
        "$defs": { "entry": { "properties": { "kind": {} } } }
    });
    let mut expected = Expected::new();
    assert!(schema_fields(&schema, "connector", &mut expected).is_empty());
    let found: Vec<&str> = expected.keys().map(|(_, id)| id.as_str()).collect();
    assert_eq!(found, ["list", "list[]/name", "map", "map/*/kind"]);
    let odd = serde_json::json!({ "patternProperties": { "^x": {} } });
    assert!(!schema_fields(&odd, "connector", &mut Expected::new()).is_empty());
}

#[test]
fn schema_paths_follow_alternatives_and_conditions_once_per_definition() {
    // The shape of wit/content.schema.json: components as `oneOf` alternatives, a list item
    // that is a string or an object, a field whose options an `if`/`then`/`else` requires, and
    // a component that holds components again
    let schema: Value = serde_json::json!({
        "properties": {
            "body": { "type": "array", "items": { "$ref": "#/$defs/component" } }
        },
        "$defs": {
            "text": { "type": "string" },
            "component": { "oneOf": [{ "$ref": "#/$defs/list" }, { "$ref": "#/$defs/stack" }] },
            "list": {
                "properties": {
                    "type": { "const": "list" },
                    "items": { "items": { "oneOf": [
                        { "$ref": "#/$defs/text" },
                        { "properties": { "text": { "$ref": "#/$defs/text" } } }
                    ] } },
                    "field": {
                        "properties": { "kind": {}, "options": {} },
                        "if": { "properties": { "kind": { "const": "select" } } },
                        "then": { "required": ["options"] },
                        "else": { "not": { "required": ["options"] } }
                    }
                }
            },
            "stack": {
                "properties": {
                    "type": { "const": "stack" },
                    "children": { "items": { "$ref": "#/$defs/component" } }
                }
            }
        }
    });
    let mut expected = Expected::new();
    assert!(schema_fields(&schema, "content", &mut expected).is_empty());
    let found: Vec<&str> = expected.keys().map(|(_, id)| id.as_str()).collect();
    assert_eq!(
        found,
        [
            "body",
            "body[]/children",
            "body[]/field",
            "body[]/field/kind",
            "body[]/field/options",
            "body[]/items",
            "body[]/items[]/text",
            "body[]/type",
        ]
    );
}

#[test]
fn an_option_in_the_usage_without_a_marker_fails() {
    let mut expected = Expected::new();
    let usage = "usage: runtime run <plugin> [--config <json>]\n       runtime bench <plugin> [--iterations <n>]";
    assert!(cli_items(usage, &mut expected).is_empty());
    let problems = check(&expected, &page("<!-- ref: cli run -->")).join("\n");
    assert!(
        problems.contains("cli run --config is missing"),
        "{problems}"
    );
    assert!(!problems.contains("bench"), "{problems}");
    assert!(!cli_items("usage: rt go", &mut Expected::new()).is_empty());
}

#[test]
fn a_link_into_the_specs_fails_unless_it_is_background() {
    let pages = page(
        "See [the spec](../../specs/006-x/spec.md).\nBackground: [why](../../specs/003/research.md).\n",
    );
    assert_eq!(spec_links(&pages), ["test.md:1: links into specs/"]);
}
