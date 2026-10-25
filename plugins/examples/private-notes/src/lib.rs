//! Example plugin for contract 1.3.0: shows notes read as the user's account. `update` calls the
//! development connector `local/notes` through `accounts`, so the host adds the account's
//! credential to the request itself; this widget never sees it. Without an account connected the
//! service answers with its public notes only, and `render` shows what the host kept.

wit_bindgen::generate!({ path: "../../../wit/v1", world: "account-widget" });

use dashboard::widget::accounts::{AccountCallError, call};
use exports::dashboard::widget::renderer::Guest as Renderer;
use exports::dashboard::widget::updater::Guest as Updater;
use serde_json::{Value as Json, json};

struct PrivateNotes;

impl Updater for PrivateNotes {
    /// Calls `list-notes` once. An account that must be connected again is not an error of the
    /// widget: it returns no notes and says so, and render shows it.
    fn update(_config: String) -> Result<String, String> {
        let response = match call("local/notes", "list-notes", &[]) {
            Ok(response) => response,
            Err(AccountCallError::AccountNotConnected) => {
                return Ok(json!({ "notes": [], "account": "not connected" }).to_string());
            }
            Err(AccountCallError::SubscriptionRequired) => {
                return Ok(json!({ "notes": [], "account": "subscription required" }).to_string());
            }
            Err(AccountCallError::NotGranted) => return Err("local/notes is not granted".to_owned()),
            Err(AccountCallError::Refused(problem)) => {
                return Err(format!("the call was refused: {}", problem.detail));
            }
            Err(AccountCallError::Failed(problem)) => {
                return Err(format!("the call failed: {}", problem.detail));
            }
        };
        if response.status != 200 {
            return Err(format!("the service answered with status {}", response.status));
        }
        let notes: Json = serde_json::from_slice(&response.body)
            .map_err(|error| format!("the service answered with JSON this widget cannot read: {error}"))?;
        let notes: Vec<Json> = notes
            .as_array()
            .ok_or("the service answered with something other than a list of notes")?
            .iter()
            .map(|note| json!({ "title": note.get("title"), "private": note.get("private") }))
            .collect();
        Ok(json!({ "notes": notes }).to_string())
    }
}

impl Renderer for PrivateNotes {
    /// `config` is the host's document: the configuration, the kept data, when it was fetched
    /// and how the last update went.
    fn render(config: String) -> Result<String, String> {
        let document: Json = serde_json::from_str(&config)
            .map_err(|error| format!("the document is not valid JSON: {error}"))?;
        let last_update = document.get("last_update").cloned().unwrap_or(Json::Null);
        let content = match document.get("data") {
            Some(data) if !data.is_null() => {
                let notes = data.get("notes").cloned().unwrap_or(json!([]));
                let empty = notes.as_array().is_none_or(Vec::is_empty);
                json!({
                    "title": "Notes",
                    "text": if empty { Json::from("No notes yet") } else { Json::Null },
                    "items": notes,
                    "account": data.get("account"),
                    "fetched_at_ms": document.get("fetched_at_ms"),
                    "last_update": last_update,
                })
            }
            _ => json!({ "title": "Notes", "text": "No notes yet", "last_update": last_update }),
        };
        Ok(content.to_string())
    }
}

export!(PrivateNotes);
