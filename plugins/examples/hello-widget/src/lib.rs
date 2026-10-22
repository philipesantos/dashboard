//! Example plugin for contract v0: writes one log message and renders a greeting.

wit_bindgen::generate!({ path: "../../../wit", world: "widget" });

use dashboard::widget::log::{Level, write};
use exports::dashboard::widget::renderer::Guest;

struct HelloWidget;

impl Guest for HelloWidget {
    fn render(_config: String) -> Result<String, String> {
        write(Level::Info, "rendering hello-widget");
        Ok(r#"{"title":"Hello","text":"Hello, world!"}"#.to_owned())
    }
}

export!(HelloWidget);
