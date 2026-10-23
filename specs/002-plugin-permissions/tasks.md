---

description: "Task list for plugin permissions (feature 002)"
---

# Tasks: Plugin Permissions

**Input**: Design documents from `specs/002-plugin-permissions/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Required. The hostile-plugin suite is the security gate (Principle VIII) and grows
with one case per rule of FR-016. Test helpers must make a test **fail, never skip**, when a
plugin fixture is missing. Every test task comes before the implementation it checks and fails
until that implementation lands.

**Organization**: Tasks are grouped by user story. The permission engine (manifest, package,
grant record, load decision, linking) blocks every story, so it is foundational and built test
first; the stories then add the CLI commands, the next-call re-linking and the author tooling,
each with its own tests.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1–US5)

## Path Conventions

Paths follow plan.md: `runtime/` and `wit/` (security core), `plugins/` (separate Cargo
workspace for guest crates), `xtask/`, `docs/`, `.github/workflows/`. Built plugins go to
`target/plugins/<crate-name>.wasm`. Error kinds, CLI output and the package format are in
`contracts/`; field rules are in `data-model.md`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Contracts in versioned directories, dependencies, manifests for every plugin and
the new fixtures. The runtime still ignores manifests at the end of this phase, so every
feature 001 test keeps passing.

- [X] T001 Move contract v0 into its own directory with `git mv wit/widget.wit wit/v0/widget.wit` (content unchanged, still `dashboard:widget@0.1.0`); change the `bindgen!` path in `runtime/src/lib.rs` to `"../wit/v0"` and the `wit_bindgen::generate!` path to `"../../../wit/v0"` in `plugins/examples/hello-widget/src/lib.rs` and in all eleven `plugins/hostile/*/src/lib.rs`; update `wit/README.md` to list `v0/widget.wit`; confirm `cargo xtask plugins` and `cargo test -p runtime` still pass
- [ ] T002 [P] Create `wit/v1/widget.wit` from `specs/002-plugin-permissions/contracts/widget-v1.wit`: package `dashboard:widget@1.0.0`; interface `log` with `enum level { debug, info, warn, error }`, `enum log-error { not-granted }` and `write: func(level: level, message: string) -> result<_, log-error>`; interface `renderer` with `render: func(config: string) -> result<string, string>`; `world widget { import log; export renderer; }`; keep the doc comments, including the rule that every capability function returns `result<T, E>` whose `E` has `not-granted` as its first case, and drop the "draft" wording
- [ ] T003 [P] Create `wit/manifest.schema.json` from `specs/002-plugin-permissions/contracts/manifest.schema.json` (JSON Schema draft 2020-12), keeping every constraint verbatim and removing "Draft of wit/manifest.schema.json" from the top-level description
- [ ] T004 Update `runtime/Cargo.toml` (research R12): dependencies `serde = { version = "1", features = ["derive"] }`, `serde_path_to_error = "0.1"`, `semver = "1"`, `sha2 = "0.10"`, `wasmparser = "=0.261.0"` (the version Wasmtime 50 uses; check `cargo tree -p runtime -i wasmparser`), `dirs = "7"`, `tempfile = "3"`; dev-dependencies `boon` (latest) and `wit-parser = "=0.261.0"`; each with a one-line comment saying why; confirm `cargo tree -d -p runtime` adds no new duplicate versions, and that `cargo deny check` and `cargo audit` pass
- [ ] T005 [P] Add a `manifest.json` beside the `Cargo.toml` of `plugins/examples/hello-widget` and of each of the eleven `plugins/hostile/*` crates: `"manifest_version": 1`, `"id": "local/<crate-name>"`, `"version"` equal to the crate version (`"0.1.0"`), `"author": "Dashboard contributors"`, `"permissions": [{ "name": "log", "required": true }]` when the crate's `src/lib.rs` calls the `log` interface's `write` (check with `grep -l "write(" plugins/*/*/src/lib.rs`) and `[]` otherwise, `"login_providers": []`, plus `"$schema": "../../../wit/manifest.schema.json"` so editors validate it
- [ ] T006 Add manifest packing to `xtask/src/main.rs` (research R10, `contracts/package.md`): add `wasm-encoder = "0.262"` to `xtask/Cargo.toml`; take each crate's directory from the `manifest_path` that `cargo metadata` reports; after componentizing (or copying a `wasm32-wasip2` output), read `<crate dir>/manifest.json` and append it as a top-level custom section named `dashboard-manifest` (its exact bytes, no re-encoding) at the end of the component; a crate whose `Cargo.toml` declares `[package.metadata.dashboard] manifest = false` is written bare; a crate without `manifest.json` and without that opt-out is an error naming the crate; any other `manifest` value is an error; document both keys in the module comment
- [ ] T007 Create `plugins/fixtures/hello-v0/` (`Cargo.toml` as `hello-widget`'s with `description = "Test fixture: a contract v0 plugin that logs"`, `src/lib.rs` with `wit_bindgen::generate!({ path: "../../../wit/v0", world: "widget" })` that writes one `info` log `rendering hello-v0` and returns `{"title":"Hello","text":"Hello, <name>!"}` like feature 001's example, and `manifest.json` with id `local/hello-v0`, `log` required); add `"fixtures/hello-v0"` to `members` in `plugins/Cargo.toml`; create `plugins/fixtures/README.md` saying these are well-behaved test fixtures, not examples for authors, with one line per fixture
- [ ] T008 Create `plugins/fixtures/probe/`: contract v1 (`path: "../../../wit/v1"`); `render` calls `write(Level::Info, "probe")` once and returns `{"log":"ok"}` when it answered `ok` and `{"log":"not-granted"}` when it answered `err(not-granted)`; `[package.metadata.dashboard] manifest = false`, so it is built bare and tests pack it with each case's manifest; add it to `plugins/Cargo.toml` members and to `plugins/fixtures/README.md`
- [ ] T009 Create `plugins/fixtures/quiet/`: contract v1; `render` returns `{"quiet":true}` and never calls `log`, so its component imports nothing (research R10); `manifest = false`; add it to `plugins/Cargo.toml` members and to `plugins/fixtures/README.md`; run `cargo xtask plugins` and confirm every package has its manifest section except `probe.wasm` and `quiet.wasm`, and that `cargo test -p runtime` still passes

**Checkpoint**: contracts v0 and v1 in place; every plugin package carries its manifest

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The permission engine: error kinds, registry, manifest parser, package section,
grant record, load decision, linking with stand-ins, and the feature 001 tests moved to the new
API

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [ ] T010 [P] Add the nine new kinds to `ErrorKind` in `runtime/src/error.rs`, with `as_str` names exactly as in `contracts/cli.md`: `no-manifest`, `manifest-mismatch`, `invalid-manifest`, `unknown-permission`, `optional-needs-v1`, `not-declared`, `not-granted`, `needs-new-approval`, `required-together`; generalize `capability_refused` into a constructor that takes a kind, a list of names and an optional explanation, escaping every name with `text::untrusted`, and rename `refused_imports()` to `names()` (update its callers); unit tests for each kind's name and for escaping
- [ ] T011 [P] Create `runtime/src/permissions.rs` with the registry of `data-model.md` (Permission registry): an enum `Contract { V0, V1 }` with the renderer export names `dashboard:widget/renderer@0.1.0` and `dashboard:widget/renderer@1.0.0`; a static table mapping permission `log` to `dashboard:widget/log@0.1.0` (v0) and `dashboard:widget/log@1.0.0` (v1); lookups by permission name and by import name; unit tests for each lookup and for an unknown name
- [ ] T012 Write the manifest tests first, in a `#[cfg(test)] mod tests` of a new `runtime/src/manifest.rs` whose parse function is still a stub, covering every rule of `data-model.md` (Manifest) verbatim: `manifest_version` must be `1`; `id` is `github.com/<owner>/<repository>` (owner 1–39 characters of `a-z`, `0-9` and inner hyphens; repository 1–100 characters of `a-z`, `0-9`, `-`, `_` and `.`, not starting with a dot) or `local/<name>` (1–64 characters of `a-z`, `0-9` and inner hyphens), lowercase only, `github.com` the only host; `version` SemVer 2.0.0, at most 64 characters; `author` 1–100 characters, no control characters (`char::is_control`: C0, DEL and C1, so U+0085 is refused); `permissions` at most 32 entries of `{ "name", "required" }` with name `[a-z][a-z0-9-]{0,31}`; `login_providers` at most 8, unique, each `github`; `$schema` accepted and ignored; unknown fields and duplicate fields refused; the same permission twice with the same `required` counts once and with different values is `invalid-manifest`; a well-formed name the registry does not know is `unknown-permission` naming it; a payload over 16 KiB or not UTF-8 is refused before parsing; a payload that is not JSON, and one nesting 10 000 arrays, are `invalid-manifest`, without overflowing the stack; every `invalid-manifest` detail names the field path (for example `permissions[0].name`) and the problem
- [ ] T013 Implement `runtime/src/manifest.rs` (research R2, R3) so T012 passes: public types `Manifest`, `PluginId` and `PermissionRequest { name, required }`; `Manifest::parse(payload: &[u8]) -> Result<Manifest, RuntimeError>` that checks the 16 KiB cap and UTF-8 first, deserializes with `serde` (`deny_unknown_fields`) through `serde_path_to_error`, validates the version with `semver`, and checks known permissions against the registry (T011); declare the module in `runtime/src/lib.rs`
- [ ] T014 [P] Write the package tests first, in `runtime/src/package.rs` with stub functions (`contracts/package.md`): one top-level `dashboard-manifest` section is found; none is `no-manifest`; two top-level sections are `manifest-mismatch`; a section inside a nested core module (built with the `wat` dev-dependency and an `@custom` annotation) is `manifest-mismatch`; the digest is `sha256:` followed by 64 lowercase hex digits of the whole file; `pack` appends a section that the finder then returns, refuses a component that already has one with `manifest-mismatch`, and refuses an invalid manifest with `invalid-manifest`
- [ ] T015 Implement `runtime/src/package.rs` so T014 passes: `manifest_section(bytes) -> Result<&[u8], RuntimeError>` walking only the top level with `wasmparser::Parser` (skip nested modules and components, but still detect a `dashboard-manifest` section inside them), `digest(bytes) -> String` with `sha2`, and `pack(component, manifest) -> Result<Vec<u8>, RuntimeError>` that parses the manifest with `Manifest::parse`, refuses an existing section and appends id 0, the LEB128 size, the name and the payload; make the module public, since the CLI and the tests use `pack`
- [ ] T016 [P] Write the grant record tests first, in `runtime/src/grants.rs` with stub methods, using temporary directories: the record format of `data-model.md` (Grant record) verbatim (`record_version` `1`; `plugins` maps a plugin id to its entry; `origin` the place the package was approved from; `granted` sorted, unique; `approved_version`, `approved_digest` for display; an unknown permission name kept and never linked); a missing file counts as empty; a file that cannot be read, is not JSON, breaks these rules or has a newer `record_version` grants nothing, reports the reason with the path, and is never overwritten (every change refused); approve from the entry's origin sets `granted` to (`granted` ∩ declared) ∪ required and records version and digest; approve with no entry, or from another origin, replaces the entry with that origin and `granted` = required, and reports the origin it replaced; turning an optional permission on or off adds or removes that one name, is refused with `required-together` for a required one and with `not-granted` from another origin; remove deletes the entry; prune removes granted names a manifest no longer declares; every change increments the generation; the in-memory store behaves the same; two threads changing the same file record lose no update
- [ ] T017 Implement `runtime/src/grants.rs` so T016 passes (research R4): `GrantRecord` with `serde`; `GrantStore` backed by memory or a file, with a generation `AtomicU64`; `GrantStore::in_memory()`, `GrantStore::open(path)`, `GrantStore::default_development_path() -> Option<PathBuf>` (`dirs::data_dir()` joined with `dev.philsantos.dashboard/development-grants.json`); reads take a shared lock and changes an exclusive lock on `<record>.lock` (`std::fs::File::lock_shared` / `lock`), re-read the record, apply the change, write a `tempfile::NamedTempFile` in the same directory, sync it and `persist` it over the record; methods `entry`, `approve`, `set_optional`, `remove`, `prune` and `generation`, where every method but `remove` and `generation` takes the package's origin; make the module public; a unit test that `default_development_path()` ends with `dev.philsantos.dashboard/development-grants.json` when the system has a data directory
- [ ] T018 Write the load-decision tests first, as unit tests in `runtime/src/permissions.rs` with WAT components and an in-memory store, for steps 5–8 of `data-model.md` (Load decision): the `renderer` export sets the contract; a `dashboard:widget` import from another version, including a newer minor such as `log@1.1.0`, is `contract-mismatch`; a v0 plugin with an optional permission is `optional-needs-v1`; a `wasi:*` import, even an empty instance, is `capability-refused` and every such import is listed, sorted (the cases of feature 001's `imports.rs` tests); an import whose permission is not declared is `not-declared`; no entry is `not-granted`; an entry from another origin is `not-granted`, naming the approved origin (FR-020); an entry missing a required permission is `needs-new-approval`, listing every missing one; a required permission not imported must still be granted; the link plan names the real interface for granted imports and the stand-in for optional ones that are off; pruning names what the record must drop; a table-driven test over every row of `data-model.md` (Combinations, SC-002), plus a plugin without an approval
- [ ] T019 Implement the load decision in `runtime/src/permissions.rs` so T018 passes: a function from the component's imports and export, the manifest, the package's origin and the store's entry to a `LinkPlan { contract, real, stand_in }` or the refusal, plus the list to prune; delete `runtime/src/imports.rs` and its module declaration, since this check replaces it
- [ ] T020 Bind both contracts and add the stand-in in `runtime/src/host.rs` and `runtime/src/lib.rs`: `bindgen!` once in `mod v0` (`"../wit/v0"`) and once in `mod v1` (`"../wit/v1"`); the real `log` for both versions keeps feature 001's caps (100 messages, 1 KiB each, escaped) and v1 returns `Ok(())` even when it drops a message; a separate stand-in type implements the v1 `log` host trait, returns `Err(LogError::NotGranted)` and records nothing (`data-model.md`, Stand-in)
- [ ] T021 Rewrite `Runtime::load` in `runtime/src/lib.rs` as `load(&self, package: &[u8], origin: &str, grants: Arc<GrantStore>) -> Result<Plugin, RuntimeError>`, following steps 1–9 of `data-model.md` (Load decision) in order: size and preamble, manifest section, manifest, compile, contract and imports, grants, pruning (written to the store), then a linker built for this plugin with the real interface for each granted import and the stand-in for each optional one that is off; `Plugin` keeps the component, manifest, contract, origin, store and the prepared instance for v0 or v1, and exposes `manifest()` and `contract()`; `render` dispatches to the right bindings; in `runtime/src/bin/runtime.rs`, `run` and `bench` accept `--grants <path>` and otherwise open `GrantStore::default_development_path()` (no data directory is a usage error), and pass the package's canonical path as its origin (a path that is not UTF-8 is a usage error); in the module docs, say the origin is what keeps another file that copies an approved id from inheriting its grants; in `runtime/examples/compile_cost.rs`, pack each generated package with a manifest and approve it in an in-memory store, so it still reaches the compile step
- [ ] T022 Move feature 001's tests to the new API: in `runtime/tests/common/mod.rs` add `manifest(id, version, permissions) -> String`, `repack(package, manifest) -> Vec<u8>` (drops the top-level manifest section and packs a new one), `approved(package) -> Arc<GrantStore>` (an in-memory store with that package's required permissions approved from a fixed test origin, which every test load passes) and `temp_grants() -> (TempDir, PathBuf)`; update `runtime/tests/contract.rs`, `runtime/tests/hostile.rs` and `runtime/tests/cli.rs` to approve their fixtures first (the CLI tests write the record with `GrantStore::open` and pass `--grants`); every feature 001 test passes, the twelve hostile cases included, with unchanged expectations

**Checkpoint**: the engine loads only approved plugins with their declared and granted
capabilities; feature 001's suite is green

---

## Phase 3: User Story 1 - A plugin gets exactly the capabilities declared and granted (Priority: P1) 🎯 MVP

**Goal**: the maintainer sees what a plugin requests, approves its required permissions all
together, and the plugin runs with exactly what is declared and granted

**Independent Test**: approve the example through the CLI and run it; a plugin whose required
permissions were never approved is refused, naming the permission (quickstart step 2)

### Tests for User Story 1

> Write these first; they fail until the implementation tasks below land

- [ ] T023 [P] [US1] Create `runtime/tests/permissions.rs` with User Story 1's acceptance scenarios at the API level: the store lists every requested permission, required or optional, and which are granted; an approved plugin runs with `log`; an import not declared is `not-declared`; a required permission not approved is `not-granted`, naming it; a package without a manifest or with two is refused with its kind; `approve` grants every required permission at once and offers no way to grant some (only `log` exists in this feature, so scenario 6 is checked by the shape of the API and the CLI: `approve` takes no permission)
- [ ] T024 [P] [US1] Update `runtime/tests/contract.rs`: `hello-widget` on contract v1 renders `{"title":"Hello","text":"Hello, world!"}` and one `info` log after approval; `hello-v0` still renders after approval (FR-013, User Story 5 scenario 3); `hello-v0` repacked with `log` optional is `optional-needs-v1`
- [ ] T025 [P] [US1] Add CLI tests to `runtime/tests/cli.rs`, every command with `--grants` in a temporary directory: `run` before `approve` exits 1 with `error: not-granted:` naming `log`; `permissions` prints exactly the format of `contracts/cli.md` (plugin, author, contract, approved, one line per permission, login providers) and changes nothing; `approve` prints `approved local/hello-widget 0.2.0` and `granted: log`, then `run` succeeds; `approve` with a permission argument is a usage error (exit 2); an author with bidirectional control characters is printed escaped; a corrupt record makes `run` fail with `not-granted`, giving the reason and the path, and leaves the file as it was; a manifest with `login_providers: ["github"]` shows `login providers: github (connecting accounts arrives in feature 005)`; `permissions` shows the approved file's canonical path; a copy of the approved file in another directory is refused by `run` with `not-granted`, naming the approved file, and `permissions` on it reads `no (approved from <path>)`; `approve` on the copy prints `replaced: approval from <path>` first, and `run` of the original is then `not-granted`

### Implementation for User Story 1

- [ ] T026 [US1] Move `plugins/examples/hello-widget` to contract v1: `path: "../../../wit/v1"`, handle the `Result` of `write` (it is always `Ok` here, since `log` is required; say so in a comment), bump the crate and `manifest.json` version to `0.2.0`, and update `plugins/examples/hello-widget/README.md` to say contract v1 and show `approve` before `run`
- [ ] T027 [US1] Add a status query to `runtime/src/grants.rs` (or `permissions.rs`): for a manifest, each declared permission with `required` and granted or not, plus the entry's origin, approved version and digest, and whether the origin is this package's; unit tests
- [ ] T028 [US1] Add `permissions <plugin>` and `approve <plugin>` to `runtime/src/bin/runtime.rs` exactly as in `contracts/cli.md`: `approve` runs load steps 1–6 first and grants nothing if they refuse; every printed text from the package is escaped; update the usage text and the module comment

**Checkpoint**: quickstart step 2 works; User Story 1 is complete

---

## Phase 4: User Story 2 - Hostile permission cases are contained (Priority: P1)

**Goal**: one hostile case per rule of FR-016, each refused with its rule (or contained, for
FR-019; for FR-009 and FR-010, the grant record left unchanged), on three systems: the security
gate of this feature

**Independent Test**: `cargo test -p runtime --test hostile`; every new case passes and a
well-behaved plugin renders right after each one

### Tests for User Story 2

- [ ] T029 [US2] Add the manifest-level cases of `data-model.md` (Hostile-plugin cases) to `runtime/tests/hostile.rs`, each followed by a normal `hello-widget` render on the same runtime: `no-manifest` (bare `probe`) → `no-manifest`; `invalid-manifest` (`probe` with an unknown field, and with an uppercase id) → `invalid-manifest` naming the field; `two-manifests` (`probe` packed with a manifest without permissions, then a second section requesting `log`) → `manifest-mismatch`; `nested-manifest` (a WAT component whose only manifest section is in a nested module) → `manifest-mismatch`; `unknown-permission` (`quiet` with a permission `telepathy`) → `unknown-permission` naming it
- [ ] T030 [US2] Add the grant-level cases to `runtime/tests/hostile.rs`: `undeclared-capability` (`probe` without `log`, approved) → `not-declared` naming `log`; `ungranted-capability` (`probe` with `log` required, no approval) → `not-granted` naming `log`; `escalating-update` (`probe` with `log` optional, approved; then `probe` with `log` required, the same id and the same origin) → the second is `needs-new-approval` naming `log` and the record entry is unchanged; `optional-off` (`probe` with `log` optional, approved, off) → renders `{"log":"not-granted"}` and the report holds no log message; `same-id-other-origin` (FR-020: `probe` with `log` required, approved from one origin, then the same bytes loaded from another) → `not-granted` naming the approved origin, and the record entry unchanged; `grant-record-untouched` (FR-009, FR-010): run every case of T029 and T030, and the probe, against a file-backed record, asserting after each render that the record's bytes and the store's generation did not change
- [ ] T031 [US2] Update `plugins/hostile/README.md`: the feature 001 crates now carry manifests, and the permission cases live in `runtime/tests/hostile.rs` on the `probe` and `quiet` fixtures, with a table of case, rule and expected outcome taken from `data-model.md`; ask the maintainer to push, then confirm the suite is green on Windows, macOS and Linux in CI, fixing the engine (never the test) if a case is not contained

**Checkpoint**: the security gate passes on three systems

---

## Phase 5: User Story 3 - Updates never widen access silently (Priority: P2)

**Goal**: an update that requires more is refused until approved; required permissions are
never taken back one by one; removing the approval stops the plugin

**Independent Test**: quickstart step 5

### Tests for User Story 3

- [ ] T032 [P] [US3] Add User Story 3's scenarios to `runtime/tests/permissions.rs`: a new version (different digest, same id, same origin) with the same permissions runs without approval; one that requires a new permission is `needs-new-approval` naming it, and the record entry stays as it was; approving it lets it run; a version that no longer declares `log` (`quiet`) drops `log` from the record at load, and a later version requiring it needs approval again; turning off a required permission is `required-together`; after removal the plugin is refused with `not-granted`; a package with the same version number and different contents is treated as a new version
- [ ] T033 [P] [US3] Add CLI tests for `remove` to `runtime/tests/cli.rs`: prints `removed local/hello-widget` and `run` is then refused with `not-granted`; removing an id without approval prints `local/x has no approval` and exits 0; a malformed id is a usage error (exit 2)

### Implementation for User Story 3

- [ ] T034 [US3] Add `remove <plugin-id>` to `runtime/src/bin/runtime.rs` as in `contracts/cli.md`, validating the id with the manifest's id rules (T013)

**Checkpoint**: quickstart step 5 works; User Story 3 is complete

---

## Phase 6: User Story 4 - Optional permissions let a plugin run with less (Priority: P2)

**Goal**: optional permissions start off and are turned on or off in the host's controls; while
off, the stand-in answers `not-granted`; every grant change applies from the plugin's next call

**Independent Test**: quickstart step 4, and a loaded plugin that sees a change on its next
render without being loaded again

### Tests for User Story 4

- [ ] T035 [P] [US4] Add User Story 4's scenarios to `runtime/tests/permissions.rs`, loading `probe` once and keeping it loaded: with `log` optional and off it renders `{"log":"not-granted"}` and no log reaches the report; turning `log` on makes the next render return `{"log":"ok"}` with the log kept; turning it off makes the next render answer `not-granted` again; removing the approval makes the next render fail with `not-granted` and instantiate nothing, and so does approving the same id from another origin; a new version that adds an optional permission runs without approval, with it off; an optional permission that is off and becomes required in a new version is `needs-new-approval`
- [ ] T036 [P] [US4] Add CLI tests for `optional` to `runtime/tests/cli.rs`: `on` and `off` print `log on` and `log off` and change what the next `run` of a `probe` package prints; refusals per the table of `contracts/cli.md`: no approval, or an approval of another file → `not-granted`; required → `required-together`; not declared → `not-declared`; unknown → `unknown-permission`; a v0 plugin → `optional-needs-v1`

### Implementation for User Story 4

- [ ] T037 [US4] Make grant changes apply from the next call in `runtime/src/lib.rs` (research R6, `data-model.md` Plugin): `Plugin` keeps the generation its linking was built for; at the start of every `render`, if the store's generation differs, re-run steps 7–9 against the current grants and link again, keeping the compiled component; a refusal there becomes that call's error and nothing is instantiated; a call already running keeps its instance
- [ ] T038 [US4] Add `optional <plugin> <permission> on|off` to `runtime/src/bin/runtime.rs` as in `contracts/cli.md`, with each refusal kind of its table

**Checkpoint**: quickstart step 4 works; User Story 4 is complete

---

## Phase 7: User Story 5 - Authors declare what their plugin needs (Priority: P2)

**Goal**: authors write a manifest against the published schema, pack it, and build against
contract v1

**Independent Test**: the schema and the parser agree on every manifest in the repository and
the corpus; `runtime pack` produces a package that loads

### Tests for User Story 5

- [ ] T039 [P] [US5] Create `runtime/tests/manifest_schema.rs`: compile `wit/manifest.schema.json` with `boon` (draft 2020-12); for a corpus of valid and invalid manifests covering every rule of T012, the schema and `Manifest::parse` must agree (the host-only rules, `unknown-permission` and mixed `required` duplicates, are listed apart); every `manifest.json` under `plugins/` validates and parses, and has a `local/` id (SC-006, FR-017)
- [ ] T040 [P] [US5] Create `runtime/tests/contract_v1.rs`: parse `wit/v1` with `wit-parser` and fail if any function of any interface other than `renderer` does not return `result<T, E>` whose `E` is an enum or variant of its interface with `not-granted` as its first case (FR-013, research R7)
- [ ] T041 [P] [US5] Add CLI tests for `pack` to `runtime/tests/cli.rs`: packing `probe.wasm` with a valid manifest prints the output path and a `sha256:` digest and the result loads after `approve`; a component that already has a manifest is `manifest-mismatch`; an invalid manifest is `invalid-manifest` naming the field; an unknown permission is `unknown-permission`; `pack` needs no `--grants`

### Implementation for User Story 5

- [ ] T042 [US5] Add `pack <component> --manifest <file> --output <file>` to `runtime/src/bin/runtime.rs` as in `contracts/cli.md`, using `runtime::package::pack`
- [ ] T043 [P] [US5] Write `docs/plugin-authors.md` (FR-017): what a manifest is and every field with its rules (link the schema), plugin ids (`local/<name>` for anything that is not installed from its repository; one plugin per repository), required versus optional permissions and handling `not-granted` in code, packing with `cargo xtask plugins` or `runtime pack`, approving and running in development with the CLI (an approval belongs to the file it was approved from, so rebuild in place), and contract v1 with its versioning rule
- [ ] T044 [P] [US5] Update `wit/README.md` (v0 and v1 directories, the not-granted rule, how features 003–005 extend v1, the manifest schema and its `dashboard-manifest` section) and `plugins/examples/README.md` (contract v1, manifests, link to `docs/plugin-authors.md`)

**Checkpoint**: an author can write, check and pack a manifest from the documentation alone

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: performance record, threat model, documentation, checks and review

- [ ] T045 Add `checks_us` and `relink_us` to `runtime bench` in `runtime/src/bin/runtime.rs` (research R11, `contracts/cli.md`): `checks_us` times finding and parsing the manifest, reading the grant record and running the import and grant checks, without the compile; `relink_us` times one re-link after a grant change; update the bench test in `runtime/tests/cli.rs`
- [ ] T046 Update `.github/workflows/bench.yml`: before benchmarking, approve `hello-widget` into a record under `$RUNNER_TEMP` and pass that `--grants` to `bench`; point the comments and the job summary to `specs/002-plugin-permissions/performance.md` as well as feature 001's
- [ ] T047 Once the maintainer has pushed, run the bench workflow on the three systems (the `bench` label on the pull request) and write `specs/002-plugin-permissions/performance.md`: for each system `checks_us` and `relink_us`, the commit measured, and the verdict against SC-007 (under 10 ms)
- [ ] T048 [P] Add rows to `docs/threat-model.md`: a package with two manifests, or one in a nested part, so a tool and the host read different ones (T1, A4, B1; exactly one top-level manifest section; Phase 2); a development package that claims an installed plugin's id (T1, A4, B1; the development record is separate from the installed one; Phase 2); a local file that copies an approved plugin's id (T1, A4, B1; a development approval belongs to the file it was approved from; Phase 2)
- [ ] T049 [P] Update the docs that show running a plugin: `README.md`, `CONTRIBUTING.md` (if it does) and any other match of `grep -rn "runtime run\|-- run" --include=*.md . | grep -v specs/`, adding the `approve` step and pointing to `docs/plugin-authors.md`
- [ ] T050 Run every local check: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, the same `fmt` and `clippy` for `plugins/` on `wasm32-unknown-unknown`, `cargo xtask plugins --locked`, `cargo test --workspace --locked`, `cargo deny check`, `cargo audit` and `cargo audit --file plugins/Cargo.lock`
- [ ] T051 Walk through `specs/002-plugin-permissions/quickstart.md` on a clean `target/demo-grants.json` and fix any drift between it, the CLI and the contracts
- [ ] T052 Human review (Principle IX): the maintainer reviews every commit under `runtime/`, `wit/` and `plugins/` line by line and ticks the pull request's review box with their handle; CI green on the three systems; merge with `gh pr merge --merge --match-head-commit <sha> --author-email 8193616+philipesantos@users.noreply.github.com`
- [ ] T053 After the merge, mark in the development guide the Phase 2 items this feature delivers (manifest schema, permission engine, re-approval on updates) and leave "Contrato WIT v1 com essas interfaces" open until features 003–006 add theirs

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
- **US2 (P1)**: after Foundational; uses `hello-widget` on v1 as the well-behaved plugin (T026)
- **US3 (P2)**: after Foundational; its CLI test for `remove` builds on `approve` (T028)
- **US4 (P2)**: after Foundational; its CLI tests build on `approve` (T028)
- **US5 (P2)**: after Foundational; `pack` is already in the runtime (T015)

### Within Each Phase

- Setup: T001 before everything that uses `wit/v0`; T005 before T006 (the xtask fails without
  manifests); T007 → T008 → T009 (each edits `plugins/Cargo.toml`)
- Foundational: T010 and T011 first; then T012 → T013; T014 and T016 after T013; T014 → T015,
  T016 → T017; T018 → T019 after T017; T020 → T021 → T022
- `runtime/src/bin/runtime.rs` tasks run in order (T021 → T028 → T034 → T038 → T042 → T045)
- `runtime/tests/cli.rs` and `runtime/tests/permissions.rs` tasks of different stories edit the
  same file: run them in story order
- Tests before their implementation; they must fail first

### Parallel Opportunities

- Setup: T002, T003 and T005
- Foundational: T010 with T011; T014 with T016
- User Story 1: T023, T024 and T025
- User Story 3: T032 and T033
- User Story 4: T035 and T036
- User Story 5: T039, T040 and T041; T043 and T044
- Polish: T048 and T049

---

## Parallel Example: User Story 5

```bash
# Tests in three different files:
Task: "runtime/tests/manifest_schema.rs: the schema and the parser agree (boon)"
Task: "runtime/tests/contract_v1.rs: every capability function can answer not-granted"
Task: "runtime/tests/cli.rs: pack writes, refuses an existing manifest, names the field"

# Then the documentation, alongside the pack command:
Task: "docs/plugin-authors.md: the first author guide"
Task: "wit/README.md and plugins/examples/README.md"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1: Setup
2. Phase 2: Foundational
3. Phase 3: User Story 1
4. **STOP and VALIDATE**: quickstart step 2. The engine already refuses every rule, but the
   gate is User Story 2: nothing may build on this until its cases pass on three systems.

### Incremental Delivery

1. Setup + Foundational → the engine, with feature 001's suite green
2. User Story 1 → approve and run through the CLI
3. User Story 2 → hostile permission cases green on three systems: **the security gate**
4. User Story 3 → updates and removal
5. User Story 4 → optional permissions and next-call changes
6. User Story 5 → schema check, contract rule, `pack`, author documentation
7. Polish → performance record, threat model, docs, local checks, quickstart, human review

---

## Notes

- [P] tasks touch different files and depend on no incomplete task
- All work happens on the `002-plugin-permissions` branch, in pull request #2 (a draft). Never
  commit this feature directly to `master`
- One commit per task or small group of tasks, without any co-author or tool attribution. The
  maintainer reviews each commit line by line (Principle IX); a fix after review is a new
  commit, never a rewrite of a reviewed one
- Push only when the maintainer asks
- Everything under `runtime/` and `wit/` is security core, and `plugins/hostile/`,
  `plugins/fixtures/` and `runtime/tests/hostile.rs` hold the security tests: line-by-line
  human review (T052)
- Never commit built `.wasm` files; CI builds every plugin from source
