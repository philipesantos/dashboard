//! Example plugin for contract 1.1.0: shows the latest releases of the GitHub repository in its
//! configuration. `update` fetches them through the catalog's GitHub connector, and `render`
//! shows what the host kept, without waiting on the network.

wit_bindgen::generate!({ path: "../../../wit/history/1.1.0", world: "connected-widget" });

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

/// `ms` since 1970 as `YYYY-MM-DD HH:MM UTC`, in the proleptic Gregorian calendar.
fn utc(ms: u64) -> String {
    let minutes = ms / 60_000;
    let (days, minute) = (minutes / 1440, minutes % 1440);
    // Days since 1970-01-01 to a civil date (Howard Hinnant's `civil_from_days`)
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 {
        shifted_month + 3
    } else {
        shifted_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        minute / 60,
        minute % 60
    )
}

/// A text of the release, or an empty cell when GitHub sent none.
fn cell(release: &Json, field: &str) -> String {
    release
        .get(field)
        .and_then(Json::as_str)
        .unwrap_or_default()
        .to_owned()
}

impl Renderer for GithubReleases {
    /// `config` is the host's document: the configuration, the kept data, when it was fetched
    /// and how the last update went. The content is in the host's format 1: a heading, the
    /// releases as a table, when they were fetched, and the last update's error, if it failed.
    fn render(config: String) -> Result<String, String> {
        let document: Json = serde_json::from_str(&config)
            .map_err(|error| format!("the document is not valid JSON: {error}"))?;
        let (owner, repo) = repository(document.get("config").unwrap_or(&Json::Null))?;
        let mut body = vec![json!({
            "type": "text",
            "text": format!("{owner}/{repo} releases"),
            "variant": "heading",
        })];
        match document.get("data").filter(|data| !data.is_null()) {
            Some(data) => {
                let releases = data
                    .get("releases")
                    .and_then(Json::as_array)
                    .cloned()
                    .unwrap_or_default();
                if releases.is_empty() {
                    body.push(
                        json!({ "type": "text", "text": "No releases yet", "variant": "muted" }),
                    );
                } else {
                    // Every row has as many cells as there are columns
                    let rows: Vec<Json> = releases
                        .iter()
                        .map(|release| {
                            let published = cell(release, "published_at");
                            let date = published.split('T').next().unwrap_or_default();
                            json!([cell(release, "name"), cell(release, "tag"), date])
                        })
                        .collect();
                    body.push(json!({
                        "type": "table",
                        "columns": ["Release", "Tag", "Published"],
                        "rows": rows,
                    }));
                }
                if let Some(fetched) = document.get("fetched_at_ms").and_then(Json::as_u64) {
                    body.push(json!({
                        "type": "text",
                        "text": format!("Fetched {}", utc(fetched)),
                        "variant": "muted",
                    }));
                }
            }
            None => body.push(json!({ "type": "text", "text": "No data yet", "variant": "muted" })),
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

export!(GithubReleases);
