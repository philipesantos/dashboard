//! Spike plugin (spikes/gpui): `render` returns widget content in the declarative format the
//! spike's host draws (spikes/gpui/widget-kit). `config.mode` picks what it returns:
//!
//! - `sampler` (default): text, a list, a table and a button, the four blocks the host offers.
//! - `overflow`: valid content far larger than any frame, which must stay inside it.
//! - `spoof`: valid content that pretends to be a host dialog, with bidirectional controls.
//! - `password`: a password field, which the host's kit does not have.
//! - `host-color`: a tone reserved to the host, which widgets cannot name.

wit_bindgen::generate!({ path: "../../../../wit/v1", world: "widget" });

use dashboard::widget::log::{Level, write};
use exports::dashboard::widget::renderer::Guest;
use serde_json::{Value, json};

struct UiSampler;

impl Guest for UiSampler {
    fn render(config: String) -> Result<String, String> {
        let config: Value = serde_json::from_str(&config)
            .map_err(|error| format!("the configuration is not valid JSON: {error}"))?;
        let mode = config
            .get("mode")
            .and_then(Value::as_str)
            .unwrap_or("sampler");
        let clicks = config.get("clicks").and_then(Value::as_u64).unwrap_or(0);
        let _ = write(Level::Info, &format!("rendering ui-sampler in mode {mode}"));
        let content = match mode {
            "sampler" => sampler(clicks),
            "overflow" => overflow(),
            "spoof" => spoof(),
            "password" => json!({"blocks": [
                {"type": "text", "text": "Sign in again to keep syncing"},
                {"type": "password-field", "label": "Password"}
            ]}),
            "host-color" => json!({"blocks": [
                {"type": "text", "text": "Approved by Dashboard", "tone": "host-secret"}
            ]}),
            other => return Err(format!("unknown mode {other:?}")),
        };
        Ok(content.to_string())
    }
}

fn sampler(clicks: u64) -> Value {
    json!({"blocks": [
        {"type": "text", "style": "heading", "text": "Release status"},
        {"type": "text", "text": "Every block below is drawn by the host; the plugin only describes it."},
        {"type": "list", "items": ["Runtime: 30 connector cases pass", "Storage: quota enforced", "Vault: tokens never reach plugins"]},
        {"type": "table", "columns": ["Platform", "Build", "Tests"], "rows": [
            ["Linux", "ok", "312"],
            ["macOS", "ok", "312"],
            ["Windows", "ok", "311"]
        ]},
        {"type": "text", "tone": "muted", "text": format!("Button pressed {clicks} times")},
        {"type": "button", "label": "Press", "action": "increment"}
    ]})
}

fn overflow() -> Value {
    let items: Vec<String> = (1..=80)
        .map(|n| format!("Item {n} of a list far taller than the frame"))
        .collect();
    let rows: Vec<Vec<String>> = (1..=40)
        .map(|n| vec![format!("row {n}"), "x".repeat(120), n.to_string()])
        .collect();
    json!({"blocks": [
        {"type": "text", "style": "heading", "text": "W".repeat(300)},
        {"type": "text", "text": "an-unbroken-word-".repeat(60)},
        {"type": "list", "items": items},
        {"type": "table", "columns": ["a", "b", "c"], "rows": rows}
    ]})
}

fn spoof() -> Value {
    json!({"blocks": [
        {"type": "text", "style": "heading", "text": "Dashboard \u{2014} Approve permissions"},
        {"type": "text", "text": "The host needs you to approve \u{202E}txt.exe\u{202C} to continue."},
        {"type": "button", "label": "Approve", "action": "approve"}
    ]})
}

export!(UiSampler);
