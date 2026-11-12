//! Example plugin for contract 1.3.0: shows notes read as the user's account. `update` calls the
//! development connector `local/notes` through `accounts`, so the host adds the account's
//! credential to the request itself; this widget never sees it. Without an account connected the
//! service answers with its public notes only, and `render` shows what the host kept, as content
//! in the host's format 1.

wit_bindgen::generate!({ path: "../../../wit/history/1.3.0", world: "account-widget" });

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
            Err(AccountCallError::NotGranted) => {
                return Err("local/notes is not granted".to_owned());
            }
            Err(AccountCallError::Refused(problem)) => {
                return Err(format!("the call was refused: {}", problem.detail));
            }
            Err(AccountCallError::Failed(problem)) => {
                return Err(format!("the call failed: {}", problem.detail));
            }
        };
        if response.status != 200 {
            return Err(format!(
                "the service answered with status {}",
                response.status
            ));
        }
        let notes: Json = serde_json::from_slice(&response.body).map_err(|error| {
            format!("the service answered with JSON this widget cannot read: {error}")
        })?;
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
    /// and how the last update went. The content is in the host's format 1: the notes as a list,
    /// the account's own in the accent tone, with a warning when the account needs the person.
    fn render(config: String) -> Result<String, String> {
        let document: Json = serde_json::from_str(&config)
            .map_err(|error| format!("the document is not valid JSON: {error}"))?;
        let mut body = vec![json!({ "type": "text", "text": "Notes", "variant": "heading" })];
        let data = document.get("data").unwrap_or(&Json::Null);
        let account = match data.get("account").and_then(Json::as_str) {
            Some("not connected") => Some("Connect your account to see your own notes"),
            Some("subscription required") => Some("Your notes need a paid plan"),
            _ => None,
        };
        if let Some(account) = account {
            body.push(json!({ "type": "text", "text": account, "tone": "warning" }));
        }
        let items: Vec<Json> = data
            .get("notes")
            .and_then(Json::as_array)
            .into_iter()
            .flatten()
            .map(|note| {
                let title = note.get("title").and_then(Json::as_str).unwrap_or_default();
                if note.get("private") == Some(&Json::Bool(true)) {
                    json!({ "text": title, "tone": "accent" })
                } else {
                    json!(title)
                }
            })
            .collect();
        if items.is_empty() {
            body.push(json!({ "type": "text", "text": "No notes yet", "variant": "muted" }));
        } else {
            body.push(json!({ "type": "list", "items": items }));
        }
        let last_update = document.get("last_update").unwrap_or(&Json::Null);
        if last_update.get("outcome").and_then(Json::as_str) == Some("failed") {
            let error = last_update
                .get("error")
                .and_then(Json::as_str)
                .unwrap_or("unknown error");
            body.push(json!({
                "type": "text",
                "text": format!("The last update failed: {error}"),
                "tone": "error",
            }));
        }
        Ok(json!({ "format": 1, "body": body }).to_string())
    }
}

export!(PrivateNotes);
