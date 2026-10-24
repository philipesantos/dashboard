---

description: "Task list for plugin connectors (feature 003)"
---

# Tasks: Plugin Connectors

**Input**: Design documents from `specs/003-plugin-connectors/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Required. The hostile suite is the security gate (Principle VIII) and gains the 30
cases of `data-model.md` (Hostile cases). Every test task comes before the implementation it
checks and fails until that implementation lands. Test helpers make a test **fail, never skip**,
when a fixture or a test service is missing. No test reaches the internet, and no test reads
`specs/` (research R20).

**Organization**: Tasks are grouped by user story. The connector format, the network module,
contract 1.1.0, the grant record v2, kept data, the request log and connector calls block every
story, so they are foundational and built test first; the stories then add the CLI, the hostile
cases, the update and render flow checks, change handling and the author tooling.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1–US5)

## Path Conventions

Paths follow plan.md: `runtime/`, `wit/` and `connectors/` (security core), `plugins/` (guest
crates), `xtask/`, `docs/`, `.github/`. Built plugins go to `target/plugins/<crate-name>.wasm`.
Field rules are in `data-model.md`; decisions and numbers in `research.md` (R1–R20); CLI output
in `contracts/cli.md`; the contract draft in `contracts/widget-v1.1.wit`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: contract 1.1.0 and its frozen predecessor, the schemas, the catalog, dependencies,
lint and license rules, and the new plugin crates. The runtime still behaves as in feature 002
at the end of this phase, so every existing test keeps passing.

- [X] T001 Freeze contract 1.0.0: copy `wit/v1/widget.wit` unchanged to `wit/history/1.0.0/widget.wit` (still `dashboard:widget@1.0.0`); point `wit_bindgen::generate!` in `plugins/examples/hello-widget/src/lib.rs`, `plugins/fixtures/probe/src/lib.rs` and `plugins/fixtures/quiet/src/lib.rs` to `"../../../wit/history/1.0.0"` (research R11, SC-006); add a line to `wit/README.md` saying `history/` holds published versions, frozen, never edited; confirm `cargo xtask plugins` and `cargo test --workspace` still pass
- [X] T002 Rewrite `wit/v1/widget.wit` as `dashboard:widget@1.1.0` from `specs/003-plugin-connectors/contracts/widget-v1.1.wit`: `log` and `renderer` unchanged from 1.0.0; interface `connectors` with `variant value { text(string), integer(s64), flag(bool) }`, `record argument { name, value }`, `record header { name: string, value: option<string> }`, `record response { status: u16, headers: list<header>, body: list<u8> }`, `enum rule` with exactly the nine cases `operation-not-declared`, `value-outside-type`, `called-outside-update`, `change-needs-user-action`, `over-limit`, `non-public-address`, `redirect-not-allowed`, `unavailable`, `invalid-response`, `record problem { rule, detail: string }`, `variant call-error { not-granted, refused(problem), failed(problem) }` (`not-granted` first) and `call: func(connector: string, operation: string, arguments: list<argument>) -> result<response, call-error>`; interface `updater` with `update: func(config: string) -> result<string, string>`; `world widget { import log; export renderer; }` and `world connected-widget { import log; import connectors; export renderer; export updater; }`; keep every doc comment (the render configuration document of `data-model.md`, What render receives, included) and drop the "Draft" wording; the runtime's `bindgen!` stays on world `widget` for now, so the build still passes; confirm, before going on, that `hello-widget`, still exporting `renderer@1.0.0`, loads and renders through the runtime's 1.1.0 bindings (Wasmtime finds an export under a compatible version; the spike of research R11, which T026 then keeps as a test); skip `updater` in `runtime/tests/contract_v1.rs` as it skips `renderer` (an export, not a capability), so that test stays green
- [X] T003 [P] Create `wit/connector.schema.json` from `specs/003-plugin-connectors/contracts/connector.schema.json` (JSON Schema draft 2020-12), every constraint verbatim, removing "Draft of wit/connector.schema.json" from the top-level description
- [X] T004 [P] Add the `connectors` field to `wit/manifest.schema.json` exactly as in `specs/003-plugin-connectors/contracts/manifest.schema.json` (optional; at most 16 entries of `{ "id", "major" (0–65535), "operations" (1–32 unique names `^[a-z][a-z0-9-]{0,63}$`), "required" }`, id `catalog/<name>`, `local/<name>` or a repository id), keeping the file's existing formatting for the other fields
- [X] T005 [P] Create `connectors/catalog/github.json` from `specs/003-plugin-connectors/contracts/catalog-github.json` (`"$schema": "../../wit/connector.schema.json"`, `"reserves": ["github.com"]`, destination `api` = `https://api.github.com`, reads `get-repository`, `list-releases`, `list-issues`) and `connectors/README.md`: the official catalog, built into the host (research R2), security core (Principle IX), every change reviewed line by line; each connector lists in `reserves` the whole domains of its service; acceptance criteria arrive in Phase 3
- [X] T006 Update `runtime/Cargo.toml` (research R5, R18): `ureq = { version = "=3.5.0", default-features = false, features = ["rustls-no-provider", "rustls-webpki-roots"] }`, `rustls = { version = "0.23", default-features = false, features = ["ring", "std", "tls12", "logging"] }`, `url = "2"`, `regex = "1"`; dev-dependencies `rcgen = "0.14"` (its default features use `ring`) and `runtime = { path = ".", features = ["test-network"] }`; `[features] test-network = []`; each with a one-line comment saying why; move `runtime/examples/compile_cost.rs` to a new workspace member `bench/` (package `dashboard-bench`, `publish = false`, dependencies `runtime` without features and `wat`, binary `compile_cost`, added to `members` in the root `Cargo.toml`), because an example builds with the dev-dependencies and would turn `test-network` on in a release build, and point `.github/workflows/bench.yml` at `cargo run -p dashboard-bench --release --locked --bin compile_cost` and `docs/decisions/0001-plugin-process-isolation.md` at the new path; confirm `cargo tree -p runtime -e normal` contains no `aws-lc-rs`, `tokio`, `rustls-platform-verifier`, `rustls-native-certs` or `native-tls`, and that `cargo tree -d -p runtime` adds no avoidable duplicate
- [X] T007 [P] Update `deny.toml` (research R18, R20): add `"CDLA-Permissive-2.0"` and `"ISC"` to `[licenses] allow`, each with a comment naming the crates that need it (`webpki-roots`; `ring`, `untrusted`, `rustls-webpki`) and that both are permissive and compatible with Apache-2.0; under `[bans]` add `deny = [{ name = "rustls-platform-verifier" }, { name = "rustls-native-certs" }, { name = "native-tls" }]` with a comment pointing to research R8; confirm `cargo deny check` passes, and `cargo deny --manifest-path plugins/Cargo.toml check` still passes
- [X] T008 [P] Create `clippy.toml` at the repository root (research R20): `disallowed-types` for `ureq::Agent`, `ureq::config::Config`, `ureq::RequestBuilder`, `ureq::RequestExt` (its `with_default_agent` builds an agent that reads the proxy variables), `std::net::TcpStream` and `std::net::UdpSocket`, and `disallowed-methods` for every free function of `ureq` 3.5.0 (`ureq::run`, `ureq::agent`, `ureq::get`, `ureq::post`, `ureq::put`, `ureq::patch`, `ureq::delete`, `ureq::head`, `ureq::options`, `ureq::connect`, `ureq::trace`; `run` sends with a default agent that reads the proxy variables), `std::net::TcpStream::connect`, `std::net::TcpStream::connect_timeout` and `std::net::UdpSocket::bind`, each with `reason = "only runtime/src/network/ opens connections (Principle III, research R20)"`; if Clippy warns that an `ureq` path does not resolve in a crate without `ureq` (`xtask`, `app`), add `allow-invalid = true` to those entries; the network module will allow them with `#![allow(clippy::disallowed_types, clippy::disallowed_methods)]` as the first line of `runtime/src/network/mod.rs`, an inner attribute that also covers its submodules; confirm `cargo clippy --workspace --all-targets --locked -- -D warnings` still passes, and so does Clippy on the `plugins/` workspace for `wasm32-unknown-unknown`, which the root `clippy.toml` also reaches (tests may allow them where they run test servers, in `runtime/tests/common/servers.rs` only, with the same inner attribute)
- [X] T009 Create `plugins/fixtures/net-probe/` (research R16): contract 1.1.0, `wit_bindgen::generate!({ path: "../../../wit/v1", world: "connected-widget" })`; `update(config)` reads `{"calls": [{"connector", "operation", "arguments": [[name, value], ...], "repeat": n}], "fail": bool, "output": "not-json" | n, "bodies": "size"}`: arguments are a list of name/value pairs, so a name can repeat (values: JSON strings → `text`, integers → `integer`, booleans → `flag`); it makes each call `repeat` times (default 1) and returns `{"answers": [...], "more": n}` where each answer is `{"status", "headers", "body"}` (body as a UTF-8 string, lossy) or `{"error": "not-granted" | "refused:<rule>" | "failed:<rule>", "detail"}`, keeping at most the first 32 answers and counting the rest in `"more"`; with `"fail": true`, an error answer makes `update` return an error naming the first one; with `"output": "not-json"` it returns `{`, and with `"output": n` a valid JSON document of n bytes, instead of the answers; with `"bodies": "size"` each answer carries `body_bytes` and `body_sha256` instead of the body, so a case that receives megabytes stays within the 256 KiB output limit; `render(config)` returns the received configuration document as is, unless `config.config.render_calls` is a list of calls in the same form as `update`'s, `repeat` included, in which case it makes them and returns the answers in the same form; `[package.metadata.dashboard] manifest = false`; add it to `members` in `plugins/Cargo.toml` and a line to `plugins/fixtures/README.md`
- [X] T010 Create `plugins/examples/github-releases/` (research R16): contract 1.1.0, world `connected-widget`; `update(config)` takes `owner` and `repo` (strings) from the configuration, calls `catalog/github` `list-releases` with `owner`, `repo` and `per_page` = 5, and returns `{"releases": [{"name", "tag", "published_at", "url"}]}` from the JSON body; a call that fails, is refused or answers a status other than 200 makes `update` return an error naming it, so the host keeps the previous data; `render(config)` returns `{"title": "<owner>/<repo> releases", "items": [...], "fetched_at_ms": …}` from the kept data, or `{"title": …, "text": "No data yet"}` when `data` is `null`, plus `"last_update"` as received; `[package.metadata.dashboard] manifest = false` for now, with no `manifest.json`, because the runtime's manifest parser learns `connectors` in T017, which adds the manifest (`runtime/tests/manifest_schema.rs` skips a crate without one); a `README.md` showing `permissions`, `approve --expect`, `update` and `run`; add it to `plugins/Cargo.toml` members and to `plugins/examples/README.md`; run `cargo xtask plugins` (the runtime does not load either new plugin yet)

**Checkpoint**: 1.1.0 and the frozen 1.0.0 in `wit/`; schemas, catalog, dependencies, lint and
license rules in place; every existing test still passes

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: error kinds and limits, connector parsing and the catalog, the manifest field, the
network module with its test seam and test servers, contract 1.1.0 in the engine, the grant
record v2, kept data, the request log, connector calls and loading; the existing tests moved to
the new API

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T011 [P] Add the eight new kinds to `ErrorKind` in `runtime/src/error.rs`, with `as_str` names exactly as in `contracts/cli.md`: `invalid-connector`, `credentials-not-supported`, `destination-not-verified`, `destination-reserved`, `connector-unavailable`, `contract-too-old`, `missing-updater`, `approval-changed`; extend the kind-name unit test; create the empty modules `runtime/src/connector.rs`, `catalog.rs`, `kept.rs`, `requests.rs`, `calls.rs` and `runtime/src/network/mod.rs` (declaring the empty `address` and `resolver`), and declare them in `runtime/src/lib.rs`, so the tasks below each fill files of their own
- [X] T012 [P] Add the update and network limits of research R10 to `Limits` in `runtime/src/limits.rs`, with their defaults and a doc comment each: `update_time_limit` 10 s; `call_time_limit` 5 s; `max_calls_per_run` 20; `max_arguments` 16; `max_argument_name_chars` 32; `max_request_address_bytes` 8 KiB; `max_request_body_bytes` 64 KiB; `max_response_body_bytes` 1 MiB; `max_update_response_bytes` 4 MiB; `max_response_header_bytes` 16 KiB; `max_redirects` 5; `max_update_output_bytes` 256 KiB; `calls_per_minute_per_widget` 30; `calls_per_minute_per_host` 60; `max_request_log_bytes` 2 MiB; `max_logged_value_chars` 256; `max_logged_name_chars` 160; render's fuel and memory apply to updates too; unit test of the defaults
- [X] T013 Write the connector tests first, in a `#[cfg(test)] mod tests` of `runtime/src/connector.rs` (created empty by T011), whose parse function is a stub, covering every rule of `data-model.md` (Connector, Destination, Operation, Parameter) verbatim: at most 64 KiB; `connector_version` `1`; `id` of at most 160 characters, `catalog/<name>`, `local/<name>` or a repository id, `<name>` = `[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?`; `version` SemVer of at most 64 characters; `author` 1–100 and `description` 1–200 characters without control characters; `reserves` only in catalog connectors and required there, 1–8 domains of at most 253 characters in lowercase DNS labels, covering every host the connector declares; `destinations` 1–8, names `[a-z][a-z0-9-]{0,31}`, origins of at most 260 characters, `https://<domain>[:<port>]` with no path other than `/`, query, fragment, user or password, the host in one normal form (lowercase, `xn--` labels) and refused when it ends with a dot, an IP address refused except `http://localhost:<port>`, `http://127.0.0.1:<port>` and `http://[::1]:<port>` in local connectors; `operations` 1–64, names `[a-z][a-z0-9-]{0,63}`; `kind` `read` or `change`; `method` `GET`, `HEAD`, `POST`, `PUT`, `PATCH`, `DELETE`, and outside the catalog a read is `GET` or `HEAD` with no `body` (else `invalid-connector` naming the operation and saying to declare it as a change); `destination` a key of `destinations`; `path` of at most 512 characters starting with `/`, each segment literal or exactly `{<parameter>}`, literal segments only of `A–Z a–z 0–9 . _ ~ ! $ & ' ( ) * + , ; = : @ % -`; `query` at most 16 pairs, names 1–64 characters of `A–Z a–z 0–9 . _ ~ -`, values literal (at most 256 characters) or exactly `{<parameter>}`; `headers` at most 8: `Accept`, `Accept-Language`, and `X-` followed by 1–60 of `A–Z a–z 0–9 -` only in catalog connectors, values at most 256 printable ASCII characters, while `Authorization`, `Cookie`, `Proxy-*`, `Host`, `Content-*`, `Transfer-Encoding`, `Connection`, `Upgrade`, `TE` and `User-Agent` are refused; `body` a JSON object of at most 4 KiB, only for changes and catalog reads; `parameters` 0–16, names `[a-z][a-z0-9_]{0,31}`, `text` (`max_length` 1–2048, optional `pattern` of 1–512 characters compiled within 64 KiB, or `values` 1–64 unique strings of at most 2048 characters, not both), `integer` (`minimum`, `maximum`, 64-bit, minimum ≤ maximum), `flag`; `required` defaults to `true`; every placeholder names a declared parameter, every parameter appears in exactly one place, an optional one never fills a path segment; `response.format` `json` or `text`; `response.headers` 0–8 unique names of 1–64 characters of `A–Z a–z 0–9 -`, never `Set-Cookie` or `Cookie`; a field named `credential`, `credentials` or `auth` anywhere is `credentials-not-supported`; unknown fields refused, `$schema` ignored; every `invalid-connector` detail names the field path; the canonical digest of an operation changes when any of its fields or its destination's origin change and not when another operation is added
- [X] T014 Implement `runtime/src/connector.rs` so T013 passes (research R1, R3, R4): `Connector::parse(bytes, kind: Source { Catalog, Local, Repository }) -> Result<Connector, RuntimeError>`, typed with `serde` (`deny_unknown_fields`) through `serde_path_to_error`; origins parsed and normalized with `url`; patterns compiled with `regex::RegexBuilder` (`size_limit(64 * 1024)`, anchored as whole matches); `Operation::digest()` = `sha256:` + hex SHA-256 of a canonical JSON of kind, method, destination origin, path, query, headers, body, parameters and response; `Connector::destinations()` sorted
- [X] T015 Fill `runtime/src/catalog.rs` (created empty by T011; research R2, R16): the built-in catalog as a static list of `(id, include_str!("../../connectors/catalog/<name>.json"))`, parsed once with `Source::Catalog`; `reserved(host) -> bool` true for every domain in a catalog connector's `reserves` and every host under it, whatever the port, in the normal form; `get(id)`; unit tests: `catalog/github` parses, its `reserves` covers each of its hosts (a test over every built-in connector, the white-box `catalog-reserves` check), and `api.github.com`, `API.GitHub.com`, `uploads.github.com` and `github.com` are reserved while `github.io` and `notgithub.com` are not
- [X] T016 Write the manifest tests for `connectors` first, in `runtime/src/manifest.rs`'s test module: absent means none; at most 16 entries, unique by `id`; `id` a catalog, local or repository id; `major` 0–65535; `operations` 1–32 unique names `[a-z][a-z0-9-]{0,63}`; `required` boolean; unknown fields refused; the field path in every `invalid-manifest` detail (for example `connectors[0].operations[1]`)
- [X] T017 Implement `connectors` in `runtime/src/manifest.rs` so T016 passes: `ConnectorRequest { id, major, operations, required }` and `Manifest::connectors()`, `Manifest::connector(id)`; then add `plugins/examples/github-releases/manifest.json` with `"manifest_version": 1`, `"id": "local/github-releases"`, `"version": "0.1.0"`, `"author": "Dashboard contributors"`, `"permissions": []`, `"login_providers": []`, `"connectors": [{"id": "catalog/github", "major": 1, "operations": ["list-releases"], "required": true}]`, remove the crate's `manifest = false`, and confirm `cargo xtask plugins` packs it and `runtime/tests/manifest_schema.rs` accepts it
- [ ] T018 [P] Write the address classifier tests first, in `runtime/src/network/address.rs` (created empty by T011) with a stub `is_public(IpAddr) -> bool`, one test per row of research R6: unspecified, loopback, 10/8, 172.16/12, 192.168/16, 100.64/10, 169.254/16, 192.0.0/24, the documentation ranges (192.0.2/24, 198.51.100/24, 203.0.113/24, 2001:db8::/32), 198.18/15, multicast (224/4, ff00::/8), 240/4, 255.255.255.255, 0/8, fe80::/10, fc00::/7, and the IPv6 forms that embed IPv4 (`::ffff:a.b.c.d`, `::a.b.c.d`, `64:ff9b::/96`, `2002::/16`, `2001::/32`) judged by the address they embed, are not public; 8.8.8.8, 1.1.1.1, 140.82.112.3 and 2606:4700::1111 are
- [ ] T019 Implement `runtime/src/network/address.rs` so T018 passes: an explicit table of the IANA IPv4 and IPv6 Special-Purpose ranges with a comment per row naming the registry entry, plus `is_loopback(IpAddr)` for the development exception
- [ ] T020 Create the test services in `runtime/tests/common/servers.rs` (research R15): a test CA and leaf certificates made with `rcgen` (names under `.test`, plus `api.github.com` for the example), one with a wrong name, one from another CA and one expired, and a certificate whose AIA, OCSP and CRL addresses point at a recording server; an HTTPS server (`rustls` server side) and a plain HTTP server, both on `127.0.0.1` with an OS-chosen port, each scripted per path (status, headers, body of a given size, a delay, a redirect `Location`, `Set-Cookie`) and recording every request it receives (method, path, query, headers, body, arrival); a recording proxy server; helpers that fail, never skip, when a server cannot start; `#![allow(clippy::disallowed_types, clippy::disallowed_methods)]` as the module's first line, in this module only
- [ ] T021 Write the network tests first, in a new `runtime/tests/network.rs`, through `Runtime::with_test_network` and `net-probe` with connectors written in the test (catalog ones added through the seam, reached by the HTTPS agent through its routes; local ones on loopback, reached by the loopback agent, which the seam leaves as in production) against the test services; they fail until T035 connects loading and calls to the network module: a GET to a declared destination returns status, declared headers and body, and a declared header the response lacks arrives as absent; values reach the server encoded inside their own path segment or query value (`/`, `?`, `#`, `&`, `=`, `%2F`, `%2e%2e`, non-ASCII); in a catalog `POST` read added through the seam, each value replaces its whole body placeholder as a JSON value of its type (text with `"`, `\`, `}`, newlines and control characters stays one string; integers and flags stay numbers and booleans), and an absent optional parameter removes its field; the `User-Agent` is `dashboard/<host version>`, no `Accept-Encoding` and no `Cookie` is ever sent, even after a `Set-Cookie`; a name that resolves only to non-public addresses fails as `non-public-address` before any connection, and a name that is public on the first lookup and private on the second fails on the second; redirects to another path of the same host are followed, to another declared origin followed under its rules, to an undeclared host, another port, a non-public address or plain HTTP not followed (`redirect-not-allowed`), past 5 not followed; a wrong name, an untrusted CA and an expired certificate fail as `unavailable` and the server receives no request, and the recording server named in the certificate's AIA, OCSP and CRL addresses receives nothing; a body over 1 MiB, headers over 16 KiB, an address over 8 KiB and a request body over 64 KiB are `refused(over-limit)` with nothing sent; a service slower than 5 s fails as `over-limit` (time) within the limit plus 500 ms; a response in another media type than `response.format` fails as `invalid-response`
- [ ] T022 Create the runtime options and the test seam in `runtime/src/lib.rs` and `runtime/src/network/resolver.rs` (research R15): `pub struct RuntimeOptions` built with `RuntimeOptions::new(data_dir)` (development off, the default connectors directory `<data_dir>/connectors/`) and the public builders `.development(bool)` and `.connectors_dir(path)`, and the constructors `Runtime::new(limits, options)` and `#[cfg(feature = "test-network")] Runtime::with_test_network(limits, options, TestNetwork)`; `#[cfg(feature = "test-network")] pub struct TestNetwork { pub routes: Vec<Route>, pub public_for_tests: Vec<SocketAddr>, pub root: Vec<u8> /* DER */, pub catalog: Vec<String> /* extra catalog connectors as JSON */ }` with `pub struct Route { pub host: String, pub port: u16, pub answers: Vec<Vec<SocketAddr>> }`, all fields public so integration tests build them; the seam replaces only the HTTPS agent's resolver and roots, and the loopback agent of local connectors keeps its production resolver; each lookup of a destination's host and port returns the next of its answers, the last one repeating (the `rebinding` case answers public, then private), and a name without a route fails as `unavailable`; the origin checks (exact scheme, host and port, research R7) run on the URL before any lookup, so a route never widens them, and every address still goes through `address::is_public` unless it is in `public_for_tests`; move the existing callers to the new constructor, so the workspace still builds: `runtime/src/bin/runtime.rs` (development off until T039) and `bench/src/bin/compile_cost.rs`; `compile_error!` when `all(feature = "test-network", not(debug_assertions))`; nothing in the CLI reaches it
- [ ] T023 Implement `runtime/src/network/mod.rs` and `runtime/src/network/resolver.rs` (research R5–R10), with unit tests in their own `mod tests` for what needs no server: request building and encoding (R3), the body, redirect target checks (R7), the media type check and the per-call timeout cut to what is left of the update; T021 passes once T035 lands. The module is the only code that uses `ureq` (`#![allow(clippy::disallowed_types, clippy::disallowed_methods)]` as the first line of `mod.rs`); one agent per update, built with `https_only(true)` (a separate agent for local loopback connectors), `proxy(None)`, `max_redirects(0)`, `http_status_as_error(false)`, `user_agent("dashboard/<version>")`, `max_response_header_size(16 KiB)`, `RootCerts::WebPki` (the seam: `RootCerts::Specific` with the test CA only), the `ring` crypto provider and per-call timeouts cut to what is left of the update; the `PublicOnly` resolver (`ureq::unversioned::resolver::Resolver`) that resolves with the system and keeps only addresses `address::is_public` accepts (loopback only for local loopback connectors), failing as `non-public-address` when none is left; request building with host-side encoding (research R3: path values percent-encoded except unreserved characters, `""`, `.` and `..` refused, query values form-encoded, the body built by the host with `serde_json` from the declared object with each whole placeholder string replaced by its typed value, absent optional parameters dropped); manual redirects (R7: `url` resolution, exact scheme, host and port among the connector's origins, `GET` on 301–303, at most 5); body read with a size limit and dropped whole when over it; media type check; when a call fails as `unavailable`, its detail says the host connects directly and does not use network proxies
- [ ] T024 Write the contract tests first, as unit tests in `runtime/src/permissions.rs` with WAT components: a v1 component whose contract names all carry `1.0.0`, or all `1.1.0`, is v1 with that minor; one mixing `1.0.0` and `1.1.0` names, or carrying `1.2.0`, is `contract-mismatch` naming both versions; a manifest that declares a connector on a 1.0.0 component is `contract-too-old` naming 1.1.0; one that declares a connector and exports no `updater` is `missing-updater`; an import of `dashboard:widget/connectors@1.1.0` without any declared connector is `not-declared`; `wasi:http/outgoing-handler@0.2.0` and the WASI 0.3 HTTP interface names are `capability-refused`; the registry maps `log` since 1.0 and the connectors since 1.1
- [ ] T025 Implement contract 1.1.0 in `runtime/src/permissions.rs` so T024 passes (research R11, `data-model.md` Contract versions): `Contract::V1` carries its minor; `package_version` checks become "major 1, not newer than 1.1.0, one version across the component"; the registry gains `since` and the `connectors` row; `contract-too-old` and `missing-updater` in the decision; the CLI prints a v1 contract as `v1 (1.<m>.<p>)`, and v0 still as `v0`
- [ ] T026 Bind contract 1.1.0 and call exports by name in `runtime/src/lib.rs` and `runtime/src/host.rs`: `bindgen!` for `mod v1` on `"../wit/v1"`, world `connected-widget`; link `log` at 1.1.0 (Wasmtime matches 1.0.0 imports against it; `connectors` is linked by T034 and T035); look up `renderer` and `updater` by the exact names the component exports (`get_export_index`) and call them as typed functions `(&str,) -> (Result<String, String>,)`; add a test in `runtime/tests/contract.rs` (the spike of research R11): `hello-widget`, built from the frozen 1.0.0 copy, loads, links and renders exactly as before on the 1.1.0 host
- [ ] T027 Write the grant record tests first, in `runtime/src/grants.rs`'s test module, for `data-model.md` (Grant record, version 2) verbatim: `record_version` `2`; a version 1 record reads as version 2 with no connectors and is written back as 2; an entry's `connectors` holds, per declared connector, `origin` (`catalog` or the local file's canonical path), `version`, `granted` (required: `true`; optional: `true` only while on), `destinations` sorted, `operations` (declared operation → digest); `approve` records every declared connector, grants the required ones, keeps an optional one's state when nothing changed and turns it off otherwise; `set_optional` on a connector id turns it on or off and is `required-together` for a required one; `prune` removes connector entries the manifest no longer declares; a comparison against a manifest and its loaded connectors names what changed (`destination added: <origin>`, `operation changed: <name>`, `operation newly declared: <name>`, `origin changed`) and is empty when only undeclared operations were added; every change increments the generation; `read_fresh()` re-reads the file under its shared lock; the approval digest of `data-model.md` (Approval digest) is stable across runs and changes when the package digest, a permission, or a connector's id, origin, version, `required`, destinations or the digest of a declared operation changes; approving with an expected digest that differs from it is `approval-changed` and records nothing
- [ ] T028 Implement grant record v2 and the approval digest in `runtime/src/grants.rs` so T027 passes (research R13, R17), keeping feature 002's locking and atomic writes; the digest is computed here, in the library, because `permissions`, `approve`, kept data and the hostile suite all use it; the `approve` of `runtime/src/bin/runtime.rs` and `bench/src/bin/compile_cost.rs` move to the new signature, so the workspace still builds (for a widget without connectors no digest is expected)
- [ ] T029 [P] Write the kept data tests first, in `runtime/src/kept.rs` (created empty by T011) with stubs: the file `widget-data/<hex SHA-256 of "<id>\n<origin>">.json` with `id`, `origin`, `digest`, `approval` (the approval digest, as `permissions` prints it, and the connectors that were on), `fetched_at_ms`, `data` (at most 256 KiB), `last_update` (`at_ms`, `outcome` `ok` or `failed`, `error`); written atomically; a successful update replaces `data` and `fetched_at_ms`; a failed one changes only `last_update`; `discard(id)` removes every file of that id; a load discards the file when the package digest differs, the approval differs, or a connector recorded as on is now off; a damaged file counts as no data and is reported, never as data; the render configuration document of `data-model.md` (What render receives), with `null`s before the first update
- [ ] T030 Implement `runtime/src/kept.rs` so T029 passes (research R14)
- [ ] T031 [P] Write the request log tests first, in `runtime/src/requests.rs` (created empty by T011) with stubs: one JSON object per line of `requests.jsonl` with the fields of `data-model.md` (Request log entry): `at_ms`, `plugin`, `connector`, `operation`, `kind`, `method`, `host`, `path`, `arguments` (each value cut to 256 characters with ` [truncated]`), `outcome`, `sent_bytes`, `received_bytes`, `duration_ms`, `redirect_of`, `summary`; connector and operation names cut to 160 characters and at most 16 arguments with names of at most 32 characters before writing; appends under a file lock from two threads lose nothing; past 2 MiB the file is renamed to `requests.1.jsonl`, replacing the older one; `read_last(n, plugin)` returns entries oldest first
- [ ] T032 Implement `runtime/src/requests.rs` so T031 passes (research R14)
- [ ] T033 Write the connector call tests first, as unit tests in `runtime/src/calls.rs` (created empty by T011) with stubs, following the steps 0–13 of `data-model.md` (A connector call) in order: the run's call counter goes up for every call; past 20 the call answers as usual but only one summary entry is written per run, counting the answers, and it is written when the run ends, whatever its outcome (a trap or a limit included); a connector missing from the table answers `not-granted` whatever else is wrong (not declared, off, called from render, bad arguments); a call from `render` to a connector in the table is `refused(called-outside-update)` without reading the grant record; an undeclared operation `refused(operation-not-declared)`; a change `refused(change-needs-user-action)`; more than 16 arguments or a value outside its type `refused(value-outside-type)`; the per-run call limit and the token buckets (30 per minute per widget, 60 per minute per destination host, shared by widgets of one `Runtime`) `refused(over-limit)`; a connector the grant record, read again just before sending, no longer grants answers `not-granted` with nothing sent, and one no longer granted when its response arrives answers `not-granted` with the response dropped; every call, refused ones included, produces one log entry or a count in the summary; the white-box `connector-table` checks: the table's keys are exactly the granted connectors, and the stand-in is chosen when none is granted
- [ ] T034 Implement `runtime/src/calls.rs` and the `connectors` host side in `runtime/src/host.rs` so T033 passes (research R10, R12): the table built at link time from the link plan (one executor per granted connector, holding its checked declaration and the declared operations); the real `connectors` implementation over it; a stand-in type that answers `not-granted` to every call; `HostState` gains the phase (`Render` or `Update`), the run's call counter, the update's response byte total and its deadline; the token buckets live in `Runtime`; for the white-box `connector-table` case, `#[cfg(feature = "test-network")] pub fn Plugin::connector_table(&self) -> Option<Vec<String>>` returns the table's keys, or `None` when the stand-in is linked
- [ ] T035 Load and run connectors in `runtime/src/lib.rs` (`data-model.md`, Load decision): step 3 with `connectors`; step 5 with minors, `contract-too-old` and `missing-updater`; step 6a resolving each declared connector from the catalog or, only when the runtime is in development mode, `local/<name>` from `<connectors dir>/<name>.json` (a repository id, or a local id outside development mode, is `destination-not-verified`; a local file whose `id` is a `catalog/` id, or that declares a reserved host, is `destination-reserved`; a local file whose `id` is anything else than `local/<name>` is `invalid-connector`, naming both; a connector not found, another major or a missing declared operation is `connector-unavailable`); step 7 comparing connectors with the entry; step 7a kept data; step 8 pruning connectors; step 9 linking `connectors` real over the table or the stand-in; the development mode, connectors directory and data directory taken from the `RuntimeOptions` of T022; `approve` takes the expected approval digest of T028 for a widget that declares a connector; `Plugin::update(config) -> UpdateReport` with the update time limit, render's fuel and memory, a fresh instance, the output checked as JSON within 256 KiB and kept; `Plugin::render` wraps the configuration in the document of `data-model.md` (What render receives) for widgets that export `updater`, and never causes a network request
- [ ] T036 Move the existing tests to the new API, with `runtime/src/bin/runtime.rs` and `bench/src/bin/compile_cost.rs` already building since T022 and T028: update `runtime/tests/common/mod.rs` (`connected_manifest(...)`, a `runtime()` helper building `Runtime::new(limits, RuntimeOptions::new(<temporary data directory>))`, `approved` recording connectors), `runtime/tests/contract.rs`, `runtime/tests/permissions.rs`, `runtime/tests/hostile.rs` and `runtime/tests/cli.rs` where the API changed; every test of features 001 and 002 passes with unchanged expectations, the v1 ones as 1.0.0 plugins on the 1.1.0 host (SC-006), except the `contract:` line that `permissions` prints for v1 plugins, now `contract: v1 (1.0.0)` (`runtime/tests/cli.rs:239` and `:258`)

**Checkpoint**: connectors load and calls run through the network module with every rule; the
suites of features 001 and 002 are green

---

## Phase 3: User Story 1 - A widget gets external data only through declared operations (Priority: P1) 🎯 MVP

**Goal**: the maintainer sees a widget's connectors, destinations, operations, parameters and
fixed values, approves exactly what was shown, and the widget updates through a declared
operation and renders the data

**Independent Test**: quickstart step 2 offline, with a seam route from `api.github.com:443` to
a test service; then an undeclared operation and a value outside its type are refused with
nothing sent

### Tests for User Story 1

> Write these first; they fail until the implementation tasks below land

- [ ] T037 [P] [US1] Create `runtime/tests/updates.rs` with User Story 1's acceptance scenarios at the API level, through the seam with a route from `api.github.com:443` to a test HTTPS service: `github-releases` approved updates with one `GET /repos/rust-lang/rust/releases?per_page=5` and renders the releases; the request log holds that one entry with its arguments; `net-probe` calling an operation its manifest does not declare gets `refused(operation-not-declared)`, a value outside its type `refused(value-outside-type)`, a connector it does not declare `not-granted`, and the test service receives nothing for any of them; a call during `render` is `refused(called-outside-update)`
- [ ] T038 [P] [US1] Add CLI tests to `runtime/tests/cli.rs`, every command with `--grants` and `--data` in a temporary directory and a local connector on a loopback HTTP test service: `permissions` prints exactly the format of `contracts/cli.md` (contract `v1 (1.1.0)`, the `connectors` section with `from: local file <path> (development)`, `destinations:`, each operation with `read (declared by the developer)` or `change (declared by the developer)`, its method and URL template, `sends:` and `fixed:`, the `data flow:` line when the widget combines this machine with another destination, and `approval digest:`) and changes nothing; a widget without connectors shows neither the `connectors:` section nor the digest; `approve` without `--expect` for a widget with connectors is a usage error (exit 2) saying to run `permissions`, granting nothing; with the printed digest it prints `approved …`, one `granted: …` line per required permission and `connectors: …`; after the connector file changes, `approve --expect <old digest>` is `approval-changed`, granting nothing; `update` prints `updated <id> <version>` and `calls: n (g not granted, r refused, f failed), b bytes received`; `run` then renders the kept data; `requests` prints the update's entry in the format of `contracts/cli.md`, `--plugin` keeps one plugin's entries, `--last 1` keeps the newest, and an update of 21 calls ends with the summary line; `update` of a widget without `updater` is `missing-updater`; every refusal names its connector, operation, parameter or destination and its rule (SC-003); texts from the package and the connector are printed escaped

### Implementation for User Story 1

- [ ] T039 [US1] Add to `runtime/src/bin/runtime.rs` exactly as in `contracts/cli.md`: `--data <dir>` (default `<data dir>/dev.philsantos.dashboard/development/`) and `--connectors <dir>` (default `<data>/connectors/`) on every command that needs them; the runtime built with development mode on; `permissions` with the contract line of T025 and, only for widgets that declare a connector, the `connectors` section, `fixed:`, the `data flow:` line and the approval digest of T028; `approve [--expect <digest>]`: without it, a widget that declares a connector is a usage error (exit 2) saying to run `permissions`, and with it the library refuses a mismatch as `approval-changed`; `update <plugin>`; `run` rendering connected widgets from kept data; `requests [--plugin <id>] [--last <n>]` (default 50, oldest first, the time in UTC formatted by the host from milliseconds, one line per entry, every text a plugin or a connector wrote escaped, the summary line); update the usage text and the module comment
- [ ] T040 [US1] Make `plugins/examples/github-releases/README.md` and `plugins/examples/README.md` show the whole flow (`permissions`, `approve --expect`, `update`, `run`, `requests`) and say that `hello-widget` is built from the frozen 1.0.0 contract to show that older minors keep running

**Checkpoint**: quickstart step 2 works offline in the tests and online by hand; User Story 1
is complete

---

## Phase 4: User Story 2 - Hostile network cases are contained (Priority: P1)

**Goal**: the 30 cases of `data-model.md` (Hostile cases) refused or contained on three systems,
with the test services receiving nothing from a refused case: the gate case of Phase 2

**Independent Test**: `cargo test -p runtime --test hostile`; every new case passes and a
well-behaved widget updates and renders right after each one

### Tests for User Story 2

- [ ] T041 [US2] Add the load-level cases of `data-model.md` (Hostile cases) to `runtime/tests/hostile.rs`, each followed by a normal update and render of `github-releases`: `generic-http`, `read-with-body`, `fixed-headers`, `credentials`, `repository-connector`, `local-outside-development`, `reserved-host` (`api.github.com`, `API.GitHub.com`, `uploads.github.com`, `github.com` and `api.github.com:8443` are `destination-reserved`; `api.github.com.` is `invalid-connector`, since a name ending with a dot is refused first; and the manifest declaring `local/github`, whose file says `"id": "catalog/github"`, is `destination-reserved`), `other-file`, `connector-changed`, `pruned-connector`, `approval-changed` (approving through the library with the digest computed before the local connector file changed is `approval-changed`, granting nothing), and the seam half of `catalog-reserves`, an extra catalog connector through the seam whose destination is outside its `reserves` (the built-in half is T015's unit test)
- [ ] T042 [US2] Add the call-level cases to `runtime/tests/hostile.rs` with `net-probe`: `value-outside-type` (too long, wrong pattern, out of range, unknown, missing, repeated, `""`, `.` and `..` in a path), `value-encoding` (path and query on a local connector; the body of a catalog `POST` read through the seam, checked as the test service received it), `undeclared-operation`, `call-in-render`, `change-in-update`, `optional-off` (off answers `not-granted`; turned off, and the approval removed, through a second `GrantStore` on the same record file, from another thread, as another process would, since each call re-reads the file, while the test service delays a response: the response is dropped; turned on from another process: still off for the loaded plugin, and on once the plugin is loaded again), and the white-box `connector-table` through `Plugin::connector_table`
- [ ] T043 [US2] Add the network-level cases to `runtime/tests/hostile.rs` through the seam: `private-address` (`10.0.0.1`, `127.0.0.1`, `::1`, `169.254.169.254`, `::ffff:192.168.0.1`, an IP destination, plain HTTP outside development), `rebinding`, `exact-port`, `redirect-escape`, `tls` (including the AIA, OCSP and CRL recording server), `cookie`, `over-limits` (every limit of `data-model.md`, the 4 MiB per update with `net-probe`'s `"bodies": "size"`, including a catalog `POST` read with a body over 64 KiB through the seam, and two widgets sharing the per-host rate), and `system-proxy`: the HTTPS half in a child test process (`std::process::Command` running this test binary with an argument, the six proxy variables set and `NO_PROXY`, `no_proxy` cleared), the loopback half through the CLI with the same environment; the recording proxy receives nothing
- [ ] T044 [US2] Add the log cases to `runtime/tests/hostile.rs`: `log-flood` (10 000 refused and not-granted calls in one update, and as many in one render through `render_calls` with `repeat`: one summary entry each, earlier entries still there), `log-spoof` (names and argument values with newlines and terminal escapes: `runtime requests` prints them escaped, one line per entry), `log-complete` (every call of the suite and every redirect hop has its entry or is counted in its run's summary; rotation keeps the newest)
- [ ] T045 [US2] Update `plugins/hostile/README.md` and `plugins/fixtures/README.md`: the connector cases live in `runtime/tests/hostile.rs` on `net-probe` and `github-releases`, with a table of case, rule and expected outcome taken from `data-model.md`; ask the maintainer to push, then confirm the suite is green on Windows, macOS and Linux in CI, fixing the host (never the test) if a case is not contained

**Checkpoint**: the gate's network case passes on three systems

---

## Phase 5: User Story 3 - The network stays out of rendering (Priority: P2)

**Goal**: data refreshed only in `update`; render shows kept data, its time and the last
outcome, offline and across runs

**Independent Test**: quickstart step 3

### Tests for User Story 3

- [ ] T046 [P] [US3] Add User Story 3's scenarios to `runtime/tests/updates.rs`: render receives `data`, `fetched_at_ms` and `last_update` and finishes within render's limits with no request reaching any test service; a failed update (service down, error status turned into an error by the widget, update over 10 s, output that is not JSON or over 256 KiB through `net-probe`'s `output`, a refused call that ends it through `net-probe`'s `fail`) keeps the previous data and the next render shows the failure; a service that answers in 3 s, beyond render's 1 s, completes within the update; an update past its limit is stopped and its calls in flight abandoned; a widget updated once renders the same data from a new `Runtime` on the same data directory; a new version of the widget starts with no data; SC-004: a service that takes 5 s does not slow a render, which takes under 1 s from kept data
- [ ] T047 [P] [US3] Add CLI tests to `runtime/tests/cli.rs`, with `net-probe` packed with a local loopback connector and `"fail": true`: after one successful update, with the loopback test service stopped, `update` exits 1 naming the failure and saying the previous data was kept, and `run` still renders the previous data with its time and the failed update

### Implementation for User Story 3

- [ ] T048 [US3] Make the failure messages of `update` and the render document match `contracts/cli.md` and `data-model.md` exactly, including `(previous data kept)` and the `unavailable` detail about network proxies, in `runtime/src/lib.rs` and `runtime/src/bin/runtime.rs`

**Checkpoint**: quickstart step 3 works; User Story 3 is complete

---

## Phase 6: User Story 4 - Changes never widen network access silently (Priority: P2)

**Goal**: new destinations or changed operations need approval; optional connectors start off;
turning one off or removing the approval applies from the next call and discards kept data

**Independent Test**: User Story 4's independent test of spec.md, through the CLI

### Tests for User Story 4

- [ ] T049 [P] [US4] Add User Story 4's scenarios to `runtime/tests/permissions.rs`, keeping the widget loaded: a connector update that only adds operations the widget does not declare runs without approval; one that adds a destination, or changes the destination, parameters, request or kind of a declared operation, and a widget version that declares a new operation, are `needs-new-approval` naming what changed, with the entry unchanged; an optional connector starts off and answers `not-granted`; turning it on applies from the next update; turning it off, or removing the approval, applies from the next call, even within a running update, and discards the kept data; a widget depending on major 1 when only major 2 is available is `connector-unavailable` naming the version; a declared connector that is not found, and one that lacks an operation the widget declares, are `connector-unavailable`, naming the connector and the operation; a local connector file deleted after approval makes the next load `connector-unavailable`
- [ ] T050 [P] [US4] Add CLI tests to `runtime/tests/cli.rs`: `optional <plugin> <connector-id> off` prints `<id> off (kept data discarded)` and the next `run` shows no data; `on` prints `<id> on`; a required connector is `required-together`; `optional … off` run in another process while an `update` waits on a slow loopback service drops the response; `remove` discards the kept data; a connector file changed after approval makes `update` fail with `needs-new-approval` naming the change

### Implementation for User Story 4

- [ ] T051 [US4] Extend `optional` and `remove` in `runtime/src/bin/runtime.rs` for connector ids and kept data, as in `contracts/cli.md`

**Checkpoint**: User Story 4 is complete

---

## Phase 7: User Story 5 - Authors build connectors and widgets on contract 1.1.0 (Priority: P3)

**Goal**: authors write connectors and widgets from the schemas and docs, test them against
their own machine, and older v1 widgets keep running

**Independent Test**: quickstart steps 4 and 6; the schemas and the parsers agree

### Tests for User Story 5

- [ ] T052 [P] [US5] Extend `runtime/tests/manifest_schema.rs`: compile `wit/connector.schema.json` with `boon`; for a corpus of valid and invalid connectors covering every rule of T013, the schema and `Connector::parse` agree, with the host-only rules listed apart (`X-` headers outside the catalog, `reserves` outside the catalog and missing in it, a destination ending with a dot, an IP address in an `https://` destination, reserved hosts, credentials fields, placeholder rules, a read with a body or with a method other than `GET` or `HEAD` outside the catalog, `minimum` greater than `maximum`, a `destination` that is not a key of `destinations`, a body over 4 KiB, a file over 64 KiB, a local file whose `id` differs from its name); the same for manifests with `connectors` against `wit/manifest.schema.json`; every `connectors/catalog/*.json` and every `manifest.json` under `plugins/` validates and parses (read from `connectors/` and `plugins/`, never `specs/`)
- [ ] T053 [P] [US5] Extend `runtime/tests/contract_v1.rs`: the capability rule now covers `connectors` (and still excludes `renderer` and the new `updater`); parse `wit/history/1.0.0` and `wit/v1` and fail unless 1.1.0 contains every interface, function and type of 1.0.0 unchanged, comparing the resolved definitions and ignoring doc comments, which 1.1.0 rewrites (research R11)
- [ ] T054 [P] [US5] Add to `runtime/tests/contract.rs`: a component claiming contract 1.2.0 is `contract-mismatch` naming both versions; a 1.0.0 component whose manifest declares a connector is `contract-too-old`; a local connector on `http://127.0.0.1:<port>` runs with development mode on and is refused without it; one on `http://192.168.1.10:<port>` or a reserved host is refused even in development
- [ ] T055 [P] [US5] Add CLI tests for `check-connector` to `runtime/tests/cli.rs`: a valid local connector prints `ok: <id> <version>, <n> destination(s), <m> operation(s) (<r> read(s))`; an unknown field, a destination with a path, a parameter without a type, a `POST` read, an `X-` header, a reserved host and a file whose name is not that of its `local/<name>` id are refused with their kind, and each message names the field and the rule

### Implementation for User Story 5

- [ ] T056 [US5] Add `check-connector <file>` to `runtime/src/bin/runtime.rs` as in `contracts/cli.md`
- [ ] T057 [P] [US5] Update `docs/plugin-authors.md` (FR-021): contract 1.1.0 and the two worlds, declaring connectors in the manifest, the `update` entry point and the render document, handling `not-granted`, `refused` and `failed`, kept data, the request log, and building a 1.0.0 widget still being fine
- [ ] T058 [P] [US5] Write `docs/connectors.md` (FR-021): the connector format field by field (link the schema), reads versus changes and what can be checked, parameters and encoding, fixed values being public, destinations and the catalog's reserved domains, testing against your own machine with a local connector in development, `check-connector`, and contributing a connector to the catalog
- [ ] T059 [P] [US5] Update `wit/README.md` (contract 1.1.0, minor versions only add, `history/`, the connector schema) and `connectors/README.md` (how to contribute, link to `docs/connectors.md`)

**Checkpoint**: an author can write and check a connector and a widget from the docs alone

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: measurements, documents, CI, checks and review

- [ ] T060 Add the connector checks (load steps 6a and 7) to `Plugin::recheck` in `runtime/src/lib.rs`, which `checks_us` measures, and `Plugin::render_with_kept(config, kept)`, which renders from a kept data document held in memory; add to `runtime bench` in `runtime/src/bin/runtime.rs` the option `--kept-data <file>`: a widget that exports `updater` renders 100 times from that document as its kept data, held in memory, with no update, no file written and no request, and the median is printed as `render_with_data_us` (research R19, `contracts/cli.md`); update the bench test in `runtime/tests/cli.rs`
- [ ] T061 Update `.github/workflows/bench.yml`: also approve `github-releases` with the digest read from the `approval digest:` line of `permissions`, then bench it with `--kept-data plugins/examples/github-releases/bench-data.json`, a releases document this task adds, so nothing reaches the network; point the summary to `specs/003-plugin-connectors/performance.md`
- [ ] T062 Add to `.github/workflows/ci.yml`, in the build job with `shell: bash`, a step that fails unless `cargo check -p runtime --release --features test-network` fails with the message of the `compile_error!` on stderr (research R15); keep the documentation-only filter as it is, since `connectors/*.json` and `wit/*.json` are not documentation
- [ ] T063 Once the maintainer has pushed, run the bench workflow on the three systems and write `specs/003-plugin-connectors/performance.md`: `checks_us` against SC-007 (under 10 ms), `render_with_data_us` against SC-004 (under 1 s), and the limits of research R10 as measured, with the commit measured
- [ ] T064 [P] Update `docs/threat-model.md` (FR-022): the B2 row and the diagram become connectors run by the host; rows for a connector that hides a change as a read, an impostor or reserved-host connector, a local or private address reached by name or redirect, a certificate that would make the system fetch outside the proxy, a flooded or forged request log, and system proxies; each with its asset, attacker, boundary, principle and Phase 2
- [ ] T065 [P] Update `SECURITY.md` (the scope line on network access bypassing the host proxy now names connectors and the catalog), `CONTRIBUTING.md` (the security core list: connector engine and proxy, catalog) and `.github/pull_request_template.md` (security core: `runtime/`, `wit/`, `connectors/`, and `Cargo.lock` when it moves `ureq`, `ureq-proto`, `rustls`, `rustls-webpki`, `ring`, `webpki-roots`, `url` or `regex`; security tests: `plugins/hostile/`, `plugins/fixtures/`) (FR-022, research R20)
- [ ] T066 Run every local check: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, the same for `plugins/` on `wasm32-unknown-unknown`, `cargo xtask plugins --locked`, `cargo test --workspace --locked`, `cargo deny check`, `cargo audit` and `cargo audit --file plugins/Cargo.lock`
- [ ] T067 Walk through `specs/003-plugin-connectors/quickstart.md` on a clean temporary directory, step 2 against the real `api.github.com` once, and fix any drift between it, the CLI and the contracts
- [ ] T068 Human review (Principle IX): the maintainer reviews every commit under `runtime/`, `wit/`, `connectors/` and `plugins/`, and every `Cargo.lock` change to the network stack, line by line, and ticks the pull request's review box with their handle; CI green on the three systems; merge with `gh pr merge --merge --match-head-commit <sha> --author-email 8193616+philipesantos@users.noreply.github.com`
- [ ] T069 After the merge, mark in the development guide the Phase 2 item "Conectores declarativos executados pelo host" as done, and record in the Phase 2 gate line that the network case passes; leave "Contrato WIT v1 com essas interfaces" open until features 004–006 add theirs

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies
- **Foundational (Phase 2)**: depends on Setup; blocks every user story
- **User Stories (Phases 3–7)**: depend on Foundational; US1 first (MVP), then US2 (the gate);
  US3, US4 and US5 can follow in any order
- **Polish (Phase 8)**: depends on every user story

### User Story Dependencies

- **US1 (P1)**: after Foundational
- **US2 (P1)**: after Foundational; uses `github-releases` as the well-behaved widget and the CLI
  of T039 for `log-spoof` and the loopback half of `system-proxy`
- **US3 (P2)**: after Foundational; its CLI test builds on `update` and `run` (T039)
- **US4 (P2)**: after Foundational; its CLI tests build on `approve` and `update` (T039)
- **US5 (P3)**: after Foundational

### Within Each Phase

- Setup: T001 before T002 (the old plugins move to the frozen copy before `wit/v1` changes);
  T002 before T009 and T010; T006 before T007 and T008 (they check the new tree); T009 → T010
  (both edit `plugins/Cargo.toml`)
- Foundational: T011 and T012 first (T011 creates the empty modules the others fill); T013 →
  T014 → T015; T016 → T017; T018 → T019; T020 → T021, which passes once T035 lands; T022 →
  T023; T024 (after T017) → T025 → T026; T027 (after T014 and T017) → T028; T029 → T030; T031
  → T032; T033 (after T014 and T017) → T034 after T023, T026, T028, T030 and T032; T035 after T015,
  T017, T026 and T034; T036 last
- `runtime/src/bin/runtime.rs` tasks run in order (T039 → T048 → T051 → T056 → T060)
- `runtime/tests/cli.rs`, `runtime/tests/updates.rs`, `runtime/tests/permissions.rs` and
  `runtime/tests/hostile.rs` tasks of different stories edit the same files: run them in order
- Tests before their implementation; they must fail first

### Parallel Opportunities

- Setup: T003, T004 and T005; T007 and T008
- Foundational: T011 with T012; T018–T019 with the connector and manifest tasks; T029–T030
  with T031–T032
- User Story 1: T037 and T038
- User Story 3: T046 and T047
- User Story 4: T049 and T050
- User Story 5: T052, T053, T054 and T055; T057, T058 and T059
- Polish: T064 and T065

---

## Parallel Example: User Story 5

```bash
# Tests in four different files:
Task: "runtime/tests/manifest_schema.rs: connector schema and parser agree (boon)"
Task: "runtime/tests/contract_v1.rs: 1.1.0 is a superset of 1.0.0; connectors answers not-granted"
Task: "runtime/tests/contract.rs: newer minor refused, contract-too-old, local connectors in development"
Task: "runtime/tests/cli.rs: check-connector"

# Then the documentation, alongside check-connector:
Task: "docs/plugin-authors.md: connectors, update, contract 1.1.0"
Task: "docs/connectors.md: writing and testing a connector"
Task: "wit/README.md and connectors/README.md"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1: Setup
2. Phase 2: Foundational
3. Phase 3: User Story 1
4. **STOP and VALIDATE**: quickstart step 2. The host already applies every rule, but the gate
   is User Story 2: nothing may build on this until its cases pass on three systems.

### Incremental Delivery

1. Setup + Foundational → connectors, the network module and contract 1.1.0, with the suites of
   features 001 and 002 green
2. User Story 1 → permissions, approve with the digest, update and run through the CLI
3. User Story 2 → the 30 hostile cases green on three systems: **the gate's network case**
4. User Story 3 → render from kept data, offline and across runs
5. User Story 4 → changes, optional connectors, revocation
6. User Story 5 → schemas, contract checks, `check-connector`, author documentation
7. Polish → measurements, threat model, documents, CI, local checks, quickstart, human review

---

## Notes

- [P] tasks touch different files and depend on no incomplete task
- All work happens on the `003-plugin-connectors` branch, in one pull request opened as a draft
  when the maintainer asks to push. Never commit this feature directly to `master`
- One commit per task or small group of tasks, without any co-author or tool attribution. The
  maintainer reviews each commit line by line (Principle IX); a fix after review is a new commit,
  never a rewrite of a reviewed one
- Push only when the maintainer asks
- Everything under `runtime/`, `wit/` and `connectors/` is security core, as are `Cargo.lock`
  changes to the network stack; `plugins/hostile/`, `plugins/fixtures/`, `runtime/tests/hostile.rs`
  and `runtime/tests/common/servers.rs` hold the security tests: line-by-line human review (T068)
- Never commit built `.wasm` files; CI builds every plugin from source
- No test reaches the internet or reads `specs/`
