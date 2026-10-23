//! Hostile plugin: returns widget content that is not JSON. The host must reject it.

wit_bindgen::generate!({ path: "../../../wit/v0", world: "widget" });

use exports::dashboard::widget::renderer::Guest;

struct MalformedOutput;

impl Guest for MalformedOutput {
    fn render(_config: String) -> Result<String, String> {
        Ok("{not json".to_owned())
    }
}

export!(MalformedOutput);
