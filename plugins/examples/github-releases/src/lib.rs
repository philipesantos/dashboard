//! Example plugin for contract 1.1.0: shows the latest releases of the GitHub repository in its
//! configuration. `update` fetches them through the catalog's GitHub connector, and `render`
//! shows what the host kept, without waiting on the network.

wit_bindgen::generate!({ path: "../../../wit/v1", world: "connected-widget" });

use dashboard::widget::connectors::{Argument, CallError, Value, call};
use exports::dashboard::widget::renderer::Guest as Renderer;
use exports::dashboard::widget::updater::Guest as Updater;
use serde_json::{Value as Json, json};

/// How many releases the widget shows.
const RELEASES: i64 = 5;

struct GithubReleases;

/// `owner` and `repo` from the widget's configuration.
fn repository(config: &Json) -> Result<(&str, &str), String> {
    let field = |name: &str| {
        config
            .get(name)
            .and_then(Json::as_str)
            .ok_or_else(|| format!(r#""{name}" in the configuration must be a string"#))
    };
    Ok((field("owner")?, field("repo")?))
}

impl Updater for GithubReleases {
    /// Calls `list-releases` once. Any answer other than a 200 with a JSON list is an error, so
    /// the host keeps the previous data and render shows that the update failed.
    fn update(config: String) -> Result<String, String> {
        let config: Json = serde_json::from_str(&config)
            .map_err(|error| format!("the configuration is not valid JSON: {error}"))?;
        let (owner, repo) = repository(&config)?;
        let arguments = [
            Argument {
                name: "owner".to_owned(),
                value: Value::Text(owner.to_owned()),
            },
            Argument {
                name: "repo".to_owned(),
                value: Value::Text(repo.to_owned()),
            },
            Argument {
                name: "per_page".to_owned(),
                value: Value::Integer(RELEASES),
            },
        ];
        let response =
            call("catalog/github", "list-releases", &arguments).map_err(|error| match error {
                CallError::NotGranted => "catalog/github is not granted".to_owned(),
                CallError::Refused(problem) => format!("the call was refused: {}", problem.detail),
                CallError::Failed(problem) => format!("the call failed: {}", problem.detail),
            })?;
        if response.status != 200 {
            return Err(format!("GitHub answered with status {}", response.status));
        }
        let releases: Json = serde_json::from_slice(&response.body).map_err(|error| {
            format!("GitHub answered with JSON this widget cannot read: {error}")
        })?;
        let releases: Vec<Json> = releases
            .as_array()
            .ok_or("GitHub answered with something other than a list of releases")?
            .iter()
            .map(|release| {
                json!({
                    "name": release.get("name"),
                    "tag": release.get("tag_name"),
                    "published_at": release.get("published_at"),
                    "url": release.get("html_url"),
                })
            })
            .collect();
        Ok(json!({ "releases": releases }).to_string())
    }
}

impl Renderer for GithubReleases {
    /// `config` is the host's document: the configuration, the kept data, when it was fetched
    /// and how the last update went.
    fn render(config: String) -> Result<String, String> {
        let document: Json = serde_json::from_str(&config)
            .map_err(|error| format!("the document is not valid JSON: {error}"))?;
        let (owner, repo) = repository(document.get("config").unwrap_or(&Json::Null))?;
        let title = format!("{owner}/{repo} releases");
        let last_update = document.get("last_update").cloned().unwrap_or(Json::Null);
        let content = match document.get("data") {
            Some(data) if !data.is_null() => json!({
                "title": title,
                "items": data.get("releases").cloned().unwrap_or(json!([])),
                "fetched_at_ms": document.get("fetched_at_ms"),
                "last_update": last_update,
            }),
            _ => json!({ "title": title, "text": "No data yet", "last_update": last_update }),
        };
        Ok(content.to_string())
    }
}

export!(GithubReleases);
