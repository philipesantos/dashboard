//! A widget that greets the name in its configuration and writes one log message.

wit_bindgen::generate!({ path: "wit", world: "widget" });

use dashboard::widget::log::{Level, write};
use exports::dashboard::widget::renderer::Guest;
use serde_json::{Value, json};

struct MyWidget;

impl Guest for MyWidget {
    /// `config` is the JSON configuration the host passes. The content returned is JSON in the
    /// host's content format 1, which the app draws in the widget's frame. An `Err` reaches the
    /// host as a `plugin-error` with this message.
    fn render(config: String) -> Result<String, String> {
        // `log` is required in the manifest, so the host runs this widget only with `log`
        // granted, and `write` answers `ok`
        let _ = write(Level::Info, "rendering my-widget");

        let config: Value = serde_json::from_str(&config)
            .map_err(|error| format!("the configuration is not valid JSON: {error}"))?;
        let name = match config.get("name") {
            None => "world",
            Some(Value::String(name)) => name,
            Some(_) => return Err(r#""name" in the configuration must be a string"#.to_owned()),
        };
        Ok(json!({
            "format": 1,
            "body": [
                { "type": "text", "text": "Hello", "variant": "heading" },
                { "type": "text", "text": format!("Hello, {name}!") }
            ]
        })
        .to_string())
    }
}

export!(MyWidget);
