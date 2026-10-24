//! Feature 003: the connector cases of `specs/003-plugin-connectors/data-model.md` (Hostile
//! cases, FR-020), the gate case of Phase 2: no request leaves the machine outside what a widget
//! declared and the user approved, and none reaches a destination it was not approved for.
//!
//! Every case runs on a [`Gate`]: a runtime whose test seam routes `api.github.com:443` to a
//! test service, so the well-behaved `github-releases` updates and renders normally right after
//! each case, on the same runtime. The test services record every request, and a refused case
//! must leave them empty. Nothing reaches the internet.

use std::net::SocketAddr;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use runtime::grants::GrantStore;
use runtime::manifest::PluginId;
use runtime::{
    ErrorKind, Limits, Plugin, Route, Runtime, RuntimeError, RuntimeOptions, TestNetwork,
};
use serde_json::{Value, json};
use tempfile::TempDir;

use crate::common::servers::{Reply, Server, TestPki};
use crate::common::{TEST_ORIGIN, World, answers, calls, connected_manifest, fixture, probe};

const RELEASES: &str = r#"[{"name":"Rust 1.97.0","tag_name":"1.97.0","published_at":"2026-10-01T00:00:00Z","html_url":"https://github.com/rust-lang/rust/releases/tag/1.97.0"}]"#;
const GITHUB_CONFIG: &str = r#"{"owner":"rust-lang","repo":"rust"}"#;

/// A runtime whose seam routes `api.github.com:443` to a test service, with what a case adds.
pub(crate) struct Gate {
    pub(crate) pki: TestPki,
    pub(crate) github: Server,
    pub(crate) world: World,
}

/// What a case adds to the seam.
#[derive(Default)]
pub(crate) struct Extra {
    pub(crate) routes: Vec<Route>,
    pub(crate) public: Vec<SocketAddr>,
    pub(crate) catalog: Vec<String>,
    pub(crate) limits: Option<Limits>,
    pub(crate) development: Option<bool>,
}

impl Gate {
    pub(crate) fn new() -> Gate {
        Gate::with(|_| Extra::default())
    }

    /// `extra` gets the test PKI, to make the certificates of the services it routes to.
    pub(crate) fn with(extra: impl FnOnce(&TestPki) -> Extra) -> Gate {
        let pki = TestPki::new();
        let github = Server::https(&pki.leaf(&["api.github.com"]));
        github.route("/repos/rust-lang/rust/releases", Reply::json(RELEASES));
        let extra = extra(&pki);
        let mut routes = vec![route("api.github.com", 443, github.address())];
        routes.extend(extra.routes);
        let mut public = vec![github.address()];
        public.extend(extra.public);
        let world = World::seam_with(
            extra.limits.unwrap_or_default(),
            TestNetwork {
                routes,
                public_for_tests: public,
                root: pki.root(),
                catalog: extra.catalog,
            },
            extra.development.unwrap_or(true),
        );
        Gate { pki, github, world }
    }

    /// Acceptance scenario 2 of User Story 2: right after a hostile case, the well-behaved
    /// example updates through `catalog/github` and renders its data, on the same runtime.
    pub(crate) fn assert_well_behaved(&self) {
        let before = self.github.received().len();
        let plugin = self.world.approve_and_load(&fixture("github-releases"));
        let report = plugin.update(GITHUB_CONFIG);
        let data = report
            .result
            .expect("github-releases updates after a hostile case");
        assert_eq!(data["releases"][0]["tag"], "1.97.0");
        let content = plugin.render(GITHUB_CONFIG).result.expect("and renders");
        assert_eq!(content["items"][0]["name"], "Rust 1.97.0");
        assert_eq!(self.github.received().len(), before + 1);
    }

    /// Writes the local connector `local/<name>`.
    pub(crate) fn connector(&self, name: &str, connector: &Value) {
        self.world.write_connector(name, connector);
    }

    /// The error that loading `package` ends with, approved or not.
    pub(crate) fn load_error(&self, package: &[u8]) -> RuntimeError {
        match self.world.runtime.check(package) {
            Err(error) => error,
            Ok(checked) => {
                let digest = checked.approval_digest();
                if let Err(error) = checked.approve(&self.world.grants, TEST_ORIGIN, Some(&digest))
                {
                    return error;
                }
                match self
                    .world
                    .runtime
                    .load(package, TEST_ORIGIN, Arc::clone(&self.world.grants))
                {
                    Ok(_) => panic!("the package loaded, but it must be refused"),
                    Err(error) => error,
                }
            }
        }
    }
}

pub(crate) fn route(host: &str, port: u16, to: SocketAddr) -> Route {
    Route {
        host: host.to_owned(),
        port,
        answers: vec![vec![to]],
    }
}

/// A local connector `local/<name>` with one read `get` of `/` on `destination`, edited.
pub(crate) fn local(name: &str, destination: &str, edit: impl FnOnce(&mut Value)) -> Value {
    let mut connector = json!({
        "connector_version": 1,
        "id": format!("local/{name}"),
        "version": "0.1.0",
        "author": "A hostile author",
        "description": "A hostile connector",
        "destinations": { "api": destination },
        "operations": {
            "get": { "kind": "read", "method": "GET", "destination": "api", "path": "/",
                     "response": { "format": "json" } }
        }
    });
    edit(&mut connector);
    connector
}

/// `net-probe` declaring `local/<name>` (major 0) with `operations`, required.
pub(crate) fn probe_of(name: &str, operations: &[&str]) -> Vec<u8> {
    probe(&connected_manifest(
        "local/net-probe",
        &[(&format!("local/{name}"), 0, operations, true)],
    ))
}

#[track_caller]
pub(crate) fn assert_kind(error: &RuntimeError, kind: ErrorKind, mentions: &[&str]) {
    assert_eq!(error.kind(), kind, "{error}");
    for part in mentions {
        assert!(
            error.to_string().contains(part),
            "{error} should name `{part}`"
        );
    }
}

// T041: cases refused when the widget or its connector loads or is approved

/// `generic-http` (FR-001): generic HTTP from WASI is no capability the host offers.
#[test]
fn generic_http() {
    let gate = Gate::new();
    for import in [
        "wasi:http/outgoing-handler@0.2.0",
        "wasi:http/outgoing-handler@0.2.6",
        "wasi:http/handler@0.3.0",
        "wasi:http/client@0.3.0",
        "wasi:http/types@0.3.0",
    ] {
        let wat = format!(
            r#"(component (import "{import}" (instance)) (instance $r) (export "dashboard:widget/renderer@1.1.0" (instance $r)))"#
        );
        let component = wat::parse_str(&wat).unwrap();
        let package = runtime::package::pack(
            &component,
            crate::common::manifest("local/http", "0.1.0", &[]).as_bytes(),
        )
        .unwrap();
        assert_kind(
            &gate.load_error(&package),
            ErrorKind::CapabilityRefused,
            &[import],
        );
    }
    gate.assert_well_behaved();
}

/// `read-with-body` (FR-002): outside the catalog, a read is a body-less GET or HEAD.
#[test]
fn read_with_body() {
    let gate = Gate::new();
    let edits: [fn(&mut Value); 3] = [
        |c| c["operations"]["get"]["method"] = json!("POST"),
        |c| c["operations"]["get"]["body"] = json!({ "q": "x" }),
        |c| {
            c["operations"]["get"]["method"] = json!("HEAD");
            c["operations"]["get"]["body"] = json!({ "q": "x" });
        },
    ];
    for edit in edits {
        gate.connector("echo", &local("echo", "https://api.echo.test", edit));
        assert_kind(
            &gate.load_error(&probe_of("echo", &["get"])),
            ErrorKind::InvalidConnector,
            &["operations.get", "change"],
        );
    }
    gate.assert_well_behaved();
}

/// `fixed-headers` (FR-002, Principle IV): a local connector fixes no `X-` header, and no
/// connector fixes the headers the host sets or forbids.
#[test]
fn fixed_headers() {
    let gate = Gate::new();
    for header in [
        "X-HTTP-Method-Override",
        "X-Original-URL",
        "X-Anything",
        "Authorization",
        "Cookie",
        "Proxy-Authorization",
        "Host",
        "User-Agent",
    ] {
        gate.connector(
            "echo",
            &local("echo", "https://api.echo.test", |c| {
                c["operations"]["get"]["headers"] = json!({ header: "x" });
            }),
        );
        assert_kind(
            &gate.load_error(&probe_of("echo", &["get"])),
            ErrorKind::InvalidConnector,
            &[header],
        );
    }
    gate.assert_well_behaved();
}

/// `credentials` (FR-017): a connector that declares a credential cannot be used yet.
#[test]
fn credentials() {
    let gate = Gate::new();
    let edits: [fn(&mut Value); 3] = [
        |c| c["credential"] = json!("token"),
        |c| c["credentials"] = json!({ "kind": "api-key" }),
        |c| c["operations"]["get"]["auth"] = json!("bearer"),
    ];
    for edit in edits {
        gate.connector("echo", &local("echo", "https://api.echo.test", edit));
        assert_kind(
            &gate.load_error(&probe_of("echo", &["get"])),
            ErrorKind::CredentialsNotSupported,
            &["005"],
        );
    }
    gate.assert_well_behaved();
}

/// `repository-connector` (FR-004): nothing verifies a repository's destinations yet.
#[test]
fn repository_connector() {
    let gate = Gate::new();
    let package = probe(&connected_manifest(
        "local/net-probe",
        &[("github.com/ada/weather", 1, &["forecast"], true)],
    ));
    assert_kind(
        &gate.load_error(&package),
        ErrorKind::DestinationNotVerified,
        &["github.com/ada/weather", "Phase 3"],
    );
    gate.assert_well_behaved();
}

/// `local-outside-development` (FR-018): only development mode loads local connectors.
#[test]
fn local_outside_development() {
    let gate = Gate::with(|_| Extra {
        development: Some(false),
        ..Extra::default()
    });
    gate.connector("echo", &local("echo", "http://127.0.0.1:9", |_| {}));
    assert_kind(
        &gate.load_error(&probe_of("echo", &["get"])),
        ErrorKind::DestinationNotVerified,
        &["local/echo", "development"],
    );
    gate.assert_well_behaved();
}

/// `reserved-host` (FR-005): every host under a domain the catalog reserves is the catalog's.
#[test]
fn reserved_host() {
    let gate = Gate::new();
    for destination in [
        "https://api.github.com",
        "https://API.GitHub.com",
        "https://uploads.github.com",
        "https://github.com",
        "https://api.github.com:8443",
    ] {
        gate.connector("github", &local("github", destination, |_| {}));
        assert_kind(
            &gate.load_error(&probe_of("github", &["get"])),
            ErrorKind::DestinationReserved,
            &["github.com"],
        );
    }
    // A name ending with a dot is refused before anything compares it
    gate.connector(
        "github",
        &local("github", "https://api.github.com.", |_| {}),
    );
    assert_kind(
        &gate.load_error(&probe_of("github", &["get"])),
        ErrorKind::InvalidConnector,
        &["dot"],
    );
    // A local file that claims the catalog's id
    gate.connector(
        "github",
        &local("github", "https://api.echo.test", |c| {
            c["id"] = json!("catalog/github")
        }),
    );
    assert_kind(
        &gate.load_error(&probe_of("github", &["get"])),
        ErrorKind::DestinationReserved,
        &["catalog/github"],
    );
    gate.assert_well_behaved();
}

/// `other-file` (FR-018): a connector file elsewhere that claims an approved connector's id
/// inherits nothing of its approval.
#[test]
fn other_file() {
    let gate = Gate::new();
    let server = Server::http();
    server.route("/", Reply::json("{}"));
    gate.connector("echo", &local("echo", &server.http_origin(), |_| {}));
    let package = probe_of("echo", &["get"]);
    gate.world.approve(&package);

    let elsewhere = TempDir::new().unwrap();
    std::fs::write(
        elsewhere.path().join("echo.json"),
        local("echo", &server.http_origin(), |_| {}).to_string(),
    )
    .unwrap();
    let other = Runtime::new(
        Limits::default(),
        RuntimeOptions::new(gate.world.data.path())
            .development(true)
            .connectors_dir(elsewhere.path()),
    )
    .unwrap();
    let error = match other.load(&package, TEST_ORIGIN, Arc::clone(&gate.world.grants)) {
        Ok(_) => panic!("a file elsewhere inherited the approval"),
        Err(error) => error,
    };
    assert_kind(
        &error,
        ErrorKind::NeedsNewApproval,
        &["origin changed", "local/echo"],
    );
    assert!(server.received().is_empty());
    gate.assert_well_behaved();
}

/// `connector-changed` (FR-014): a connector that adds a destination, or changes an operation
/// the widget calls, needs a new approval, naming the change.
#[test]
fn connector_changed() {
    let gate = Gate::new();
    let base = local("echo", "https://api.echo.test", |c| {
        c["operations"]["other"] = c["operations"]["get"].clone();
    });
    let package = probe_of("echo", &["get"]);
    for (edit, change) in [
        (
            (|c: &mut Value| c["destinations"]["more"] = json!("https://more.echo.test"))
                as fn(&mut Value),
            "destination added: https://more.echo.test",
        ),
        (
            |c| c["operations"]["get"]["path"] = json!("/elsewhere"),
            "operation changed: get",
        ),
        (
            |c| c["destinations"]["api"] = json!("https://api2.echo.test"),
            "operation changed: get",
        ),
    ] {
        gate.connector("echo", &base);
        gate.world.approve(&package);
        let mut changed = base.clone();
        edit(&mut changed);
        gate.connector("echo", &changed);
        let error =
            match gate
                .world
                .runtime
                .load(&package, TEST_ORIGIN, Arc::clone(&gate.world.grants))
            {
                Ok(_) => panic!("{change}: the widget ran without a new approval"),
                Err(error) => error,
            };
        assert_kind(&error, ErrorKind::NeedsNewApproval, &[change]);
    }
    // Only an operation the widget does not call: it runs as before
    gate.connector("echo", &base);
    gate.world.approve(&package);
    let mut more = base.clone();
    more["operations"]["third"] = more["operations"]["get"].clone();
    more["version"] = json!("0.2.0");
    gate.connector("echo", &more);
    gate.world.load(&package);
    gate.assert_well_behaved();
}

/// `pruned-connector` (FR-014): approved with a connector, then a version without it, then
/// one with it again: it needs a new approval.
#[test]
fn pruned_connector() {
    let gate = Gate::new();
    gate.connector("echo", &local("echo", "https://api.echo.test", |_| {}));
    let with = probe(&connected_manifest(
        "local/net-probe",
        &[
            ("local/echo", 0, &["get"], true),
            ("catalog/github", 1, &["list-releases"], true),
        ],
    ));
    let without = probe(&connected_manifest(
        "local/net-probe",
        &[("catalog/github", 1, &["list-releases"], true)],
    ));
    gate.world.approve(&with);
    // Loading the version without it prunes it from the approval
    let checked = gate.world.runtime.check(&without).unwrap();
    gate.world
        .grants
        .prune(checked.manifest(), TEST_ORIGIN)
        .unwrap();
    let entry = gate
        .world
        .grants
        .entry(&runtime::manifest::PluginId::parse("local/net-probe").unwrap())
        .unwrap()
        .unwrap();
    assert!(!entry.connectors.contains_key("local/echo"));
    let error = match gate
        .world
        .runtime
        .load(&with, TEST_ORIGIN, Arc::clone(&gate.world.grants))
    {
        Ok(_) => panic!("the pruned connector came back without approval"),
        Err(error) => error,
    };
    assert_kind(&error, ErrorKind::NeedsNewApproval, &["local/echo"]);
    gate.assert_well_behaved();
}

/// `approval-changed` (FR-014): approving with the digest of what was shown before the
/// connector changed grants nothing.
#[test]
fn approval_changed() {
    let gate = Gate::new();
    gate.connector("echo", &local("echo", "https://api.echo.test", |_| {}));
    let package = probe_of("echo", &["get"]);
    let shown = gate
        .world
        .runtime
        .check(&package)
        .unwrap()
        .approval_digest();
    gate.connector(
        "echo",
        &local("echo", "https://api.echo.test", |c| {
            c["destinations"]["api"] = json!("https://exfiltrate.echo.test");
        }),
    );
    let checked = gate.world.runtime.check(&package).unwrap();
    let error = checked
        .approve(&gate.world.grants, TEST_ORIGIN, Some(&shown))
        .unwrap_err();
    assert_kind(&error, ErrorKind::ApprovalChanged, &[&shown]);
    let id = runtime::manifest::PluginId::parse("local/net-probe").unwrap();
    assert!(
        gate.world.grants.entry(&id).unwrap().is_none(),
        "nothing granted"
    );
    gate.assert_well_behaved();
}

/// `catalog-reserves`, its seam half (FR-005): a catalog connector whose destination is
/// outside the domains it reserves is refused, even in the catalog. The built-in half is a
/// unit test of `runtime/src/catalog.rs`.
#[test]
fn catalog_reserves() {
    let pki = TestPki::new();
    let outside = json!({
        "connector_version": 1, "id": "catalog/echo", "version": "1.0.0", "author": "Tests",
        "description": "Outside its reserves", "reserves": ["echo.test"],
        "destinations": { "api": "https://api.elsewhere.test" },
        "operations": { "get": { "kind": "read", "method": "GET", "destination": "api",
            "path": "/", "response": { "format": "json" } } }
    });
    let data = TempDir::new().unwrap();
    let result = Runtime::with_test_network(
        Limits::default(),
        RuntimeOptions::new(data.path()),
        TestNetwork {
            root: pki.root(),
            catalog: vec![outside.to_string()],
            ..TestNetwork::default()
        },
    );
    let error = result.err().expect("the catalog refuses it");
    assert!(
        error.to_string().contains("api.elsewhere.test"),
        "{error:#}"
    );
    Gate::new().assert_well_behaved();
}

// T042: cases refused or contained when the widget calls

/// The answers of one `net-probe` update with `config`.
pub(crate) fn update(plugin: &Plugin, config: &str) -> Vec<Value> {
    answers(
        &plugin
            .update(config)
            .result
            .expect("net-probe returns its answers"),
    )
}

#[track_caller]
pub(crate) fn assert_error(answer: &Value, error: &str) {
    assert_eq!(answer["error"], error, "{answer}");
}

/// A local connector on `server` whose read `item` puts `{id}` (text, 1–4 of a–z) in its path
/// and `{n}` (1–9) in its query.
fn typed(server: &Server) -> Value {
    local("echo", &server.http_origin(), |c| {
        c["operations"]["item"] = json!({
            "kind": "read", "method": "GET", "destination": "api", "path": "/items/{id}",
            "query": { "n": "{n}" },
            "parameters": {
                "id": { "type": "text", "max_length": 4, "pattern": "[a-z.]*" },
                "n": { "type": "integer", "minimum": 1, "maximum": 9, "required": false }
            },
            "response": { "format": "json" }
        });
        c["operations"]["post"] = json!({
            "kind": "change", "method": "POST", "destination": "api", "path": "/post",
            "response": { "format": "json" }
        });
    })
}

/// `value-outside-type` (FR-003): every value is checked against its type before anything is
/// sent.
#[test]
fn value_outside_type() {
    let gate = Gate::new();
    let server = Server::http();
    gate.connector("echo", &typed(&server));
    let plugin = gate.world.approve_and_load(&probe_of("echo", &["item"]));
    let cases = [
        json!([["id", "toolong"]]),
        json!([["id", "ABC"]]),
        json!([["id", "abc"], ["n", 10]]),
        json!([["id", "abc"], ["n", 0]]),
        json!([["id", "abc"], ["x", 1]]),
        json!([["n", 3]]),
        json!([["id", "abc"], ["id", "abd"]]),
        json!([["id", "abc"], ["n", "3"]]),
        json!([["id", ""]]),
        json!([["id", "."]]),
        json!([["id", ".."]]),
    ];
    for arguments in &cases {
        let answer = update(
            &plugin,
            &calls(&[("local/echo", "item", arguments.clone())]),
        );
        assert_error(&answer[0], "refused:value-outside-type");
    }
    assert!(
        server.received().is_empty(),
        "a value outside its type was sent"
    );
    gate.assert_well_behaved();
}

/// `value-encoding` (FR-003): valid values stay in their own path segment, query value or body
/// field, encoded by the host.
#[test]
fn value_encoding() {
    let mut https = None;
    let gate = Gate::with(|pki| {
        let server = Server::https(&pki.leaf(&["api.echo.test"]));
        let extra = Extra {
            routes: vec![route("api.echo.test", 443, server.address())],
            public: vec![server.address()],
            catalog: vec![
                json!({
                    "connector_version": 1, "id": "catalog/echo", "version": "1.0.0",
                    "author": "Tests", "description": "Echo", "reserves": ["echo.test"],
                    "destinations": { "api": "https://api.echo.test" },
                    "operations": {
                        "search": { "kind": "read", "method": "POST", "destination": "api",
                            "path": "/search", "body": { "q": "{q}", "n": "{n}" },
                            "parameters": {
                                "q": { "type": "text", "max_length": 64 },
                                "n": { "type": "integer", "minimum": 0, "maximum": 9 }
                            },
                            "response": { "format": "json" } }
                    }
                })
                .to_string(),
            ],
            ..Extra::default()
        };
        https = Some(server);
        extra
    });
    let https = https.unwrap();
    let http = Server::http();
    gate.connector(
        "echo",
        &local("echo", &http.http_origin(), |c| {
            c["operations"]["item"] = json!({
                "kind": "read", "method": "GET", "destination": "api", "path": "/a/{v}/b",
                "query": { "q": "{v2}" },
                "parameters": {
                    "v": { "type": "text", "max_length": 64 },
                    "v2": { "type": "text", "max_length": 64 }
                },
                "response": { "format": "json" }
            });
        }),
    );
    let package = probe(&connected_manifest(
        "local/net-probe",
        &[
            ("local/echo", 0, &["item"], true),
            ("catalog/echo", 1, &["search"], true),
        ],
    ));
    let plugin = gate.world.approve_and_load(&package);
    let values = ["/", "?", "#", "&x=1", "=", "%2F", "%2e%2e", "é", "a/../b"];
    for value in values {
        update(
            &plugin,
            &calls(&[
                ("local/echo", "item", json!([["v", value], ["v2", value]])),
                (
                    "catalog/echo",
                    "search",
                    json!([["q", format!("{value}\"}}\n")], ["n", 3]]),
                ),
            ]),
        );
    }
    for (request, value) in http.received().iter().zip(values) {
        let segments: Vec<&str> = request.path.split('/').collect();
        assert_eq!(segments.len(), 4, "{value}: {}", request.path);
        assert_eq!((segments[1], segments[3]), ("a", "b"));
        let query: Vec<(String, String)> =
            url::form_urlencoded::parse(request.query.as_deref().unwrap_or_default().as_bytes())
                .into_owned()
                .collect();
        assert_eq!(query, [("q".to_owned(), value.to_owned())]);
    }
    for (request, value) in https.received().iter().zip(values) {
        let body: Value = serde_json::from_slice(&request.body).unwrap();
        assert_eq!(body, json!({ "q": format!("{value}\"}}\n"), "n": 3 }));
    }
    assert_eq!(http.received().len(), values.len());
    assert_eq!(https.received().len(), values.len());
    gate.assert_well_behaved();
}

/// `undeclared-operation` (FR-006): an operation the manifest does not declare is refused; a
/// connector it does not declare is not granted.
#[test]
fn undeclared_operation() {
    let gate = Gate::new();
    let server = Server::http();
    gate.connector("echo", &typed(&server));
    let plugin = gate.world.approve_and_load(&probe_of("echo", &["get"]));
    let answers = update(
        &plugin,
        &calls(&[
            ("local/echo", "item", json!([["id", "abc"]])),
            ("local/echo", "nothing", json!([])),
            ("local/other", "get", json!([])),
            (
                "catalog/github",
                "list-releases",
                json!([["owner", "a"], ["repo", "b"]]),
            ),
        ]),
    );
    assert_error(&answers[0], "refused:operation-not-declared");
    assert_error(&answers[1], "refused:operation-not-declared");
    assert_error(&answers[2], "not-granted");
    assert_error(&answers[3], "not-granted");
    assert!(server.received().is_empty());
    gate.assert_well_behaved();
}

/// `call-in-render` (FR-010): render never reaches the network.
#[test]
fn call_in_render() {
    let gate = Gate::new();
    let server = Server::http();
    server.route("/", Reply::json("{}"));
    gate.connector("echo", &typed(&server));
    let package = probe(&connected_manifest(
        "local/net-probe",
        &[
            ("local/echo", 0, &["get"], true),
            ("catalog/github", 1, &["list-releases"], false),
        ],
    ));
    let plugin = gate.world.approve_and_load(&package);
    let config = json!({ "render_calls": [
        { "connector": "local/echo", "operation": "get", "arguments": [], "repeat": 3 },
        { "connector": "catalog/github", "operation": "list-releases", "arguments": [["owner", "a"], ["repo", "b"]] }
    ] })
    .to_string();
    let content = plugin.render(&config).result.expect("renders");
    let answers = answers(&content);
    for answer in &answers[..3] {
        assert_error(answer, "refused:called-outside-update");
    }
    assert_error(&answers[3], "not-granted");
    assert!(server.received().is_empty());
    assert!(gate.github.received().is_empty());
    gate.assert_well_behaved();
}

/// `change-in-update` (FR-013): until user actions exist, an update only reads.
#[test]
fn change_in_update() {
    let gate = Gate::new();
    let server = Server::http();
    gate.connector("echo", &typed(&server));
    let plugin = gate.world.approve_and_load(&probe_of("echo", &["post"]));
    let answer = update(&plugin, &calls(&[("local/echo", "post", json!([]))]));
    assert_error(&answer[0], "refused:change-needs-user-action");
    assert!(server.received().is_empty());
    gate.assert_well_behaved();
}

/// `optional-off` (FR-015): an optional connector that is off answers not granted; turning it
/// off or removing the approval from another process drops a response in flight; turning one
/// on from another process applies once the plugin is loaded again.
#[test]
fn optional_off() {
    let gate = Gate::new();
    let server = Server::http();
    server.route("/", Reply::json(r#"{"ok":true}"#));
    server.route(
        "/slow",
        Reply::json(r#"{"late":true}"#).delay(Duration::from_millis(1500)),
    );
    gate.connector(
        "echo",
        &local("echo", &server.http_origin(), |c| {
            c["operations"]["slow"] = json!({ "kind": "read", "method": "GET", "destination": "api",
                "path": "/slow", "response": { "format": "json" } });
        }),
    );
    let manifest_text = connected_manifest(
        "local/net-probe",
        &[
            ("local/echo", 0, &["get", "slow"], false),
            ("catalog/github", 1, &["list-releases"], true),
        ],
    );
    let package = probe(&manifest_text);
    let plugin = gate.world.approve_and_load(&package);
    // Off: not granted, nothing sent
    assert_error(
        &update(&plugin, &calls(&[("local/echo", "get", json!([]))]))[0],
        "not-granted",
    );
    assert!(server.received().is_empty());

    // Turned on from another store, as another process would: still off for this plugin
    let manifest = runtime::manifest::Manifest::parse(manifest_text.as_bytes()).unwrap();
    let other = GrantStore::open(gate.world.data.path().join("grants.json"));
    other
        .set_connector(&manifest, TEST_ORIGIN, "local/echo", true)
        .unwrap();
    assert_error(
        &update(&plugin, &calls(&[("local/echo", "get", json!([]))]))[0],
        "not-granted",
    );
    assert!(server.received().is_empty());
    // On once the plugin is loaded again
    let plugin = gate.world.load(&package);
    assert_eq!(
        update(&plugin, &calls(&[("local/echo", "get", json!([]))]))[0]["status"],
        200
    );

    // Turned off while a response is in flight: the response is dropped
    let flip = {
        let manifest = manifest.clone();
        let path = gate.world.data.path().join("grants.json");
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(500));
            GrantStore::open(path)
                .set_connector(&manifest, TEST_ORIGIN, "local/echo", false)
                .unwrap();
        })
    };
    let answer = update(&plugin, &calls(&[("local/echo", "slow", json!([]))]));
    flip.join().unwrap();
    assert_error(&answer[0], "not-granted");
    assert!(answer[0].get("body").is_none());
    assert_eq!(
        server.received().last().unwrap().path,
        "/slow",
        "it was sent, then dropped"
    );

    // The approval removed while a response is in flight: dropped too
    other
        .set_connector(&manifest, TEST_ORIGIN, "local/echo", true)
        .unwrap();
    let plugin = gate.world.load(&package);
    let remove = {
        let path = gate.world.data.path().join("grants.json");
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(500));
            GrantStore::open(path)
                .remove(&PluginId::parse("local/net-probe").unwrap())
                .unwrap();
        })
    };
    let answer = update(&plugin, &calls(&[("local/echo", "slow", json!([]))]));
    remove.join().unwrap();
    assert_error(&answer[0], "not-granted");
    gate.assert_well_behaved();
}

/// `connector-table` (white box, FR-015): the table built at link time holds exactly the
/// granted connectors, and the stand-in is linked when none is granted. No plugin can observe
/// the difference, so the test reads the host's state.
#[test]
fn connector_table() {
    let gate = Gate::new();
    gate.connector("echo", &local("echo", "https://api.echo.test", |_| {}));
    let manifest_text = connected_manifest(
        "local/net-probe",
        &[
            ("local/echo", 0, &["get"], false),
            ("catalog/github", 1, &["list-releases"], true),
        ],
    );
    let package = probe(&manifest_text);
    let plugin = gate.world.approve_and_load(&package);
    assert_eq!(
        plugin.connector_table(),
        Some(vec!["catalog/github".to_owned()])
    );
    let manifest = runtime::manifest::Manifest::parse(manifest_text.as_bytes()).unwrap();
    gate.world
        .grants
        .set_connector(&manifest, TEST_ORIGIN, "local/echo", true)
        .unwrap();
    assert_eq!(
        plugin.connector_table(),
        Some(vec!["catalog/github".to_owned(), "local/echo".to_owned()])
    );

    // Only optional connectors, all off: the stand-in
    let only_optional = probe(&connected_manifest(
        "local/net-probe-off",
        &[("local/echo", 0, &["get"], false)],
    ));
    let plugin = gate.world.approve_and_load(&only_optional);
    assert_eq!(plugin.connector_table(), None);
    assert_error(
        &update(&plugin, &calls(&[("local/echo", "get", json!([]))]))[0],
        "not-granted",
    );
    gate.assert_well_behaved();
}
