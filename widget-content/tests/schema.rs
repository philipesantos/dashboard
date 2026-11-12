//! The published schema, `wit/content.schema.json`, and the host's reader agree
//! (`specs/009-declarative-widget-ui/contracts/content-format.md`, Agreement test): every
//! document the schema accepts, the host draws with no log entry; every document the host
//! refuses, the schema refuses too, except the rules a schema cannot express; and the lenient
//! cases, which the schema refuses so authors' tools catch them, the host draws with a log entry.

use std::path::Path;

use boon::{Compiler, SchemaIndex, Schemas};
use serde_json::{Value, json};
use widget_content::{ErrorKind, parse};

fn schema() -> (Schemas, SchemaIndex) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../wit/content.schema.json");
    let mut schemas = Schemas::new();
    let index = Compiler::new()
        .compile(path.to_str().expect("a UTF-8 path"), &mut schemas)
        .unwrap_or_else(|error| panic!("the schema compiles: {error}"));
    (schemas, index)
}

fn schema_accepts(schema: &(Schemas, SchemaIndex), document: &Value) -> bool {
    schema.0.validate(document, schema.1).is_ok()
}

/// A document whose body is `components`.
fn body(components: Value) -> Value {
    json!({ "format": 1, "body": components })
}

fn text(value: &str) -> Value {
    json!({ "type": "text", "text": value })
}

/// Documents both accept, covering every component and field.
fn valid() -> Vec<(&'static str, Value)> {
    vec![
        ("one text", body(json!([text("Hoje")]))),
        (
            "every text variant and tone",
            body(json!([
                { "type": "text", "text": "a", "variant": "heading" },
                { "type": "text", "text": "b", "variant": "muted", "tone": "accent" },
                { "type": "text", "text": "c", "tone": "ok" },
                { "type": "text", "text": "d", "tone": "warning" },
                { "type": "text", "text": "e", "tone": "error" },
                { "type": "text", "text": "f", "variant": "default", "tone": "default" },
                { "type": "text", "text": "g", "tone": "muted" }
            ])),
        ),
        (
            "a list of strings and toned items",
            body(json!([{ "type": "list", "items": ["a", { "text": "b", "tone": "ok" }] }])),
        ),
        (
            "an empty list",
            body(json!([{ "type": "list", "items": [] }])),
        ),
        (
            "a table",
            body(
                json!([{ "type": "table", "columns": ["a", "b"], "rows": [["1", "2"], ["3", "4"]] }]),
            ),
        ),
        (
            "a bar and a line chart",
            body(json!([
                { "type": "chart", "labels": ["seg", "ter"], "series": [{ "name": "x", "values": [1, 2.5] }] },
                { "type": "chart", "kind": "line", "labels": ["a"],
                  "series": [{ "name": "x", "values": [1] }, { "name": "y", "values": [-2] }] }
            ])),
        ),
        (
            "every button variant",
            body(json!([
                { "type": "button", "label": "a", "action": "go" },
                { "type": "button", "label": "b", "action": "go", "variant": "primary" },
                { "type": "button", "label": "c", "action": "go", "variant": "secondary" },
                { "type": "button", "label": "d", "action": "go", "variant": "outline" },
                { "type": "button", "label": "e", "action": "go", "variant": "ghost" },
                { "type": "button", "label": "f", "action": "go", "variant": "destructive" }
            ])),
        ),
        (
            "a form with every field kind",
            body(json!([{ "type": "form",
                "fields": [
                    { "id": "title", "label": "Título", "kind": "text" },
                    { "id": "count", "label": "Quantos", "kind": "number" },
                    { "id": "done", "label": "Feita", "kind": "checkbox" },
                    { "id": "level", "label": "Nível", "kind": "select", "options": ["baixo", "alto"] }
                ],
                "submit": { "label": "Salvar", "action": "save" } }])),
        ),
        (
            "a stack in a stack",
            body(
                json!([{ "type": "stack", "children": [{ "type": "stack", "children": [text("x")] }] }]),
            ),
        ),
        (
            "a string of 2,000 characters",
            body(json!([text(&"x".repeat(2000))])),
        ),
        (
            "a list of 200 items",
            body(json!([{ "type": "list", "items": vec!["x"; 200] }])),
        ),
        (
            "a form of 20 fields",
            body(json!([{ "type": "form",
                "fields": (0..20).map(|n| json!({ "id": format!("f{n}"), "label": "x", "kind": "text" })).collect::<Vec<_>>(),
                "submit": { "label": "ok", "action": "save" } }])),
        ),
    ]
}

/// Documents both refuse.
fn invalid() -> Vec<(&'static str, Value)> {
    vec![
        ("not an object", json!([])),
        ("no format", json!({ "body": [text("x")] })),
        ("format 2", json!({ "format": 2, "body": [text("x")] })),
        ("no body", json!({ "format": 1 })),
        ("an empty body", body(json!([]))),
        ("a component that is not an object", body(json!(["x"]))),
        ("a text with no text", body(json!([{ "type": "text" }]))),
        (
            "a text that is a number",
            body(json!([{ "type": "text", "text": 5 }])),
        ),
        (
            "a string over 2,000 characters",
            body(json!([text(&"x".repeat(2001))])),
        ),
        (
            "a list over 200 items",
            body(json!([{ "type": "list", "items": vec!["x"; 201] }])),
        ),
        (
            "a list item that is a number",
            body(json!([{ "type": "list", "items": [3] }])),
        ),
        (
            "a table with no column",
            body(json!([{ "type": "table", "columns": [], "rows": [] }])),
        ),
        (
            "a table over 20 columns",
            body(json!([{ "type": "table", "columns": vec!["c"; 21], "rows": [] }])),
        ),
        (
            "a chart with no series",
            body(json!([{ "type": "chart", "labels": [], "series": [] }])),
        ),
        (
            "a chart over 5 series",
            body(json!([{ "type": "chart", "labels": ["a"],
                "series": (0..6).map(|_| json!({ "name": "s", "values": [1] })).collect::<Vec<_>>() }])),
        ),
        (
            "a chart value that is text",
            body(
                json!([{ "type": "chart", "labels": ["a"], "series": [{ "name": "s", "values": ["1"] }] }]),
            ),
        ),
        (
            "a chart of another kind",
            body(
                json!([{ "type": "chart", "kind": "pie", "labels": [], "series": [{ "name": "s", "values": [] }] }]),
            ),
        ),
        (
            "a button with no action",
            body(json!([{ "type": "button", "label": "x" }])),
        ),
        (
            "a password field",
            body(
                json!([{ "type": "form", "fields": [{ "id": "p", "label": "Senha", "kind": "password" }],
                "submit": { "label": "ok", "action": "save" } }]),
            ),
        ),
        (
            "a masked field",
            body(
                json!([{ "type": "form", "fields": [{ "id": "p", "label": "PIN", "kind": "masked" }],
                "submit": { "label": "ok", "action": "save" } }]),
            ),
        ),
        (
            "a select with no option",
            body(
                json!([{ "type": "form", "fields": [{ "id": "s", "label": "x", "kind": "select" }],
                "submit": { "label": "ok", "action": "save" } }]),
            ),
        ),
        (
            "options on a text field",
            body(
                json!([{ "type": "form", "fields": [{ "id": "s", "label": "x", "kind": "text", "options": ["a"] }],
                "submit": { "label": "ok", "action": "save" } }]),
            ),
        ),
        (
            "a field id in capitals",
            body(
                json!([{ "type": "form", "fields": [{ "id": "Title", "label": "x", "kind": "text" }],
                "submit": { "label": "ok", "action": "save" } }]),
            ),
        ),
        (
            "a form over 20 fields",
            body(json!([{ "type": "form",
                "fields": (0..21).map(|n| json!({ "id": format!("f{n}"), "label": "x", "kind": "text" })).collect::<Vec<_>>(),
                "submit": { "label": "ok", "action": "save" } }])),
        ),
        (
            "a form with no submit",
            body(
                json!([{ "type": "form", "fields": [{ "id": "a", "label": "x", "kind": "text" }] }]),
            ),
        ),
        (
            "a variant that is a number",
            body(json!([{ "type": "text", "text": "x", "variant": 3 }])),
        ),
        (
            "a stack with no children",
            body(json!([{ "type": "stack" }])),
        ),
    ]
}

#[test]
fn the_schema_and_the_host_accept_the_same_content() {
    let schema = schema();
    for (case, document) in valid() {
        assert!(
            schema_accepts(&schema, &document),
            "the schema refused {case}"
        );
        let (_, log) =
            parse(&document).unwrap_or_else(|error| panic!("the host refused {case}: {error}"));
        assert!(log.is_empty(), "{case} logged {log:?}");
    }
}

#[test]
fn the_schema_and_the_host_refuse_the_same_content() {
    let schema = schema();
    for (case, document) in invalid() {
        assert!(
            !schema_accepts(&schema, &document),
            "the schema accepted {case}"
        );
        assert!(parse(&document).is_err(), "the host accepted {case}");
    }
}

/// Rules a schema cannot express: the schema accepts these, the host refuses them.
#[test]
fn the_host_also_refuses_what_a_schema_cannot_see() {
    let schema = schema();
    let mut deep = text("x");
    for _ in 0..8 {
        deep = json!({ "type": "stack", "children": [deep] });
    }
    let many: Vec<Value> = (0..30)
        .map(|_| json!({ "type": "stack", "children": vec![text("x"); 20] }))
        .collect();
    let host_only = [
        ("nested 9 deep", body(json!([deep])), ErrorKind::OverBound),
        (
            "more than 500 components in all",
            body(json!(many)),
            ErrorKind::OverBound,
        ),
        (
            "a row shorter than the columns",
            body(json!([{ "type": "table", "columns": ["a", "b"], "rows": [["1"]] }])),
            ErrorKind::BadShape,
        ),
        (
            "fewer values than labels",
            body(
                json!([{ "type": "chart", "labels": ["a", "b"], "series": [{ "name": "s", "values": [1] }] }]),
            ),
            ErrorKind::BadShape,
        ),
        (
            "two fields with one id",
            body(json!([{ "type": "form",
                "fields": [{ "id": "a", "label": "x", "kind": "text" }, { "id": "a", "label": "y", "kind": "text" }],
                "submit": { "label": "ok", "action": "save" } }])),
            ErrorKind::BadShape,
        ),
    ];
    for (case, document, kind) in host_only {
        assert!(
            schema_accepts(&schema, &document),
            "the schema refused {case}"
        );
        let error = parse(&document).expect_err(case);
        assert_eq!(error.kind, kind, "{case}: {error}");
    }
}

/// The lenient cases (HS-09): the schema refuses them, the host draws them with a log entry.
#[test]
fn the_host_draws_what_only_the_schema_refuses_and_logs_it() {
    let schema = schema();
    let lenient = [
        (
            "a color field",
            body(json!([{ "type": "text", "text": "x", "color": "#f00" }])),
        ),
        (
            "style and class",
            body(json!([{ "type": "text", "text": "x", "style": "x", "class": "y" }])),
        ),
        (
            "an unknown tone",
            body(json!([{ "type": "text", "text": "x", "tone": "danger" }])),
        ),
        (
            "an unknown variant",
            body(json!([{ "type": "button", "label": "x", "action": "go", "variant": "huge" }])),
        ),
        (
            "an unknown component",
            body(json!([{ "type": "video", "src": "x" }, text("rest")])),
        ),
        (
            "an unknown top-level field",
            json!({ "format": 1, "body": [text("x")], "theme": "dark" }),
        ),
    ];
    for (case, document) in lenient {
        assert!(
            !schema_accepts(&schema, &document),
            "the schema accepted {case}"
        );
        let (_, log) =
            parse(&document).unwrap_or_else(|error| panic!("the host refused {case}: {error}"));
        assert_eq!(log.len(), 1, "{case}: {log:?}");
    }
}
