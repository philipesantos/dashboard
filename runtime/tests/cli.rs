//! The command-line test tool, run as a separate process: output streams and exit codes as in
//! `specs/001-plugin-runtime/contracts/cli.md` and `specs/002-plugin-permissions/contracts/cli.md`.
//! Every test passes `--grants` with a record in a temporary directory, so none touches the
//! user's own record.

mod common;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use common::{approve, fixture_path, temp_grants};
use runtime::grants::GrantStore;
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

/// The origin the CLI gives a package: its file's canonical path.
fn origin(plugin: &Path) -> String {
    std::fs::canonicalize(plugin)
        .expect("the plugin file exists")
        .into_os_string()
        .into_string()
        .expect("test paths are UTF-8")
}

/// Approves the plugin file's required permissions in the record at `grants`, as the CLI's
/// `approve` would.
fn approve_file(grants: &Path, plugin: &Path) {
    let bytes = std::fs::read(plugin).expect("the plugin file is readable");
    approve(&GrantStore::open(grants), &bytes, &origin(plugin));
}

/// Runs `args` with `--grants` set to a fresh record where `plugin` is approved, if one is
/// given.
fn runtime_approved(plugin: Option<&Path>, args: &[&str]) -> Output {
    let (_dir, grants) = temp_grants();
    if let Some(plugin) = plugin {
        approve_file(&grants, plugin);
    }
    let mut args = args.to_vec();
    args.extend(["--grants", path_str(&grants)]);
    runtime(&args)
}

#[test]
fn run_prints_the_content_on_stdout_and_the_logs_on_stderr() {
    let plugin = fixture_path("hello-widget");
    let output = runtime_approved(Some(&plugin), &["run", path_str(&plugin)]);

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

    let output = runtime_approved(None, &["run", path_str(&path)]);

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
    let output = runtime_approved(None, &["run", path_str(&path)]);
    assert_eq!(output.status.code(), Some(2), "stderr: {}", stderr(&output));
}

#[test]
fn an_invalid_configuration_is_a_usage_error() {
    let plugin = fixture_path("hello-widget");
    let output = runtime_approved(
        Some(&plugin),
        &["run", path_str(&plugin), "--config", "{bad"],
    );
    assert_eq!(output.status.code(), Some(2), "stderr: {}", stderr(&output));
}

#[test]
fn plugin_text_cannot_add_lines_or_control_the_terminal() {
    let plugin = fixture_path("log-spoof");
    let output = runtime_approved(Some(&plugin), &["run", path_str(&plugin)]);

    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr(&output);
    let lines: Vec<&str> = stderr.lines().collect();
    assert_eq!(lines.len(), 4, "stderr: {stderr}");
    assert!(lines[..3].iter().all(|line| line.starts_with("[plugin] ")));
    assert!(lines[3].starts_with("error: plugin-error: "));
    assert!(
        !stderr.contains('\u{1b}'),
        "stderr holds an escape character"
    );
}

#[test]
fn bench_prints_the_load_time_and_render_percentiles() {
    let plugin = fixture_path("hello-widget");
    let output = runtime_approved(
        Some(&plugin),
        &["bench", path_str(&plugin), "--iterations", "20"],
    );

    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let stdout = stdout(&output);
    let keys: Vec<&str> = stdout
        .lines()
        .map(|line| {
            let (key, value) = line.split_once(": ").expect("each line is `key: value`");
            value
                .parse::<f64>()
                .unwrap_or_else(|_| panic!("{key} is not a number: {value}"));
            key
        })
        .collect();
    assert_eq!(
        keys,
        ["load_ms", "render_p50_us", "render_p95_us", "render_max_us"]
    );
}

#[test]
fn bench_needs_at_least_one_iteration() {
    let plugin = fixture_path("hello-widget");
    let output = runtime_approved(
        Some(&plugin),
        &["bench", path_str(&plugin), "--iterations", "0"],
    );
    assert_eq!(output.status.code(), Some(2), "stderr: {}", stderr(&output));
}

#[test]
fn a_failing_render_stops_the_bench() {
    let plugin = fixture_path("malformed-output");
    let output = runtime_approved(
        Some(&plugin),
        &["bench", path_str(&plugin), "--iterations", "3"],
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).starts_with("error: invalid-output: "));
}

#[test]
fn an_unknown_command_is_a_usage_error() {
    let output = runtime(&["explode"]);
    assert_eq!(output.status.code(), Some(2), "stderr: {}", stderr(&output));
}
