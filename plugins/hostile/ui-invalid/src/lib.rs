//! Hostile widget for the window probe (specs/009-declarative-widget-ui/research.md, R14): its
//! render returns content with a text over the 2,000-character bound, or, after the action
//! `wrong-type`, a text that is a number. The action `spin` never returns, so the time limit
//! ends it. The frame must show the host's error state each time, and the window must keep
//! answering while the action spins (FR-005, FR-017).

wit_bindgen::generate!({ path: "../../../wit/history/1.4.0", world: "interactive-widget" });

use exports::dashboard::widget::actor::Guest as Actor;
use exports::dashboard::widget::renderer::Guest as Renderer;
use serde_json::{Value, json};

struct UiInvalid;

impl Renderer for UiInvalid {
    fn render(config: String) -> Result<String, String> {
        let document: Value = serde_json::from_str(&config).unwrap_or(Value::Null);
        let text = if document["data"]["mode"] == "wrong-type" {
            json!(5)
        } else {
            json!("x".repeat(2001))
        };
        Ok(json!({ "format": 1, "body": [{ "type": "text", "text": text }] }).to_string())
    }
}

impl Actor for UiInvalid {
    fn act(action: String, _input: String) -> Result<String, String> {
        if action == "spin" {
            let mut i = 0u64;
            loop {
                i = std::hint::black_box(i.wrapping_add(1));
            }
        }
        Ok(json!({ "mode": "wrong-type" }).to_string())
    }
}

export!(UiInvalid);
