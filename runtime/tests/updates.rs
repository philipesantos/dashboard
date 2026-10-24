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
