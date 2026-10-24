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
use tempfile::TempDir;

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
    runtime_with(&grants, args)
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
        [
            "load_ms",
            "render_p50_us",
            "render_p95_us",
            "render_max_us",
            "checks_us",
            "relink_us"
        ]
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

/// Runs `args` followed by `--grants <grants>` and, so no test touches the user's own data,
/// `--data` with the record's directory.
fn runtime_with(grants: &Path, args: &[&str]) -> Output {
    let mut args = args.to_vec();
    let data = grants.parent().expect("the record is in a directory");
    args.extend(["--grants", path_str(grants), "--data", path_str(data)]);
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
contract: v1 (1.0.0)
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
contract: v1 (1.0.0)
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

// User Story 3

#[test]
fn remove_deletes_the_approval_and_run_is_then_refused() {
    let (_dir, grants) = temp_grants();
    let plugin = fixture_path("hello-widget");
    approve_file(&grants, &plugin);

    let output = runtime_with(&grants, &["remove", "local/hello-widget"]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "removed local/hello-widget\n");

    let output = runtime_with(&grants, &["run", path_str(&plugin)]);
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert!(stderr(&output).starts_with("error: not-granted: "));
}

#[test]
fn removing_a_plugin_without_approval_says_so() {
    let (_dir, grants) = temp_grants();
    let output = runtime_with(&grants, &["remove", "local/x"]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "local/x has no approval\n");
}

#[test]
fn remove_needs_a_well_formed_id() {
    let (_dir, grants) = temp_grants();
    for id in ["Local/X", "hello-widget", "local/"] {
        let output = runtime_with(&grants, &["remove", id]);
        assert_eq!(output.status.code(), Some(2), "{id}: {}", stderr(&output));
    }
}

// User Story 4

/// Writes the `probe` fixture, packed with `log` optional or required, to `dir/probe.wasm`.
fn packed_probe(dir: &Path, log_required: bool) -> PathBuf {
    let manifest = common::manifest("local/probe", "1.0.0", &[("log", log_required)]);
    let package = runtime::package::pack(&common::fixture("probe"), manifest.as_bytes())
        .expect("the manifest is valid");
    let path = dir.join("probe.wasm");
    std::fs::write(&path, package).expect("written");
    path
}

#[test]
fn optional_turns_a_permission_on_and_off_for_the_next_run() {
    let (dir, grants) = temp_grants();
    let plugin = packed_probe(dir.path(), false);
    approve_file(&grants, &plugin);
    let says = |grants: &Path| {
        let output = runtime_with(grants, &["run", path_str(&plugin)]);
        assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
        serde_json::from_str::<serde_json::Value>(&stdout(&output)).expect("JSON")["log"].clone()
    };
    assert_eq!(says(&grants), "not-granted");

    let output = runtime_with(&grants, &["optional", path_str(&plugin), "log", "on"]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "log on\n");
    assert_eq!(says(&grants), "ok");

    let output = runtime_with(&grants, &["optional", path_str(&plugin), "log", "off"]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "log off\n");
    assert_eq!(says(&grants), "not-granted");
}

#[test]
fn optional_refuses_what_is_not_an_optional_permission_of_an_approved_v1_plugin() {
    let (dir, grants) = temp_grants();
    let optional = packed_probe(dir.path(), false);
    let refusal = |plugin: &Path, permission: &str| {
        let output = runtime_with(&grants, &["optional", path_str(plugin), permission, "on"]);
        assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
        stderr(&output)
    };

    // No approval
    assert!(refusal(&optional, "log").starts_with("error: not-granted: "));

    // An approval of another file
    let copy = dir.path().join("copy.wasm");
    std::fs::copy(&optional, &copy).unwrap();
    approve_file(&grants, &copy);
    assert!(refusal(&optional, "log").starts_with("error: not-granted: "));

    // A required permission
    let other = TempDir::new().unwrap();
    let required = packed_probe(other.path(), true);
    approve_file(&grants, &required);
    assert!(refusal(&required, "log").starts_with("error: required-together: "));

    // A permission the manifest does not declare
    let quiet = packed_quiet(dir.path(), "quiet", quiet_manifest("A", &[]));
    approve_file(&grants, &quiet);
    assert!(refusal(&quiet, "log").starts_with("error: not-declared: "));

    // A permission the host does not know
    assert!(refusal(&quiet, "telepathy").starts_with("error: unknown-permission: "));

    // A contract v0 plugin
    let v0 = fixture_path("hello-v0");
    approve_file(&grants, &v0);
    assert!(refusal(&v0, "log").starts_with("error: optional-needs-v1: "));
}

#[test]
fn optional_takes_on_or_off() {
    let (dir, grants) = temp_grants();
    let plugin = packed_probe(dir.path(), false);
    for args in [
        vec!["optional", path_str(&plugin), "log"],
        vec!["optional", path_str(&plugin), "log", "yes"],
        vec!["optional", path_str(&plugin)],
    ] {
        let output = runtime_with(&grants, &args);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            stderr(&output)
        );
    }
}

// User Story 5

/// Writes `manifest` to `dir/manifest.json` and returns its path.
fn manifest_file(dir: &Path, manifest: &str) -> PathBuf {
    let path = dir.join("manifest.json");
    std::fs::write(&path, manifest).expect("written");
    path
}

#[test]
fn pack_writes_a_package_that_loads_once_approved() {
    let (dir, grants) = temp_grants();
    let manifest = manifest_file(
        dir.path(),
        &common::manifest("local/probe", "1.0.0", &[("log", true)]),
    );
    let output_path = dir.path().join("probe.wasm");
    let probe = fixture_path("probe");

    // No --grants: packing touches no record
    let output = runtime(&[
        "pack",
        path_str(&probe),
        "--manifest",
        path_str(&manifest),
        "--output",
        path_str(&output_path),
    ]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let package = std::fs::read(&output_path).expect("the package was written");
    assert_eq!(
        stdout(&output),
        format!(
            "packed {}\ndigest: {}\n",
            path_str(&output_path),
            runtime::package::digest(&package)
        )
    );

    let output = runtime_with(&grants, &["approve", path_str(&output_path)]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let output = runtime_with(&grants, &["run", path_str(&output_path)]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert!(
        stdout(&output).contains(r#""log": "ok""#),
        "{}",
        stdout(&output)
    );
}

#[test]
fn pack_refuses_a_component_that_has_a_manifest_or_a_bad_manifest() {
    let dir = TempDir::new().unwrap();
    let output_path = dir.path().join("out.wasm");
    let pack = |component: &Path, manifest: &str| {
        let manifest = manifest_file(dir.path(), manifest);
        let output = runtime(&[
            "pack",
            path_str(component),
            "--manifest",
            path_str(&manifest),
            "--output",
            path_str(&output_path),
        ]);
        assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
        stderr(&output)
    };
    let valid = common::manifest("local/probe", "1.0.0", &[]);

    let error = pack(&fixture_path("hello-widget"), &valid);
    assert!(error.starts_with("error: manifest-mismatch: "), "{error}");

    let error = pack(
        &fixture_path("probe"),
        &valid.replacen('{', r#"{"homepage":"x","#, 1),
    );
    assert!(error.starts_with("error: invalid-manifest: "), "{error}");
    assert!(error.contains("homepage"), "{error}");

    let error = pack(
        &fixture_path("probe"),
        &common::manifest("local/probe", "1.0.0", &[("telepathy", true)]),
    );
    assert!(error.starts_with("error: unknown-permission: "), "{error}");
    assert!(error.contains("telepathy"), "{error}");

    assert!(!output_path.exists(), "a refused pack wrote its output");
}

#[test]
fn pack_needs_its_options_and_takes_no_grant_record() {
    let probe = fixture_path("probe");
    for args in [
        vec!["pack", path_str(&probe)],
        vec!["pack", path_str(&probe), "--manifest", "m.json"],
        vec!["pack", path_str(&probe), "--output", "o.wasm"],
        vec![
            "pack",
            path_str(&probe),
            "--manifest",
            "m.json",
            "--output",
            "o.wasm",
            "--grants",
            "g.json",
        ],
    ] {
        let output = runtime(&args);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            stderr(&output)
        );
    }
}

// Connectors (specs/003-plugin-connectors/contracts/cli.md). Every command gets --grants and
// --data in a temporary directory, and local connectors reach a test service on loopback.

use common::servers::{Reply, Server};

/// A data directory with `local/hello` on `server`, and `net-probe` packed with a manifest
/// declaring `connectors`.
struct Connected {
    dir: TempDir,
    grants: PathBuf,
    plugin: PathBuf,
}

impl Connected {
    fn new(server: &Server, connectors: &[(&str, u16, &[&str], bool)]) -> Connected {
        let dir = TempDir::new().unwrap();
        let connectors_dir = dir.path().join("connectors");
        std::fs::create_dir_all(&connectors_dir).unwrap();
        std::fs::write(connectors_dir.join("hello.json"), hello(server).to_string()).unwrap();
        let plugin = dir.path().join("net-probe.wasm");
        std::fs::write(
            &plugin,
            common::probe(&common::connected_manifest("local/net-probe", connectors)),
        )
        .unwrap();
        let grants = dir.path().join("grants.json");
        Connected {
            dir,
            grants,
            plugin,
        }
    }

    fn hello_only(server: &Server) -> Connected {
        Connected::new(server, &[("local/hello", 0, &["get"], true)])
    }

    fn run(&self, args: &[&str]) -> Output {
        let mut args = args.to_vec();
        args.extend([
            "--grants",
            path_str(&self.grants),
            "--data",
            path_str(self.dir.path()),
        ]);
        runtime(&args)
    }

    fn connector_file(&self) -> PathBuf {
        std::fs::canonicalize(self.dir.path().join("connectors").join("hello.json")).unwrap()
    }

    fn digest(&self) -> String {
        let output = self.run(&["permissions", path_str(&self.plugin)]);
        stdout(&output)
            .lines()
            .find_map(|line| line.strip_prefix("approval digest: "))
            .unwrap_or_else(|| panic!("no digest in {}", stdout(&output)))
            .to_owned()
    }

    fn approve(&self) {
        let digest = self.digest();
        let output = self.run(&["approve", path_str(&self.plugin), "--expect", &digest]);
        assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    }
}

/// `local/hello`: a read `get` of `/hello.json`, with a query parameter and fixed values.
fn hello(server: &Server) -> serde_json::Value {
    json!({
        "connector_version": 1,
        "id": "local/hello",
        "version": "0.1.0",
        "author": "You",
        "description": "A file served on this machine",
        "destinations": { "here": server.http_origin() },
        "operations": {
            "get": {
                "kind": "read", "method": "GET", "destination": "here", "path": "/hello.json",
                "query": { "lang": "{lang}", "v": "2" },
                "headers": { "Accept": "application/json" },
                "parameters": { "lang": { "type": "text", "max_length": 8, "required": false } },
                "response": { "format": "json" }
            },
            "put": {
                "kind": "change", "method": "PUT", "destination": "here", "path": "/hello.json",
                "response": { "format": "json" }
            }
        }
    })
}

fn hello_server() -> Server {
    let server = Server::http();
    server.route("/hello.json", Reply::json(r#"{"hello":"world"}"#));
    server
}

#[test]
fn permissions_shows_each_connector_its_destinations_and_operations_and_changes_nothing() {
    let server = hello_server();
    let connected = Connected::new(&server, &[("local/hello", 0, &["get", "put"], true)]);
    let output = connected.run(&["permissions", path_str(&connected.plugin)]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let origin = server.http_origin();
    let text = stdout(&output);
    let digest = text
        .lines()
        .find_map(|line| line.strip_prefix("approval digest: "))
        .expect("a digest");
    assert!(
        digest.starts_with("sha256:") && digest.len() == 71,
        "{digest}"
    );
    assert_eq!(
        text,
        format!(
            "plugin: local/net-probe 0.1.0
author: Dashboard contributors
contract: v1 (1.1.0)
approved: no
permissions: none
connectors:
  local/hello 0.1.0  required  not granted
    from: local file {} (development)
    destinations: {origin}
    get  read (declared by the developer)  GET {origin}/hello.json
      sends: lang (text, optional)
      fixed: v=2, Accept: application/json
    put  change (declared by the developer)  PUT {origin}/hello.json
      sends: nothing
login providers: none
approval digest: {digest}
",
            connected.connector_file().display()
        )
    );
    assert!(!connected.grants.exists(), "permissions wrote the record");
    assert!(server.received().is_empty());
}

#[test]
fn permissions_says_when_data_from_this_machine_can_reach_another_destination() {
    let server = hello_server();
    let connected = Connected::new(
        &server,
        &[
            ("local/hello", 0, &["get"], true),
            ("catalog/github", 1, &["list-releases"], false),
        ],
    );
    let text = stdout(&connected.run(&["permissions", path_str(&connected.plugin)]));
    let flow = format!(
        "data flow: data read from this machine ({}) can reach https://api.github.com\nlogin providers: none\n",
        server.http_origin()
    );
    assert!(text.contains(&flow), "{text}");
    assert!(
        text.contains(
            "  catalog/github 1.0.0  optional  not granted\n    from: official catalog\n"
        ),
        "{text}"
    );
}

#[test]
fn approve_needs_the_digest_permissions_printed_and_grants_exactly_that() {
    let server = hello_server();
    let connected = Connected::hello_only(&server);
    // Without --expect: a usage error, nothing granted
    let output = connected.run(&["approve", path_str(&connected.plugin)]);
    assert_eq!(output.status.code(), Some(2), "stderr: {}", stderr(&output));
    assert!(
        stderr(&output).contains("runtime permissions"),
        "{}",
        stderr(&output)
    );
    assert!(!connected.grants.exists());

    let digest = connected.digest();
    let output = connected.run(&["approve", path_str(&connected.plugin), "--expect", &digest]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        format!(
            "approved local/net-probe 0.1.0\nconnectors: local/hello ({})\n",
            server.http_origin()
        )
    );
    let text = stdout(&connected.run(&["permissions", path_str(&connected.plugin)]));
    assert!(
        text.contains("  local/hello 0.1.0  required  granted\n"),
        "{text}"
    );

    // The connector file changes after it was shown: the old digest grants nothing
    let mut changed = hello(&server);
    changed["operations"]["get"]["query"]["v"] = json!("3");
    std::fs::write(connected.connector_file(), changed.to_string()).unwrap();
    let record = std::fs::read(&connected.grants).unwrap();
    let output = connected.run(&["approve", path_str(&connected.plugin), "--expect", &digest]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).starts_with("error: approval-changed: "),
        "{}",
        stderr(&output)
    );
    assert_eq!(
        std::fs::read(&connected.grants).unwrap(),
        record,
        "nothing changed"
    );
}

#[test]
fn update_keeps_the_data_and_run_renders_it_without_the_network() {
    let server = hello_server();
    let connected = Connected::hello_only(&server);
    connected.approve();
    let config = json!({ "calls": [
        { "connector": "local/hello", "operation": "get", "arguments": [["lang", "pt"]] },
        { "connector": "local/hello", "operation": "nope", "arguments": [] }
    ] })
    .to_string();
    let output = connected.run(&["update", path_str(&connected.plugin), "--config", &config]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "updated local/net-probe 0.1.0\ncalls: 2 (0 not granted, 1 refused, 0 failed), 17 bytes received\n"
    );
    assert_eq!(server.received().len(), 1);
    assert_eq!(server.received()[0].query.as_deref(), Some("lang=pt&v=2"));

    let output = connected.run(&["run", path_str(&connected.plugin)]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let content: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(content["data"]["answers"][0]["status"], 200);
    assert_eq!(content["last_update"]["outcome"], "ok");
    assert!(content["fetched_at_ms"].as_u64().is_some());
    assert_eq!(server.received().len(), 1, "run made a request");
}

#[test]
fn requests_prints_the_log_oldest_first_escaped_and_filtered() {
    let server = hello_server();
    let connected = Connected::hello_only(&server);
    connected.approve();
    let config = json!({ "calls": [
        { "connector": "local/hello", "operation": "get", "arguments": [] },
        { "connector": "evil\n2026-01-01T00:00:00.000Z forged\u{1b}[2J", "operation": "x", "arguments": [] },
        { "connector": "local/hello", "operation": "get", "arguments": [["lang", "far too long"]] }
    ] })
    .to_string();
    connected.run(&["update", path_str(&connected.plugin), "--config", &config]);
    let output = runtime(&["requests", "--data", path_str(connected.dir.path())]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let text = stdout(&output);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 3, "{text}");
    let time = |line: &str| line.split(' ').next().unwrap().to_owned();
    for line in &lines {
        let at = time(line);
        assert_eq!(at.len(), "2026-10-23T14:02:11.204Z".len(), "{line}");
        assert!(at.ends_with('Z') && at.as_bytes()[10] == b'T', "{line}");
    }
    assert!(
        lines[0].ends_with(&format!(
            " local/net-probe local/hello get GET 127.0.0.1 /hello.json?v=2 200 17B {}",
            lines[0].rsplit(' ').next().unwrap()
        )),
        "{}",
        lines[0]
    );
    assert!(
        lines[1].contains(r"evil\n2026-01-01T00:00:00.000Z forged\u{1b}[2J x not-granted"),
        "{}",
        lines[1]
    );
    assert!(
        lines[2].ends_with(
            "local/hello get refused value-outside-type: `lang`: longer than 8 characters (12)"
        ),
        "{}",
        lines[2]
    );

    let last = runtime(&[
        "requests",
        "--last",
        "1",
        "--data",
        path_str(connected.dir.path()),
    ]);
    assert_eq!(stdout(&last).lines().collect::<Vec<_>>(), [lines[2]]);
    let other = runtime(&[
        "requests",
        "--plugin",
        "local/other",
        "--data",
        path_str(connected.dir.path()),
    ]);
    assert_eq!(stdout(&other), "");
}

#[test]
fn an_update_past_the_call_limit_ends_its_log_with_one_summary_line() {
    let server = hello_server();
    let connected = Connected::hello_only(&server);
    connected.approve();
    let config = json!({ "calls": [
        { "connector": "local/hello", "operation": "nope", "arguments": [], "repeat": 21 },
        { "connector": "local/none", "operation": "x", "arguments": [], "repeat": 2 }
    ] })
    .to_string();
    connected.run(&["update", path_str(&connected.plugin), "--config", &config]);
    let text = stdout(&runtime(&[
        "requests",
        "--last",
        "100",
        "--data",
        path_str(connected.dir.path()),
    ]));
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 21, "{text}");
    assert!(
        lines[20].ends_with(" local/net-probe … 3 more calls past the limit of this run (not-granted: 2, refused: 1)"),
        "{}",
        lines[20]
    );
}

#[test]
fn update_of_a_widget_without_updater_is_missing_updater() {
    let plugin = fixture_path("hello-widget");
    let dir = TempDir::new().unwrap();
    let grants = dir.path().join("grants.json");
    approve_file(&grants, &plugin);
    let output = runtime(&[
        "update",
        path_str(&plugin),
        "--grants",
        path_str(&grants),
        "--data",
        path_str(dir.path()),
    ]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).starts_with("error: missing-updater: "),
        "{}",
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("(previous data kept)"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn with_the_service_down_update_fails_keeping_the_data_and_run_still_renders_it() {
    let server = hello_server();
    let connected = Connected::hello_only(&server);
    connected.approve();
    let config = json!({ "fail": true, "calls": [
        { "connector": "local/hello", "operation": "get", "arguments": [] }
    ] })
    .to_string();
    let output = connected.run(&["update", path_str(&connected.plugin), "--config", &config]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));

    server.stop();
    let output = connected.run(&["update", path_str(&connected.plugin), "--config", &config]);
    assert_eq!(output.status.code(), Some(1));
    let error = stderr(&output);
    assert!(error.starts_with("error: plugin-error: "), "{error}");
    assert!(error.contains("failed:unavailable"), "{error}");
    assert!(error.contains("does not use network proxies"), "{error}");
    assert!(
        error.trim_end().ends_with("(previous data kept)"),
        "{error}"
    );

    let output = connected.run(&["run", path_str(&connected.plugin)]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let content: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(
        content["data"]["answers"][0]["status"], 200,
        "the previous data"
    );
    assert!(content["fetched_at_ms"].as_u64().is_some());
    assert_eq!(content["last_update"]["outcome"], "failed");
    assert!(
        content["last_update"]["error"]
            .as_str()
            .unwrap()
            .starts_with("plugin-error: "),
        "{content}"
    );
}

#[test]
fn optional_turns_a_connector_on_and_off_and_off_discards_the_kept_data() {
    let server = hello_server();
    let connected = Connected::new(
        &server,
        &[
            ("local/hello", 0, &["get"], false),
            ("catalog/github", 1, &["list-releases"], true),
        ],
    );
    connected.approve();
    let plugin = path_str(&connected.plugin);
    let config = calls_config(&[("local/hello", "get")]);

    let output = connected.run(&["optional", plugin, "local/hello", "on"]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "local/hello on\n");
    connected.run(&["update", plugin, "--config", &config]);
    let content = run_content(&connected);
    assert_eq!(content["data"]["answers"][0]["status"], 200);

    let output = connected.run(&["optional", plugin, "local/hello", "off"]);
    assert_eq!(stdout(&output), "local/hello off (kept data discarded)\n");
    assert!(run_content(&connected)["data"].is_null());

    let output = connected.run(&["optional", plugin, "catalog/github", "off"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).starts_with("error: required-together: catalog/github"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn turning_a_connector_off_in_another_process_drops_a_response_in_flight() {
    let server = Server::http();
    server.route(
        "/hello.json",
        Reply::json(r#"{"late":true}"#).delay(std::time::Duration::from_secs(2)),
    );
    let connected = Connected::new(&server, &[("local/hello", 0, &["get"], false)]);
    connected.approve();
    let plugin = path_str(&connected.plugin).to_owned();
    connected.run(&["optional", &plugin, "local/hello", "on"]);
    let updating = {
        let args: Vec<String> = [
            "update",
            &plugin,
            "--config",
            &calls_config(&[("local/hello", "get")]),
            "--grants",
            path_str(&connected.grants),
            "--data",
            path_str(connected.dir.path()),
        ]
        .iter()
        .map(|arg| (*arg).to_owned())
        .collect();
        std::process::Command::new(env!("CARGO_BIN_EXE_runtime"))
            .args(&args)
            .stdout(std::process::Stdio::piped())
            .spawn()
            .unwrap()
    };
    std::thread::sleep(std::time::Duration::from_millis(700));
    connected.run(&["optional", &plugin, "local/hello", "off"]);
    let output = updating.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(server.received().len(), 1, "it was sent");
    let text = stdout(&runtime(&[
        "requests",
        "--data",
        path_str(connected.dir.path()),
    ]));
    assert!(
        text.trim_end()
            .ends_with("local/hello get GET 127.0.0.1 /hello.json?v=2 not-granted"),
        "{text}"
    );
}

#[test]
fn remove_discards_the_kept_data_and_a_changed_connector_needs_a_new_approval() {
    let server = hello_server();
    let connected = Connected::hello_only(&server);
    connected.approve();
    let plugin = path_str(&connected.plugin);
    let config = calls_config(&[("local/hello", "get")]);
    connected.run(&["update", plugin, "--config", &config]);
    assert!(!run_content(&connected)["data"].is_null());
    let widget_data = connected.dir.path().join("widget-data");
    assert_eq!(std::fs::read_dir(&widget_data).unwrap().count(), 1);

    let output = connected.run(&["remove", "local/net-probe"]);
    assert_eq!(stdout(&output), "removed local/net-probe\n");
    assert_eq!(
        std::fs::read_dir(&widget_data).unwrap().count(),
        0,
        "the kept data stayed"
    );

    connected.approve();
    let mut changed = hello(&server);
    changed["destinations"]["more"] = json!("http://localhost:9");
    std::fs::write(connected.connector_file(), changed.to_string()).unwrap();
    let output = connected.run(&["update", plugin, "--config", &config]);
    assert_eq!(output.status.code(), Some(1));
    let error = stderr(&output);
    assert!(error.starts_with("error: needs-new-approval: "), "{error}");
    assert!(
        error.contains("destination added: http://localhost:9"),
        "{error}"
    );
}

fn calls_config(calls: &[(&str, &str)]) -> String {
    let calls: Vec<serde_json::Value> = calls
        .iter()
        .map(|(connector, operation)| json!({ "connector": connector, "operation": operation, "arguments": [] }))
        .collect();
    json!({ "calls": calls }).to_string()
}

fn run_content(connected: &Connected) -> serde_json::Value {
    let output = connected.run(&["run", path_str(&connected.plugin)]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    serde_json::from_str(&stdout(&output)).unwrap()
}

#[test]
fn check_connector_validates_a_connector_as_the_host_would_load_it() {
    let dir = TempDir::new().unwrap();
    let write = |name: &str, connector: &serde_json::Value| {
        let path = dir.path().join(format!("{name}.json"));
        std::fs::write(&path, connector.to_string()).unwrap();
        path
    };
    let base = || {
        json!({
            "connector_version": 1, "id": "local/weather", "version": "0.1.0", "author": "You",
            "description": "Weather", "destinations": { "api": "https://api.weather.test" },
            "operations": {
                "now": { "kind": "read", "method": "GET", "destination": "api", "path": "/now",
                         "response": { "format": "json" } },
                "week": { "kind": "read", "method": "GET", "destination": "api", "path": "/week",
                          "response": { "format": "json" } }
            }
        })
    };
    let output = runtime(&["check-connector", path_str(&write("weather", &base()))]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "ok: local/weather 0.1.0, 1 destination, 2 operations (2 reads)\n"
    );

    let mut more = base();
    more["destinations"]["cdn"] = json!("https://cdn.weather.test");
    more["operations"]["set"] = json!({ "kind": "change", "method": "POST", "destination": "cdn", "path": "/set", "response": { "format": "json" } });
    more["operations"].as_object_mut().unwrap().remove("week");
    let output = runtime(&["check-connector", path_str(&write("weather", &more))]);
    assert_eq!(
        stdout(&output),
        "ok: local/weather 0.1.0, 2 destinations, 2 operations (1 read, 1 change)\n"
    );

    for (case, edit, kind, mentions) in [
        (
            "an unknown field",
            (|c: &mut serde_json::Value| c["colour"] = json!(1)) as fn(&mut serde_json::Value),
            "invalid-connector",
            "colour",
        ),
        (
            "a destination with a path",
            |c| c["destinations"]["api"] = json!("https://api.weather.test/v1"),
            "invalid-connector",
            "destinations.api",
        ),
        (
            "a parameter without a type",
            |c| {
                c["operations"]["now"]["path"] = json!("/now/{city}");
                c["operations"]["now"]["parameters"] = json!({ "city": { "max_length": 8 } });
            },
            "invalid-connector",
            "operations.now.parameters.city",
        ),
        (
            "a POST read",
            |c| c["operations"]["now"]["method"] = json!("POST"),
            "invalid-connector",
            "operations.now",
        ),
        (
            "an X- header",
            |c| c["operations"]["now"]["headers"] = json!({ "X-Version": "2" }),
            "invalid-connector",
            "X-Version",
        ),
        (
            "a reserved host",
            |c| c["destinations"]["api"] = json!("https://api.github.com"),
            "destination-reserved",
            "api.github.com",
        ),
    ] {
        let mut connector = base();
        edit(&mut connector);
        let output = runtime(&["check-connector", path_str(&write("weather", &connector))]);
        assert_eq!(output.status.code(), Some(1), "{case}");
        let error = stderr(&output);
        assert!(
            error.starts_with(&format!("error: {kind}: ")),
            "{case}: {error}"
        );
        assert!(error.contains(mentions), "{case}: {error}");
    }
    // The file's name is the connector's
    let output = runtime(&["check-connector", path_str(&write("other", &base()))]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("local/other"),
        "{}",
        stderr(&output)
    );
    let output = runtime(&[
        "check-connector",
        path_str(&dir.path().join("missing.json")),
    ]);
    assert_eq!(output.status.code(), Some(2));
}

#[test]
fn bench_of_a_connected_widget_measures_a_render_from_kept_data() {
    let server = hello_server();
    let connected = Connected::hello_only(&server);
    connected.approve();
    let data = connected.dir.path().join("data.json");
    std::fs::write(
        &data,
        r#"{"answers":[{"status":200,"headers":[],"body":"{}"}],"more":0}"#,
    )
    .unwrap();
    let output = connected.run(&[
        "bench",
        path_str(&connected.plugin),
        "--iterations",
        "5",
        "--kept-data",
        path_str(&data),
    ]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let text = stdout(&output);
    let last = text.lines().last().unwrap();
    let (key, value) = last.split_once(": ").unwrap();
    assert_eq!(key, "render_with_data_us");
    assert!(value.parse::<u64>().unwrap() < 1_000_000, "{last}");
    assert!(server.received().is_empty(), "bench made a request");
}
