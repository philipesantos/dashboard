//! Test fixture: a contract 1.4.0 widget of the `interactive-connected-stored-widget` world
//! whose actions make the storage and connector calls their configuration lists, and return
//! every answer, so a case sees what the host answered in the action phase
//! (specs/009-declarative-widget-ui/research.md, R7).
//!
//! `act` reads, from the `config` of its input, `{"steps": [...], "end": "ok" | "error" |
//! "trap" | "loop"}`. A step is `{"op": "set", "name", "value"}`, `{"op": "get", "name"}` or
//! `{"op": "call", "connector", "operation"}`. It returns `{"from": "action", "answers":
//! [...]}`. `update` returns `{"from": "update", "n": <config.n>}`, and `render` returns
//! `{"document": <the document the host gave it>}`.

wit_bindgen::generate!({
    path: "../../../wit/history/1.4.0",
    world: "interactive-connected-stored-widget"
});

use dashboard::widget::{connectors, storage};
use exports::dashboard::widget::actor::Guest as Actor;
use exports::dashboard::widget::renderer::Guest as Renderer;
use exports::dashboard::widget::updater::Guest as Updater;
use serde_json::{Value as Json, json};

struct ActionSteps;

impl Renderer for ActionSteps {
    fn render(config: String) -> Result<String, String> {
        let document: Json = serde_json::from_str(&config).unwrap_or(Json::String(config));
        Ok(json!({ "document": document }).to_string())
    }
}

impl Updater for ActionSteps {
    fn update(config: String) -> Result<String, String> {
        let config: Json = serde_json::from_str(&config).unwrap_or(Json::Null);
        Ok(json!({ "from": "update", "n": config["n"] }).to_string())
    }
}

fn text<'a>(step: &'a Json, field: &str) -> &'a str {
    step.get(field).and_then(Json::as_str).unwrap_or_default()
}

fn step(step: &Json) -> Json {
    match text(step, "op") {
        "set" => match storage::set(text(step, "name"), text(step, "value").as_bytes()) {
            Ok(()) => json!({ "ok": null }),
            Err(error) => json!({ "error": format!("{error:?}") }),
        },
        "get" => match storage::get(text(step, "name")) {
            Ok(value) => {
                json!({ "ok": value.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()) })
            }
            Err(error) => json!({ "error": format!("{error:?}") }),
        },
        "call" => match connectors::call(text(step, "connector"), text(step, "operation"), &[]) {
            Ok(response) => json!({ "ok": response.status }),
            Err(error) => json!({ "error": format!("{error:?}") }),
        },
        other => json!({ "error": format!("unknown op {other}") }),
    }
}

impl Actor for ActionSteps {
    fn act(_action: String, input: String) -> Result<String, String> {
        let input: Json = serde_json::from_str(&input).map_err(|error| error.to_string())?;
        let config = &input["config"];
        let answers: Vec<Json> = config["steps"]
            .as_array()
            .map(|steps| steps.iter().map(step).collect())
            .unwrap_or_default();
        match text(config, "end") {
            "error" => Err("asked to fail after its steps".to_owned()),
            "trap" => panic!("asked to trap after its steps"),
            "loop" => {
                let mut i = 0u64;
                loop {
                    i = std::hint::black_box(i.wrapping_add(1));
                }
            }
            _ => Ok(json!({ "from": "action", "answers": answers }).to_string()),
        }
    }
}

export!(ActionSteps);
