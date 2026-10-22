//! The command-line test tool, run as a separate process: output streams and exit codes as in
//! `specs/001-plugin-runtime/contracts/cli.md`.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::fixture_path;
use serde_json::json;

fn runtime(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_runtime"))
        .args(args)
        .output()
        .expect("the runtime binary starts")
}

fn path_str(path: &Path) -> &str {
    path.to_str().expect("test paths are UTF-8")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is UTF-8")
}

#[test]
fn run_prints_the_content_on_stdout_and_the_logs_on_stderr() {
    let plugin = fixture_path("hello-widget");
    let output = runtime(&["run", path_str(&plugin)]);

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let content: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("stdout is the widget content as JSON");
    assert_eq!(content, json!({"title": "Hello", "text": "Hello, world!"}));
    assert_eq!(stderr(&output), "[plugin] info: rendering hello-widget\n");
}

#[test]
fn a_refused_plugin_exits_1_with_its_error_kind_and_no_content() {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cli-not-wasm.wasm");
    std::fs::write(&path, b"not wasm").unwrap();

    let output = runtime(&["run", path_str(&path)]);

    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "");
    assert!(
        stderr(&output).starts_with("error: invalid-package: "),
        "stderr: {}",
        stderr(&output)
    );
}

#[test]
fn a_missing_plugin_file_is_a_usage_error() {
    let path = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("cli-no-such-plugin.wasm");
    let output = runtime(&["run", path_str(&path)]);
    assert_eq!(output.status.code(), Some(2), "stderr: {}", stderr(&output));
}

#[test]
fn an_invalid_configuration_is_a_usage_error() {
    let plugin = fixture_path("hello-widget");
    let output = runtime(&["run", path_str(&plugin), "--config", "{bad"]);
    assert_eq!(output.status.code(), Some(2), "stderr: {}", stderr(&output));
}

#[test]
fn an_unknown_command_is_a_usage_error() {
    let output = runtime(&["explode"]);
    assert_eq!(output.status.code(), Some(2), "stderr: {}", stderr(&output));
}
