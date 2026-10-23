//! Example plugin for contract v1: greets the name in its configuration and writes one log
//! message.

wit_bindgen::generate!({ path: "../../../wit/history/1.0.0", world: "widget" });

use dashboard::widget::log::{Level, write};
use exports::dashboard::widget::renderer::Guest;
use serde_json::{Value, json};

struct HelloWidget;

impl Guest for HelloWidget {
    /// `config` is the JSON configuration the host passes; the widget content returned is JSON
    /// too. An `err` reaches the host as a `plugin-error` with this message.
    fn render(config: String) -> Result<String, String> {
        // `log` is required in this plugin's manifest, so the host runs it only with `log`
        // granted, and `write` always answers `ok`. A plugin whose `log` is optional checks for
        // `Err(LogError::NotGranted)` and carries on without it.
        let _ = write(Level::Info, "rendering hello-widget");

        let config: Value = serde_json::from_str(&config)
            .map_err(|error| format!("the configuration is not valid JSON: {error}"))?;
        let name = match config.get("name") {
            None => "world",
            Some(Value::String(name)) => name,
            Some(_) => return Err(r#""name" in the configuration must be a string"#.to_owned()),
        };

        // Built with serde_json, so any name, quotes included, still makes valid JSON
        Ok(json!({"title": "Hello", "text": format!("Hello, {name}!")}).to_string())
    }
}

export!(HelloWidget);
