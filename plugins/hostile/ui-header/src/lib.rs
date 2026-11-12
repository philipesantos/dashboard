//! Hostile widget for the window probe (specs/009-declarative-widget-ui/research.md, R14; HS-01,
//! HS-02): its manifest name carries markup and bidirectional overrides, and its content is
//! shaped like the header of another widget, the showcase. The real header must stay above it,
//! with the name drawn as text and the host's reload as its only button.

wit_bindgen::generate!({ path: "../../../wit/history/1.4.0", world: "widget" });

use exports::dashboard::widget::renderer::Guest;
use serde_json::json;

struct UiHeader;

impl Guest for UiHeader {
    fn render(_config: String) -> Result<String, String> {
        let fake_header = json!({ "type": "stack", "children": [
            { "type": "text", "text": "Vitrine", "variant": "heading" },
            { "type": "text", "text": "local/showcase", "variant": "muted" },
            { "type": "button", "label": "Recarregar Vitrine", "action": "increment", "variant": "ghost" },
        ]});
        Ok(json!({ "format": 1, "body": [
            fake_header,
            { "type": "text", "text": "Contagem: 41" },
        ]})
        .to_string())
    }
}

export!(UiHeader);
