---

description: "Task list for plugin credentials (feature 005)"
---

# Tasks: Plugin Credentials

**Input**: Design documents from `specs/005-plugin-credentials/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Required. The hostile suite is the security gate (Principle VIII) and gains the 32
cases of `data-model.md` (Hostile cases), plus the 14 tests of its "Other tests named" list.
Every test task comes before the implementation it checks and fails until that implementation
lands. Test helpers make a test **fail, never skip**, when a fixture is missing. No test reaches
the internet or a real service, and no test reads `specs/`. Every token and key a test issues is
a fresh random value, and the hostile cases end with the scan of research R15.

**Organization**: Tasks are grouped by user story. Contract 1.3.0, the schemas, the vault, the
credential declaration, account use in the manifest, the accounts record, grant record v4, the
`accounts` interface, adding the credential in the network module, the account table and loading
block every story, so they are foundational and built test first; the stories then add the
sign-in and the `accounts` commands, the hostile cases, the account lifecycle, the change rules
and the author tooling.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1–US6)

## Path Conventions

Paths follow plan.md: `runtime/`, `wit/` and `connectors/` (security core), `plugins/` (guest
crates), `bench/`, `docs/`, `.github/`. Built plugins go to `target/plugins/<crate-name>.wasm`.
Field rules are in `data-model.md`; decisions and numbers in `research.md` (R1–R19); CLI output
in `contracts/cli.md`; the contract draft in `contracts/widget-v1.3.wit`; the credential schema
draft in `contracts/connector-credential.schema.json`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: contract 1.3.0 and its frozen predecessor, the schemas, dependencies, lint rules, CI
and the new plugin crates. The runtime still behaves as in feature 004 at the end of this phase,
so every existing test keeps passing.

- [X] T001 Freeze contract 1.2.0: copy `wit/v1/widget.wit` unchanged to `wit/history/1.2.0/widget.wit` (still `dashboard:widget@1.2.0`); point `wit_bindgen::generate!` in `plugins/examples/update-counter/src/lib.rs`, `plugins/fixtures/store-probe/src/lib.rs` and `plugins/hostile/storage-file/src/lib.rs` to `"../../../wit/history/1.2.0"` (research R9, SC-007); confirm `cargo xtask plugins` and `cargo test --workspace` still pass
- [X] T002 Rewrite `wit/v1/widget.wit` as `dashboard:widget@1.3.0` from `specs/005-plugin-credentials/contracts/widget-v1.3.wit`: the header says minor version 3 (feature 005); `log`, `connectors`, `storage`, `renderer` and `updater` unchanged from 1.2.0; a new `interface accounts` with `use connectors.{argument, response, problem};`, `variant account-call-error { not-granted, refused(problem), failed(problem), account-not-connected, subscription-required }` (`not-granted` first) and `call: func(connector: string, operation: string, arguments: list<argument>) -> result<response, account-call-error>`; worlds `widget`, `connected-widget`, `stored-widget` and `connected-stored-widget` unchanged, plus `account-widget { import log; import connectors; import accounts; export renderer; export updater; }` and `account-stored-widget { import log; import connectors; import accounts; import storage; export renderer; export updater; }`; keep every doc comment and drop the draft wording; the runtime's `bindgen!` stays on world `connected-stored-widget` for now, so the build passes and every older plugin still loads; `runtime/tests/contract_v1.rs` keeps passing, since `account-call-error` starts with `not-granted`
- [X] T003 [P] Add `credential` to `wit/connector.schema.json` exactly as in `specs/005-plugin-credentials/contracts/connector-credential.schema.json` (a top-level optional property; `$defs` `https_address`, `credential`, `account`, `access`; `flow` only `authorization-code`; `identity` optional with `operation`, `name` and `id` required; `scheme` required when `header` is `Authorization`), keeping `credentials` and `auth` refused as feature 003 refuses them; and in `wit/manifest.schema.json`, add `account` (boolean, default `false`) to each entry of `connectors`, describe `login_providers` as "Obsolete since feature 005: accepted and ignored; accounts come from the connectors a widget declares" (research R7, FR-016), keeping each file's formatting for the other fields
- [X] T004 Update `runtime/Cargo.toml` (research R5, R17): `keyring-core = "=1.0.1"`; under `[target.'cfg(windows)'.dependencies]` `windows-native-keyring-store = { version = "=1.1.0", default-features = false }`; under `[target.'cfg(target_os = "macos")'.dependencies]` `apple-native-keyring-store = { version = "=1.0.2", features = ["keychain"] }`; under `[target.'cfg(target_os = "linux")'.dependencies]` `dbus-secret-service-keyring-store = { version = "=1.0.1", features = ["crypto-rust"] }` (never `vendored`); `zeroize = "1"`, `memchr = "2"`, `httparse = "1"`; `rpassword = "7"` and `open = { version = "5", features = ["shellexecute-on-windows"] }`, used only by `src/bin/runtime.rs`; each with a one-line comment saying why, as the file does for its other dependencies; install `libdbus-1-dev` locally if missing; confirm `cargo tree -p runtime -e normal --target all` lists the three stores and no `libdbus-sys` with `vendored`, `cargo tree -d -p runtime` adds no avoidable duplicate, and `cargo deny check` passes with no new license
- [X] T005 [P] Extend `clippy.toml` (research R19): `disallowed-types` gains `keyring_core::Entry`, `keyring_core::api::CredentialStoreApi` (or the trait the crate exports for stores), and `std::net::TcpListener`; `disallowed-methods` gains `std::net::TcpListener::bind`; reasons `only runtime/src/vault/ touches the keychain (Principle IX, research R19)` and `only runtime/src/network/ opens sockets (Principle III, research R19)`; the vault module allows them with `#![allow(clippy::disallowed_types, clippy::disallowed_methods)]` as the first line of `runtime/src/vault/mod.rs`, and `runtime/src/network/callback.rs` is covered by the network module's existing allow; confirm `cargo clippy --workspace --all-targets --locked -- -D warnings` passes
- [X] T006 [P] Update `.github/workflows/ci.yml` (research R15): on `ubuntu-24.04`, the Tauri dependencies step also installs `gnome-keyring` and `libdbus-1-dev`; the test step runs on Linux as `dbus-run-session -- bash -c "gnome-keyring-daemon --components=secrets --daemonize --unlock <<< 'ci'; cargo test --workspace --locked"` (a step with `if: runner.os == 'Linux'`), and as today on macOS and Windows (`if: runner.os != 'Linux'`); `.github/workflows/bench.yml` installs `libdbus-1-dev`, `pkg-config` and `gnome-keyring` on Linux (the bench step that reads the keychain runs inside the same session, added with it in T057); the "CI result" job is unchanged
- [X] T007 Create `plugins/fixtures/account-probe/` (research R16): contract 1.3.0, `wit_bindgen::generate!({ path: "../../../wit/v1", world: "account-widget" })`; `update(config)` reads `{"calls": [{"via": "accounts" | "connectors", "connector", "operation", "arguments": [[name, value], ...], "repeat": n}], "dump": bool, "fail": bool}` and returns `{"answers": [...], "more": n}`, each answer `{"ok": {"status", "headers", "body_text"}}` or `{"error": "not-granted" | "refused:<rule>" | "failed:<rule>" | "account-not-connected" | "subscription-required", "detail"}`, keeping 32 answers; with `"dump": true` it returns everything it was given (its configuration) as text, for the scan; `render(config)` returns the update's data as received; `[package.metadata.dashboard] manifest = false` for good, since each test packs it with its own manifest, as `net-probe`; add it to `members` in `plugins/Cargo.toml` and a line to `plugins/fixtures/README.md`
- [X] T008 Create `plugins/examples/private-notes/` (research R16, FR-020): contract 1.3.0, world `account-widget`; `connectors/notes.json`, a local connector `local/notes` 1.0.0 with destination `api` = `https://notes.test`, reads `list-notes` (`GET /notes`, response JSON) and `get-me` (`GET /me`, no parameters), and `credential` `{ "kind": "account", "required": false, "destinations": ["api"], "header": "Authorization", "scheme": "Bearer", "account": { "flow": "authorization-code", "client": "notes", "authorize": "https://notes.test/oauth/authorize", "token": "https://notes.test/oauth/token", "revoke": "https://notes.test/oauth/revoke", "scopes": ["notes:read"], "access": [{ "name": "notes", "level": "read" }], "identity": { "operation": "get-me", "name": "/name", "id": "/id" } } }`; `update` calls `list-notes` through `accounts` and returns `{"notes": [...]}` (an `account-not-connected` answer returns `{"notes": [], "account": "not connected"}`); `render` shows the notes, or "No notes yet"; `manifest = false` until T013; add it to `plugins/Cargo.toml` members and to `plugins/examples/README.md`
- [X] T009 [P] Create `plugins/hostile/token-hunt/` (research R15, data-model Hostile cases): `[package.metadata.dashboard] target = "wasm32-wasip2"`, as `read-file`, on `"../../../wit/v1"`, world `account-widget`; its `update` reads every environment variable, its arguments and the files its configuration names with `std::fs`, and returns them; its manifest (T013) declares `local/notes` with `account: true`, so only load step 6 refuses it, for its WASI imports; and create `plugins/hostile/connectors/` with the connector files of the load-time cases, each a variant of `notes.json`: `plain-http-key.json` (`kind: key` on `http://127.0.0.1:<port>`), `plain-http-beside.json` (the credential on `https://notes.test` and a second destination `http://127.0.0.1:<port>`), `token-host-elsewhere.json` (`token` on a second destination `https://other.test` the credential is not for), `identity-elsewhere.json` (`get-me` on `https://other.test`), `fixed-auth-header.json` (an operation fixing `authorization`, lowercase), with a line each in `plugins/hostile/README.md`; add the crate to `plugins/Cargo.toml` members; run `cargo xtask plugins`

**Checkpoint**: 1.3.0 and the frozen 1.2.0 in `wit/`; schemas, dependencies, lint rules and CI in
place; the new crates build; every existing test still passes

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: error kinds and limits, the vault, the credential declaration, account use, the
accounts record, grant record v4, contract 1.3.0 in the engine, the credential on requests, the
account table and loading; the test seam and the test sign-in service; the existing tests moved to
the new API

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T010 [P] Add the new kinds to `ErrorKind` in `runtime/src/error.rs` (`data-model.md` Error kinds): `credential-needs-https`, `account-use-not-declared`, `sign-in-failed`, `another-account-connected`, `key-refused`, `keychain-unavailable`, `no-client-configured`, `account-not-connected`; extend the kind-name unit test; create the empty modules `runtime/src/vault/mod.rs` (with the Clippy allow of T005 as its first line), `runtime/src/vault/store.rs`, `runtime/src/accounts.rs`, `runtime/src/signin.rs` and `runtime/src/network/callback.rs`, declared in `runtime/src/lib.rs` and `runtime/src/network/mod.rs`, so the tasks below each fill files of their own
- [X] T011 [P] Add the limits of research R13 to `Limits` in `runtime/src/limits.rs`, each with a doc comment: `sign_in_time_limit` 10 min; `max_callback_head_bytes` 8 KiB and `callback_read_time` 2 s; `max_sign_in_response_bytes` 64 KiB; `max_secret_bytes` 2,560; `max_key_bytes` 1,024; `max_account_name_chars` 100; `vault_wait_tool` 2 min (widget calls wait at most the call's time limit); `refresh_margin` 60 s; `refreshes_per_call` 1; unit test of the defaults
- [X] T012 Write the vault tests first, in `runtime/src/vault/mod.rs`'s test module with stubs, on `keyring_core::mock::Store` (research R5, data-model Vault entry): an entry's service is `dev.philsantos.dashboard.development` and its user `<vault id>:<hex SHA-256 of "<connector id>\n<origin>">`; `write`, `read`, `delete` and `NoEntry` round-trip a JSON document `{"access", "refresh"}` or `{"key"}`; a document over 2,560 bytes is refused before the store sees it; every operation runs on one vault thread, in the order given (a slow mock write followed by a read returns the written value: `vault_order`); a caller gives up after its bound and gets `keychain-unavailable`, and an operation given up on still runs before any later one; `NoStorageAccess` and `PlatformFailure` map to `keychain-unavailable`; a mock error `BadEncoding(<token bytes>)` or `BadDataFormat(<token bytes>, …)`, and a stored document that is not valid JSON, give errors whose text and `Debug` hold none of the token's bytes (`secret_free_errors`); `Secret`'s `Debug` prints `[credential]`, it has no `Display`, and its memory is zeroed on drop
- [X] T013 Implement `runtime/src/vault/mod.rs` and `runtime/src/vault/store.rs` so T012 passes: `store.rs` is the only code that names `keyring-core` or a store crate, builds the system's store (`cfg`-selected, as in research R5) or takes an injected `Arc<CredentialStore>`, and maps every `keyring-core` error to a payload-free `VaultError` before it leaves the file; `mod.rs` holds `Secret` (`zeroize::Zeroizing<String>`), the vault thread (a channel of operations, each answered on a one-shot channel), `Vault::{read, write, delete}` with a bound, and the entry naming; never format a `keyring-core` value or a `serde_json` error of a document that holds a secret (the manifests left out in T008–T009 are added in T015, once the parser reads them)
- [X] T014 Write the manifest tests first, in `runtime/src/manifest.rs`'s test module (`data-model.md` Manifest: `account`): `connectors[].account` optional, `true` or `false`, default `false`; any other type is `invalid-manifest` naming `connectors[<i>].account`; `login_providers` optional, its old values (`["github"]`) still accepted, an unknown value still refused as before, and `Manifest::login_providers()` unchanged for the tool's obsolete line; a manifest without `login_providers` is valid
- [X] T015 Implement `account` and the optional `login_providers` in `runtime/src/manifest.rs` so T014 passes, and drop `login_providers` from `required` in `wit/manifest.schema.json` (moved here from T003, so the schema and the parser change together and `runtime/tests/manifest_schema.rs` passes at every commit); then add the manifests left out in T008–T009 (`plugins/examples/private-notes/manifest.json` with `"connectors": [{"id": "local/notes", "major": 1, "operations": ["list-notes"], "required": true, "account": true}]` and no `login_providers`, `plugins/hostile/token-hunt/manifest.json`) and remove their `manifest = false` (moved here from T013); confirm `runtime/tests/manifest_schema.rs` accepts every manifest of the repository
- [X] T016 Write the credential declaration tests first, in `runtime/src/connector.rs`'s test module, following the checks of `data-model.md` (Connector: `credential`) in order, each failure `invalid-connector` naming the field unless said otherwise: the schema (`kind` `account` or `key`; `account` only with `kind: account`; `flow` only `authorization-code`; `scopes` 0–32 of `\x21\x23-\x5B\x5D-\x7E`, 1–64 each; `access` 1–32 items of `name` 1–64 characters and `level` `read` or `write`; `subscription_required` 0–4 statuses from 400–499); `destinations` 1–8 keys that exist; every destination of a connector with a credential `https`, a loopback `http://` one included, else `credential-needs-https` (FR-007); `header` `Authorization` with a `scheme` of 1–32 token characters, another name only in a catalog connector, never a host-forbidden name; header names compared without regard to case, against every header an operation fixes (`fixed-auth-header`); `authorize`, `access_page` and `revoke_page` `https`, no user information or fragment, on a host the catalog connector reserves or the host of one of a local connector's destinations; `token` and `revoke` with an origin among the credential's destinations (`token-host-elsewhere`); `identity`, when present, a declared `GET` read without parameters on a credential destination (`identity-elsewhere`), with `id` required; `credentials` and `auth` still refused as `credentials-not-supported`, naming `credential` as the field to use; a connector without `credential` parses exactly as in feature 003 (its operation digests unchanged); and, in `runtime/src/catalog.rs`'s unit test (`catalog`), every sign-in address and page of a catalog credential is under the domains the connector reserves, checked on a catalog connector with a credential added through the seam, since none ships in this feature (research R19)
- [X] T017 Implement the credential declaration in `runtime/src/connector.rs` so T016 passes (research R3): `Credential { kind, required, destinations, header, scheme, account: Option<AccountDecl>, access, subscription_required }`, its canonical digest without `access` (research R8: kind, required, destination origins, header, scheme, flow, client, addresses, scopes, identity, subscription statuses), and `Connector::credential()`; drop `credential` from `CREDENTIAL_FIELDS` at the top level only
- [X] T018 Write the accounts record tests first, in `runtime/src/accounts.rs`'s test module (`data-model.md` Accounts record, Account states): `<data dir>/accounts.json`, `record_version` 1, a newer version refused; `vault` 16 random bytes in hex, created with the record and never changed; one account per connector id and origin, with the fields of the data model, `name` cut to 100 characters, `needs_connecting` `null` or `{reason: refused | refresh-failed | missing-from-keychain | less-access, at_ms}`; writes under the record's lock with an atomic rename, as the grant record; `connect` keeps `account_id` for the same `service_id` and refuses another, or any new key while one is stored, as `another-account-connected` naming the current account (clarification 2); `mark_needs_connecting`; `disconnect(id, origin)` under the lock deletes the vault entry and then the account, so a call between them cannot mark it (G5 of the review); `list()` gives every account with whether its origin's file still exists (clarification 1); the record never holds a credential (a test writes a token through every path and greps the file)
- [X] T019 Implement `runtime/src/accounts.rs` so T018 passes (research R6, R11, R12), reusing the grant record's lock and atomic-write helpers; `refresh_locked(id, origin, f)` takes the lock, re-reads the record and the vault, skips the refresh when another process did it meanwhile, and keeps the lock until the vault thread has answered the write; a write that fails or does not finish marks the account `refresh-failed`
- [X] T020 Write the grant record tests first, in `runtime/src/grants.rs`'s test module, for `data-model.md` (Grant record, version 4; Approval digest) verbatim: `record_version` `4`; records of versions 1–3 read as 4 without `account` and written back as 4, a newer one refused; a connector entry gains `account` `{uses, credential, access}` only when the connector declares a credential, `access` sorted by name; the approval digest of a widget whose connectors declare no credential is byte for byte the one feature 004 computed (a literal pinned in the test, computed before this change), and with a credential gains `"account": {"uses", "credential", "access"}`; at load (step 7) the table of research R8: a credential added, or a `credential` digest that differs, is `needs-new-approval` naming what changed; `uses` false then `true` in the manifest is `needs-new-approval`; an `access` item added, or a level raised from `read` to `write`, is `needs-new-approval` naming the items; access narrowed, or the widget no longer using the account, runs with the approval unchanged, and the table uses what both allow; approving with no expected digest or another one is `approval-changed`
- [X] T021 Implement grant record v4 and the digest in `runtime/src/grants.rs` so T020 passes (research R8), keeping the locking and atomic writes of features 002–004; move `runtime/src/bin/runtime.rs` and the bench binaries to any new signatures, so the workspace still builds
- [X] T022 Write the contract tests first, as unit tests in `runtime/src/permissions.rs` with WAT components: a v1 component whose contract names all carry `1.3.0` is v1 with minor 3; one carrying `1.4.0`, or mixing `1.2.0` and `1.3.0` names, is `contract-mismatch` naming both versions; a manifest with `account: true` on a 1.2.0 or older component is `contract-too-old` naming 1.3.0; an import of `dashboard:widget/accounts@1.3.0` with no connector declared `account: true` is `not-declared`; a component that imports `connectors` only for the types `accounts` uses, while every declared connector is `account: true`, is accepted (`connectors_type_only`, research R9); `account: true` on a connector without a credential is `invalid-manifest`; a required credential without `account: true` is `account-use-not-declared`; the registry maps each connector with an account to `dashboard:widget/accounts` since 1.3
- [X] T023 Implement contract 1.3.0 in `runtime/src/permissions.rs` so T022 passes (research R9); the runtime's `bindgen!` moves to world `account-stored-widget` in T029, with the `accounts` host it then needs; confirm every plugin of features 001–004 still loads and runs, the 1.2.0 ones from `wit/history/1.2.0`
- [X] T024 Write the network tests first, in `runtime/src/network/mod.rs`'s test module with stubs (research R10): a `Request` built by `build_request` holds no credential, and no template, parameter value or placeholder reaches one; `Session::once` adds the header only when the request's origin is one of the credential's and its scheme is `https`, with the value marked sensitive; `follow` keeps the credential field and `once` decides per hop: a redirect to another host of the connector goes without the header, one to a host the credential is for keeps it; a credential is in the bearer-token set (letters, digits, `-._~+/=`), else refused before any request; the echo check finds the secret, and the whole header value, in the body, a declared header and `Location`, in each form of R10 (as is; `/` as `\/`; `+`, `/`, `=` as `+`, `/`, `=` in either case) and answers `invalid-response` without logging a redirect target; the host's own requests (`token`, `refresh`, `revoke`, `identity`) are form-encoded `POST`s with `Accept: application/json` (identity a `GET`), follow no redirect, parse within 64 KiB into typed structs, and report fixed text, never a `serde_json` error, for a response that does not parse
- [X] T025 Implement the credential on requests and the host's own requests in `runtime/src/network/mod.rs` so T024 passes, using `memchr::memmem` for the echo check
- [X] T026 Write the callback listener tests first, in `runtime/src/network/callback.rs`'s test module with a stub (research R2): it binds `127.0.0.1:0` only; the first `GET /callback` ends the sign-in, whatever it carries, and its query is handed over; any other path gets `404` and does not count; the page it returns is fixed text with `Cache-Control: no-store`, never anything from the request; a head over 8 KiB or slower than 2 s is dropped; the listener is closed after that request or at the time limit, and a request after it finds nothing listening
- [X] T027 Implement `runtime/src/network/callback.rs` so T026 passes, with `httparse`
- [X] T028 Write the account call tests first, in `runtime/src/calls.rs`'s test module, following `data-model.md` (An account call) on top of the steps of feature 003: the account table holds exactly the granted connectors with `account: true` and `uses`, the connectors table the others, and a call through the other interface misses and answers `not-granted`; the account snapshot is taken when the widget is linked (R11 step 0): an account connected after it is not used in this run (`connect-mid-run`); no account in the snapshot and the credential optional sends without a credential; no account, or one needing connecting again, `account-not-connected` with nothing sent; less access than declared, `account-not-connected` (reason `less-access`); the secret read once per run; a token expiring within 60 s refreshed first, at most once per call; a `401` refreshed and resent once, then confirmed with identity (refused: marked; answered: only this call `account-not-connected`); a `subscription_required` status answers `subscription-required` without the response; before sending and before handing over, an account no longer connected with the snapshot's `account_id` answers `account-not-connected` and drops the response; the refresh, the resend and the identity confirmation are not counted against the widget's calls per run or rate, and count against the destination host's rate; every entry of a call that carried a credential records the account's name, or `key`
- [X] T029 Implement the account table and steps 7a–12a in `runtime/src/calls.rs` so T028 passes, and the `accounts` interface, real and stand-in, in `runtime/src/host.rs`; record `account` in each `requests::Entry` and add the host's own entries (`plugin: "host"`, operation `token` | `refresh` | `revoke` | `identity`, no query for the first three, no arguments) in `runtime/src/requests.rs` (`data-model.md` Request log entry)
- [X] T030 Wire loading in `runtime/src/lib.rs` (`data-model.md` Load decision): step 3 with `account`; step 5 accepting up to 1.3.0; step 6 with `accounts` through the registry; step 6a checking each credential and `account-use-not-declared`; step 7 with the account approval of R8; step 7a discarding kept data whose `accounts` no longer match (kept data gains `accounts` in `runtime/src/kept.rs`); step 9 linking `connectors` and `accounts` over their tables, the stand-ins when declared and none granted, and taking the account snapshot; `Runtime::with_test_network` also takes a credential store, so tests inject `keyring_core::mock::Store`, under the seam's existing guard (debug builds with `test-network` only); confirm the suites of features 001–004 pass
- [X] T031 Create the test sign-in service in `runtime/tests/common/signin.rs` (research R15), on the HTTPS test servers of `runtime/tests/common/servers.rs`, reached through the seam's routes as `https://notes.test`: `/oauth/authorize` (plays the person: redirects to the loopback callback with a code and the state, or, when scripted, a wrong state, no state, a used code, an error, or after a delay), `/oauth/token` (checks `code_verifier` against the challenge, `redirect_uri` and `client_id`; issues fresh random tokens in the bearer-token set; rotates refresh tokens; scripted to refuse, to answer another account, or to issue a token outside the set), `/oauth/revoke`, `/me` (name and id of the current account; scripted to refuse), `/notes` (public notes without a credential, the account's own with one; scripted to answer `401`, a `subscription_required` status, or to echo the `Authorization` value in its body, a header, `Location` or an escaped JSON string), recording every request; and a `scan(dir, outputs, tokens)` helper that fails naming the file and the token when any token, refresh token, code or verifier appears in any file under the data directory, the grant record, the request log, kept data, stores, the captured output or what a fixture returned
- [X] T032 Move the tests that build grant records, manifests and connectors by hand to the new API (`runtime/tests/common/mod.rs`, `runtime/tests/permissions.rs`, `runtime/tests/network.rs`, `runtime/tests/storage.rs`, `runtime/tests/cli.rs`), so `cargo test --workspace` passes with the cases of features 001–004 unchanged

**Checkpoint**: credentials load, link and ride on requests with every rule; the suites of features
001 to 004 pass; no story is reachable from the CLI yet

---

## Phase 3: User Story 1 - A widget shows private data through the user's account (Priority: P1) 🎯 MVP

**Goal**: the maintainer connects an account, sees what a widget asks and approves it, and the
widget shows data only that account can read, never holding the token.

**Independent Test**: quickstart steps 3 and 4

### Tests for User Story 1

- [X] T033 [P] [US1] Write `runtime/tests/accounts.rs` (library, through the seam and the mock store): `private_notes` runs the example end to end against the test sign-in service (connect through `signin::connect` with `--no-browser` behavior and the service playing the person, approve with the digest, update, render: the account's own notes appear; the request log has entries `as <name>` and the host's `token` and `identity` entries without a body; disconnect: kept data deleted at once, public notes only), then `scan`; a key connected through the library's key path is sent as `Authorization: Bearer <key>`; an optional credential with no account sends without one; a widget without `account: true` sends without one while an account is connected
- [X] T034 [P] [US1] Write the CLI tests in `runtime/tests/cli.rs` (no seam, `contracts/cli.md`): `permissions` of `private-notes` prints the `credential:` block (kind, optional, header, destinations, `sign-in:` and `token:` addresses, `asks for: notes read (declared by the developer)`, `this widget acts as the account: none connected (calls go without one)`) and `login providers: none`; a widget whose `login_providers` lists `github` prints `login providers: github (obsolete: accounts come from connectors)`; `accounts list` prints `nothing connected`; `accounts connect local/notes` without `--client` and without a remembered one fails as `no-client-configured`, naming `--client`; `accounts connect` on `plugins/hostile/connectors/plain-http-key.json` fails as `credential-needs-https`; `approve` adds `account use: local/notes (notes read)`

### Implementation for User Story 1

- [X] T035 [US1] Implement the sign-in in `runtime/src/signin.rs` (`data-model.md` Sign-in, steps 1–8 in order; research R2): the client registration from `--client` or the record; the listener of T027; `state`, verifier and challenge from `ring::rand::SystemRandom` and `sha2`, base64url without padding (a small encoder, unit-tested with RFC 7636 appendix B: `pkce`); the authorization address built and returned for the tool to open; the code exchanged; identity called with the new credential; the comparison of clarification 2; the vault entry written, then the record; nothing written before step 8; a token obtained before a refusal at step 7 revoked when the connector declares `revoke`, then dropped; and the key path: `connect_key(id, origin, key)` checking the key (`key_checks`: 1–1,024 bytes of letters, digits and `-._~+/=`) before it reaches the vault
- [X] T036 [US1] Add to `runtime/src/bin/runtime.rs` (`contracts/cli.md`): `accounts connect <connector-id> [--origin <path>] [--client <id>] [--no-browser]` (opens the address with `open`, always prints it escaped, waits with the sign-in's time limit; for `kind: key` reads the key with `rpassword::read_password`), `accounts list` and `accounts disconnect <connector-id> [--origin <path>]` in their first form; the `credential:` block, the account line and the obsolete `login providers:` line in `permissions`; the `account use:` line in `approve`; `as <name>` and the host's entries in `requests`; the `account: … needs connecting again` line in `update`; every text a connector or a service wrote escaped
- [X] T037 [US1] Add the `data flow:` line to `permissions` in `runtime/src/bin/runtime.rs` (research R14, FR-005, Principle III): for a widget that uses an account, every origin of its connectors other than the credential's own destinations, as `data flow: data read as the account of <connector> can reach <origins>`, beside 003's line for data read from this machine; a CLI test `data_flow` in `runtime/tests/cli.rs` with a widget that declares `local/notes` with an account and a second connector

**Checkpoint**: quickstart steps 3 and 4 work; User Story 1 is complete

---

## Phase 4: User Story 2 - No plugin can obtain a credential (Priority: P1)

**Goal**: every case of `data-model.md` (Hostile cases) about getting hold of a credential
contained on three systems, with the scan clean after each: the gate's token case.

**Independent Test**: `cargo test -p runtime --test hostile credentials`; every case below passes
and the scan finds nothing

### Tests for User Story 2

- [X] T038 [US2] Create `runtime/tests/hostile/credentials.rs`, declared in `runtime/tests/hostile.rs` (`#[path = "hostile/credentials.rs"] mod credentials;`, and the module comment naming feature 005), with helpers that start the test sign-in service, connect an account through the library, pack `account-probe` with a manifest, approve it, run it and call `scan` at the end of every case; then the cases `token-hunt` (the WASI build refused before running; the probe's `dump` holds no token), `echo-body`, `echo-header`, `echo-location` (target not logged) and `echo-json-escaped` (each `failed(invalid-response)`, nothing of the response to the widget), and `widget-connect` (every call while nothing is connected `account-not-connected`; no listener bound, no request to `/oauth/authorize`)
- [X] T039 [US2] Add the cases `copied-widget-id` (a file elsewhere claiming the approved probe's id: not granted, no request carries the header) and the named test `secret_free_errors` at the library level (the vault's `BadEncoding(<token bytes>)`, a token response that does not parse, a stored document that does not parse: no token in the error, the log or the output) to `runtime/tests/hostile/credentials.rs`

### Implementation for User Story 2

- [X] T040 [US2] Fix whatever T038–T039 find in `runtime/src/` (any leak is a bug of the rule that should have stopped it, fixed at that rule, never in the test); confirm the cases pass and the well-behaved `private-notes` updates and renders right after each, on the same runtime

**Checkpoint**: the gate's token case passes on three systems

---

## Phase 5: User Story 3 - A credential goes only where its connector sends it (Priority: P1)

**Goal**: the credential reaches only the destinations it is for, over HTTPS, in the declared
header, from widgets approved to use it.

**Independent Test**: User Story 3's independent test of spec.md; the test service records what
each destination received

### Tests for User Story 3

- [X] T041 [US3] Add to `runtime/tests/hostile/credentials.rs` the cases `other-destination` and `redirect-other-host` (sent without the header), `header-smuggling` (CR/LF, `Authorization:` text, the scheme, the header's name: refused by type or sent as data, no header added, never replaced), `fixed-auth-header`, `plain-http-credential`, `plain-http-beside`, `token-host-elsewhere` and `identity-elsewhere` (each refused at load with its kind, from `plugins/hostile/connectors/`), `bad-certificate` (`failed(unavailable)` before any header is sent), `wrong-interface`, `no-account-use`, `optional-off` and `copied-connector-id`, each as in `data-model.md` (Hostile cases)
- [X] T042 [P] [US3] Write the combination test `credential_combinations` in `runtime/tests/accounts.rs` (SC-002, `data-model.md` Combinations): every combination of credential absent, optional, required × `account` false, true × approval `uses` false, true × connector required, optional on, optional off × account absent, connected, needs connecting × destination the credential is for, another of the connector's; the test service records whether `Authorization` arrived, and it did exactly when credential declared, `account: true`, `uses: true`, connector granted, account connected, destination one it is for; plus the named rules `credential_rules` (a redirect to a host the credential is for keeps it; the whole header value echoed is caught; a `401` body never reaches the widget)

### Implementation for User Story 3

- [X] T043 [US3] Fix whatever T041–T042 find in `runtime/src/network/mod.rs`, `runtime/src/calls.rs` or `runtime/src/connector.rs`

**Checkpoint**: User Story 3 is complete

---

## Phase 6: User Story 4 - The maintainer manages accounts (Priority: P2)

**Goal**: accounts listed, refreshed, marked and disconnected; forged sign-ins connect nothing;
the vault behaves on the real keychains.

**Independent Test**: User Story 4's independent test of spec.md; quickstart step 1

### Tests for User Story 4

- [X] T044 [US4] Add to `runtime/tests/hostile/credentials.rs` the cases `forged-callback`, `replayed-code` and `late-callback` (nothing connected, the earlier account kept), `another-account` (`another-account-connected`; the new token revoked at `/oauth/revoke` and dropped; the earlier account kept), `revoked-token` (one refresh, one resend, one identity call, then marked; no further request), `foreign-401` (only that call answers `account-not-connected`; the account stays connected for a second widget), `expired-refresh` (marked `refresh-failed`) and `account-spoof` (a name with line breaks and terminal escapes printed escaped by `accounts list`, `permissions` and `requests`)
- [X] T045 [P] [US4] Write `runtime/tests/vault.rs` on the system's keychain, each test in its own data directory, under a process-wide lock, deleting its entries from the same test binary: `vault_round_trip` (write, read, overwrite, delete, `NoEntry`; a document of 2,560 bytes kept and one byte more refused); `keychain_locked_call` with the mock store answering `NoStorageAccess` during a call (`failed(unavailable)` naming the keychain; nothing sent, with or without a credential)
- [X] T046 [P] [US4] Write `accounts_record` concurrency in `runtime/tests/accounts.rs`: two processes (a child test process, as feature 004 does) refreshing the same account at once leave one valid token at the test service and one consistent record; and `vault_order` with a slow mock write during a refresh: the account is marked `refresh-failed` when the write does not finish
- [X] T047 [P] [US4] Write the CLI tests in `runtime/tests/cli.rs` (`contracts/cli.md`): `accounts list` with an account the CLI made (a key typed in a pseudo-terminal on Linux and macOS: `key_checks` for empty, over 1,024 bytes and a character outside the set, `key-refused`); an account whose connector file was moved shows `(no file)` and `accounts disconnect --origin <path>` removes it (clarification 1); `accounts disconnect` prints `deleted from the keychain`, the revocation line and `kept data discarded: <widget>`, and the kept data file is gone at once; disconnecting nothing is `account-not-connected`; connecting a second key while one is stored is `another-account-connected`

### Implementation for User Story 4

- [X] T048 [US4] Complete `accounts list` and `accounts disconnect` in `runtime/src/bin/runtime.rs` and `runtime/src/accounts.rs`: the fields and reasons of `contracts/cli.md`, `(no file)`, `--origin`, eager deletion of the kept data that names the account (`runtime/src/kept.rs`), revocation or `revoke_page`; the key path in a pseudo-terminal; fix whatever T044–T047 find

**Checkpoint**: User Story 4 is complete

---

## Phase 7: User Story 5 - Changes never widen account access silently (Priority: P2)

**Goal**: a new credential, wider access or starting to use an account needs a new approval;
taking an account away mid-run applies at the next call; connecting one mid-run waits for the next
load.

**Independent Test**: User Story 5's independent test of spec.md

### Tests for User Story 5

- [X] T049 [US5] Add to `runtime/tests/hostile/credentials.rs` the cases `credential-added-update` and `access-widened-update` (`needs-new-approval`; once approved, `account-not-connected` with reason `less-access` until connected again), `disconnect-mid-run` (in-flight response dropped; later requests without the header or `account-not-connected`) and `connect-mid-run` (the run keeps going without the credential; the next load uses the account), with the account changed from another process, as feature 004's mid-run cases do
- [X] T050 [P] [US5] Write in `runtime/tests/permissions.rs`: narrowed access and stopping to use the account run without a new approval, with the approval unchanged; a widget version that starts using the account is refused until approved; `kept_data_account` (reconnecting the same account keeps kept data; disconnecting, or connecting another account after a disconnect, discards it, at once and at the next load)

### Implementation for User Story 5

- [X] T051 [US5] Fix whatever T049–T050 find in `runtime/src/grants.rs`, `runtime/src/calls.rs`, `runtime/src/kept.rs` or `runtime/src/lib.rs`

**Checkpoint**: User Story 5 is complete

---

## Phase 8: User Story 6 - Authors build on contract 1.3.0 (Priority: P3)

**Goal**: authors declare credentials and account use from the schemas and docs; older plugins keep
running and never see the new answers.

**Independent Test**: User Story 6's independent test of spec.md; the schemas and the parsers agree

### Tests for User Story 6

- [X] T052 [P] [US6] Extend `runtime/tests/contract_v1.rs`: 1.3.0 only adds to 1.2.0, and 1.2.0 to 1.1.0 and 1.0.0 (the pair added to the existing check); `wit/history/1.2.0/widget.wit` pinned by hash beside the other frozen contracts; `accounts` answers `not-granted` first
- [X] T053 [P] [US6] Extend `runtime/tests/contract.rs`: every 1.0.0, 1.1.0 and 1.2.0 plugin of the repository runs on the 1.3.0 host as before; a component claiming 1.4.0 is refused naming both versions; `account: true` on a 1.2.0 component is `contract-too-old` naming 1.3.0; `connectors_type_only` (a widget whose connectors are all `account: true` loads, `connectors` linked to the stand-in, its calls work through `accounts`); `subscription-required` answers only 1.3.0 widgets
- [X] T054 [P] [US6] Extend `runtime/tests/manifest_schema.rs` to `wit/connector.schema.json` too (`connector_schema`): every connector of `connectors/`, `plugins/examples/*/connectors/` and `plugins/hostile/connectors/` validates or fails exactly as the parser does; manifests with `account` and without `login_providers` agree

### Implementation for User Story 6

- [X] T055 [P] [US6] Update `docs/plugin-authors.md`: using a connector's account (`account: true`, calling through `accounts`, the snapshot at load), the answers `account-not-connected` and `subscription-required`, contract 1.3.0 and the `account-widget` worlds, `login_providers` obsolete; and `docs/connectors.md`: declaring a credential (every field of research R3), every destination HTTPS, the narrowest access, the bearer-token set, testing with a development client (`--client`)
- [X] T056 [P] [US6] Update `wit/README.md` (contract 1.3.0, `history/1.2.0`, `accounts`, why the answers are a new interface) and `connectors/README.md` (catalog review of a credential: access, addresses under `reserves`, the header)

**Checkpoint**: an author can declare and use a credential from the docs alone

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: measurements, documents, checks and review

- [ ] T057 Create `bench/src/bin/credential_cost.rs` in the `dashboard-bench` crate (research R18): in-process, with no network and no seam, the medians of 100 requests built and checked with and without a credential (the header per hop and the echo check over a 1 MiB body), printed as `request_us`, `request_with_credential_us` and `credential_added_us` (SC-006: under 5 ms), and the first and a later read of an entry from the system's keychain as `vault_first_read_us` and `vault_read_us`, deleting its entry at the end; `--iterations <n>` (default 100); `bench/tests/credential_cost.rs` runs it with `--iterations 3` and checks every line is printed; add a step to `.github/workflows/bench.yml`, with its summary pointing to `specs/005-plugin-credentials/performance.md`
- [ ] T058 Once the maintainer has pushed, and only when they ask, run the bench workflow on the three systems and write `specs/005-plugin-credentials/performance.md`: `credential_added_us` against SC-006, the keychain reads, a refresh timed in the library tests, and the limits of R13 confirmed or changed
- [ ] T059 [P] Update `docs/threat-model.md` (FR-021): rows for reading a credential from the plugin's side, a destination echoing it (raw and escaped), the credential sent to another host (destinations, redirects, token and identity addresses, plain HTTP), forged and replayed sign-ins, another account slipped in, an account locked out by one widget, an account appearing mid-run, secrets in errors and logs, keychain access by other processes (outside the model) and the device flow left out (and why); each with its asset, attacker, boundary, principle, test and Phase 2
- [ ] T060 [P] Update `SECURITY.md` (scope names the vault and accounts), `CONTRIBUTING.md` (the security core list gains the vault; Linux contributors need `libdbus-1-dev` and a Secret Service), `plugins/hostile/README.md` (the credential cases) and `.github/pull_request_template.md` (the security core gains `runtime/src/vault/`; its `Cargo.lock` line gains `keyring-core`, the three stores and `libdbus-sys`, never with `vendored`; its security tests line gains `plugins/fixtures/account-probe`, `plugins/hostile/token-hunt` and `plugins/hostile/connectors`) (FR-021, research R19)
- [ ] T061 Run every local check: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, the same for `plugins/` on `wasm32-unknown-unknown`, `cargo xtask plugins --locked`, `cargo test --workspace --locked` (on Linux inside the session of T006), `cargo deny check`, `cargo audit` and `cargo audit --file plugins/Cargo.lock`
- [ ] T062 Walk through `specs/005-plugin-credentials/quickstart.md` on a clean temporary directory and fix any drift between it, the CLI and the contracts
- [ ] T063 Human review (Principle IX): the maintainer reviews every commit under `runtime/`, `wit/`, `connectors/` and `plugins/`, and the `Cargo.lock` change that adds `keyring-core` and the stores, line by line, and ticks the pull request's review box with their handle; CI green on the three systems; merge with `gh pr merge --merge --match-head-commit <sha> --author-email 8193616+philipesantos@users.noreply.github.com`
- [ ] T064 After the merge, and only when the maintainer asks, mark in the development guide the Phase 2 items for the vault and for OAuth with PKCE as done and record in the Phase 2 gate line that the token case passes (the gate's last case); leave "Contrato WIT v1 com essas interfaces" open as the guide says

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies
- **Foundational (Phase 2)**: depends on Setup; blocks every user story
- **User Stories (Phases 3–8)**: depend on Foundational; US1 first (MVP), then US2 (the gate)
  and US3; US4, US5 and US6 can follow in any order
- **Polish (Phase 9)**: depends on every user story

### User Story Dependencies

- **US1 (P1)**: after Foundational; T035 (the sign-in) is what every later story connects with
- **US2 (P1)**: after US1's T035; uses `private-notes` as the well-behaved widget
- **US3 (P1)**: after US1's T035; uses the helpers of T038
- **US4 (P2)**: after US1 (T035, T036); uses the helpers of T038
- **US5 (P2)**: after US1's T035; uses the helpers of T038
- **US6 (P3)**: after Foundational

### Within Each Phase

- Setup: T001 before T002 (the 1.2.0 plugins move to the frozen copy before `wit/v1` changes);
  T002 before T003, T007, T008 and T009; T004 before T005; T007 → T008 → T009 (all edit
  `plugins/Cargo.toml`)
- Foundational: T010 and T011 first (T010 creates the empty modules the others fill); T012 →
  T013; T014 → T015 (T013's manifests after T015); T016 → T017; T018 → T019 (after T013);
  T020 → T021 (after T017); T022 → T023; T024 → T025; T026 → T027; T028 → T029 (after T019,
  T021, T023, T025); T030 after T029; T031 after T027 and T030; T032 last
- `runtime/src/bin/runtime.rs` tasks run in order (T036 → T037 → T048)
- `runtime/tests/hostile/credentials.rs` tasks run in order (T038 → T039 → T041 → T044 →
  T049); so do the tasks of different stories on `runtime/tests/cli.rs`,
  `runtime/tests/accounts.rs` and `runtime/tests/permissions.rs`
- Tests before their implementation; they must fail first

### Parallel Opportunities

- Setup: T003 with T004; T005 and T006 with each other; T009 with T003
- Foundational: T010 with T011; T014–T015 with T012–T013; T016–T017 with T018–T019; T022–T023
  and T026–T027 with T024–T025
- User Story 1: T033 and T034
- User Story 3: T042 with T041
- User Story 4: T045, T046 and T047 with T044
- User Story 5: T050 with T049
- User Story 6: T052, T053 and T054; T055 and T056
- Polish: T059 and T060

---

## Parallel Example: User Story 6

```bash
# Tests in three different files:
Task: "runtime/tests/contract_v1.rs: 1.3.0 ⊇ 1.2.0 ⊇ 1.1.0 ⊇ 1.0.0; frozen contracts pinned by hash"
Task: "runtime/tests/contract.rs: older plugins run; account on 1.2.0 is contract-too-old; type-only connectors"
Task: "runtime/tests/manifest_schema.rs: connector and manifest schemas agree with the parsers"

# Then the documentation:
Task: "docs/plugin-authors.md and docs/connectors.md: accounts, credentials, contract 1.3.0"
Task: "wit/README.md and connectors/README.md: contract 1.3.0, history/1.2.0, catalog review"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1: Setup
2. Phase 2: Foundational
3. Phase 3: User Story 1
4. **STOP and VALIDATE**: quickstart steps 3 and 4. The host already applies every rule, but the
   gate is User Story 2: nothing may build on this until its cases pass on three systems.

### Incremental Delivery

1. Setup + Foundational → contract 1.3.0, the vault, credentials on requests, grant record v4 and
   the accounts record, with the suites of features 001 to 004 green
2. User Story 1 → sign-in, `accounts` commands, permissions with the credential and the data flow
3. User Story 2 → the cases of obtaining a credential green on three systems: **the gate's token
   case, its last**
4. User Story 3 → the credential only where it is for
5. User Story 4 → refresh, marking, disconnecting, forged sign-ins, real keychains
6. User Story 5 → approvals of credential changes, mid-run changes
7. User Story 6 → contract checks, schemas, author documentation
8. Polish → measurements, threat model, documents, local checks, quickstart, human review

---

## Notes

- [P] tasks touch different files and depend on no incomplete task
- All work happens on the `005-plugin-credentials` branch, in one pull request opened as a draft
  when the maintainer asks to push. Never commit this feature directly to `master`
- One commit per task or small group of tasks, without any co-author or tool attribution. The
  maintainer reviews each commit line by line (Principle IX); a fix after review is a new commit,
  never a rewrite of a reviewed one
- Push only when the maintainer asks; starting a bench run is the maintainer's call too
- Everything under `runtime/`, `wit/` and `connectors/` is security core, as is the `Cargo.lock`
  change that adds `keyring-core` and the stores; `plugins/hostile/`, `plugins/fixtures/` and
  `runtime/tests/hostile/` hold the security tests: line-by-line human review (T063)
- Never commit built `.wasm` files; CI builds every plugin from source
- No test reaches the internet or a real service, and no test reads `specs/`
- Linux builds need `libdbus-1-dev`; tests that use the real keychain on Linux need a session bus
  with an unlocked Secret Service (T006, quickstart prerequisites)
