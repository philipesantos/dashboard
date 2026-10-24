//! Updates and renders of connected widgets (`specs/003-plugin-connectors/`, User Stories 1 and
//! 3): the example widget through the catalog's GitHub connector, routed by the test seam to a
//! test service, and `net-probe` for what the rules refuse. Nothing reaches the internet.

mod common;

use common::servers::{Reply, Server, TestPki};
use common::{World, answers, calls, connected_manifest, fixture, probe};
use runtime::{ErrorKind, Route, TestNetwork};
use serde_json::{Value, json};

const RELEASES: &str = r#"[
    {"name": "Rust 1.97.0", "tag_name": "1.97.0", "published_at": "2026-10-01T00:00:00Z",
     "html_url": "https://github.com/rust-lang/rust/releases/tag/1.97.0", "body": "…"},
    {"name": "Rust 1.96.0", "tag_name": "1.96.0", "published_at": "2026-08-20T00:00:00Z",
     "html_url": "https://github.com/rust-lang/rust/releases/tag/1.96.0"}
]"#;

/// A test service answering as `api.github.com` does for the example, and a world whose seam
/// routes `api.github.com:443` to it.
fn github(pki: &TestPki) -> (Server, World) {
    let server = Server::https(&pki.leaf(&["api.github.com"]));
    server.route(
        "/repos/rust-lang/rust/releases",
        Reply::json(RELEASES).header("Content-Type", "application/json; charset=utf-8"),
    );
    let world = World::seam(
        TestNetwork {
            routes: vec![Route {
                host: "api.github.com".to_owned(),
                port: 443,
                answers: vec![vec![server.address()]],
            }],
            public_for_tests: vec![server.address()],
            root: pki.root(),
            catalog: Vec::new(),
        },
        false,
    );
    (server, world)
}

const CONFIG: &str = r#"{"owner":"rust-lang","repo":"rust"}"#;

#[test]
fn the_example_updates_through_catalog_github_and_renders_the_releases() {
    let pki = TestPki::new();
    let (server, world) = github(&pki);
    let plugin = world.approve_and_load(&fixture("github-releases"));

    // Before any update, render says there is no data yet
    let content = plugin.render(CONFIG).result.expect("renders");
    assert_eq!(content["text"], "No data yet");
    assert!(server.received().is_empty(), "render made a request");

    let report = plugin.update(CONFIG);
    let data = report.result.expect("the update succeeds");
    assert_eq!(data["releases"][0]["tag"], "1.97.0");
    assert_eq!(report.calls.calls, 1);
    assert_eq!(
        report.calls.refused + report.calls.failed + report.calls.not_granted,
        0
    );
    assert_eq!(report.calls.received_bytes, RELEASES.len() as u64);

    let received = server.received();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].method, "GET");
    assert_eq!(
        received[0].target,
        "/repos/rust-lang/rust/releases?per_page=5"
    );
    assert_eq!(
        received[0].header("Accept"),
        Some("application/vnd.github+json")
    );
    assert_eq!(
        received[0].header("X-GitHub-Api-Version"),
        Some("2022-11-28")
    );

    let content = plugin.render(CONFIG).result.expect("renders");
    assert_eq!(content["title"], "rust-lang/rust releases");
    assert_eq!(content["items"][1]["name"], "Rust 1.96.0");
    assert!(content["fetched_at_ms"].as_u64().is_some());
    assert_eq!(content["last_update"]["outcome"], "ok");
    assert_eq!(server.received().len(), 1, "render made a request");

    let log = world.runtime.read_requests(50, None).unwrap();
    assert_eq!(log.len(), 1);
    let entry = &log[0];
    assert_eq!(entry.plugin, "local/github-releases");
    assert_eq!(entry.connector, "catalog/github");
    assert_eq!(entry.operation, "list-releases");
    assert_eq!(entry.kind.as_deref(), Some("read"));
    assert_eq!(entry.method.as_deref(), Some("GET"));
    assert_eq!(entry.host.as_deref(), Some("api.github.com"));
    assert_eq!(
        entry.path.as_deref(),
        Some("/repos/rust-lang/rust/releases?per_page=5")
    );
    assert_eq!(entry.arguments["owner"], "rust-lang");
    assert_eq!(entry.arguments["repo"], "rust");
    assert_eq!(entry.arguments["per_page"], "5");
    assert_eq!(
        entry.outcome,
        runtime::requests::Outcome::Status { status: 200 }
    );
}

/// `net-probe` declaring `catalog/github` with `list-releases` only.
fn github_probe() -> Vec<u8> {
    probe(&connected_manifest(
        "local/net-probe",
        &[("catalog/github", 1, &["list-releases"], true)],
    ))
}

fn error(answer: &Value) -> &str {
    answer["error"]
        .as_str()
        .unwrap_or_else(|| panic!("not an error: {answer}"))
}

#[test]
fn calls_outside_the_declaration_are_refused_with_nothing_sent() {
    let pki = TestPki::new();
    let (server, world) = github(&pki);
    let plugin = world.approve_and_load(&github_probe());
    let report = plugin.update(&calls(&[
        // Declared by the connector, not by the manifest
        (
            "catalog/github",
            "get-repository",
            json!([["owner", "rust-lang"], ["repo", "rust"]]),
        ),
        // Outside its type: the owner pattern, and per_page's range
        (
            "catalog/github",
            "list-releases",
            json!([["owner", "rust lang"], ["repo", "rust"]]),
        ),
        (
            "catalog/github",
            "list-releases",
            json!([["owner", "rust-lang"], ["repo", "rust"], ["per_page", 101]]),
        ),
        (
            "catalog/github",
            "list-releases",
            json!([["owner", "rust-lang"]]),
        ),
        // A connector the manifest does not declare
        ("catalog/other", "list-releases", json!([])),
        ("local/anything", "get", json!([])),
    ]));
    let answers = answers(&report.result.expect("net-probe returns its answers"));
    let errors: Vec<&str> = answers.iter().map(error).collect();
    assert_eq!(
        errors,
        [
            "refused:operation-not-declared",
            "refused:value-outside-type",
            "refused:value-outside-type",
            "refused:value-outside-type",
            "not-granted",
            "not-granted",
        ]
    );
    assert!(
        answers[1]["detail"].as_str().unwrap().contains("owner"),
        "{}",
        answers[1]
    );
    assert!(
        answers[2]["detail"].as_str().unwrap().contains("per_page"),
        "{}",
        answers[2]
    );
    assert!(
        server.received().is_empty(),
        "a refused call reached the service"
    );
    assert_eq!(world.runtime.read_requests(50, None).unwrap().len(), 6);
}

#[test]
fn a_call_during_render_is_refused_and_reaches_nothing() {
    let pki = TestPki::new();
    let (server, world) = github(&pki);
    let plugin = world.approve_and_load(&github_probe());
    let config = json!({
        "render_calls": [{
            "connector": "catalog/github", "operation": "list-releases",
            "arguments": [["owner", "rust-lang"], ["repo", "rust"]]
        }]
    })
    .to_string();
    let content = plugin.render(&config).result.expect("renders");
    let answers = answers(&content);
    assert_eq!(error(&answers[0]), "refused:called-outside-update");
    assert!(server.received().is_empty());
}

#[test]
fn a_widget_without_updater_has_nothing_to_update() {
    let world = World::production(false);
    let plugin = world.approve_and_load(&fixture("hello-widget"));
    let report = plugin.update("{}");
    assert_eq!(report.result.unwrap_err().kind(), ErrorKind::MissingUpdater);
}

// User Story 3: the network stays out of rendering

use std::time::{Duration, Instant};

use runtime::{Limits, RuntimeOptions};

/// A world whose seam routes `api.github.com:443` to `server`.
fn github_world(pki: &TestPki, server: &Server, limits: Limits) -> World {
    World::seam_with(
        limits,
        TestNetwork {
            routes: vec![Route {
                host: "api.github.com".to_owned(),
                port: 443,
                answers: vec![vec![server.address()]],
            }],
            public_for_tests: vec![server.address()],
            root: pki.root(),
            catalog: Vec::new(),
        },
        false,
    )
}

#[test]
fn render_gets_the_kept_data_its_time_and_the_last_outcome_without_any_request() {
    let pki = TestPki::new();
    let (server, world) = github(&pki);
    let plugin = world.approve_and_load(&fixture("github-releases"));
    plugin.update(CONFIG).result.unwrap();
    let requests = server.received().len();
    let start = Instant::now();
    for _ in 0..5 {
        let content = plugin.render(CONFIG).result.unwrap();
        assert_eq!(content["items"][0]["tag"], "1.97.0");
        assert_eq!(content["last_update"]["outcome"], "ok");
    }
    assert!(start.elapsed() < Duration::from_secs(5));
    assert_eq!(server.received().len(), requests, "render made a request");
}

#[test]
fn a_failed_update_keeps_the_previous_data_and_render_shows_the_failure() {
    let pki = TestPki::new();
    let (server, world) = github(&pki);
    let plugin = world.approve_and_load(&fixture("github-releases"));
    plugin.update(CONFIG).result.unwrap();
    let fetched = plugin.render(CONFIG).result.unwrap()["fetched_at_ms"].clone();

    // An error status, which the example turns into an error
    server.route(
        "/repos/rust-lang/rust/releases",
        Reply::status(500)
            .header("Content-Type", "application/json")
            .body(b"{}".to_vec()),
    );
    let error = plugin.update(CONFIG).result.unwrap_err();
    assert_eq!(error.kind(), ErrorKind::PluginError);
    let content = plugin.render(CONFIG).result.unwrap();
    assert_eq!(
        content["items"][0]["tag"], "1.97.0",
        "the previous data stays"
    );
    assert_eq!(content["fetched_at_ms"], fetched);
    assert_eq!(content["last_update"]["outcome"], "failed");
    assert!(
        content["last_update"]["error"]
            .as_str()
            .unwrap()
            .contains("500"),
        "{content}"
    );

    // The service down
    server.stop();
    let error = plugin.update(CONFIG).result.unwrap_err();
    assert!(error.detail().contains("failed"), "{error}");
    let content = plugin.render(CONFIG).result.unwrap();
    assert_eq!(content["items"][0]["tag"], "1.97.0");
    assert_eq!(content["last_update"]["outcome"], "failed");
}

#[test]
fn output_that_is_not_json_or_too_large_and_a_refused_call_fail_the_update() {
    let pki = TestPki::new();
    let (_server, world) = github(&pki);
    let plugin = world.approve_and_load(&github_probe());
    let ok = calls(&[(
        "catalog/github",
        "list-releases",
        json!([["owner", "rust-lang"], ["repo", "rust"]]),
    )]);
    plugin.update(&ok).result.unwrap();
    let kept = plugin.render("{}").result.unwrap()["data"].clone();
    assert!(!kept.is_null());
    for (config, kind) in [
        (json!({ "output": "not-json" }).to_string(), ErrorKind::InvalidOutput),
        (json!({ "output": 256 * 1024 + 1 }).to_string(), ErrorKind::OutputTooLarge),
        (
            json!({ "fail": true, "calls": [{ "connector": "catalog/github", "operation": "get-repository", "arguments": [] }] }).to_string(),
            ErrorKind::PluginError,
        ),
    ] {
        let error = plugin.update(&config).result.unwrap_err();
        assert_eq!(error.kind(), kind, "{error}");
        let content = plugin.render("{}").result.unwrap();
        assert_eq!(content["data"], kept, "the previous data stays");
        assert_eq!(content["last_update"]["outcome"], "failed");
    }
    // Output of exactly 256 KiB is kept
    plugin
        .update(&json!({ "output": 256 * 1024 }).to_string())
        .result
        .unwrap();
}

#[test]
fn a_service_slower_than_render_allows_still_completes_within_the_update() {
    let pki = TestPki::new();
    let (server, world) = github(&pki);
    server.route(
        "/repos/rust-lang/rust/releases",
        Reply::json(RELEASES).delay(Duration::from_secs(3)),
    );
    let plugin = world.approve_and_load(&fixture("github-releases"));
    let data = plugin
        .update(CONFIG)
        .result
        .expect("the update waits for the service");
    assert_eq!(data["releases"][0]["tag"], "1.97.0");
}

#[test]
fn an_update_past_its_time_limit_is_stopped_and_its_calls_abandoned() {
    let pki = TestPki::new();
    let server = Server::https(&pki.leaf(&["api.github.com"]));
    server.route(
        "/repos/rust-lang/rust/releases",
        Reply::json(RELEASES).delay(Duration::from_secs(10)),
    );
    let world = github_world(
        &pki,
        &server,
        Limits {
            update_time_limit: Duration::from_secs(2),
            ..Limits::default()
        },
    );
    let plugin = world.approve_and_load(&fixture("github-releases"));
    let start = Instant::now();
    let error = plugin.update(CONFIG).result.unwrap_err();
    let took = start.elapsed();
    assert!(took < Duration::from_secs(4), "took {took:?}");
    // The call fails at the update's deadline, and the example ends with that error, or the
    // update is stopped first
    assert!(
        matches!(
            error.kind(),
            ErrorKind::TimeLimitExceeded | ErrorKind::PluginError
        ),
        "{error}"
    );
    assert_eq!(plugin.render(CONFIG).result.unwrap()["text"], "No data yet");
}

#[test]
fn kept_data_outlives_the_runtime_and_a_new_version_starts_without_it() {
    let pki = TestPki::new();
    let server = Server::https(&pki.leaf(&["api.github.com"]));
    server.route("/repos/rust-lang/rust/releases", Reply::json(RELEASES));
    let data = tempfile::TempDir::new().unwrap();
    let network = || TestNetwork {
        routes: vec![Route {
            host: "api.github.com".to_owned(),
            port: 443,
            answers: vec![vec![server.address()]],
        }],
        public_for_tests: vec![server.address()],
        root: pki.root(),
        catalog: Vec::new(),
    };
    let grants = std::sync::Arc::new(runtime::grants::GrantStore::open(
        data.path().join("grants.json"),
    ));
    let package = fixture("github-releases");
    {
        let runtime = runtime::Runtime::with_test_network(
            Limits::default(),
            RuntimeOptions::new(data.path()),
            network(),
        )
        .unwrap();
        let checked = runtime.check(&package).unwrap();
        checked
            .approve(
                &grants,
                common::TEST_ORIGIN,
                Some(&checked.approval_digest()),
            )
            .unwrap();
        let plugin = runtime
            .load(
                &package,
                common::TEST_ORIGIN,
                std::sync::Arc::clone(&grants),
            )
            .unwrap();
        plugin.update(CONFIG).result.unwrap();
    }
    // A later run of the host, with the service gone
    server.stop();
    let runtime = runtime::Runtime::with_test_network(
        Limits::default(),
        RuntimeOptions::new(data.path()),
        network(),
    )
    .unwrap();
    let plugin = runtime
        .load(
            &package,
            common::TEST_ORIGIN,
            std::sync::Arc::clone(&grants),
        )
        .unwrap();
    let content = plugin.render(CONFIG).result.unwrap();
    assert_eq!(content["items"][0]["tag"], "1.97.0");

    // A new version of the widget: no data from the old one
    let manifest = String::from_utf8(
        runtime::package::manifest_section(&package)
            .unwrap()
            .to_vec(),
    )
    .unwrap()
    .replace("\"0.1.0\"", "\"0.2.0\"");
    let newer = common::repack(&package, &manifest);
    let checked = runtime.check(&newer).unwrap();
    checked
        .approve(
            &grants,
            common::TEST_ORIGIN,
            Some(&checked.approval_digest()),
        )
        .unwrap();
    let plugin = runtime
        .load(&newer, common::TEST_ORIGIN, std::sync::Arc::clone(&grants))
        .unwrap();
    assert_eq!(plugin.render(CONFIG).result.unwrap()["text"], "No data yet");
}

/// SC-004: a service that takes 5 s to answer does not slow a render from kept data.
#[test]
fn a_slow_service_never_slows_a_render() {
    let pki = TestPki::new();
    let (server, world) = github(&pki);
    let plugin = world.approve_and_load(&fixture("github-releases"));
    plugin.update(CONFIG).result.unwrap();
    server.route(
        "/repos/rust-lang/rust/releases",
        Reply::json(RELEASES).delay(Duration::from_secs(5)),
    );
    for _ in 0..10 {
        let start = Instant::now();
        let report = plugin.render(CONFIG);
        assert!(start.elapsed() < Duration::from_secs(1), "render waited");
        assert_eq!(report.result.unwrap()["items"][0]["tag"], "1.97.0");
    }
}
