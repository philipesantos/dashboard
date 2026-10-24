//! The network module through the runtime (`specs/003-plugin-connectors/`, FR-003, FR-007,
//! FR-008; research R3, R5–R10): `net-probe` calls connectors written in the test, catalog ones
//! added through the seam and reached over HTTPS through its routes, against test services
//! that record every request. Nothing reaches the internet.

mod common;

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use common::servers::{Reply, Server, TestPki};
use common::{World, answers, calls, connected_manifest, probe};
use runtime::{Limits, Route, TestNetwork};
use serde_json::{Value, json};

/// The catalog connector `catalog/echo`, reserving `echo.test`, with `operations`.
fn catalog_echo(destinations: Value, operations: Value) -> String {
    json!({
        "connector_version": 1,
        "id": "catalog/echo",
        "version": "1.0.0",
        "author": "Tests",
        "description": "Test services",
        "reserves": ["echo.test"],
        "destinations": destinations,
        "operations": operations
    })
    .to_string()
}

/// A GET read of `path` on the destination `api`.
fn get(path: &str, parameters: Value, query: Value, headers: &[&str]) -> Value {
    json!({
        "kind": "read", "method": "GET", "destination": "api", "path": path,
        "query": query, "parameters": parameters,
        "response": { "format": "json", "headers": headers }
    })
}

/// A route from `host:port` to `to`.
fn route(host: &str, port: u16, to: SocketAddr) -> Route {
    Route {
        host: host.to_owned(),
        port,
        answers: vec![vec![to]],
    }
}

/// A world whose catalog has `connector`, with `api.echo.test:443` routed to `server` and every
/// test server address counted as public.
struct Echo {
    /// Keeps the data directory and the grant record alive while the plugin runs.
    _world: World,
    plugin: runtime::Plugin,
}

impl Echo {
    fn new(
        pki: &TestPki,
        connector: String,
        routes: Vec<Route>,
        public: Vec<SocketAddr>,
        operations: &[&str],
    ) -> Echo {
        Echo::with_limits(
            Limits::default(),
            pki,
            connector,
            routes,
            public,
            operations,
        )
    }

    fn with_limits(
        limits: Limits,
        pki: &TestPki,
        connector: String,
        routes: Vec<Route>,
        public: Vec<SocketAddr>,
        operations: &[&str],
    ) -> Echo {
        let world = World::seam_with(
            limits,
            TestNetwork {
                routes,
                public_for_tests: public,
                root: pki.root(),
                catalog: vec![connector],
            },
            false,
        );
        let package = probe(&connected_manifest(
            "local/net-probe",
            &[("catalog/echo", 1, operations, true)],
        ));
        let plugin = world.approve_and_load(&package);
        Echo {
            _world: world,
            plugin,
        }
    }

    /// Runs one update making `calls` and returns its answers.
    fn update(&self, config: &str) -> Vec<Value> {
        let report = self.plugin.update(config);
        answers(
            &report
                .result
                .expect("net-probe's update returns its answers"),
        )
    }
}

fn one(answers: Vec<Value>) -> Value {
    assert_eq!(answers.len(), 1, "{answers:?}");
    answers.into_iter().next().unwrap()
}

#[track_caller]
fn assert_error(answer: &Value, error: &str) {
    assert_eq!(answer["error"], error, "{answer}");
}

#[test]
fn a_declared_get_returns_the_status_the_declared_headers_and_the_body() {
    let pki = TestPki::new();
    let server = Server::https(&pki.leaf(&["api.echo.test"]));
    server.route(
        "/v1/items",
        Reply::json(r#"{"items":[1,2]}"#)
            .header("ETag", "\"abc\"")
            .header("X-Secret", "never passed on")
            .header("Set-Cookie", "session=1"),
    );
    let echo = Echo::new(
        &pki,
        catalog_echo(
            json!({ "api": "https://api.echo.test" }),
            json!({ "items": get("/v1/items", json!({}), json!({}), &["ETag", "Link"]) }),
        ),
        vec![route("api.echo.test", 443, server.address())],
        vec![server.address()],
        &["items"],
    );
    let answer = one(echo.update(&calls(&[("catalog/echo", "items", json!([]))])));
    assert_eq!(answer["status"], 200, "{answer}");
    assert_eq!(answer["body"], r#"{"items":[1,2]}"#);
    assert_eq!(
        answer["headers"],
        json!([{ "name": "ETag", "value": "\"abc\"" }, { "name": "Link", "value": null }])
    );
    let received = server.received();
    assert_eq!(received.len(), 1);
    assert_eq!(received[0].method, "GET");
    assert_eq!(received[0].path, "/v1/items");
}

#[test]
fn the_host_sets_its_own_user_agent_asks_for_no_compression_and_sends_no_cookie() {
    let pki = TestPki::new();
    let server = Server::https(&pki.leaf(&["api.echo.test"]));
    server.route(
        "/",
        Reply::json("{}").header("Set-Cookie", "session=secret; Path=/"),
    );
    let echo = Echo::new(
        &pki,
        catalog_echo(
            json!({ "api": "https://api.echo.test" }),
            json!({ "root": get("/", json!({}), json!({}), &[]) }),
        ),
        vec![route("api.echo.test", 443, server.address())],
        vec![server.address()],
        &["root"],
    );
    let config = calls(&[
        ("catalog/echo", "root", json!([])),
        ("catalog/echo", "root", json!([])),
    ]);
    for answer in echo.update(&config) {
        assert_eq!(answer["status"], 200, "{answer}");
    }
    // And in a second update, with new agents
    echo.update(&calls(&[("catalog/echo", "root", json!([]))]));
    let received = server.received();
    assert_eq!(received.len(), 3);
    for request in received {
        assert_eq!(
            request.header("User-Agent"),
            Some(format!("dashboard/{}", env!("CARGO_PKG_VERSION")).as_str())
        );
        assert_eq!(request.header("Accept-Encoding"), None, "{request:?}");
        assert_eq!(request.header("Cookie"), None, "{request:?}");
        assert_eq!(request.header("Host"), Some("api.echo.test"));
    }
}

#[test]
fn values_reach_the_server_inside_their_own_segment_or_query_value() {
    let pki = TestPki::new();
    let server = Server::https(&pki.leaf(&["api.echo.test"]));
    let echo = Echo::new(
        &pki,
        catalog_echo(
            json!({ "api": "https://api.echo.test" }),
            json!({ "item": get(
                "/v1/{id}/items",
                json!({
                    "id": { "type": "text", "max_length": 64 },
                    "q": { "type": "text", "max_length": 64 }
                }),
                json!({ "q": "{q}" }),
                &[]
            ) }),
        ),
        vec![route("api.echo.test", 443, server.address())],
        vec![server.address()],
        &["item"],
    );
    let hostile = [
        "a/b", "a?b", "a#b", "&x=1", "=", "%2F", "%2e%2e", "é ü", "...",
    ];
    for value in hostile {
        let answer = one(echo.update(&calls(&[(
            "catalog/echo",
            "item",
            json!([["id", value], ["q", value]]),
        )])));
        assert_eq!(answer["status"], 404, "{value}: {answer}");
    }
    let received = server.received();
    assert_eq!(received.len(), hostile.len());
    for (request, value) in received.iter().zip(hostile) {
        let segments: Vec<&str> = request.path.split('/').collect();
        assert_eq!(
            segments.len(),
            4,
            "{value} added a segment: {}",
            request.path
        );
        assert_eq!(segments[3], "items");
        let decoded: Vec<(String, String)> =
            url::form_urlencoded::parse(request.query.as_deref().unwrap_or_default().as_bytes())
                .into_owned()
                .collect();
        assert_eq!(decoded, [("q".to_owned(), value.to_owned())], "{value}");
    }
}

#[test]
fn body_values_reach_the_server_as_json_values_of_their_type() {
    let pki = TestPki::new();
    let server = Server::https(&pki.leaf(&["api.echo.test"]));
    let search = json!({
        "kind": "read", "method": "POST", "destination": "api", "path": "/graphql",
        "body": { "query": "{q}", "page": "{page}", "all": "{all}", "fixed": "literal" },
        "parameters": {
            "q": { "type": "text", "max_length": 64 },
            "page": { "type": "integer", "minimum": 1, "maximum": 9, "required": false },
            "all": { "type": "flag", "required": false }
        },
        "response": { "format": "json" }
    });
    let echo = Echo::new(
        &pki,
        catalog_echo(
            json!({ "api": "https://api.echo.test" }),
            json!({ "search": search }),
        ),
        vec![route("api.echo.test", 443, server.address())],
        vec![server.address()],
        &["search"],
    );
    let hostile = "a\"b\\c}d\ne\u{1}f";
    echo.update(&calls(&[(
        "catalog/echo",
        "search",
        json!([["q", hostile], ["page", 3], ["all", true]]),
    )]));
    echo.update(&calls(&[("catalog/echo", "search", json!([["q", "x"]]))]));
    let received = server.received();
    assert_eq!(received.len(), 2);
    assert_eq!(received[0].method, "POST");
    assert_eq!(received[0].header("Content-Type"), Some("application/json"));
    let body: Value = serde_json::from_slice(&received[0].body).unwrap();
    assert_eq!(
        body,
        json!({ "query": hostile, "page": 3, "all": true, "fixed": "literal" })
    );
    let body: Value = serde_json::from_slice(&received[1].body).unwrap();
    assert_eq!(body, json!({ "query": "x", "fixed": "literal" }));
}

#[test]
fn a_name_that_leads_only_to_non_public_addresses_fails_before_any_connection() {
    let pki = TestPki::new();
    let server = Server::https(&pki.leaf(&["api.echo.test"]));
    let private: SocketAddr = "10.0.0.1:443".parse().unwrap();
    let loopback: SocketAddr = format!("127.0.0.1:{}", server.port()).parse().unwrap();
    let echo = Echo::new(
        &pki,
        catalog_echo(
            json!({ "api": "https://api.echo.test" }),
            json!({ "root": get("/", json!({}), json!({}), &[]) }),
        ),
        // The server itself, but not counted as public: as a name that leads to this machine
        vec![Route {
            host: "api.echo.test".to_owned(),
            port: 443,
            answers: vec![vec![private, loopback]],
        }],
        vec![],
        &["root"],
    );
    let answer = one(echo.update(&calls(&[("catalog/echo", "root", json!([]))])));
    assert_error(&answer, "failed:non-public-address");
    assert!(
        answer["detail"].as_str().unwrap().contains("api.echo.test"),
        "{answer}"
    );
    assert!(server.received().is_empty());
}

#[test]
fn a_name_that_turns_private_after_a_first_call_is_refused_at_the_next() {
    let pki = TestPki::new();
    let server = Server::https(&pki.leaf(&["api.echo.test"]));
    let echo = Echo::new(
        &pki,
        catalog_echo(
            json!({ "api": "https://api.echo.test" }),
            json!({ "root": get("/", json!({}), json!({}), &[]) }),
        ),
        vec![Route {
            host: "api.echo.test".to_owned(),
            port: 443,
            answers: vec![
                vec![server.address()],
                vec!["192.168.1.1:443".parse().unwrap()],
            ],
        }],
        vec![server.address()],
        &["root"],
    );
    let answers = echo.update(&calls(&[
        ("catalog/echo", "root", json!([])),
        ("catalog/echo", "root", json!([])),
    ]));
    assert_eq!(answers[0]["status"], 404, "{answers:?}");
    assert_error(&answers[1], "failed:non-public-address");
    assert_eq!(server.received().len(), 1);
}

/// A catalog connector with destinations `api` (`api.echo.test`), `files`
/// (`files.echo.test`) and `private` (`private.echo.test`), and a read `go` of `/start`.
fn redirecting(pki: &TestPki, start: Reply) -> (Echo, Server, Server) {
    let api = Server::https(&pki.leaf(&["api.echo.test"]));
    let files = Server::https(&pki.leaf(&["files.echo.test"]));
    api.route("/start", start);
    api.route("/next", Reply::json(r#"{"at":"next"}"#));
    files.route("/there", Reply::json(r#"{"at":"files"}"#));
    let echo = Echo::new(
        pki,
        catalog_echo(
            json!({
                "api": "https://api.echo.test",
                "files": "https://files.echo.test",
                "private": "https://private.echo.test"
            }),
            json!({ "go": get("/start", json!({}), json!({}), &[]) }),
        ),
        vec![
            route("api.echo.test", 443, api.address()),
            route("files.echo.test", 443, files.address()),
            route("private.echo.test", 443, "10.1.2.3:443".parse().unwrap()),
            route("elsewhere.test", 443, files.address()),
            route("api.echo.test", 8443, files.address()),
        ],
        vec![api.address(), files.address()],
        &["go"],
    );
    (echo, api, files)
}

fn go(echo: &Echo) -> Value {
    one(echo.update(&calls(&[("catalog/echo", "go", json!([]))])))
}

#[test]
fn redirects_are_followed_to_the_same_host_and_to_the_connectors_other_origins() {
    let pki = TestPki::new();
    let (echo, api, _files) = redirecting(&pki, Reply::redirect(302, "/next"));
    let answer = go(&echo);
    assert_eq!(answer["status"], 200, "{answer}");
    assert_eq!(answer["body"], r#"{"at":"next"}"#);
    assert_eq!(api.received().len(), 2);

    let (echo, _api, files) =
        redirecting(&pki, Reply::redirect(301, "https://files.echo.test/there"));
    let answer = go(&echo);
    assert_eq!(answer["body"], r#"{"at":"files"}"#, "{answer}");
    assert_eq!(files.received().len(), 1);
}

#[test]
fn redirects_anywhere_else_are_not_followed_and_nothing_reaches_the_target() {
    let pki = TestPki::new();
    for (location, rule) in [
        (
            "https://elsewhere.test/there",
            "failed:redirect-not-allowed",
        ),
        (
            "https://api.echo.test:8443/there",
            "failed:redirect-not-allowed",
        ),
        ("http://api.echo.test/there", "failed:redirect-not-allowed"),
        (
            "https://private.echo.test/there",
            "failed:redirect-not-allowed",
        ),
    ] {
        let (echo, api, files) = redirecting(&pki, Reply::redirect(302, location));
        let answer = go(&echo);
        assert_error(&answer, rule);
        assert_eq!(api.received().len(), 1, "{location}");
        assert!(files.received().is_empty(), "{location} reached its target");
    }
}

#[test]
fn past_five_redirects_the_host_follows_no_more() {
    let pki = TestPki::new();
    let (echo, api, _files) = redirecting(&pki, Reply::redirect(302, "/start"));
    let answer = go(&echo);
    assert_error(&answer, "failed:redirect-not-allowed");
    assert_eq!(
        api.received().len(),
        6,
        "the first request and five redirects"
    );
}

#[test]
fn certificates_that_are_not_valid_for_the_destination_fail_with_nothing_sent() {
    let pki = TestPki::new();
    for (case, cert) in [
        ("wrong name", pki.leaf(&["other.echo.test"])),
        ("untrusted CA", pki.untrusted(&["api.echo.test"])),
        ("expired", pki.expired(&["api.echo.test"])),
    ] {
        let server = Server::https(&cert);
        let echo = Echo::new(
            &pki,
            catalog_echo(
                json!({ "api": "https://api.echo.test" }),
                json!({ "root": get("/", json!({}), json!({}), &[]) }),
            ),
            vec![route("api.echo.test", 443, server.address())],
            vec![server.address()],
            &["root"],
        );
        let answer = one(echo.update(&calls(&[("catalog/echo", "root", json!([]))])));
        assert_error(&answer, "failed:unavailable");
        assert!(
            server.received().is_empty(),
            "{case}: a request passed the handshake"
        );
    }
}

#[test]
fn the_verifier_fetches_nothing_the_certificate_points_at() {
    let pki = TestPki::new();
    let recorder = Server::recorder();
    let server =
        Server::https(&pki.with_fetch_addresses(&["api.echo.test"], &recorder.http_origin()));
    server.route("/", Reply::json("{}"));
    let echo = Echo::new(
        &pki,
        catalog_echo(
            json!({ "api": "https://api.echo.test" }),
            json!({ "root": get("/", json!({}), json!({}), &[]) }),
        ),
        vec![route("api.echo.test", 443, server.address())],
        vec![server.address()],
        &["root"],
    );
    let answer = one(echo.update(&calls(&[("catalog/echo", "root", json!([]))])));
    assert_eq!(answer["status"], 200, "{answer}");
    // A wrong-name certificate with the same addresses, so the verifier would look harder
    let bad =
        Server::https(&pki.with_fetch_addresses(&["other.echo.test"], &recorder.http_origin()));
    let echo = Echo::new(
        &pki,
        catalog_echo(
            json!({ "api": "https://api.echo.test" }),
            json!({ "root": get("/", json!({}), json!({}), &[]) }),
        ),
        vec![route("api.echo.test", 443, bad.address())],
        vec![bad.address()],
        &["root"],
    );
    assert_error(
        &one(echo.update(&calls(&[("catalog/echo", "root", json!([]))]))),
        "failed:unavailable",
    );
    assert!(recorder.received().is_empty(), "{:?}", recorder.received());
}

#[test]
fn responses_and_requests_over_their_limits_fail_and_the_widget_gets_nothing() {
    let pki = TestPki::new();
    let server = Server::https(&pki.leaf(&["api.echo.test"]));
    server.route("/big", Reply::json_of_size(1024 * 1024 + 1));
    server.route("/fits", Reply::json_of_size(1024 * 1024));
    server.route(
        "/headers",
        Reply::json("{}").header("X-Big", &"h".repeat(17 * 1024)),
    );
    let text = |name: &str| {
        (
            name.to_owned(),
            json!({ "type": "text", "max_length": 2048 }),
        )
    };
    let big_body: serde_json::Map<String, Value> = (0..6)
        .map(|i| (format!("f{i}"), json!(format!("{{p{i}}}"))))
        .collect();
    let body_parameters: serde_json::Map<String, Value> =
        (0..6).map(|i| text(&format!("p{i}"))).collect();
    let echo = Echo::new(
        &pki,
        catalog_echo(
            json!({ "api": "https://api.echo.test" }),
            json!({
                "big": get("/big", json!({}), json!({}), &[]),
                "fits": get("/fits", json!({}), json!({}), &[]),
                "headers": get("/headers", json!({}), json!({}), &[]),
                "address": get("/a/{v}", json!({ "v": { "type": "text", "max_length": 2048 } }), json!({}), &[]),
                "body": {
                    "kind": "read", "method": "POST", "destination": "api", "path": "/body",
                    "body": big_body, "parameters": body_parameters,
                    "response": { "format": "json" }
                }
            }),
        ),
        vec![route("api.echo.test", 443, server.address())],
        vec![server.address()],
        &["big", "fits", "headers", "address", "body"],
    );
    let config = json!({
        "bodies": "size",
        "calls": [
            { "connector": "catalog/echo", "operation": "big", "arguments": [] },
            { "connector": "catalog/echo", "operation": "fits", "arguments": [] },
            { "connector": "catalog/echo", "operation": "headers", "arguments": [] },
            { "connector": "catalog/echo", "operation": "address", "arguments": [["v", "é".repeat(2048)]] },
            { "connector": "catalog/echo", "operation": "body",
              "arguments": (0..6).map(|i| json!([format!("p{i}"), "\u{1}".repeat(2048)])).collect::<Vec<_>>() }
        ]
    })
    .to_string();
    let answers = echo.update(&config);
    assert_error(&answers[0], "failed:over-limit");
    assert!(answers[0].get("body_bytes").is_none());
    assert_eq!(answers[1]["body_bytes"], 1024 * 1024, "{}", answers[1]);
    assert_error(&answers[2], "failed:over-limit");
    assert_error(&answers[3], "refused:over-limit");
    assert_error(&answers[4], "refused:over-limit");
    let paths: Vec<String> = server.received().into_iter().map(|r| r.path).collect();
    assert_eq!(
        paths,
        ["/big", "/fits", "/headers"],
        "nothing over a request limit was sent"
    );
}

#[test]
fn a_slow_service_fails_at_the_calls_time_limit() {
    let pki = TestPki::new();
    let server = Server::https(&pki.leaf(&["api.echo.test"]));
    server.route("/slow", Reply::json("{}").delay(Duration::from_secs(3)));
    let echo = Echo::with_limits(
        Limits {
            call_time_limit: Duration::from_secs(1),
            ..Limits::default()
        },
        &pki,
        catalog_echo(
            json!({ "api": "https://api.echo.test" }),
            json!({ "slow": get("/slow", json!({}), json!({}), &[]) }),
        ),
        vec![route("api.echo.test", 443, server.address())],
        vec![server.address()],
        &["slow"],
    );
    let start = Instant::now();
    let answer = one(echo.update(&calls(&[("catalog/echo", "slow", json!([]))])));
    let took = start.elapsed();
    assert_error(&answer, "failed:over-limit");
    assert!(took < Duration::from_millis(1500), "took {took:?}");
}

#[test]
fn a_response_in_another_format_than_declared_is_invalid() {
    let pki = TestPki::new();
    let server = Server::https(&pki.leaf(&["api.echo.test"]));
    server.route(
        "/html",
        Reply::text("<html>").header("Content-Type", "text/html"),
    );
    server.route("/none", Reply::status(200).body(b"{}".to_vec()));
    let echo = Echo::new(
        &pki,
        catalog_echo(
            json!({ "api": "https://api.echo.test" }),
            json!({
                "html": get("/html", json!({}), json!({}), &[]),
                "none": get("/none", json!({}), json!({}), &[])
            }),
        ),
        vec![route("api.echo.test", 443, server.address())],
        vec![server.address()],
        &["html", "none"],
    );
    let answers = echo.update(&calls(&[
        ("catalog/echo", "html", json!([])),
        ("catalog/echo", "none", json!([])),
    ]));
    assert_error(&answers[0], "failed:invalid-response");
    assert_error(&answers[1], "failed:invalid-response");
}

#[test]
fn an_error_status_is_a_normal_answer() {
    let pki = TestPki::new();
    let server = Server::https(&pki.leaf(&["api.echo.test"]));
    server.route(
        "/gone",
        Reply::status(410)
            .header("Content-Type", "application/json")
            .body(br#"{"message":"gone"}"#.to_vec()),
    );
    let echo = Echo::new(
        &pki,
        catalog_echo(
            json!({ "api": "https://api.echo.test" }),
            json!({ "gone": get("/gone", json!({}), json!({}), &[]) }),
        ),
        vec![route("api.echo.test", 443, server.address())],
        vec![server.address()],
        &["gone"],
    );
    let answer = one(echo.update(&calls(&[("catalog/echo", "gone", json!([]))])));
    assert_eq!(answer["status"], 410, "{answer}");
    assert_eq!(answer["body"], r#"{"message":"gone"}"#);
}
