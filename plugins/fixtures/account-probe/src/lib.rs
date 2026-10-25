//! Test fixture: a contract 1.3.0 widget that makes, in `update`, the calls its configuration
//! lists, through `accounts` or `connectors`, and returns every answer, so a hostile case sees
//! exactly what the host answered (specs/005-plugin-credentials/research.md, R16). With `dump`
//! it returns everything it was given instead, for the scan that looks for credentials.
//!
//! `update` reads `{"calls": [{"via": "accounts" | "connectors", "connector", "operation",
//! "arguments": [[name, value], ...], "repeat": n}], "dump": bool, "fail": bool}`; `render`
//! returns the document the host gives it, kept data included.

wit_bindgen::generate!({ path: "../../../wit/v1", world: "account-widget" });

use dashboard::widget::accounts::{self, AccountCallError};
use dashboard::widget::connectors::{self, Argument, CallError, Problem, Response, Rule, Value};
use exports::dashboard::widget::renderer::Guest as Renderer;
use exports::dashboard::widget::updater::Guest as Updater;
use serde_json::{Value as Json, json};

/// Answers kept in the output; the rest are counted in `more`.
const KEPT_ANSWERS: usize = 32;

struct AccountProbe;

/// Makes every call of `calls`, `repeat` times each, and returns the answers and the first error.
fn make_calls(calls: Option<&Json>) -> Result<(Json, Option<String>), String> {
    let mut answers = Vec::new();
    let mut more = 0u64;
    let mut first_error = None;
    for entry in calls
        .and_then(Json::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        let via = entry.get("via").and_then(Json::as_str).unwrap_or("accounts");
        let connector = text_field(entry, "connector")?;
        let operation = text_field(entry, "operation")?;
        let arguments = arguments(entry.get("arguments"))?;
        let repeat = entry.get("repeat").and_then(Json::as_u64).unwrap_or(1);
        for _ in 0..repeat {
            let answer = match via {
                "connectors" => match connectors::call(connector, operation, &arguments) {
                    Ok(response) => response_json(&response),
                    Err(error) => call_error_json(&error),
                },
                "accounts" => match accounts::call(connector, operation, &arguments) {
                    Ok(response) => response_json(&response),
                    Err(error) => account_error_json(&error),
                },
                other => return Err(format!("\"via\" is accounts or connectors, not {other}")),
            };
            if answer.get("error").is_some() && first_error.is_none() {
                first_error = Some(answer.to_string());
            }
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

/// `[[name, value], ...]`: JSON strings are text, integers are integers and booleans are flags.
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

fn response_json(response: &Response) -> Json {
    let headers: Vec<Json> = response
        .headers
        .iter()
        .map(|header| json!({ "name": header.name, "value": header.value }))
        .collect();
    json!({
        "ok": {
            "status": response.status,
            "headers": headers,
            "body_text": String::from_utf8_lossy(&response.body),
        }
    })
}

fn problem_json(kind: &str, problem: &Problem) -> Json {
    json!({ "error": format!("{kind}:{}", rule_name(problem.rule)), "detail": problem.detail })
}

fn call_error_json(error: &CallError) -> Json {
    match error {
        CallError::NotGranted => json!({ "error": "not-granted" }),
        CallError::Refused(problem) => problem_json("refused", problem),
        CallError::Failed(problem) => problem_json("failed", problem),
    }
}

fn account_error_json(error: &AccountCallError) -> Json {
    match error {
        AccountCallError::NotGranted => json!({ "error": "not-granted" }),
        AccountCallError::Refused(problem) => problem_json("refused", problem),
        AccountCallError::Failed(problem) => problem_json("failed", problem),
        AccountCallError::AccountNotConnected => json!({ "error": "account-not-connected" }),
        AccountCallError::SubscriptionRequired => json!({ "error": "subscription-required" }),
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

impl Updater for AccountProbe {
    fn update(config: String) -> Result<String, String> {
        let parsed: Json =
            serde_json::from_str(&config).map_err(|error| format!("configuration: {error}"))?;
        if parsed.get("dump").and_then(Json::as_bool) == Some(true) {
            // Everything the host gave this widget, which is all a plugin can see
            return Ok(json!({ "dump": config }).to_string());
        }
        let (answers, first_error) = make_calls(parsed.get("calls"))?;
        if parsed.get("fail").and_then(Json::as_bool) == Some(true)
            && let Some(error) = first_error
        {
            return Err(format!("a call did not succeed: {error}"));
        }
        Ok(answers.to_string())
    }
}

impl Renderer for AccountProbe {
    fn render(config: String) -> Result<String, String> {
        Ok(config)
    }
}

export!(AccountProbe);
