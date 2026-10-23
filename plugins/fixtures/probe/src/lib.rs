//! Test fixture: a contract v1 plugin that calls `log` once and puts what it answered in its
//! output, so a test sees whether the real capability or the stand-in was linked.

wit_bindgen::generate!({ path: "../../../wit/v1", world: "widget" });

use dashboard::widget::log::{Level, LogError, write};
use exports::dashboard::widget::renderer::Guest;

struct Probe;

impl Guest for Probe {
    fn render(_config: String) -> Result<String, String> {
        let answer = match write(Level::Info, "probe") {
            Ok(()) => "ok",
            Err(LogError::NotGranted) => "not-granted",
        };
        Ok(format!(r#"{{"log":"{answer}"}}"#))
    }
}

export!(Probe);
