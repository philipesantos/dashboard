//! Hostile widget for the window probe (specs/009-declarative-widget-ui/research.md, R14): its
//! manifest's name is over the 60-character limit, so the host refuses it at load. Its frame
//! must show the not-loaded state, under the file's name, and nothing of it must run (FR-005).

wit_bindgen::generate!({ path: "../../../wit/history/1.4.0", world: "widget" });

use exports::dashboard::widget::renderer::Guest;

struct UiBadManifest;

impl Guest for UiBadManifest {
    fn render(_config: String) -> Result<String, String> {
        Ok(r#"{"format":1,"body":[{"type":"text","text":"this must never be drawn"}]}"#.to_owned())
    }
}

export!(UiBadManifest);
