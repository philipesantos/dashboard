//! The GitHub release source on the test seam (`specs/010-plugin-installation/research.md`, R4;
//! tasks T010, T012): `api.github.com` and GitHub's asset hosts are routed to a local HTTPS server
//! that records every request. Nothing reaches the internet.

use std::net::SocketAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use runtime::install::{FetchLimits, GitHub, Identity, Release, ReleaseSource, Repository};
use runtime::{ErrorKind, Limits, Route, Runtime, RuntimeOptions, TestNetwork, package};
use serde_json::json;
use tempfile::TempDir;

use crate::common::servers::{Reply, Server, TestPki};

const HOSTS: [&str; 4] = [
    "api.github.com",
    "release-assets.githubusercontent.com",
    "objects.githubusercontent.com",
    "elsewhere.example",
];

/// GitHub on a test server, and a runtime whose requests reach it.
struct Fake {
    server: Server,
    runtime: Runtime,
    _data: TempDir,
}

impl Fake {
    fn new() -> Fake {
        Fake::with_limits(Limits::default())
    }

    fn with_limits(limits: Limits) -> Fake {
        let pki = TestPki::new();
        let server = Server::https(&pki.leaf(&HOSTS));
        let to: SocketAddr = server.address();
        let routes = HOSTS
            .iter()
            .map(|host| Route {
                host: (*host).to_owned(),
                port: 443,
                answers: vec![vec![to]],
            })
            .collect();
        let data = TempDir::new().expect("a temporary directory");
        let runtime = Runtime::with_test_network(
            limits,
            RuntimeOptions::new(data.path()),
            TestNetwork {
                routes,
                public_for_tests: vec![to],
                root: pki.root(),
                catalog: Vec::new(),
            },
        )
        .expect("the runtime starts");
        Fake {
            server,
            runtime,
            _data: data,
        }
    }

    fn github(&self) -> GitHub {
        self.runtime.github(FetchLimits::default())
    }
}

fn repository() -> Repository {
    Repository::from_plugin_id("github.com/owner/repo").expect("a repository id")
}

fn identity() -> Identity {
    Identity {
        repository: repository(),
        repository_id: 42,
    }
}

fn release_json(tag: &str, assets: serde_json::Value) -> String {
    json!({ "tag_name": tag, "draft": false, "prerelease": false, "assets": assets }).to_string()
}

fn asset(id: u64, name: &str, size: usize, digest: Option<&str>) -> serde_json::Value {
    json!({ "id": id, "name": name, "size": size, "digest": digest })
}

fn release(digest: Option<String>, found_by_name: bool) -> Release {
    Release {
        tag: "v1.2.0".to_owned(),
        version: semver::Version::new(1, 2, 0),
        asset_id: 7,
        asset_name: "widget.wasm".to_owned(),
        asset_size: 5,
        asset_digest: digest,
        found_by_name,
    }
}

#[test]
fn identity_sends_only_the_api_headers_and_keeps_the_permanent_id() {
    let fake = Fake::new();
    fake.server.route(
        "/repos/owner/repo",
        Reply::json(r#"{"id": 42, "full_name": "Owner/Repo"}"#),
    );
    let found = fake.github().identity(&repository()).expect("found");
    assert_eq!(found, identity());
    let received = fake.server.received();
    let request = received.last().expect("one request");
    assert_eq!(request.method, "GET");
    assert_eq!(
        request.header("accept"),
        Some("application/vnd.github+json")
    );
    assert_eq!(request.header("x-github-api-version"), Some("2022-11-28"));
    assert!(
        request
            .header("user-agent")
            .is_some_and(|agent| agent.starts_with("dashboard/")),
        "{request:?}"
    );
    for absent in ["authorization", "cookie", "accept-encoding"] {
        assert_eq!(request.header(absent), None, "{absent} was sent");
    }
}

#[test]
fn a_renamed_repository_is_refused_at_installation() {
    let fake = Fake::new();
    fake.server.route(
        "/repos/owner/repo",
        Reply::redirect(301, "https://api.github.com/repositories/42"),
    );
    let error = fake.github().identity(&repository()).expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::RepositoryChanged, "{error}");
    // The redirect was not followed
    assert_eq!(fake.server.received().len(), 1);
}

#[test]
fn a_name_that_answers_for_another_repository_is_refused() {
    let fake = Fake::new();
    fake.server.route(
        "/repos/owner/repo",
        Reply::json(r#"{"id": 42, "full_name": "someone/else"}"#),
    );
    let error = fake.github().identity(&repository()).expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::RepositoryChanged, "{error}");
}

#[test]
fn the_rate_limit_is_named_with_when_to_try_again() {
    let fake = Fake::new();
    let reset = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("after 1970")
        .as_secs()
        + 600;
    fake.server.route(
        "/repos/owner/repo",
        Reply::status(403)
            .header("x-ratelimit-remaining", "0")
            .header("x-ratelimit-reset", &reset.to_string()),
    );
    let error = fake.github().identity(&repository()).expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::RateLimited);
    assert!(error.to_string().contains("10 minutes"), "{error}");

    fake.server.route(
        "/repositories/42",
        Reply::status(429).header("retry-after", "30"),
    );
    let error = fake.github().identity_by_id(42).expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::RateLimited);

    // A 403 that is not the rate limit is not called one
    fake.server.route("/repos/owner/repo", Reply::status(403));
    let error = fake.github().identity(&repository()).expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::SourceUnreachable);
}

#[test]
fn an_api_answer_over_its_cap_or_its_time_is_refused() {
    let fake = Fake::new();
    fake.server
        .route("/repos/owner/repo", Reply::json_of_size(256 * 1024 + 1));
    let error = fake.github().identity(&repository()).expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::SourceUnreachable, "{error}");

    fake.server.route(
        "/repos/owner/repo",
        Reply::json(r#"{"id": 42, "full_name": "owner/repo"}"#).delay(Duration::from_secs(2)),
    );
    let slow = fake.runtime.github(FetchLimits {
        api_time: Duration::from_millis(300),
        ..FetchLimits::default()
    });
    let error = slow.identity(&repository()).expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::SourceUnreachable, "{error}");
}

#[test]
fn the_latest_release_is_asked_by_permanent_id() {
    let fake = Fake::new();
    fake.server.route(
        "/repositories/42/releases/latest",
        Reply::json(&release_json(
            "v1.2.0",
            json!([
                asset(6, "README.md", 10, None),
                asset(7, "widget.wasm", 5, None)
            ]),
        )),
    );
    let found = fake
        .github()
        .latest_release(&identity())
        .expect("a release");
    assert_eq!(found, release(None, false));
    assert!(
        fake.server
            .received()
            .iter()
            .all(|request| !request.path.starts_with("/repos/")),
        "asked by name"
    );
}

#[test]
fn a_release_without_one_package_or_with_a_bad_tag_is_refused() {
    let fake = Fake::new();
    let cap = Limits::default().max_package_bytes;
    let cases = [
        ("v1.0.0", json!([]), ErrorKind::NoPackageAsset),
        (
            "v1.0.0",
            json!([asset(1, "a.wasm", 5, None), asset(2, "b.wasm", 5, None)]),
            ErrorKind::NoPackageAsset,
        ),
        (
            "v1.0.0",
            json!([asset(1, "a.wasm", cap + 1, None)]),
            ErrorKind::NoPackageAsset,
        ),
        (
            "release-one",
            json!([asset(1, "a.wasm", 5, None)]),
            ErrorKind::TagMismatch,
        ),
    ];
    for (tag, assets, kind) in cases {
        fake.server.route(
            "/repositories/42/releases/latest",
            Reply::json(&release_json(tag, assets.clone())),
        );
        let error = fake
            .github()
            .latest_release(&identity())
            .expect_err("refused");
        assert_eq!(error.kind(), kind, "{tag} {assets}: {error}");
    }
}

#[test]
fn without_answers_by_id_the_release_is_found_by_name_and_checked_after() {
    let fake = Fake::new();
    let body = b"wasm!".to_vec();
    fake.server
        .route("/repositories/42/releases/latest", Reply::status(404));
    fake.server.route(
        "/repos/owner/repo/releases/latest",
        Reply::json(&release_json(
            "v1.2.0",
            json!([asset(7, "widget.wasm", 5, None)]),
        )),
    );
    fake.server.route(
        "/repos/owner/repo/releases/assets/7",
        Reply::status(200).body(body.clone()),
    );
    fake.server.route(
        "/repositories/42",
        Reply::json(r#"{"id": 42, "full_name": "owner/repo"}"#),
    );
    let github = fake.github();
    let found = github.latest_release(&identity()).expect("found by name");
    assert!(found.found_by_name);
    assert_eq!(
        github.download(&identity(), &found).expect("downloaded"),
        body
    );

    // The name led elsewhere by the time the package arrived
    fake.server.route(
        "/repositories/42",
        Reply::json(r#"{"id": 42, "full_name": "owner/renamed"}"#),
    );
    let error = github.download(&identity(), &found).expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::RepositoryChanged, "{error}");
}

#[test]
fn a_download_follows_redirects_only_to_the_asset_hosts() {
    let fake = Fake::new();
    let body = b"wasm!".to_vec();
    fake.server.route(
        "/repositories/42/releases/assets/7",
        Reply::redirect(302, "https://release-assets.githubusercontent.com/blob"),
    );
    fake.server
        .route("/blob", Reply::status(200).body(body.clone()));
    let github = fake.github();
    let digest = package::digest(&body);
    let downloaded = github
        .download(&identity(), &release(Some(digest), false))
        .expect("downloaded");
    assert_eq!(downloaded, body);
    let asset_request = fake
        .server
        .received()
        .into_iter()
        .find(|request| request.path == "/repositories/42/releases/assets/7")
        .expect("asked");
    assert_eq!(
        asset_request.header("accept"),
        Some("application/octet-stream")
    );

    // Anywhere else is refused
    fake.server.route(
        "/repositories/42/releases/assets/7",
        Reply::redirect(302, "https://elsewhere.example/blob"),
    );
    let error = github
        .download(&identity(), &release(None, false))
        .expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::SourceUnreachable, "{error}");
    assert!(error.to_string().contains("elsewhere.example"), "{error}");

    // As is a redirect chain longer than three
    for hop in 0..4 {
        fake.server.route(
            &format!("/hop{hop}"),
            Reply::redirect(
                302,
                &format!("https://objects.githubusercontent.com/hop{}", hop + 1),
            ),
        );
    }
    fake.server.route(
        "/repositories/42/releases/assets/7",
        Reply::redirect(302, "https://objects.githubusercontent.com/hop0"),
    );
    let error = github
        .download(&identity(), &release(None, false))
        .expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::SourceUnreachable, "{error}");
}

#[test]
fn a_package_that_does_not_match_its_digest_or_grows_past_the_cap_is_refused() {
    let fake = Fake::with_limits(Limits {
        max_package_bytes: 1024,
        ..Limits::default()
    });
    fake.server.route(
        "/repositories/42/releases/assets/7",
        Reply::status(200).body(b"wasm!".to_vec()),
    );
    let other = package::digest(b"something else");
    let error = fake
        .github()
        .download(&identity(), &release(Some(other), false))
        .expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::SourceUnreachable, "{error}");

    // The size GitHub declared was small; the body is not
    fake.server.route(
        "/repositories/42/releases/assets/7",
        Reply::status(200).body(vec![0; 2048]),
    );
    let error = fake
        .github()
        .download(&identity(), &release(None, false))
        .expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::NoPackageAsset, "{error}");
}

#[test]
fn an_update_finds_a_deleted_or_renamed_repository_by_its_id() {
    let fake = Fake::new();
    fake.server.route("/repositories/42", Reply::status(404));
    let error = fake.github().identity_by_id(42).expect_err("refused");
    assert_eq!(error.kind(), ErrorKind::RepositoryChanged, "{error}");

    fake.server.route(
        "/repositories/42",
        Reply::json(r#"{"id": 42, "full_name": "owner/renamed"}"#),
    );
    let now = fake.github().identity_by_id(42).expect("found");
    assert_eq!(now.repository.plugin_id(), "github.com/owner/renamed");
}
