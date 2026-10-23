//! Test fixture: a contract v1 plugin that never calls `log`, so its component imports nothing
//! (wit-component drops imports a plugin does not use).

wit_bindgen::generate!({ path: "../../../wit/v1", world: "widget" });

use exports::dashboard::widget::renderer::Guest;

struct Quiet;

impl Guest for Quiet {
    fn render(_config: String) -> Result<String, String> {
        Ok(r#"{"quiet":true}"#.to_owned())
    }
}

export!(Quiet);
