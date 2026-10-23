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

// Plugin permissions (feature 002), User Story 1

/// Runs `args` followed by `--grants <grants>`.
fn runtime_with(grants: &Path, args: &[&str]) -> Output {
    let mut args = args.to_vec();
    args.extend(["--grants", path_str(grants)]);
    runtime(&args)
}

/// Writes the `quiet` fixture, packed with `manifest`, to `dir/name.wasm`.
fn packed_quiet(dir: &Path, name: &str, manifest: serde_json::Value) -> PathBuf {
    let package =
        runtime::package::pack(&common::fixture("quiet"), manifest.to_string().as_bytes())
            .expect("the manifest is valid");
    let path = dir.join(format!("{name}.wasm"));
    std::fs::write(&path, package).expect("written");
    path
}

fn quiet_manifest(author: &str, login_providers: &[&str]) -> serde_json::Value {
    json!({
        "manifest_version": 1,
        "id": "local/quiet",
        "version": "1.0.0",
        "author": author,
        "permissions": [],
        "login_providers": login_providers
    })
}

#[test]
fn run_before_approve_is_not_granted_and_names_log() {
    let (_dir, grants) = temp_grants();
    let plugin = fixture_path("hello-widget");
    let output = runtime_with(&grants, &["run", path_str(&plugin)]);

    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "");
    let stderr = stderr(&output);
    assert!(stderr.starts_with("error: not-granted: "), "{stderr}");
    assert!(stderr.contains("log"), "{stderr}");
}

#[test]
fn permissions_shows_what_is_requested_and_granted_and_changes_nothing() {
    let (_dir, grants) = temp_grants();
    let plugin = fixture_path("hello-widget");

    let output = runtime_with(&grants, &["permissions", path_str(&plugin)]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "plugin: local/hello-widget 0.2.0
author: Dashboard contributors
contract: v1
approved: no
permissions:
  log  required  not granted
login providers: none
"
    );
    assert!(!grants.exists(), "permissions wrote the record");

    approve_file(&grants, &plugin);
    let record = std::fs::read(&grants).expect("the record");
    let digest = runtime::package::digest(&std::fs::read(&plugin).expect("the plugin"));
    let output = runtime_with(&grants, &["permissions", path_str(&plugin)]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        format!(
            "plugin: local/hello-widget 0.2.0
author: Dashboard contributors
contract: v1
approved: version 0.2.0 ({digest}) from {}
permissions:
  log  required  granted
login providers: none
",
            origin(&plugin)
        )
    );
    assert_eq!(std::fs::read(&grants).expect("the record"), record);
}

#[test]
fn approve_grants_every_required_permission_then_run_works() {
    let (_dir, grants) = temp_grants();
    let plugin = fixture_path("hello-widget");

    let output = runtime_with(&grants, &["approve", path_str(&plugin)]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "approved local/hello-widget 0.2.0\ngranted: log\n"
    );

    let output = runtime_with(&grants, &["run", path_str(&plugin)]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
}

#[test]
fn approve_takes_no_permission() {
    let (_dir, grants) = temp_grants();
    let plugin = fixture_path("hello-widget");
    let output = runtime_with(&grants, &["approve", path_str(&plugin), "log"]);
    assert_eq!(output.status.code(), Some(2), "stderr: {}", stderr(&output));
    assert!(!grants.exists(), "a refused approve wrote the record");
}

#[test]
fn approve_refuses_a_package_the_checks_refuse_and_grants_nothing() {
    let (dir, grants) = temp_grants();
    let path = dir.path().join("not-wasm.wasm");
    std::fs::write(&path, b"not wasm").unwrap();
    let output = runtime_with(&grants, &["approve", path_str(&path)]);
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert!(stderr(&output).starts_with("error: invalid-package: "));
    assert!(!grants.exists(), "a refused approve wrote the record");
}

#[test]
fn package_text_is_escaped_when_shown() {
    let (dir, grants) = temp_grants();
    let plugin = packed_quiet(dir.path(), "quiet", quiet_manifest("\u{202e}evil", &[]));
    let output = runtime_with(&grants, &["permissions", path_str(&plugin)]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let stdout = stdout(&output);
    assert!(stdout.contains(r"author: \u{202e}evil"), "{stdout}");
    assert!(!stdout.contains('\u{202e}'), "{stdout}");
}

#[test]
fn login_providers_are_shown_and_give_nothing() {
    let (dir, grants) = temp_grants();
    let plugin = packed_quiet(dir.path(), "quiet", quiet_manifest("A", &["github"]));
    let output = runtime_with(&grants, &["permissions", path_str(&plugin)]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert!(
        stdout(&output)
            .contains("login providers: github (connecting accounts arrives in feature 005)\n"),
        "{}",
        stdout(&output)
    );
}

#[test]
fn a_corrupt_record_grants_nothing_and_stays_as_it_was() {
    let (_dir, grants) = temp_grants();
    std::fs::write(&grants, "{").unwrap();
    let plugin = fixture_path("hello-widget");

    let output = runtime_with(&grants, &["run", path_str(&plugin)]);
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    let stderr = stderr(&output);
    assert!(stderr.starts_with("error: not-granted: "), "{stderr}");
    assert!(stderr.contains(path_str(&grants)), "{stderr}");
    assert!(stderr.contains("not JSON"), "{stderr}");
    assert_eq!(std::fs::read_to_string(&grants).unwrap(), "{");
}

#[test]
fn a_copy_of_an_approved_file_gets_none_of_its_grants() {
    let (dir, grants) = temp_grants();
    let original = fixture_path("hello-widget");
    let copy = dir.path().join("hello-widget.wasm");
    std::fs::copy(&original, &copy).unwrap();
    approve_file(&grants, &original);

    let output = runtime_with(&grants, &["run", path_str(&copy)]);
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    let error = stderr(&output);
    assert!(error.starts_with("error: not-granted: "), "{error}");
    assert!(error.contains(&origin(&original)), "{error}");

    let output = runtime_with(&grants, &["permissions", path_str(&copy)]);
    let shown = stdout(&output);
    assert!(
        shown.contains(&format!(
            "approved: no (approved from {})\n",
            origin(&original)
        )),
        "{shown}"
    );
    assert!(shown.contains("  log  required  not granted\n"), "{shown}");

    let output = runtime_with(&grants, &["approve", path_str(&copy)]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert!(
        stdout(&output).starts_with(&format!(
            "replaced: approval from {}\napproved local/hello-widget 0.2.0\n",
            origin(&original)
        )),
        "{}",
        stdout(&output)
    );
    let output = runtime_with(&grants, &["run", path_str(&original)]);
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert!(stderr(&output).starts_with("error: not-granted: "));
}
