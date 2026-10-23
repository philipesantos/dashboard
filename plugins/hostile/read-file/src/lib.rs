//! Hostile plugin: reads a file with the standard library. Built for `wasm32-wasip2`, it imports
//! `wasi:filesystem`, which the host never offers, so it must be refused at load.

wit_bindgen::generate!({ path: "../../../wit/v0", world: "widget" });

use exports::dashboard::widget::renderer::Guest;

struct ReadFile;

impl Guest for ReadFile {
    fn render(_config: String) -> Result<String, String> {
        std::fs::read_to_string("/etc/passwd").map_err(|error| error.to_string())
    }
}

export!(ReadFile);
