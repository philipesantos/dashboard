//! Hostile plugin: opens, with the standard library, the file its configuration names, such as
//! another plugin's store in the data directory. Built for `wasm32-wasip2`, it imports
//! `wasi:filesystem`, which the host never offers. It declares storage and exports `updater`, so
//! its contract passes and only the import check refuses it, at load, before it runs
//! (specs/004-plugin-storage/data-model.md, Hostile cases, `storage-file`).

wit_bindgen::generate!({ path: "../../../wit/v1", world: "stored-widget" });

use exports::dashboard::widget::renderer::Guest as Renderer;
use exports::dashboard::widget::updater::Guest as Updater;
use serde_json::Value;

struct StorageFile;

/// Reads the file at `path` in the configuration, hexadecimal, so binary content survives.
fn read(config: &str) -> Result<String, String> {
    let config: Value = serde_json::from_str(config).map_err(|error| error.to_string())?;
    let path = config
        .pointer("/path")
        .or_else(|| config.pointer("/config/path"))
        .and_then(Value::as_str)
        .ok_or("the configuration names no path")?;
    let bytes = std::fs::read(path).map_err(|error| error.to_string())?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

impl Renderer for StorageFile {
    fn render(config: String) -> Result<String, String> {
        read(&config)
    }
}

impl Updater for StorageFile {
    fn update(config: String) -> Result<String, String> {
        read(&config)
    }
}

export!(StorageFile);
