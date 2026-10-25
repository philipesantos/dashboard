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
            .contains("login providers: github (obsolete: accounts come from connectors)\n"),
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
        // Under the 5 s call limit, with room to change the record while the response waits
        Reply::json(r#"{"late":true}"#).delay(std::time::Duration::from_secs(4)),
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
    // Turned off once the request is in flight: the service answers 4 s after it arrives. A
    // fixed delay here lost the race to a slow start of the child process
    let started = std::time::Instant::now();
    while server.received().is_empty() {
        assert!(
            started.elapsed() < std::time::Duration::from_secs(30),
            "the update never sent its request"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    // Off in the record from this process, which is another process to the update's. Not with
    // `runtime optional`: a debug build of it compiles the package first, which on Windows
    // under load outlasted the service's delay
    let package = std::fs::read(&connected.plugin).unwrap();
    let manifest = runtime::package::manifest_section(&package)
        .and_then(runtime::manifest::Manifest::parse)
        .unwrap();
    GrantStore::open(&connected.grants)
        .set_connector(&manifest, &origin(&connected.plugin), "local/hello", false)
        .unwrap();
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

// Storage (specs/004-plugin-storage/contracts/cli.md). Every command gets --grants and --data in
// a temporary directory.

/// A data directory and grant record, and `update-counter` copied into it, so the test can
/// replace the package at the same path, as a new build would.
struct Stored {
    dir: TempDir,
    grants: PathBuf,
    plugin: PathBuf,
}

impl Stored {
    fn new() -> Stored {
        let dir = TempDir::new().unwrap();
        let plugin = dir.path().join("update-counter.wasm");
        std::fs::copy(fixture_path("update-counter"), &plugin).unwrap();
        let grants = dir.path().join("grants.json");
        Stored {
            dir,
            grants,
            plugin,
        }
    }

    /// Replaces the package with `update-counter` declaring `quota_bytes`.
    fn declare_quota(&self, quota_bytes: u64) {
        let manifest = common::stored_manifest("local/update-counter", "0.1.0", true, quota_bytes);
        let package = common::repack(&common::fixture("update-counter"), &manifest);
        std::fs::write(&self.plugin, package).unwrap();
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

    fn permissions(&self) -> String {
        let output = self.run(&["permissions", path_str(&self.plugin)]);
        assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
        stdout(&output)
    }

    fn digest(&self) -> String {
        self.permissions()
            .lines()
            .find_map(|line| line.strip_prefix("approval digest: "))
            .unwrap_or_else(|| panic!("no digest in {}", self.permissions()))
            .to_owned()
    }

    fn approve(&self) -> Output {
        let digest = self.digest();
        self.run(&["approve", path_str(&self.plugin), "--expect", &digest])
    }

    fn line(text: &str, prefix: &str) -> String {
        text.lines()
            .find(|line| line.starts_with(prefix))
            .unwrap_or_else(|| panic!("no `{prefix}` line in {text}"))
            .to_owned()
    }
}

#[test]
fn permissions_shows_storage_its_quota_and_usage_and_changes_nothing() {
    let stored = Stored::new();
    let shown = stored.permissions();
    assert!(shown.contains("contract: v1 (1.2.0)\n"), "{shown}");
    assert!(
        shown.contains("permissions:\n  storage  required  not granted\n"),
        "{shown}"
    );
    assert_eq!(
        Stored::line(&shown, "storage:"),
        "storage: quota 65536 bytes (64 KiB), not approved, used 0 bytes"
    );
    assert!(Stored::line(&shown, "approval digest: ").starts_with("approval digest: sha256:"));
    assert!(!stored.grants.exists(), "permissions wrote the record");
    assert!(
        !stored.dir.path().join("storage").exists(),
        "permissions created the storage directory"
    );
    // A widget without storage shows no storage line
    let hello = fixture_path("hello-widget");
    let output = stored.run(&["permissions", path_str(&hello)]);
    assert!(!stdout(&output).contains("storage:"), "{}", stdout(&output));
}

#[test]
fn approve_needs_the_digest_permissions_printed_for_storage() {
    let stored = Stored::new();
    let output = stored.run(&["approve", path_str(&stored.plugin)]);
    assert_eq!(output.status.code(), Some(2), "stderr: {}", stderr(&output));
    assert!(
        stderr(&output).contains("runtime permissions"),
        "{}",
        stderr(&output)
    );
    assert!(!stored.grants.exists(), "nothing was granted");
    let output = stored.run(&[
        "approve",
        path_str(&stored.plugin),
        "--expect",
        "sha256:0000",
    ]);
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert!(
        stderr(&output).starts_with("error: approval-changed: "),
        "{}",
        stderr(&output)
    );
    let output = stored.approve();
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "approved local/update-counter 0.1.0\ngranted: storage\nstorage: quota 65536 bytes\n"
    );
    assert_eq!(
        Stored::line(&stored.permissions(), "storage:"),
        "storage: quota 65536 bytes (64 KiB), approved up to 65536 bytes, used 0 bytes"
    );
}

#[test]
fn update_prints_its_storage_and_run_renders_what_it_kept() {
    let stored = Stored::new();
    assert_eq!(stored.approve().status.code(), Some(0));
    let output = stored.run(&[
        "update",
        path_str(&stored.plugin),
        "--config",
        r#"{"note":"first"}"#,
    ]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "updated local/update-counter 0.1.0\nstorage: 13 calls, 7 writes kept, used 223 of 65536 bytes\n"
    );
    let output = stored.run(&["run", path_str(&stored.plugin)]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let content: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(content["text"], "Updated 1 times");
    assert_eq!(content["items"], json!(["first"]));
    assert_eq!(
        Stored::line(&stored.permissions(), "storage:"),
        "storage: quota 65536 bytes (64 KiB), approved up to 65536 bytes, used 223 bytes"
    );
}

#[test]
fn a_quota_above_the_approved_one_needs_approval_again() {
    let stored = Stored::new();
    assert_eq!(stored.approve().status.code(), Some(0));
    stored.declare_quota(131072);
    assert_eq!(
        Stored::line(&stored.permissions(), "storage:"),
        "storage: quota 131072 bytes (128 KiB), approved up to 65536 bytes, used 0 bytes (above the approved quota: approve again)"
    );
    let output = stored.run(&["update", path_str(&stored.plugin)]);
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    let error = stderr(&output);
    assert!(
        error.starts_with("error: needs-new-approval: storage"),
        "{error}"
    );
    assert!(
        error.contains("65536") && error.contains("131072"),
        "{error}"
    );
    assert_eq!(stored.approve().status.code(), Some(0));
    let output = stored.run(&["update", path_str(&stored.plugin)]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
}

impl Stored {
    /// `store-probe` as `local/store-probe`, approved, in its own data directory.
    fn probe() -> Stored {
        let stored = Stored::new();
        let manifest = common::stored_manifest("local/store-probe", "0.1.0", true, 65536);
        std::fs::write(
            &stored.plugin,
            common::repack(&common::fixture("store-probe"), &manifest),
        )
        .unwrap();
        assert_eq!(stored.approve().status.code(), Some(0));
        stored
    }

    fn update_with(&self, config: &serde_json::Value) -> Output {
        self.run(&[
            "update",
            path_str(&self.plugin),
            "--config",
            &config.to_string(),
        ])
    }
}

#[test]
fn a_failed_update_says_its_storage_writes_were_not_kept() {
    let stored = Stored::probe();
    let write = |value: &str| json!({ "op": "set", "name": "a", "value": value });
    let output = stored.update_with(&json!({ "steps": [write("before")] }));
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let output = stored.update_with(&json!({ "steps": [write("during")], "end": "fuel" }));
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    let error = stderr(&output);
    assert!(error.starts_with("error: cpu-budget-exceeded: "), "{error}");
    assert!(
        error.ends_with(" (storage writes not kept, previous data kept)\n"),
        "{error}"
    );
    // A store that cannot be written fails the update as storage-failed
    let store = std::fs::read_dir(stored.dir.path().join("storage"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| {
            path.extension()
                .is_some_and(|extension| extension == "sqlite")
        })
        .unwrap();
    let mut permissions = std::fs::metadata(&store).unwrap().permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(&store, permissions.clone()).unwrap();
    let output = stored.update_with(&json!({ "steps": [write("during")] }));
    #[allow(clippy::permissions_set_readonly_false)]
    permissions.set_readonly(false);
    std::fs::set_permissions(&store, permissions).unwrap();
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    let error = stderr(&output);
    assert!(error.starts_with("error: storage-failed: "), "{error}");
    assert!(
        error.ends_with("; nothing this update wrote was kept (previous data kept)\n"),
        "{error}"
    );
    // run renders the values from before both updates
    let output = stored.run(&[
        "run",
        path_str(&stored.plugin),
        "--config",
        &json!({ "render_steps": [{ "op": "get", "name": "a" }] }).to_string(),
    ]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let content: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(
        content["answers"][0],
        json!({ "ok": { "hex": "6265666f7265" } })
    );
}

impl Stored {
    /// A second plugin in the same data directory: `store-probe`, approved.
    fn add_probe(&self) -> PathBuf {
        let manifest = common::stored_manifest("local/store-probe", "0.1.0", true, 65536);
        let plugin = self.dir.path().join("store-probe.wasm");
        std::fs::write(
            &plugin,
            common::repack(&common::fixture("store-probe"), &manifest),
        )
        .unwrap();
        let digest = stdout(&self.run(&["permissions", path_str(&plugin)]))
            .lines()
            .find_map(|line| line.strip_prefix("approval digest: ").map(str::to_owned))
            .unwrap();
        let output = self.run(&["approve", path_str(&plugin), "--expect", &digest]);
        assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
        plugin
    }

    fn update_counter(&self, note: &str) {
        let output = self.run(&[
            "update",
            path_str(&self.plugin),
            "--config",
            &json!({ "note": note }).to_string(),
        ]);
        assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    }

    fn list(&self) -> String {
        let output = self.run(&["storage", "list"]);
        assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
        stdout(&output)
    }
}

#[test]
fn storage_list_prints_every_store_sorted_and_changes_nothing() {
    let stored = Stored::new();
    assert_eq!(stored.list(), "no stores\n");
    assert_eq!(stored.approve().status.code(), Some(0));
    stored.update_counter("first");
    let probe = stored.add_probe();
    let output = stored.run(&[
        "update",
        path_str(&probe),
        "--config",
        &json!({ "steps": [{ "op": "set", "name": "a", "value": "b" }] }).to_string(),
    ]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let counter_origin = origin(&stored.plugin);
    let probe_origin = origin(&probe);
    assert_eq!(
        stored.list(),
        format!(
            "local/store-probe  {probe_origin}  used 66 of 65536 bytes\nlocal/update-counter  {counter_origin}  used 223 of 65536 bytes\n"
        )
    );
    // The package file moved away: no file; its approval removed: not approved
    std::fs::rename(&probe, stored.dir.path().join("moved.wasm")).unwrap();
    let output = stored.run(&["remove", "local/update-counter"]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(
        stored.list(),
        format!(
            "local/store-probe  {probe_origin}  used 66 of 65536 bytes, no file\nlocal/update-counter  {counter_origin}  used 223 bytes, not approved\n"
        )
    );
    // A file that is no store the host wrote
    let garbage = stored.dir.path().join("storage").join("0000.sqlite");
    std::fs::write(&garbage, vec![7u8; 4096]).unwrap();
    let listed = stored.list();
    let last = listed.lines().last().unwrap();
    assert!(last.starts_with("0000.sqlite  -  damaged: "), "{listed}");
    assert_eq!(std::fs::read(&garbage).unwrap(), vec![7u8; 4096]);
}

#[test]
fn storage_show_prints_every_entry_quoted() {
    let stored = Stored::new();
    let output = stored.run(&["storage", "show", path_str(&stored.plugin)]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let header = format!(
        "store: local/update-counter from {}\n",
        origin(&stored.plugin)
    );
    assert_eq!(stdout(&output), format!("{header}no store yet\n"));
    assert_eq!(stored.approve().status.code(), Some(0));
    stored.update_counter("remember the milk\n");
    let output = stored.run(&["storage", "show", path_str(&stored.plugin)]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        format!(
            "{header}used 236 of 65536 bytes, 3 entries\n\"count\" = \"1\"\n\"first_run_ms\" = \"0\"\n\"notes/0\" = \"remember the milk\\n\"\n"
        )
    );
    // A damaged store
    let store = std::fs::read_dir(stored.dir.path().join("storage"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .next()
        .unwrap();
    std::fs::write(&store, vec![7u8; 4096]).unwrap();
    let output = stored.run(&["storage", "show", path_str(&stored.plugin)]);
    assert_eq!(output.status.code(), Some(1), "stderr: {}", stderr(&output));
    assert!(
        stderr(&output).starts_with("error: storage-failed: damaged: "),
        "{}",
        stderr(&output)
    );
}

#[test]
fn storage_clear_empties_one_store_and_no_other() {
    let stored = Stored::new();
    assert_eq!(stored.approve().status.code(), Some(0));
    stored.update_counter("first");
    let probe = stored.add_probe();
    let output = stored.run(&[
        "update",
        path_str(&probe),
        "--config",
        &json!({ "steps": [{ "op": "set", "name": "a", "value": "b" }] }).to_string(),
    ]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let output = stored.run(&["storage", "clear", path_str(&stored.plugin)]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        format!(
            "cleared local/update-counter from {} (223 bytes)\n",
            origin(&stored.plugin)
        )
    );
    let listed = stored.list();
    assert!(listed.contains("local/update-counter  "), "{listed}");
    assert!(listed.contains("used 0 of 65536 bytes"), "{listed}");
    assert!(
        listed.contains("used 66 of 65536 bytes"),
        "the other store: {listed}"
    );
    let output = stored.run(&["run", path_str(&stored.plugin)]);
    let content: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert_eq!(content["text"], "Not updated yet");
    // A store whose package file is gone, by id and origin
    let probe_origin = origin(&probe);
    std::fs::remove_file(&probe).unwrap();
    let output = stored.run(&[
        "storage",
        "clear",
        "--id",
        "local/store-probe",
        "--origin",
        &probe_origin,
    ]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        format!("removed local/store-probe from {probe_origin} (its package file is gone)\n")
    );
    assert!(!stored.list().contains("local/store-probe"));
    // --id needs --origin, and a plugin file goes alone
    for args in [
        &["storage", "clear", "--id", "local/store-probe"][..],
        &["storage", "clear"][..],
        &["storage", "unknown"][..],
    ] {
        let output = stored.run(args);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            stderr(&output)
        );
    }
}

#[test]
fn storage_clear_of_a_store_in_use_is_busy_and_changes_nothing() {
    use std::sync::Arc;

    let stored = Stored::probe();
    let set = |value: &str| json!({ "op": "set", "name": "a", "value": value });
    let output = stored.update_with(&json!({ "steps": [set("b")] }));
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let store = std::fs::read_dir(stored.dir.path().join("storage"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .next()
        .unwrap();
    // Another process holds the store: an update in this one, which writes, then spins with no
    // CPU budget until its time limit stops it, 15 s later, rolling its writes back. A fixed
    // hold, so a slow start of the CLI under load cannot outlast it
    let limits = runtime::Limits {
        fuel: u64::MAX,
        update_time_limit: std::time::Duration::from_secs(15),
        ..runtime::Limits::default()
    };
    let holder = runtime::Runtime::new(limits, runtime::RuntimeOptions::new(stored.dir.path()))
        .unwrap()
        .load(
            &std::fs::read(&stored.plugin).unwrap(),
            &origin(&stored.plugin),
            Arc::new(GrantStore::open(&stored.grants)),
        )
        .unwrap();
    let spin = u64::MAX;
    let journal = store.with_extension("sqlite-journal");
    let output = std::thread::scope(|scope| {
        let running = scope.spawn(|| {
            holder.update(
                &json!({ "steps": [set("held"), { "op": "spin", "iterations": spin }] })
                    .to_string(),
            )
        });
        let started = std::time::Instant::now();
        while !journal.exists() {
            assert!(
                started.elapsed() < std::time::Duration::from_secs(30),
                "no journal"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let output = stored.run(&["storage", "clear", path_str(&stored.plugin)]);
        let held = running
            .join()
            .unwrap()
            .result
            .expect_err("stopped at its time limit");
        assert_eq!(held.kind(), runtime::ErrorKind::TimeLimitExceeded, "{held}");
        output
    });
    assert_eq!(output.status.code(), Some(1), "stdout: {}", stdout(&output));
    assert!(
        stderr(&output).starts_with("error: storage-failed: busy: "),
        "{}",
        stderr(&output)
    );
    // The clear changed nothing, and the holder's update rolled back: the value from before
    let output = stored.run(&["storage", "show", path_str(&stored.plugin)]);
    assert!(
        stdout(&output).contains("\"a\" = \"b\""),
        "{}",
        stdout(&output)
    );
}

#[test]
fn optional_storage_off_keeps_the_data_and_remove_says_where_it_is() {
    let stored = Stored::new();
    let manifest = common::stored_manifest("local/update-counter", "0.1.0", false, 65536);
    std::fs::write(
        &stored.plugin,
        common::repack(&common::fixture("update-counter"), &manifest),
    )
    .unwrap();
    assert_eq!(stored.approve().status.code(), Some(0));
    let plugin = path_str(&stored.plugin);
    let output = stored.run(&["optional", plugin, "storage", "on"]);
    assert_eq!(
        stdout(&output),
        "storage on\n",
        "stderr: {}",
        stderr(&output)
    );
    stored.update_counter("kept");
    let output = stored.run(&["optional", plugin, "storage", "off"]);
    assert_eq!(
        stdout(&output),
        "storage off (data kept)\n",
        "stderr: {}",
        stderr(&output)
    );
    assert_eq!(
        stored
            .run(&["optional", plugin, "storage", "on"])
            .status
            .code(),
        Some(0)
    );
    let content: serde_json::Value =
        serde_json::from_str(&stdout(&stored.run(&["run", plugin]))).unwrap();
    assert_eq!(content["items"], json!(["kept"]));
    // Turning it on above the approved quota needs a new approval
    stored.run(&["optional", plugin, "storage", "off"]);
    let larger = common::stored_manifest("local/update-counter", "0.1.0", false, 131072);
    std::fs::write(
        &stored.plugin,
        common::repack(&common::fixture("update-counter"), &larger),
    )
    .unwrap();
    let output = stored.run(&["optional", plugin, "storage", "on"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).starts_with("error: needs-new-approval: storage"),
        "{}",
        stderr(&output)
    );
    // remove keeps the store and says so; approving again from the same file finds the data
    let output = stored.run(&["remove", "local/update-counter"]);
    assert_eq!(
        stdout(&output),
        "removed local/update-counter\nstorage kept: run runtime storage clear to delete it\n"
    );
    std::fs::write(
        &stored.plugin,
        common::repack(&common::fixture("update-counter"), &manifest),
    )
    .unwrap();
    assert_eq!(stored.approve().status.code(), Some(0));
    stored.run(&["optional", plugin, "storage", "on"]);
    let content: serde_json::Value =
        serde_json::from_str(&stdout(&stored.run(&["run", plugin]))).unwrap();
    assert_eq!(content["items"], json!(["kept"]));
}

/// The CLI half of `data_dir_is_absolute` (`specs/004-plugin-storage/data-model.md`): a
/// relative `--data` puts the store at an absolute path under that directory.
#[test]
fn a_relative_data_directory_holds_its_stores() {
    let stored = Stored::probe();
    // Relative to the package directory, where Cargo runs the tests and so the CLI
    let relative = PathBuf::from("..")
        .join("target")
        .join("storage-tests")
        .join(format!("cli-{}", std::process::id()));
    let absolute = Path::new(env!("CARGO_MANIFEST_DIR")).join(&relative);
    let _ = std::fs::remove_dir_all(&absolute);
    let output = Command::new(env!("CARGO_BIN_EXE_runtime"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .args([
            "update",
            path_str(&stored.plugin),
            "--config",
            &json!({ "steps": [{ "op": "set", "name": "a", "value": "b" }] }).to_string(),
            "--grants",
            path_str(&stored.grants),
            "--data",
            path_str(&relative),
        ])
        .output()
        .unwrap();
    let stores: Vec<PathBuf> = std::fs::read_dir(absolute.join("storage"))
        .map(|entries| entries.map(|entry| entry.unwrap().path()).collect())
        .unwrap_or_default();
    let _ = std::fs::remove_dir_all(&absolute);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(stores.len(), 1, "{stores:?}");
    assert!(
        stores[0]
            .extension()
            .is_some_and(|extension| extension == "sqlite")
    );
}

// Accounts (specs/005-plugin-credentials/contracts/cli.md). No seam: nothing here signs in, and
// nothing reaches a service; the accounts record is in the temporary data directory.

/// A data directory with `local/notes` as shipped beside `private-notes`, and `plugin` copied
/// into it, so its origin is a file of the test.
struct Accounts {
    dir: TempDir,
    grants: PathBuf,
    plugin: PathBuf,
}

const NOTES: &str = include_str!("../../plugins/examples/private-notes/connectors/notes.json");

impl Accounts {
    fn new(package: &[u8]) -> Accounts {
        let dir = TempDir::new().unwrap();
        let connectors = dir.path().join("connectors");
        std::fs::create_dir_all(&connectors).unwrap();
        std::fs::write(connectors.join("notes.json"), NOTES).unwrap();
        let plugin = dir.path().join("widget.wasm");
        std::fs::write(&plugin, package).unwrap();
        let grants = dir.path().join("grants.json");
        Accounts {
            dir,
            grants,
            plugin,
        }
    }

    fn private_notes() -> Accounts {
        Accounts::new(&std::fs::read(fixture_path("private-notes")).unwrap())
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

    fn connector_file(&self, name: &str) -> PathBuf {
        std::fs::canonicalize(self.dir.path().join("connectors").join(name)).unwrap()
    }

    fn permissions(&self) -> String {
        let output = self.run(&["permissions", path_str(&self.plugin)]);
        assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
        stdout(&output)
    }
}

#[test]
fn permissions_shows_the_credential_and_whom_the_widget_acts_as() {
    let accounts = Accounts::private_notes();
    let text = accounts.permissions();
    let block = format!(
        "  local/notes 1.0.0  required  not granted
    from: local file {} (development)
    destinations: https://notes.test
    credential: account (optional), Authorization: Bearer, sent to https://notes.test
      sign-in: https://notes.test/oauth/authorize
      token: https://notes.test/oauth/token
      asks for: notes read (declared by the developer)
      this widget acts as the account: none connected (calls go without one)
    list-notes  read (declared by the developer)  GET https://notes.test/notes
",
        accounts.connector_file("notes.json").display()
    );
    assert!(text.contains(&block), "{text}");
    assert!(text.contains("\nlogin providers: none\n"), "{text}");
    // One connector, whose only destination is the credential's: no data-flow line
    assert!(!text.contains("data flow:"), "{text}");
}

#[test]
fn accounts_list_says_when_nothing_is_connected() {
    let accounts = Accounts::private_notes();
    let output = accounts.run(&["accounts", "list"]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "nothing connected\n");
    assert!(!accounts.dir.path().join("accounts.json").exists());
}

#[test]
fn accounts_connect_without_a_client_fails_naming_the_option() {
    let accounts = Accounts::private_notes();
    let output = accounts.run(&["accounts", "connect", "local/notes", "--no-browser"]);
    assert_eq!(output.status.code(), Some(1), "stdout: {}", stdout(&output));
    let error = stderr(&output);
    assert!(
        error.starts_with("error: no-client-configured: "),
        "{error}"
    );
    assert!(error.contains("--client"), "{error}");
    // Nothing was opened, and nothing waited for a browser
    assert!(!stdout(&output).contains("waiting"), "{}", stdout(&output));
}

#[test]
fn accounts_connect_refuses_a_credential_over_plain_http() {
    let accounts = Accounts::private_notes();
    let hostile = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../plugins/hostile/connectors/plain-http-key.json");
    std::fs::copy(
        hostile,
        accounts.dir.path().join("connectors/plain-http-key.json"),
    )
    .unwrap();
    let output = accounts.run(&["accounts", "connect", "local/plain-http-key"]);
    assert_eq!(output.status.code(), Some(1), "stdout: {}", stdout(&output));
    assert!(
        stderr(&output).starts_with("error: credential-needs-https: "),
        "{}",
        stderr(&output)
    );
    assert_eq!(stdout(&output), "");
}

#[test]
fn accounts_disconnect_with_nothing_connected_fails() {
    let accounts = Accounts::private_notes();
    let output = accounts.run(&["accounts", "disconnect", "local/notes"]);
    assert_eq!(output.status.code(), Some(1), "stdout: {}", stdout(&output));
    assert!(
        stderr(&output).starts_with("error: account-not-connected: "),
        "{}",
        stderr(&output)
    );
}

#[test]
fn approve_names_the_account_use() {
    let accounts = Accounts::private_notes();
    let digest = accounts
        .permissions()
        .lines()
        .find_map(|line| line.strip_prefix("approval digest: ").map(str::to_owned))
        .expect("a digest");
    let output = accounts.run(&["approve", path_str(&accounts.plugin), "--expect", &digest]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "approved local/private-notes 0.1.0
connectors: local/notes (https://notes.test)
account use: local/notes (notes read)
"
    );
    let text = accounts.permissions();
    assert!(
        text.contains("  local/notes 1.0.0  required  granted\n"),
        "{text}"
    );
}

/// `data_flow`: data read as the account can reach the widget's other destinations (FR-005).
#[test]
fn data_flow() {
    let manifest = json!({
        "manifest_version": 1, "id": "local/flow-probe", "version": "0.1.0", "author": "A",
        "permissions": [],
        "connectors": [
            { "id": "local/notes", "major": 1, "operations": ["list-notes"], "required": true, "account": true },
            { "id": "catalog/github", "major": 1, "operations": ["list-releases"], "required": false }
        ]
    });
    let package = runtime::package::pack(
        &common::fixture("account-probe"),
        manifest.to_string().as_bytes(),
    )
    .expect("the manifest is valid");
    let accounts = Accounts::new(&package);
    let text = accounts.permissions();
    assert!(
        text.contains(
            "data flow: data read as the account of local/notes can reach https://api.github.com\n"
        ),
        "{text}"
    );
    // The widget that does not use the account says so in its credential block
    let mut plain = manifest.clone();
    plain["connectors"][0]["account"] = json!(false);
    plain["connectors"][1] = json!({ "id": "local/other", "major": 1, "operations": ["list-notes"], "required": true, "account": true });
    let mut other: serde_json::Value = serde_json::from_str(NOTES).unwrap();
    other["id"] = json!("local/other");
    other["destinations"]["api"] = json!("https://other.test");
    other["credential"]["account"]["authorize"] = json!("https://other.test/oauth/authorize");
    other["credential"]["account"]["token"] = json!("https://other.test/oauth/token");
    other["credential"]["account"]["revoke"] = json!("https://other.test/oauth/revoke");
    let package = runtime::package::pack(
        &common::fixture("account-probe"),
        plain.to_string().as_bytes(),
    )
    .expect("the manifest is valid");
    let accounts = Accounts::new(&package);
    std::fs::write(
        accounts.dir.path().join("connectors/other.json"),
        other.to_string(),
    )
    .unwrap();
    let text = accounts.permissions();
    assert!(
        text.contains("      this widget does not use the account\n"),
        "{text}"
    );
    assert!(
        text.contains(
            "data flow: data read as the account of local/other can reach https://notes.test\n"
        ),
        "{text}"
    );
    assert!(!text.contains("account of local/notes"), "{text}");
}

/// `local/keyed`: `local/notes` with a key the maintainer types instead of an account, written
/// beside it; returns its file's canonical path.
#[cfg(unix)]
fn keyed(accounts: &Accounts) -> PathBuf {
    let mut connector: serde_json::Value = serde_json::from_str(NOTES).unwrap();
    connector["id"] = json!("local/keyed");
    connector["credential"] = json!({
        "kind": "key", "required": true, "destinations": ["api"],
        "header": "Authorization", "scheme": "Bearer"
    });
    let path = accounts.dir.path().join("connectors/keyed.json");
    std::fs::write(&path, connector.to_string()).unwrap();
    std::fs::canonicalize(path).unwrap()
}

/// Runs the CLI with `args` on a pseudo-terminal of its own, its controlling terminal, as a
/// person at a terminal does; types `typed` and Enter once it asks for the key. Returns the exit
/// status and everything the terminal showed.
#[cfg(unix)]
fn on_terminal(args: &[&str], typed: &str) -> (Option<i32>, String) {
    use std::io::{Read, Write};
    use std::os::fd::{FromRawFd, OwnedFd};
    use std::os::unix::process::CommandExt;
    use std::sync::{Arc, Mutex};

    let (mut master, mut slave) = (0, 0);
    // SAFETY: openpty writes the two descriptors it opens into the two integers; the null
    // pointers ask for no name, no terminal settings and no window size
    let opened = unsafe {
        libc::openpty(
            &mut master,
            &mut slave,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
        )
    };
    assert_eq!(opened, 0, "openpty: {}", std::io::Error::last_os_error());
    // SAFETY: both descriptors were just opened, and nothing else owns them
    let (master, slave) = unsafe { (OwnedFd::from_raw_fd(master), OwnedFd::from_raw_fd(slave)) };
    let mut command = Command::new(env!("CARGO_BIN_EXE_runtime"));
    command
        .args(args)
        .stdin(slave.try_clone().unwrap())
        .stdout(slave.try_clone().unwrap())
        .stderr(slave);
    // SAFETY: between fork and exec, only async-signal-safe calls: a new session, then the
    // terminal on standard input as its controlling terminal
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 || libc::ioctl(0, libc::TIOCSCTTY as _, 0) == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn().unwrap();
    // The parent's copies of the terminal side, so its reads end when the child's do
    drop(command);
    let mut terminal = std::fs::File::from(master);
    let mut reader = terminal.try_clone().unwrap();
    let shown = Arc::new(Mutex::new(Vec::new()));
    let reading = {
        let shown = Arc::clone(&shown);
        std::thread::spawn(move || {
            let mut chunk = [0u8; 1024];
            // Ends with an error once the child and its terminal are gone
            while let Ok(read @ 1..) = reader.read(&mut chunk) {
                shown.lock().unwrap().extend_from_slice(&chunk[..read]);
            }
        })
    };
    let started = std::time::Instant::now();
    loop {
        let text = String::from_utf8_lossy(&shown.lock().unwrap()).into_owned();
        if text.contains("not shown): ") {
            break;
        }
        if started.elapsed() > std::time::Duration::from_secs(30) {
            let _ = child.kill();
            panic!("the CLI never asked for the key: {text}");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    // The prompt appears only once the terminal's echo is off (R14)
    #[cfg(target_os = "linux")]
    assert!(
        !echoes(&terminal),
        "the terminal echoed when the prompt appeared"
    );
    terminal.write_all(format!("{typed}\n").as_bytes()).unwrap();
    let status = child.wait().unwrap();
    // And the CLI leaves it as it found it
    #[cfg(target_os = "linux")]
    assert!(echoes(&terminal), "the terminal's echo stayed off");
    drop(terminal);
    reading.join().unwrap();
    let shown = String::from_utf8_lossy(&shown.lock().unwrap()).into_owned();
    (status.code(), shown)
}

/// Whether the pseudo-terminal echoes what is typed. On Linux its controller side reports the
/// settings the program on the other side set.
#[cfg(target_os = "linux")]
fn echoes(terminal: &std::fs::File) -> bool {
    use std::os::fd::AsRawFd as _;
    let mut settings = std::mem::MaybeUninit::<libc::termios>::uninit();
    // SAFETY: the descriptor is open for as long as `terminal` lives, and tcgetattr writes a
    // whole termios into `settings` when it returns 0
    let read = unsafe { libc::tcgetattr(terminal.as_raw_fd(), settings.as_mut_ptr()) };
    assert_eq!(read, 0, "tcgetattr: {}", std::io::Error::last_os_error());
    // SAFETY: tcgetattr returned 0, so it filled `settings`
    let settings = unsafe { settings.assume_init() };
    settings.c_lflag & libc::ECHO != 0
}

/// Disconnects `local/keyed` from `origin` when dropped, so the system's keychain keeps nothing
/// of a test that failed.
#[cfg(unix)]
struct ForgetKey<'a> {
    accounts: &'a Accounts,
    origin: String,
}

#[cfg(unix)]
impl Drop for ForgetKey<'_> {
    fn drop(&mut self) {
        let _ = self.accounts.run(&[
            "accounts",
            "disconnect",
            "local/keyed",
            "--origin",
            &self.origin,
        ]);
    }
}

/// A key typed at the terminal, never echoed, kept in the system's keychain, listed, and
/// disconnected after its connector's file moved (clarification 1); `key_checks` on the CLI's
/// key path, and a second key refused while one is kept.
#[cfg(unix)]
#[test]
fn a_key_typed_at_the_terminal_is_kept_listed_and_disconnected() {
    let accounts = Accounts::private_notes();
    let file = keyed(&accounts);
    let _forget = ForgetKey {
        accounts: &accounts,
        origin: file.to_string_lossy().into_owned(),
    };
    let grants = path_str(&accounts.grants).to_owned();
    let data = path_str(accounts.dir.path()).to_owned();
    let connect = [
        "accounts",
        "connect",
        "local/keyed",
        "--grants",
        &grants,
        "--data",
        &data,
    ];

    // `key_checks`: refused before the keychain sees it. A line over 1,024 bytes is cut by
    // macOS's terminal before the CLI reads it, so only Linux types one
    let mut refused = vec!["".to_owned(), "has space".to_owned(), "clé".to_owned()];
    if cfg!(target_os = "linux") {
        refused.push("k".repeat(1025));
    }
    for key in &refused {
        let (status, shown) = on_terminal(&connect, key);
        assert_eq!(status, Some(1), "{key:?}: {shown}");
        assert!(shown.contains("error: key-refused: "), "{key:?}: {shown}");
    }
    assert_eq!(
        stdout(&accounts.run(&["accounts", "list"])),
        "nothing connected\n"
    );

    let key = "k3y-typed_at.the~terminal";
    let (status, shown) = on_terminal(&connect, key);
    assert_eq!(status, Some(0), "{shown}");
    assert!(shown.contains("connected: key"), "{shown}");
    assert!(!shown.contains(key), "the key was echoed: {shown}");
    let record = std::fs::read_to_string(accounts.dir.path().join("accounts.json")).unwrap();
    assert!(!record.contains(key), "{record}");
    // A second key while one is kept
    let (status, shown) = on_terminal(&connect, "another-k3y");
    assert_eq!(status, Some(1), "{shown}");
    assert!(
        shown.contains("error: another-account-connected: "),
        "{shown}"
    );

    let listed = stdout(&accounts.run(&["accounts", "list"]));
    assert!(
        listed.starts_with(&format!(
            "local/keyed  local file {}\n  account: key, connected ",
            file.display()
        )),
        "{listed}"
    );
    // The connector's file moves: listed as gone, and disconnected by its origin
    std::fs::rename(&file, accounts.dir.path().join("connectors/moved.json")).unwrap();
    let listed = stdout(&accounts.run(&["accounts", "list"]));
    assert!(
        listed.starts_with(&format!(
            "local/keyed  local file {} (no file)\n",
            file.display()
        )),
        "{listed}"
    );
    let output = accounts.run(&["accounts", "disconnect", "local/keyed"]);
    assert_eq!(output.status.code(), Some(1), "{}", stdout(&output));
    assert!(stderr(&output).contains("--origin"), "{}", stderr(&output));
    let output = accounts.run(&[
        "accounts",
        "disconnect",
        "local/keyed",
        "--origin",
        &file.to_string_lossy(),
    ]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    assert_eq!(
        stdout(&output),
        "disconnected local/keyed: key
deleted from the keychain
the service offers no revocation: revoke it at the service
kept data discarded: none
"
    );
    assert_eq!(
        stdout(&accounts.run(&["accounts", "list"])),
        "nothing connected\n"
    );
}

/// `accounts disconnect` deletes at once the kept data read as the account. The account and the
/// data come from the library, through the seam and the mock vault, in the data directory the
/// CLI then works on.
#[test]
fn accounts_disconnect_discards_the_kept_data_read_as_the_account() {
    use common::signin::SignInService;

    let pki = common::servers::TestPki::new();
    let service = SignInService::start(&pki);
    let world = common::World::seam(
        runtime::TestNetwork {
            routes: vec![service.route()],
            public_for_tests: vec![service.server.address()],
            root: pki.root(),
            catalog: Vec::new(),
        },
        true,
    );
    world.write_connector("notes", &serde_json::from_str(NOTES).unwrap());
    world
        .runtime
        .connect_account("local/notes", Some("dashboard-dev"), &service.browser())
        .unwrap();
    let package = std::fs::read(fixture_path("private-notes")).unwrap();
    let plugin_file = world.data.path().join("private-notes.wasm");
    std::fs::write(&plugin_file, &package).unwrap();
    let origin = origin(&plugin_file);
    world.approve_at(&package, &origin);
    let update = world.load_at(&package, &origin).update("{}");
    assert!(update.result.is_ok(), "{update:?}");
    let kept = || -> Vec<PathBuf> {
        std::fs::read_dir(world.data.path().join("widget-data"))
            .map(|entries| entries.map(|entry| entry.unwrap().path()).collect())
            .unwrap_or_default()
    };
    assert_eq!(kept().len(), 1);

    let output = runtime(&[
        "accounts",
        "disconnect",
        "local/notes",
        "--grants",
        path_str(&world.data.path().join("grants.json")),
        "--data",
        path_str(world.data.path()),
    ]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    // The CLI's vault is the system's keychain, which never held this test's token
    assert_eq!(
        stdout(&output),
        "disconnected local/notes: ada (id 1234567)
deleted from the keychain
revocation failed: the keychain held no token to revoke; revoke it at the service
kept data discarded: local/private-notes
"
    );
    assert!(kept().is_empty(), "{:?}", kept());
    common::signin::scan(
        &[world.data.path()],
        &[("the output", &stdout(&output))],
        &service.secrets(),
    );
}
