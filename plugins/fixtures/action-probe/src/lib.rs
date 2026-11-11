//! Test fixture: a contract 1.4.0 widget of the `interactive-widget` world, which renders and
//! has actions but imports nothing beyond `log` and exports no `updater`
//! (specs/009-declarative-widget-ui/research.md, R7). It shows that a plugin with only `actor`
//! sees the data its actions return.
//!
//! `render` returns `{"document": <the document the host gave it, parsed>}`. `act` answers by
//! the action's name: `remember` returns `{"remembered": <values>, "seen": <data>}`; `big`
//! returns 300 KiB; `loop` never returns; `fail` fails; any other name returns
//! `{"action": <name>}`.

wit_bindgen::generate!({ path: "../../../wit/history/1.4.0", world: "interactive-widget" });

use exports::dashboard::widget::actor::Guest as Actor;
use exports::dashboard::widget::renderer::Guest as Renderer;
use serde_json::{Value as Json, json};

struct ActionProbe;

impl Renderer for ActionProbe {
    fn render(config: String) -> Result<String, String> {
        let document: Json = serde_json::from_str(&config).unwrap_or(Json::String(config));
        Ok(json!({ "document": document }).to_string())
    }
}

impl Actor for ActionProbe {
    fn act(action: String, input: String) -> Result<String, String> {
        let input: Json = serde_json::from_str(&input).map_err(|error| error.to_string())?;
        match action.as_str() {
            "remember" => {
                Ok(json!({ "remembered": input["values"], "seen": input["data"] }).to_string())
            }
            "big" => Ok(json!({ "big": "x".repeat(300 * 1024) }).to_string()),
            "loop" => {
                let mut i = 0u64;
                loop {
                    i = std::hint::black_box(i.wrapping_add(1));
                }
            }
            "fail" => Err("asked to fail".to_owned()),
            other => Ok(json!({ "action": other }).to_string()),
        }
    }
}

export!(ActionProbe);
