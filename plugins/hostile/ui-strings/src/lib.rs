//! Hostile widget for the window probe (specs/009-declarative-widget-ui/research.md, R14; HS-06,
//! HS-07, HS-08, HS-24, HS-25): markup, script and image tags, bidirectional overrides, web
//! addresses, file paths and custom schemes in every component that holds text; a select with
//! the most options a field may have; a form with a number field the probe fills wrongly; the
//! three status tones. Every string must appear as literal text, isolated, and nothing it names
//! may exist, load or open.

wit_bindgen::generate!({ path: "../../../wit/history/1.4.0", world: "interactive-widget" });

use exports::dashboard::widget::actor::Guest as Actor;
use exports::dashboard::widget::renderer::Guest as Renderer;
use serde_json::json;

/// What a page that treated content as markup or a link would act on. `__probeInjected` is the
/// flag the probe looks for.
const HOSTILE: [&str; 12] = [
    "<script>window.__probeInjected = 1</script>",
    "<img src=x onerror=\"window.__probeInjected = 1\">",
    "<b>negrito</b> <a href=\"https://probe.invalid/\">link</a>",
    "\u{202E}gnp.exe",
    "https://probe.invalid/caminho?q=1",
    "javascript:window.__probeInjected = 1",
    "file:///etc/passwd",
    "C:\\Windows\\System32\\drivers\\etc\\hosts",
    "/home/user/.ssh/id_ed25519",
    "dashboard://settings",
    "tauri://localhost/index.html",
    "mailto:someone@probe.invalid",
];

/// Follows a right-to-left override, so the probe can see whether the override reaches it.
const SENTINEL: &str = "SENTINELA";

struct UiStrings;

impl Renderer for UiStrings {
    fn render(_config: String) -> Result<String, String> {
        let texts: Vec<_> = HOSTILE
            .iter()
            .zip(
                ["default", "muted", "accent", "ok", "warning", "error"]
                    .iter()
                    .cycle(),
            )
            .map(|(text, tone)| json!({ "type": "text", "text": text, "tone": tone }))
            .collect();
        let mut items: Vec<_> = HOSTILE.iter().map(|text| json!(text)).collect();
        items.push(json!(SENTINEL));
        let options: Vec<String> = (0..50)
            .map(|i| format!("{i} {}", HOSTILE[i % HOSTILE.len()]))
            .collect();
        let buttons: Vec<_> = HOSTILE
            .iter()
            .take(5)
            .zip(["primary", "secondary", "outline", "ghost", "destructive"])
            .map(|(label, variant)| {
                json!({ "type": "button", "label": label, "action": "send", "variant": variant })
            })
            .collect();
        Ok(json!({ "format": 1, "body": [
            { "type": "text", "text": "Tudo certo", "tone": "ok" },
            { "type": "text", "text": "Atenção", "tone": "warning" },
            { "type": "text", "text": "Falhou", "tone": "error" },
            { "type": "stack", "children": texts },
            { "type": "list", "items": items },
            { "type": "table", "columns": &HOSTILE[..4], "rows": [&HOSTILE[4..8], &HOSTILE[8..12]] },
            { "type": "chart", "kind": "line", "labels": &HOSTILE[..4],
              "series": [{ "name": HOSTILE[1], "values": [1, 2, 3, 4] }] },
            { "type": "stack", "children": buttons },
            { "type": "form",
              "fields": [
                { "id": "title", "label": HOSTILE[0], "kind": "text" },
                { "id": "amount", "label": HOSTILE[3], "kind": "number" },
                { "id": "done", "label": HOSTILE[2], "kind": "checkbox" },
                { "id": "where", "label": HOSTILE[5], "kind": "select", "options": options },
              ],
              "submit": { "label": HOSTILE[1], "action": "send" } },
        ]})
        .to_string())
    }
}

impl Actor for UiStrings {
    fn act(_action: String, _input: String) -> Result<String, String> {
        Ok("{}".to_owned())
    }
}

export!(UiStrings);
