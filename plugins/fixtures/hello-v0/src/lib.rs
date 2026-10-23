//! Test fixture: a well-behaved plugin of contract v0, which the host keeps running (FR-013).
//! It greets the name in its configuration and writes one log message, like feature 001's
//! example.

wit_bindgen::generate!({ path: "../../../wit/v0", world: "widget" });

use dashboard::widget::log::{Level, write};
use exports::dashboard::widget::renderer::Guest;
use serde_json::{Value, json};

struct HelloV0;

impl Guest for HelloV0 {
    fn render(config: String) -> Result<String, String> {
        write(Level::Info, "rendering hello-v0");

        let config: Value = serde_json::from_str(&config)
            .map_err(|error| format!("the configuration is not valid JSON: {error}"))?;
        let name = match config.get("name") {
            None => "world",
            Some(Value::String(name)) => name,
            Some(_) => return Err(r#""name" in the configuration must be a string"#.to_owned()),
        };
        Ok(json!({"title": "Hello", "text": format!("Hello, {name}!")}).to_string())
    }
}

export!(HelloV0);
