//! The criteria of feature 007 that the content reader carries
//! (`docs/design/host-surfaces.md`): HS-06, HS-08 and HS-09, and the placeholder of FR-003.

use serde_json::{Value, json};
use widget_content::{Component, ErrorKind, parse};

fn body(components: Value) -> Value {
    json!({ "format": 1, "body": components })
}

/// HS-09: content carrying style fields draws exactly as without them, and an unknown tone draws
/// as the default and adds one entry to the plugin's log.
#[test]
fn style_fields_are_ignored_and_an_unknown_tone_draws_as_the_default() {
    let plain = body(json!([{ "type": "text", "text": "x", "tone": "ok" }]));
    let styled = body(json!([{
        "type": "text", "text": "x", "tone": "ok",
        "color": "red", "font": "Comic Sans", "size": 72, "class": "evil", "style": "position:fixed"
    }]));
    let (plain, plain_log) = parse(&plain).unwrap();
    let (styled, styled_log) = parse(&styled).unwrap();
    assert_eq!(plain, styled, "style fields changed what is drawn");
    assert!(plain_log.is_empty());
    assert_eq!(styled_log.len(), 1, "{styled_log:?}");
    for field in ["color", "font", "size", "class", "style"] {
        assert!(styled_log[0].contains(field), "{styled_log:?}");
    }

    let (content, log) = parse(&body(
        json!([{ "type": "text", "text": "x", "tone": "danger" }]),
    ))
    .unwrap();
    let Component::Text { tone, .. } = &content.body[0] else {
        panic!()
    };
    assert_eq!(tone.name, "default");
    assert_eq!(log.len(), 1, "{log:?}");
    assert!(log[0].contains("danger"));
}

/// HS-08: every status tone of the widget map carries its icon, so status reads without color.
#[test]
fn every_status_tone_carries_its_icon() {
    let map: Value =
        serde_json::from_str(include_str!("../../design/widget-map.generated.json")).unwrap();
    let mut status = 0;
    for (name, tone) in map["tones"].as_object().unwrap() {
        let (content, _) = parse(&body(json!([
            { "type": "text", "text": "x", "tone": name },
            { "type": "list", "items": [{ "text": "y", "tone": name }] }
        ])))
        .unwrap();
        let Component::Text {
            tone: text_tone, ..
        } = &content.body[0]
        else {
            panic!()
        };
        let Component::List { items } = &content.body[1] else {
            panic!()
        };
        let expected = tone["icon"].as_str().map(str::to_owned);
        assert_eq!(text_tone.icon, expected, "{name}");
        assert_eq!(items[0].tone.icon, expected, "{name}");
        status += usize::from(expected.is_some());
    }
    assert_eq!(status, 3, "ok, warning and error");
}

/// HS-06: the form has no password or masked field, and no field kind carries a credential.
#[test]
fn the_form_has_no_password_or_masked_field() {
    for kind in [
        "password",
        "masked",
        "secret",
        "email",
        "username",
        "current-password",
        "date",
        "file",
        "link",
    ] {
        let document = body(json!([{ "type": "form",
            "fields": [{ "id": "a", "label": "Senha", "kind": kind }],
            "submit": { "label": "Entrar", "action": "go" } }]));
        let error = parse(&document).expect_err(kind);
        assert_eq!(error.kind, ErrorKind::BadShape, "{kind}");
    }
    // A credential-looking field name or hint from content never reaches a field: only its id,
    // label, kind and options are kept
    let (content, log) = parse(&body(json!([{ "type": "form",
        "fields": [{ "id": "user", "label": "Senha", "kind": "text",
                     "autocomplete": "current-password", "name": "password", "inputmode": "email" }],
        "submit": { "label": "Entrar", "action": "go" } }])))
    .unwrap();
    let Component::Form { fields, .. } = &content.body[0] else {
        panic!()
    };
    let drawn = serde_json::to_value(&fields[0]).unwrap();
    assert_eq!(
        drawn,
        json!({ "id": "user", "label": "Senha", "kind": "text" })
    );
    assert_eq!(log.len(), 1, "{log:?}");
}

/// FR-003: a component type the host does not know draws as its placeholder, logged once, and
/// the rest draws.
#[test]
fn an_unknown_component_is_a_placeholder_and_the_rest_draws() {
    let (content, log) = parse(&body(json!([
        { "type": "iframe", "src": "https://probe.invalid" },
        { "type": "text", "text": "after" }
    ])))
    .unwrap();
    assert_eq!(
        content.body[0],
        Component::Placeholder {
            name: "iframe".into()
        }
    );
    assert!(matches!(&content.body[1], Component::Text { text, .. } if text == "after"));
    assert_eq!(log.len(), 1, "{log:?}");
}

/// An error names the path and never quotes the plugin's text.
#[test]
fn an_error_names_its_path_and_quotes_nothing_of_the_plugin() {
    let error = parse(&body(json!([
        { "type": "text", "text": "fine" },
        { "type": "table", "columns": ["<script>alert(1)</script>"], "rows": [["a", "b"]] }
    ])))
    .unwrap_err();
    assert_eq!(error.path, "$.body[1].rows[0]");
    assert!(!error.to_string().contains("script"), "{error}");
}
