//! Hostile plugin: looks for a credential everywhere the standard library can reach: every
//! environment variable, its arguments, and the files its configuration names, such as the
//! keychain's files, the accounts record and the grant record. Built for `wasm32-wasip2`, it
//! imports `wasi:cli` and `wasi:filesystem`, which the host never offers. It declares a connector
//! with an account and exports `updater`, so its contract passes and only the import check refuses
//! it, at load, before it runs (specs/005-plugin-credentials/data-model.md, Hostile cases,
//! `token-hunt`).

wit_bindgen::generate!({ path: "../../../wit/v1", world: "account-widget" });

use exports::dashboard::widget::renderer::Guest as Renderer;
use exports::dashboard::widget::updater::Guest as Updater;
use serde_json::{Value, json};

struct TokenHunt;

/// Everything found: the environment, the arguments and each named file, hexadecimal.
fn hunt(config: &str) -> Result<String, String> {
    let config: Value = serde_json::from_str(config).map_err(|error| error.to_string())?;
    let environment: Vec<(String, String)> = std::env::vars().collect();
    let arguments: Vec<String> = std::env::args().collect();
    let files: Vec<Value> = config
        .pointer("/paths")
        .or_else(|| config.pointer("/config/paths"))
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
        .iter()
        .filter_map(Value::as_str)
        .map(|path| match std::fs::read(path) {
            Ok(bytes) => json!({ "path": path, "hex": bytes.iter().map(|byte| format!("{byte:02x}")).collect::<String>() }),
            Err(error) => json!({ "path": path, "error": error.to_string() }),
        })
        .collect();
    Ok(json!({ "environment": environment, "arguments": arguments, "files": files }).to_string())
}

impl Renderer for TokenHunt {
    fn render(config: String) -> Result<String, String> {
        hunt(&config)
    }
}

impl Updater for TokenHunt {
    fn update(config: String) -> Result<String, String> {
        hunt(&config)
    }
}

export!(TokenHunt);
