//! Example plugin for contract 1.2.0: remembers how many times it has updated, when it first
//! ran, and the last five notes its configuration gave it, in its own storage. `update` writes
//! them, and `render` reads the count and the notes back, on any later run of the host, as
//! content in the host's format 1.

wit_bindgen::generate!({ path: "../../../wit/history/1.2.0", world: "stored-widget" });

use dashboard::widget::storage::{StorageError, delete, get, list, set};
use exports::dashboard::widget::renderer::Guest as Renderer;
use exports::dashboard::widget::updater::Guest as Updater;
use serde_json::{Value, json};

/// Notes kept, newest first, under `notes/0` to `notes/4`.
const NOTES: usize = 5;

struct UpdateCounter;

/// A message for the maintainer naming the call that did not succeed.
fn describe(call: &str, error: StorageError) -> String {
    match error {
        StorageError::NotGranted => format!("{call}: storage is not granted"),
        StorageError::Refused(problem) => format!("{call} refused: {}", problem.detail),
        StorageError::Failed(problem) => format!("{call} failed: {}", problem.detail),
    }
}

fn read_text(name: &str) -> Result<Option<String>, String> {
    let value = get(name).map_err(|error| describe(&format!("get {name}"), error))?;
    Ok(value.map(|bytes| String::from_utf8_lossy(&bytes).into_owned()))
}

fn write_text(name: &str, text: &str) -> Result<(), String> {
    set(name, text.as_bytes()).map_err(|error| describe(&format!("set {name}"), error))
}

fn note_name(index: usize) -> String {
    format!("notes/{index}")
}

impl Updater for UpdateCounter {
    fn update(config: String) -> Result<String, String> {
        let config: Value =
            serde_json::from_str(&config).map_err(|error| format!("configuration: {error}"))?;
        let count = read_text("count")?
            .and_then(|text| text.parse::<u64>().ok())
            .unwrap_or(0)
            + 1;
        write_text("count", &count.to_string())?;
        if read_text("first_run_ms")?.is_none() {
            let now = config.get("now_ms").and_then(Value::as_u64).unwrap_or(0);
            write_text("first_run_ms", &now.to_string())?;
        }
        if let Some(note) = config.get("note").and_then(Value::as_str) {
            // Shift the older notes down by one; the oldest falls off
            for index in (0..NOTES - 1).rev() {
                match read_text(&note_name(index))? {
                    Some(text) => write_text(&note_name(index + 1), &text)?,
                    None => {
                        delete(&note_name(index + 1)).map_err(|error| describe("delete", error))?
                    }
                }
            }
            write_text(&note_name(0), note)?;
        }
        // Writes are kept only because this returns ok: an error above keeps none of them
        Ok(json!({ "count": count }).to_string())
    }
}

/// Content in the host's format 1: the heading, then `body`.
fn content(body: Vec<Value>) -> String {
    let mut all = vec![json!({ "type": "text", "text": "Update counter", "variant": "heading" })];
    all.extend(body);
    json!({ "format": 1, "body": all }).to_string()
}

impl Renderer for UpdateCounter {
    fn render(_config: String) -> Result<String, String> {
        // Storage that the user turned off answers not-granted: say so instead of failing
        if let Err(StorageError::NotGranted) = get("count") {
            return Ok(content(vec![json!({
                "type": "text",
                "text": "Storage is off: turn it on to keep a count",
                "tone": "warning",
            })]));
        }
        let Some(count) = read_text("count")? else {
            return Ok(content(vec![
                json!({ "type": "text", "text": "Not updated yet", "variant": "muted" }),
            ]));
        };
        let mut body = vec![json!({ "type": "text", "text": format!("Updated {count} times") })];
        let page = list("notes/", None).map_err(|error| describe("list notes/", error))?;
        let mut notes = Vec::new();
        for name in page.names {
            if let Some(note) = read_text(&name)? {
                notes.push(note);
            }
        }
        if !notes.is_empty() {
            body.push(json!({ "type": "list", "items": notes }));
        }
        Ok(content(body))
    }
}

export!(UpdateCounter);
