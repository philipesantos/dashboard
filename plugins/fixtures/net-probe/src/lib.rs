//! Test fixture: a contract 1.1.0 widget that makes, in `update`, the connector calls its
//! configuration lists and returns every answer, so a hostile case sees exactly what the host
//! answered (specs/003-plugin-connectors/research.md, R16). It can also call from `render`, fail
//! its update, return output that is not JSON or is of a given size, and report body sizes
//! instead of bodies.
//!
//! `update` reads `{"calls": [{"connector", "operation", "arguments": [[name, value], ...],
//! "repeat": n}], "fail": bool, "output": "not-json" | n, "bodies": "size"}`; `render` reads the
//! same list from `config.render_calls` of the document the host gives it.

wit_bindgen::generate!({ path: "../../../wit/history/1.1.0", world: "connected-widget" });

use dashboard::widget::connectors::{Argument, CallError, Response, Rule, Value, call};
use exports::dashboard::widget::renderer::Guest as Renderer;
use exports::dashboard::widget::updater::Guest as Updater;
use serde_json::{Map, Value as Json, json};
use sha2::{Digest, Sha256};

/// Answers kept in the output; the rest are counted in `more`.
const KEPT_ANSWERS: usize = 32;

struct NetProbe;

/// Makes every call of `calls`, `repeat` times each, and returns the answers and the first error.
fn make_calls(calls: Option<&Json>, sizes_only: bool) -> Result<(Json, Option<String>), String> {
    let mut answers = Vec::new();
    let mut more = 0u64;
    let mut first_error = None;
    for entry in calls
        .and_then(Json::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        let connector = text_field(entry, "connector")?;
        let operation = text_field(entry, "operation")?;
        let arguments = arguments(entry.get("arguments"))?;
        let repeat = entry.get("repeat").and_then(Json::as_u64).unwrap_or(1);
        for _ in 0..repeat {
            let answer = match call(connector, operation, &arguments) {
                Ok(response) => response_json(&response, sizes_only),
                Err(error) => {
                    let answer = error_json(&error);
                    if first_error.is_none() {
                        first_error = Some(answer.to_string());
                    }
                    answer
                }
            };
            if answers.len() < KEPT_ANSWERS {
                answers.push(answer);
            } else {
                more += 1;
            }
        }
    }
    Ok((json!({ "answers": answers, "more": more }), first_error))
}

fn text_field<'a>(entry: &'a Json, name: &str) -> Result<&'a str, String> {
    entry
        .get(name)
        .and_then(Json::as_str)
        .ok_or_else(|| format!("a call needs \"{name}\" as a string"))
}

/// `[[name, value], ...]`, so a test can repeat a name; JSON strings are text, integers are
/// integers and booleans are flags.
fn arguments(list: Option<&Json>) -> Result<Vec<Argument>, String> {
    let mut arguments = Vec::new();
    for pair in list
        .and_then(Json::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        let (Some(name), Some(value)) = (pair.get(0).and_then(Json::as_str), pair.get(1)) else {
            return Err("an argument is [name, value]".to_owned());
        };
        let value = match value {
            Json::String(text) => Value::Text(text.clone()),
            Json::Bool(flag) => Value::Flag(*flag),
            Json::Number(number) => Value::Integer(
                number
                    .as_i64()
                    .ok_or_else(|| format!("{name}: an integer argument must fit in 64 bits"))?,
            ),
            _ => {
                return Err(format!(
                    "{name}: a value is a string, an integer or a boolean"
                ));
            }
        };
        arguments.push(Argument {
            name: name.to_owned(),
            value,
        });
    }
    Ok(arguments)
}

fn response_json(response: &Response, sizes_only: bool) -> Json {
    let headers: Vec<Json> = response
        .headers
        .iter()
        .map(|header| json!({ "name": header.name, "value": header.value }))
        .collect();
    let mut answer = Map::new();
    answer.insert("status".to_owned(), json!(response.status));
    answer.insert("headers".to_owned(), Json::Array(headers));
    if sizes_only {
        let digest: String = Sha256::digest(&response.body)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        answer.insert("body_bytes".to_owned(), json!(response.body.len()));
        answer.insert("body_sha256".to_owned(), json!(digest));
    } else {
        answer.insert(
            "body".to_owned(),
            json!(String::from_utf8_lossy(&response.body)),
        );
    }
    Json::Object(answer)
}

fn error_json(error: &CallError) -> Json {
    match error {
        CallError::NotGranted => json!({ "error": "not-granted" }),
        CallError::Refused(problem) => {
            json!({ "error": format!("refused:{}", rule_name(problem.rule)), "detail": problem.detail })
        }
        CallError::Failed(problem) => {
            json!({ "error": format!("failed:{}", rule_name(problem.rule)), "detail": problem.detail })
        }
    }
}

fn rule_name(rule: Rule) -> &'static str {
    match rule {
        Rule::OperationNotDeclared => "operation-not-declared",
        Rule::ValueOutsideType => "value-outside-type",
        Rule::CalledOutsideUpdate => "called-outside-update",
        Rule::ChangeNeedsUserAction => "change-needs-user-action",
        Rule::OverLimit => "over-limit",
        Rule::NonPublicAddress => "non-public-address",
        Rule::RedirectNotAllowed => "redirect-not-allowed",
        Rule::Unavailable => "unavailable",
        Rule::InvalidResponse => "invalid-response",
    }
}

/// A valid JSON document of exactly `size` bytes (at least 10): `{"pad":"xx…"}`.
fn sized_output(size: usize) -> String {
    let mut output = String::from("{\"pad\":\"\"}");
    let padding = size.saturating_sub(output.len());
    output.insert_str(output.len() - 2, &"x".repeat(padding));
    output
}

impl Updater for NetProbe {
    fn update(config: String) -> Result<String, String> {
        let config: Json =
            serde_json::from_str(&config).map_err(|error| format!("configuration: {error}"))?;
        let sizes_only = config.get("bodies").and_then(Json::as_str) == Some("size");
        let (answers, first_error) = make_calls(config.get("calls"), sizes_only)?;
        if config.get("fail").and_then(Json::as_bool) == Some(true)
            && let Some(error) = first_error
        {
            return Err(format!("a call did not succeed: {error}"));
        }
        match config.get("output") {
            Some(Json::String(kind)) if kind == "not-json" => Ok("{".to_owned()),
            Some(Json::Number(size)) => Ok(sized_output(size.as_u64().unwrap_or(0) as usize)),
            _ => Ok(answers.to_string()),
        }
    }
}

impl Renderer for NetProbe {
    fn render(config: String) -> Result<String, String> {
        let document: Json =
            serde_json::from_str(&config).map_err(|error| format!("configuration: {error}"))?;
        match document
            .get("config")
            .and_then(|config| config.get("render_calls"))
        {
            Some(calls) => Ok(make_calls(Some(calls), false)?.0.to_string()),
            None => Ok(config),
        }
    }
}

export!(NetProbe);
