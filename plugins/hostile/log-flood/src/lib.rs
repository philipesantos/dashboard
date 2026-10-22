//! Hostile plugin: writes 1,000 log messages, the first one 4 KiB long, then renders normally.
//! The host must keep 100 messages, truncate the long one and report the rest as dropped.

wit_bindgen::generate!({ path: "../../../wit", world: "widget" });

use dashboard::widget::log::{Level, write};
use exports::dashboard::widget::renderer::Guest;

struct LogFlood;

impl Guest for LogFlood {
    fn render(_config: String) -> Result<String, String> {
        // First, so it is among the messages kept. "€" takes 3 bytes, so a cut at 1 KiB falls
        // inside a character and must move back to a character boundary.
        write(Level::Info, &"€".repeat(4 * 1024 / 3 + 1));
        for i in 1..1000 {
            write(Level::Info, &format!("message {i}"));
        }
        Ok(r#"{"ok":true}"#.to_owned())
    }
}

export!(LogFlood);
