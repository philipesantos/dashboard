//! Test fixture: a contract 1.2.0 widget that makes the storage calls its configuration lists, in
//! `update` and in `render`, and returns every answer, so a hostile case sees exactly what the
//! host answered (specs/004-plugin-storage/research.md, R17). It can also end its update with an
//! error or a trap after the calls, return output that is not JSON or is of a given size, and
//! spin between calls so a test can change the grants while it runs.
//!
//! It returns `{"answers": [...], "more", "errors", "first_error", "last_error"}`: the first 32
//! answers, how many more there were, and the count, first and last of the error answers, so a
//! case that makes thousands of calls still sees every refusal.
//!
//! `update` reads `{"steps": [...], "end": "ok" | "error" | "panic" | "fuel" | "loop" | "memory",
//! "output": "not-json" | n}`; `render` reads `render_steps` and `render_end` from `config` of the
//! document the host gives it. A step is `{"op": "get" | "set" | "delete" | "list" | "usage",
//! "name", "value" | "value_hex" | "value_size", "prefix", "after", "repeat": n}` or
//! `{"op": "spin", "iterations": n}`; `{i}` in a name, value, prefix or `after` becomes the
//! repetition's index. `fuel` and `loop` both loop without end: the test's limits decide which
//! limit stops them.

wit_bindgen::generate!({ path: "../../../wit/history/1.2.0", world: "stored-widget" });

use dashboard::widget::storage::{self, Rule, StorageError};
use exports::dashboard::widget::renderer::Guest as Renderer;
use exports::dashboard::widget::updater::Guest as Updater;
use serde_json::{Value as Json, json};

/// Answers kept in the output; the rest are counted in `more`.
const KEPT_ANSWERS: usize = 32;

struct StoreProbe;

/// Every answer of a run, the first ones kept, and the first error.
#[derive(Default)]
struct Answers {
    kept: Vec<Json>,
    more: u64,
    first_error: Option<Json>,
    last_error: Option<Json>,
    errors: u64,
}

impl Answers {
    fn push(&mut self, answer: Json) {
        if answer.get("error").is_some() {
            self.errors += 1;
            if self.first_error.is_none() {
                self.first_error = Some(answer.clone());
            }
            self.last_error = Some(answer.clone());
        }
        if self.kept.len() < KEPT_ANSWERS {
            self.kept.push(answer);
        } else {
            self.more += 1;
        }
    }
}

/// Makes every step of `steps`, `repeat` times each.
fn run_steps(steps: Option<&Json>) -> Result<Answers, String> {
    let mut answers = Answers::default();
    for step in steps
        .and_then(Json::as_array)
        .map(Vec::as_slice)
        .unwrap_or_default()
    {
        let op = step
            .get("op")
            .and_then(Json::as_str)
            .ok_or("a step needs \"op\" as a string")?;
        if op == "spin" {
            let iterations = step.get("iterations").and_then(Json::as_u64).unwrap_or(0);
            for i in 0..iterations {
                std::hint::black_box(i);
            }
            continue;
        }
        let repeat = step.get("repeat").and_then(Json::as_u64).unwrap_or(1);
        for i in 0..repeat {
            let answer = call(op, step, i)?;
            answers.push(answer);
        }
    }
    Ok(answers)
}

/// The text field `field` of `step`, with `{i}` replaced by `index`.
fn text(step: &Json, field: &str, index: u64) -> Option<String> {
    step.get(field)
        .and_then(Json::as_str)
        .map(|text| text.replace("{i}", &index.to_string()))
}

fn value(step: &Json, index: u64) -> Result<Vec<u8>, String> {
    if let Some(text) = text(step, "value", index) {
        return Ok(text.into_bytes());
    }
    if let Some(hex) = step.get("value_hex").and_then(Json::as_str) {
        return from_hex(hex);
    }
    let size = step.get("value_size").and_then(Json::as_u64).unwrap_or(0);
    Ok(vec![b'x'; size as usize])
}

fn call(op: &str, step: &Json, index: u64) -> Result<Json, String> {
    let name = || text(step, "name", index).unwrap_or_default();
    let answer = match op {
        "get" => storage::get(&name())
            .map(|value| value.map_or(Json::Null, |bytes| json!({ "hex": to_hex(&bytes) }))),
        "set" => storage::set(&name(), &value(step, index)?).map(|()| Json::Null),
        "delete" => storage::delete(&name()).map(|()| Json::Null),
        "list" => storage::list(
            &text(step, "prefix", index).unwrap_or_default(),
            text(step, "after", index).as_deref(),
        )
        .map(|page| json!({ "names": page.names, "next": page.next })),
        "usage" => storage::usage()
            .map(|usage| json!({ "used": usage.used_bytes, "quota": usage.quota_bytes })),
        other => return Err(format!("unknown op {other}")),
    };
    Ok(match answer {
        Ok(value) => json!({ "ok": value }),
        Err(error) => error_json(&error),
    })
}

fn error_json(error: &StorageError) -> Json {
    match error {
        StorageError::NotGranted => json!({ "error": "not-granted" }),
        StorageError::Refused(problem) => {
            json!({ "error": format!("refused:{}", rule_name(problem.rule)), "detail": problem.detail })
        }
        StorageError::Failed(problem) => {
            json!({ "error": format!("failed:{}", rule_name(problem.rule)), "detail": problem.detail })
        }
    }
}

fn rule_name(rule: Rule) -> &'static str {
    match rule {
        Rule::RenderOnlyReads => "render-only-reads",
        Rule::OverQuota => "over-quota",
        Rule::OverLimit => "over-limit",
        Rule::InvalidName => "invalid-name",
        Rule::Damaged => "damaged",
        Rule::Busy => "busy",
        Rule::Unavailable => "unavailable",
        Rule::RunAborted => "run-aborted",
    }
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn from_hex(hex: &str) -> Result<Vec<u8>, String> {
    if !hex.len().is_multiple_of(2) {
        return Err("value_hex needs an even number of digits".to_owned());
    }
    (0..hex.len())
        .step_by(2)
        .map(|at| {
            u8::from_str_radix(&hex[at..at + 2], 16).map_err(|error| format!("value_hex: {error}"))
        })
        .collect()
}

/// A valid JSON document of exactly `size` bytes (at least 10): `{"pad":"xx…"}`.
fn sized_output(size: usize) -> String {
    let mut output = String::from("{\"pad\":\"\"}");
    let padding = size.saturating_sub(output.len());
    output.insert_str(output.len() - 2, &"x".repeat(padding));
    output
}

/// Ends the run as `end` says, after the steps: returns `output`, returns an error naming the
/// first error answer, panics, loops without end, or grows memory until the host refuses.
fn finish(end: Option<&str>, answers: &Answers, output: Json) -> Result<String, String> {
    match end.unwrap_or("ok") {
        "ok" => Ok(output.to_string()),
        "error" => Err(format!(
            "ending with an error as asked; first error answer: {}",
            answers
                .first_error
                .as_ref()
                .map_or_else(|| "none".to_owned(), Json::to_string)
        )),
        "panic" => panic!("ending with a panic as asked"),
        "fuel" | "loop" => loop {
            std::hint::black_box(());
        },
        "memory" => {
            let block = std::hint::black_box(vec![1u8; 1 << 30]);
            Ok(block.len().to_string())
        }
        other => Err(format!("unknown end {other}")),
    }
}

impl Updater for StoreProbe {
    fn update(config: String) -> Result<String, String> {
        let config: Json =
            serde_json::from_str(&config).map_err(|error| format!("configuration: {error}"))?;
        let answers = run_steps(config.get("steps"))?;
        let output = json!({
            "answers": answers.kept,
            "more": answers.more,
            "errors": answers.errors,
            "first_error": answers.first_error,
            "last_error": answers.last_error,
        });
        let end = config.get("end").and_then(Json::as_str);
        let result = finish(end, &answers, output)?;
        match config.get("output") {
            Some(Json::String(kind)) if kind == "not-json" => Ok("{".to_owned()),
            Some(Json::Number(size)) => Ok(sized_output(size.as_u64().unwrap_or(0) as usize)),
            _ => Ok(result),
        }
    }
}

impl Renderer for StoreProbe {
    fn render(config: String) -> Result<String, String> {
        let document: Json =
            serde_json::from_str(&config).map_err(|error| format!("configuration: {error}"))?;
        let widget = document.get("config");
        let answers = run_steps(widget.and_then(|config| config.get("render_steps")))?;
        let output = json!({
            "answers": answers.kept,
            "more": answers.more,
            "errors": answers.errors,
            "first_error": answers.first_error,
            "last_error": answers.last_error,
            "render_config": document,
        });
        let end = widget
            .and_then(|config| config.get("render_end"))
            .and_then(Json::as_str);
        finish(end, &answers, output)
    }
}

export!(StoreProbe);
