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

// T043: cases contained on the network, through the seam

/// A gate whose catalog has `catalog/echo` (reserving `echo.test`) with `destinations` and
/// `operations`, its HTTPS services made by `services` from the gate's PKI, and `net-probe`
/// declaring `operations` of it, approved and loaded.
struct EchoGate {
    gate: Gate,
    plugin: Plugin,
    servers: Vec<Server>,
}

fn echo_gate(
    limits: Limits,
    destinations: Value,
    operations: Value,
    services: impl FnOnce(&TestPki) -> (Vec<Server>, Vec<Route>, Vec<SocketAddr>),
) -> EchoGate {
    let names: Vec<String> = operations.as_object().unwrap().keys().cloned().collect();
    let mut servers = Vec::new();
    let gate = Gate::with(|pki| {
        let (made, routes, public) = services(pki);
        let mut public = public;
        public.extend(made.iter().map(Server::address));
        servers = made;
        Extra {
            routes,
            public,
            catalog: vec![
                json!({
                    "connector_version": 1, "id": "catalog/echo", "version": "1.0.0",
                    "author": "Tests", "description": "Echo", "reserves": ["echo.test"],
                    "destinations": destinations, "operations": operations
                })
                .to_string(),
            ],
            limits: Some(limits),
            development: Some(true),
        }
    });
    let operations: Vec<&str> = names.iter().map(String::as_str).collect();
    let plugin = gate.world.approve_and_load(&probe(&connected_manifest(
        "local/net-probe",
        &[("catalog/echo", 1, &operations, true)],
    )));
    EchoGate {
        gate,
        plugin,
        servers,
    }
}

fn read(path: &str) -> Value {
    json!({ "kind": "read", "method": "GET", "destination": "api", "path": path,
            "response": { "format": "json" } })
}

fn api(pki: &TestPki) -> Server {
    let server = Server::https(&pki.leaf(&["api.echo.test"]));
    server.route("/", Reply::json(r#"{"ok":true}"#));
    server
}

/// `private-address` (FR-007): a name that leads to this machine, the local network or a
/// cloud metadata service is refused with nothing sent, as are IP destinations and plain HTTP.
#[test]
fn private_address() {
    for address in [
        "10.0.0.1:443",
        "127.0.0.1:443",
        "[::1]:443",
        "169.254.169.254:443",
        "[::ffff:192.168.0.1]:443",
    ] {
        let address: SocketAddr = address.parse().unwrap();
        let echo = echo_gate(
            Limits::default(),
            json!({ "api": "https://api.echo.test" }),
            json!({ "root": read("/") }),
            |_| {
                (
                    Vec::new(),
                    vec![route("api.echo.test", 443, address)],
                    Vec::new(),
                )
            },
        );
        let answer = update(&echo.plugin, &calls(&[("catalog/echo", "root", json!([]))]));
        assert_error(&answer[0], "failed:non-public-address");
        echo.gate.assert_well_behaved();
    }
    // An IP destination, and plain HTTP to anything but this machine, never load
    let gate = Gate::new();
    for destination in [
        "https://10.0.0.1",
        "https://[::1]",
        "http://192.168.1.10:8765",
        "http://api.echo.test",
    ] {
        gate.connector("echo", &local("echo", destination, |_| {}));
        assert_kind(
            &gate.load_error(&probe_of("echo", &["get"])),
            ErrorKind::InvalidConnector,
            &["destinations.api"],
        );
    }
    // Plain HTTP outside development: a local connector does not load at all
    let production = Gate::with(|_| Extra {
        development: Some(false),
        ..Extra::default()
    });
    production.connector("echo", &local("echo", "http://127.0.0.1:9", |_| {}));
    assert_kind(
        &production.load_error(&probe_of("echo", &["get"])),
        ErrorKind::DestinationNotVerified,
        &[],
    );
    gate.assert_well_behaved();
}

/// `rebinding` (FR-007): a name public on the first lookup and private on the second is
/// refused at the second.
#[test]
fn rebinding() {
    let echo = echo_gate(
        Limits::default(),
        json!({ "api": "https://api.echo.test" }),
        json!({ "root": read("/") }),
        |pki| {
            let server = api(pki);
            let route = Route {
                host: "api.echo.test".to_owned(),
                port: 443,
                answers: vec![
                    vec![server.address()],
                    vec!["192.168.0.1:443".parse().unwrap()],
                ],
            };
            (vec![server], vec![route], Vec::new())
        },
    );
    let answers = update(
        &echo.plugin,
        &calls(&[
            ("catalog/echo", "root", json!([])),
            ("catalog/echo", "root", json!([])),
        ]),
    );
    assert_eq!(answers[0]["status"], 200);
    assert_error(&answers[1], "failed:non-public-address");
    assert_eq!(echo.servers[0].received().len(), 1);
    echo.gate.assert_well_behaved();
}

/// A gate with `api` (`api.echo.test`) whose `/go` answers `go`, `files` (`files.echo.test`),
/// and `private` (`private.echo.test`, routed to a private address).
fn redirect_gate(go: Reply) -> EchoGate {
    echo_gate(
        Limits::default(),
        json!({
            "api": "https://api.echo.test",
            "files": "https://files.echo.test",
            "private": "https://private.echo.test"
        }),
        json!({ "go": read("/go") }),
        |pki| {
            let api = Server::https(&pki.leaf(&["api.echo.test"]));
            api.route("/go", go);
            let other =
                Server::https(&pki.leaf(&["api.echo.test", "elsewhere.test", "files.echo.test"]));
            other.route("/there", Reply::json("{}"));
            let routes = vec![
                route("api.echo.test", 443, api.address()),
                route("files.echo.test", 443, other.address()),
                route("api.echo.test", 8443, other.address()),
                route("elsewhere.test", 443, other.address()),
                route("private.echo.test", 443, "10.9.8.7:443".parse().unwrap()),
            ];
            (vec![api, other], routes, Vec::new())
        },
    )
}

/// `exact-port` (FR-007): a redirect to the same host on another port is not followed.
#[test]
fn exact_port() {
    let echo = redirect_gate(Reply::redirect(302, "https://api.echo.test:8443/there"));
    let answer = update(&echo.plugin, &calls(&[("catalog/echo", "go", json!([]))]));
    assert_error(&answer[0], "failed:redirect-not-allowed");
    assert!(
        echo.servers[1].received().is_empty(),
        "the other port was reached"
    );
    echo.gate.assert_well_behaved();
}

/// `redirect-escape` (FR-007): redirects to an undeclared host, to a private address and to
/// plain HTTP are not followed.
#[test]
fn redirect_escape() {
    for location in [
        "https://elsewhere.test/there",
        "https://private.echo.test/there",
        "http://api.echo.test/there",
        "//elsewhere.test/there",
    ] {
        let echo = redirect_gate(Reply::redirect(307, location));
        let answer = update(&echo.plugin, &calls(&[("catalog/echo", "go", json!([]))]));
        assert_error(&answer[0], "failed:redirect-not-allowed");
        assert!(
            echo.servers[1].received().is_empty(),
            "{location} was reached"
        );
        echo.gate.assert_well_behaved();
    }
}

/// `tls` (FR-007): a wrong name, an untrusted CA and an expired certificate fail with nothing
/// sent past the handshake, and the verifier fetches nothing a certificate points at.
#[test]
fn tls() {
    let recorder = Server::recorder();
    let fetch = recorder.http_origin();
    let certificates: [fn(&TestPki, &str) -> crate::common::servers::ServerCert; 4] = [
        |pki, _| pki.leaf(&["other.echo.test"]),
        |pki, _| pki.untrusted(&["api.echo.test"]),
        |pki, _| pki.expired(&["api.echo.test"]),
        |pki, fetch| pki.with_fetch_addresses(&["other.echo.test"], fetch),
    ];
    for certificate in certificates {
        let fetch = fetch.clone();
        let echo = echo_gate(
            Limits::default(),
            json!({ "api": "https://api.echo.test" }),
            json!({ "root": read("/") }),
            move |pki| {
                let server = Server::https(&certificate(pki, &fetch));
                server.route("/", Reply::json("{}"));
                let routes = vec![route("api.echo.test", 443, server.address())];
                (vec![server], routes, Vec::new())
            },
        );
        let answer = update(&echo.plugin, &calls(&[("catalog/echo", "root", json!([]))]));
        assert_error(&answer[0], "failed:unavailable");
        assert!(
            echo.servers[0].received().is_empty(),
            "a request passed the handshake"
        );
        echo.gate.assert_well_behaved();
    }
    // A valid certificate with the same addresses: the call works and nothing is fetched
    let echo = echo_gate(
        Limits::default(),
        json!({ "api": "https://api.echo.test" }),
        json!({ "root": read("/") }),
        |pki| {
            let server = Server::https(
                &pki.with_fetch_addresses(&["api.echo.test"], &recorder.http_origin()),
            );
            server.route("/", Reply::json("{}"));
            let routes = vec![route("api.echo.test", 443, server.address())];
            (vec![server], routes, Vec::new())
        },
    );
    assert_eq!(
        update(&echo.plugin, &calls(&[("catalog/echo", "root", json!([]))]))[0]["status"],
        200
    );
    assert!(recorder.received().is_empty(), "{:?}", recorder.received());
    assert!(!echo.gate.pki.root().is_empty());
}

/// `cookie` (FR-007): a `Set-Cookie` answer is never sent back.
#[test]
fn cookie() {
    let echo = echo_gate(
        Limits::default(),
        json!({ "api": "https://api.echo.test" }),
        json!({ "root": read("/") }),
        |pki| {
            let server = Server::https(&pki.leaf(&["api.echo.test"]));
            server.route(
                "/",
                Reply::json("{}").header("Set-Cookie", "session=stolen; Path=/"),
            );
            let routes = vec![route("api.echo.test", 443, server.address())];
            (vec![server], routes, Vec::new())
        },
    );
    let config = json!({ "calls": [{ "connector": "catalog/echo", "operation": "root", "arguments": [], "repeat": 3 }] }).to_string();
    update(&echo.plugin, &config);
    update(&echo.plugin, &config);
    let received = echo.servers[0].received();
    assert_eq!(received.len(), 6);
    assert!(
        received
            .iter()
            .all(|request| request.header("Cookie").is_none())
    );
    echo.gate.assert_well_behaved();
}

/// `over-limits` (FR-008): every limit of the data model refuses or stops what passes it, and
/// the widget gets nothing over a limit.
#[test]
fn over_limits() {
    let text = || json!({ "type": "text", "max_length": 2048 });
    let body: serde_json::Map<String, Value> = (0..6)
        .map(|i| (format!("f{i}"), json!(format!("{{p{i}}}"))))
        .collect();
    let body_parameters: serde_json::Map<String, Value> =
        (0..6).map(|i| (format!("p{i}"), text())).collect();
    let echo = echo_gate(
        Limits {
            call_time_limit: Duration::from_secs(1),
            calls_per_minute_per_widget: 1000,
            calls_per_minute_per_host: 1000,
            ..Limits::default()
        },
        json!({ "api": "https://api.echo.test" }),
        json!({
            "big": read("/big"),
            "mib": read("/mib"),
            "headers": read("/headers"),
            "slow": read("/slow"),
            "loop": read("/loop"),
            "address": { "kind": "read", "method": "GET", "destination": "api", "path": "/a/{v}",
                "parameters": { "v": text() }, "response": { "format": "json" } },
            "body": { "kind": "read", "method": "POST", "destination": "api", "path": "/body",
                "body": body, "parameters": body_parameters, "response": { "format": "json" } }
        }),
        |pki| {
            let server = Server::https(&pki.leaf(&["api.echo.test"]));
            server.route("/big", Reply::json_of_size(1024 * 1024 + 1));
            server.route("/mib", Reply::json_of_size(1024 * 1024));
            server.route(
                "/headers",
                Reply::json("{}").header("X-Big", &"h".repeat(17 * 1024)),
            );
            server.route("/slow", Reply::json("{}").delay(Duration::from_secs(3)));
            server.route("/loop", Reply::redirect(302, "/loop"));
            let routes = vec![route("api.echo.test", 443, server.address())];
            (vec![server], routes, Vec::new())
        },
    );
    let call = |operation: &str, arguments: Value| json!({ "connector": "catalog/echo", "operation": operation, "arguments": arguments });
    // Per call: body, headers, address, request body, time, redirects
    let config = json!({ "bodies": "size", "calls": [
        call("big", json!([])),
        call("headers", json!([])),
        call("address", json!([["v", "é".repeat(2048)]])),
        call("body", Value::Array((0..6).map(|i| json!([format!("p{i}"), "\u{1}".repeat(2048)])).collect())),
        call("slow", json!([])),
        call("loop", json!([]))
    ] })
    .to_string();
    let answers = update(&echo.plugin, &config);
    assert_error(&answers[0], "failed:over-limit");
    assert!(
        answers[0].get("body_bytes").is_none(),
        "part of a body over the limit"
    );
    assert_error(&answers[1], "failed:over-limit");
    assert_error(&answers[2], "refused:over-limit");
    assert_error(&answers[3], "refused:over-limit");
    assert_error(&answers[4], "failed:over-limit");
    assert_error(&answers[5], "failed:redirect-not-allowed");
    let paths: Vec<String> = echo.servers[0]
        .received()
        .into_iter()
        .map(|r| r.path)
        .collect();
    assert!(
        !paths.contains(&"/body".to_owned()) && !paths.iter().any(|p| p.starts_with("/a/")),
        "{paths:?}"
    );
    assert_eq!(
        paths.iter().filter(|p| *p == "/loop").count(),
        6,
        "the first request and five redirects"
    );

    // 4 MiB across an update: four whole bodies of 1 MiB fit, the fifth does not
    let config = json!({ "bodies": "size", "calls": [{ "connector": "catalog/echo", "operation": "mib", "arguments": [], "repeat": 5 }] }).to_string();
    let answers = update(&echo.plugin, &config);
    for answer in &answers[..4] {
        assert_eq!(answer["body_bytes"], 1024 * 1024, "{answer}");
    }
    assert_error(&answers[4], "failed:over-limit");

    // 21 calls in one run: the 21st is refused, though valid
    let config = json!({ "calls": [{ "connector": "catalog/echo", "operation": "address", "arguments": [["v", "a"]], "repeat": 21 }] }).to_string();
    let answers = update(&echo.plugin, &config);
    assert_eq!(answers[19]["status"], 404, "{}", answers[19]);
    assert_error(&answers[20], "refused:over-limit");

    // Output over 256 KiB fails the update, and the previous data stays
    let report = echo
        .plugin
        .update(&json!({ "output": 256 * 1024 + 1 }).to_string());
    assert_eq!(report.result.unwrap_err().kind(), ErrorKind::OutputTooLarge);
    echo.gate.assert_well_behaved();
}

/// `over-limits`, the rates (FR-008): calls per minute per widget, and per destination host
/// across every widget of one runtime.
#[test]
fn over_limits_rates() {
    let echo = echo_gate(
        Limits {
            max_calls_per_run: 1000,
            calls_per_minute_per_widget: 5,
            calls_per_minute_per_host: 8,
            ..Limits::default()
        },
        json!({ "api": "https://api.echo.test" }),
        json!({ "root": read("/") }),
        |pki| {
            let server = api(pki);
            let routes = vec![route("api.echo.test", 443, server.address())];
            (vec![server], routes, Vec::new())
        },
    );
    // Both loaded first, so the buckets barely refill between the bursts
    let other = echo.gate.world.approve_and_load(&probe(&connected_manifest(
        "local/net-probe-two",
        &[("catalog/echo", 1, &["root"], true)],
    )));
    let burst = json!({ "calls": [{ "connector": "catalog/echo", "operation": "root", "arguments": [], "repeat": 6 }] }).to_string();
    let answers = update(&echo.plugin, &burst);
    assert!(
        answers[..5].iter().all(|answer| answer["status"] == 200),
        "{answers:?}"
    );
    assert_error(&answers[5], "refused:over-limit");
    // The second widget on the same host: the host has 3 calls left of its 8
    let answers = update(&other, &burst);
    assert!(
        answers[..3].iter().all(|answer| answer["status"] == 200),
        "{answers:?}"
    );
    assert_error(&answers[3], "refused:over-limit");
    assert!(
        answers[3]["detail"]
            .as_str()
            .unwrap()
            .contains("api.echo.test"),
        "{}",
        answers[3]
    );
    assert_eq!(echo.servers[0].received().len(), 8);
}

/// The child half of `system-proxy`: run by [`system_proxy`] in a process of its own, with the
/// proxy variables set, since setting them in this process is not safe. Ignored when the suite
/// runs; the parent runs it by name and fails if it does.
#[test]
#[ignore = "run by system_proxy in a child process, with the proxy variables set"]
fn system_proxy_child_process() {
    assert!(
        std::env::var("HTTPS_PROXY").is_ok(),
        "run through system_proxy"
    );
    let echo = echo_gate(
        Limits::default(),
        json!({ "api": "https://api.echo.test" }),
        json!({ "root": read("/") }),
        |pki| {
            let server = api(pki);
            let routes = vec![route("api.echo.test", 443, server.address())];
            (vec![server], routes, Vec::new())
        },
    );
    let answer = update(&echo.plugin, &calls(&[("catalog/echo", "root", json!([]))]));
    assert_eq!(answer[0]["status"], 200, "{answer:?}");
    assert_eq!(
        echo.servers[0].received().len(),
        1,
        "the request went straight to the service"
    );
}

/// `system-proxy` (FR-007, clarification 5): with every proxy variable set, connector requests
/// still go straight to their destination, for the HTTPS agent (a child test process, through
/// the seam) and for the loopback agent (the CLI); the proxy receives nothing.
#[test]
fn system_proxy() {
    let proxy = Server::recorder();
    let proxy_url = proxy.http_origin();
    let with_proxy = |command: &mut std::process::Command| {
        for name in [
            "HTTPS_PROXY",
            "HTTP_PROXY",
            "ALL_PROXY",
            "https_proxy",
            "http_proxy",
            "all_proxy",
        ] {
            command.env(name, &proxy_url);
        }
        command.env_remove("NO_PROXY").env_remove("no_proxy");
    };
    let mut child = std::process::Command::new(std::env::current_exe().unwrap());
    child.args([
        "--exact",
        "connectors::system_proxy_child_process",
        "--ignored",
        "--nocapture",
    ]);
    with_proxy(&mut child);
    let output = child.output().expect("the child test runs");
    assert!(
        output.status.success() && String::from_utf8_lossy(&output.stdout).contains("1 passed"),
        "the child failed: {}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    // The loopback agent, through the CLI
    let service = Server::http();
    service.route("/", Reply::json("{}"));
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join("connectors")).unwrap();
    std::fs::write(
        dir.path().join("connectors").join("echo.json"),
        local("echo", &service.http_origin(), |_| {}).to_string(),
    )
    .unwrap();
    let plugin = dir.path().join("net-probe.wasm");
    std::fs::write(&plugin, probe_of("echo", &["get"])).unwrap();
    let cli = |args: &[&str]| {
        let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_runtime"));
        command.args(args).args([
            "--grants",
            dir.path().join("grants.json").to_str().unwrap(),
            "--data",
            dir.path().to_str().unwrap(),
        ]);
        with_proxy(&mut command);
        command.output().unwrap()
    };
    let shown = String::from_utf8(cli(&["permissions", plugin.to_str().unwrap()]).stdout).unwrap();
    let digest = shown
        .lines()
        .find_map(|line| line.strip_prefix("approval digest: "))
        .unwrap()
        .to_owned();
    assert!(
        cli(&["approve", plugin.to_str().unwrap(), "--expect", &digest])
            .status
            .success()
    );
    let output = cli(&[
        "update",
        plugin.to_str().unwrap(),
        "--config",
        &calls(&[("local/echo", "get", json!([]))]),
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        service.received().len(),
        1,
        "the request went straight to the service"
    );
    assert!(
        proxy.received().is_empty(),
        "the proxy received {:?}",
        proxy.received()
    );
}
