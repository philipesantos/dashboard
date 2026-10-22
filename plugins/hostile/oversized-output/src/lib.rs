//! Hostile plugin: returns valid JSON of 128 KiB, twice the size cap. The host must reject it
//! before parsing it.

wit_bindgen::generate!({ path: "../../../wit", world: "widget" });

use exports::dashboard::widget::renderer::Guest;

struct OversizedOutput;

impl Guest for OversizedOutput {
    fn render(_config: String) -> Result<String, String> {
        Ok(format!("\"{}\"", "a".repeat(128 * 1024)))
    }
}

export!(OversizedOutput);
