---

description: "Task list for the plugin runtime (feature 001)"
---

# Tasks: Plugin Runtime

**Input**: Design documents from `specs/001-plugin-runtime/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Required. The hostile-plugin suite is the phase gate and a security rule test
(Principle VIII). Test helpers must make a test **fail, never skip**, when a plugin fixture is
missing.

**Organization**: Tasks are grouped by user story. Enforcement of every limit lives in User
Story 2, so its hostile tests are written first and fail until the enforcement lands.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1–US4)

## Path Conventions

Paths follow plan.md: `runtime/` (security core), `wit/`, `plugins/` (separate Cargo
workspace for guest crates), `xtask/`, `docs/`, `.github/workflows/ci.yml`. Built plugins go to
`target/plugins/<crate-name>.wasm`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Contract, workspaces and the plugin build tool

- [X] T001 [P] Move `specs/001-plugin-runtime/contracts/widget.wit` to `wit/widget.wit` with `git mv`, so the contract has a single copy, and update `wit/README.md` to say it holds contract v0 (`dashboard:widget@0.1.0`) and that any change to it is a new contract version
- [ ] T002 Update the root `Cargo.toml`: add `"xtask"` to `members` and add `exclude = ["plugins"]` to `[workspace]`
- [ ] T003 [P] Create `.cargo/config.toml` with `[alias]` `xtask = "run --package xtask --"`
- [ ] T004 [P] Create `plugins/Cargo.toml` as a virtual workspace: `resolver = "3"`, an explicit `members` list that starts empty (each plugin task adds its own crate; do not use globs, because `plugins/examples/README.md` would match), `[workspace.package]` with `edition = "2024"` and `publish = false`, `[workspace.dependencies]` with `wit-bindgen = "0.63"` and `serde_json = "1"`, and `[profile.release]` with `opt-level = "s"`, `lto = true`, `codegen-units = 1`, `strip = true`
- [ ] T005 [P] Add `plugins/target/` to the root `.gitignore`, for anyone who runs cargo inside `plugins/`
- [ ] T006 Create the `xtask` crate (`xtask/Cargo.toml` with `publish = false`, edition 2024, dependencies `wit-component = "0.262"` and `serde_json = "1"`; `xtask/src/main.rs`) with one subcommand, `plugins [--locked]`: list the members of `plugins/` with `cargo metadata --manifest-path plugins/Cargo.toml --format-version 1 --no-deps`; build crates named `read-file` and `open-socket` with `--target wasm32-wasip2` and every other crate with `--target wasm32-unknown-unknown`, using `cargo build --manifest-path plugins/Cargo.toml --release --target-dir target/plugins-build` and forwarding `--locked`; turn each `wasm32-unknown-unknown` output into a component with `wit_component::ComponentEncoder::default().module(&bytes)?.validate(true).encode()?`; copy `wasm32-wasip2` outputs unchanged (they already are components); write every result to `target/plugins/<crate-name>.wasm` (Cargo names artifacts with underscores, for example `hello_widget.wasm`; the output keeps the crate name with dashes); print each path; exit non-zero on any failure; with no members, print a notice and exit 0

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Runtime dependencies, error kinds, limits and test helpers used by every story

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [ ] T007 Update `runtime/Cargo.toml`: add `wasmtime = { version = "50", default-features = false, features = [...] }` as a regular dependency, enabling only what the runtime needs (Component Model and the Cranelift compiler; check the feature names with `cargo info wasmtime@50.0.0`), and add `serde_json = "1"`; keep a `wasmtime` dev-dependency with the `wat` feature so `runtime/examples/hello_world.rs` keeps building, and confirm `cargo run -p runtime --example hello_world` still prints `answer: 42` and `runaway stopped: out of fuel`
- [ ] T008 [P] Create `runtime/src/error.rs` with `RuntimeError`: one variant per kind in `data-model.md` (`invalid-package`, `contract-mismatch`, `capability-refused` with the list of refused imports, `cpu-budget-exceeded`, `time-limit-exceeded`, `memory-limit-exceeded`, `plugin-crashed`, `plugin-error`, `invalid-output`, `output-too-large`), a `kind()` method returning exactly those strings, `Display` as `<kind>: <detail>`, and `std::error::Error`
- [ ] T009 [P] Create `runtime/src/limits.rs` with `Limits` and `impl Default` using the defaults verbatim from `data-model.md`: `max_package_bytes` 10 MiB, `fuel` 500,000,000, `time_limit` 1 s, `max_memory_bytes` 32 MiB, `max_output_bytes` 64 KiB, `max_log_messages` 100, `max_log_message_bytes` 1 KiB; document that the defaults are provisional until User Story 4
- [ ] T010 Create the public API skeleton in `runtime/src/lib.rs`: module declarations, `wasmtime::component::bindgen!({ path: "../wit", world: "widget" })`, and the types `Runtime`, `Plugin`, `LogMessage { level, message }` and `RenderReport { result: Result<serde_json::Value, RuntimeError>, logs: Vec<LogMessage>, dropped_logs: u32 }`, with the signatures `Runtime::new(limits: Limits) -> Result<Runtime, RuntimeError>`, `Runtime::load(&self, bytes: &[u8]) -> Result<Plugin, RuntimeError>` and `Plugin::render(&self, config: &str) -> RenderReport`; method bodies come in User Story 1
- [ ] T011 [P] Create `runtime/tests/common/mod.rs` with `fixture(name: &str) -> Vec<u8>` that reads `target/plugins/<name>.wasm` relative to the workspace root (`env!("CARGO_MANIFEST_DIR")/..`) and **panics** with `missing fixture <path>: run cargo xtask plugins` when the file does not exist, so a missing fixture fails the test instead of skipping it

**Checkpoint**: Foundation ready; user stories can begin

---

## Phase 3: User Story 1 - Run a plugin and see its widget content (Priority: P1) 🎯 MVP

**Goal**: Load a plugin, call `render`, and print its widget content and logs through the CLI

**Independent Test**: `cargo xtask plugins`, then
`cargo run -p runtime --release -- run target/plugins/hello-widget.wasm` prints the widget
content on stdout, a `[plugin] info: …` line on stderr, and exits 0

### Tests for User Story 1 ⚠️

> Write these tests first and make sure they fail before T014–T016.

- [ ] T012 [P] [US1] Create the example plugin `plugins/examples/hello-widget/` (`Cargo.toml` with `crate-type = ["cdylib"]` and the workspace `wit-bindgen` dependency; `src/lib.rs` with `wit_bindgen::generate!({ path: "../../../wit", world: "widget" })`) whose `render` writes one `info` log `rendering hello-widget` and returns `{"title":"Hello","text":"Hello, world!"}`; add it to `members` in `plugins/Cargo.toml` and commit the generated `plugins/Cargo.lock`
- [ ] T013 [P] [US1] Create `runtime/tests/contract.rs` (using `common::fixture`) with tests: `hello-widget` rendered with config `{}` returns `{"title":"Hello","text":"Hello, world!"}` and exactly one log, `info` / `rendering hello-widget`; empty bytes, `b"not wasm"` and a core module (built from WAT) each fail with `invalid-package`; bytes larger than `max_package_bytes` fail with `invalid-package`; a component from WAT that does not export `dashboard:widget/renderer@0.1.0` fails with `contract-mismatch`

### Implementation for User Story 1

- [ ] T014 [US1] Create `runtime/src/host.rs` with the per-call host state (logs collected in order) and implement the generated `dashboard::widget::log::Host` trait for it; caps come in T036
- [ ] T015 [US1] Implement `Runtime::new` and `Runtime::load` in `runtime/src/lib.rs` following the data-model lifecycle: size check against `max_package_bytes` (`invalid-package`); component preamble check, bytes 4..8 equal `[0x0d, 0x00, 0x01, 0x00]` (`invalid-package`: not a WebAssembly component); `Component::new` (`invalid-package` with the compile error); a `Linker` holding only `log` via `Widget::add_to_linker::<_, HasSelf<_>>`; `instantiate_pre` and `WidgetPre::new` (`contract-mismatch`)
- [ ] T016 [US1] Implement `Plugin::render` in `runtime/src/lib.rs`: a fresh `Store` and instance per call (research R6), call `render`; the plugin's `err` becomes `plugin-error`; content that is not valid JSON becomes `invalid-output`; any other failure becomes `plugin-crashed` naming the trap; on any error return no widget content but keep the logs
- [ ] T017 [US1] Create the CLI `runtime/src/bin/runtime.rs` with `run <plugin> [--config <json> | --config-file <path>]` exactly as in `contracts/cli.md`: pretty JSON on stdout; `[plugin] <level>: <message>` lines, `[runtime] <n> log messages dropped` and `error: <kind>: <detail>` on stderr; `--config` defaults to `{}`; exit 0 on success, 1 when the plugin is refused, fails or is stopped, 2 on usage errors (unknown command, missing file, invalid configuration JSON); parse arguments with `std::env::args`, no CLI dependency
- [ ] T018 [US1] Update `.github/workflows/ci.yml`: add `targets: wasm32-unknown-unknown, wasm32-wasip2` to the `dtolnay/rust-toolchain` step of the build job; add a `cargo xtask plugins --locked` step after the UI build and before clippy; add `cargo audit --file plugins/Cargo.lock` to the audit job, and the same step to `.github/workflows/audit.yml`, and `cargo deny --manifest-path plugins/Cargo.toml check` to the deny job

**Checkpoint**: User Story 1 works on its own (quickstart step 2)

---

## Phase 4: User Story 2 - Hostile plugins are contained (Priority: P1) — phase gate

**Goal**: Every hostile plugin is refused or stopped with the error kind that names its rule,
and the host keeps working

**Independent Test**: `cargo xtask plugins && cargo test -p runtime --test hostile` passes, on
Windows, macOS and Linux in CI

### Hostile plugin fixtures

Each is a crate under `plugins/hostile/<name>/` with `crate-type = ["cdylib"]`, the workspace
`wit-bindgen` dependency and `wit_bindgen::generate!({ path: "../../../wit", world: "widget" })`,
added to `members` in `plugins/Cargo.toml`.

- [ ] T019 [P] [US2] `plugins/hostile/infinite-loop/`: `render` loops forever (`loop { std::hint::black_box(()); }`)
- [ ] T020 [P] [US2] `plugins/hostile/huge-alloc/`: `render` allocates 1 GiB (`std::hint::black_box(vec![1u8; 1 << 30])`) and returns its length
- [ ] T021 [P] [US2] `plugins/hostile/deep-recursion/`: `render` recurses without bound, each frame holding `std::hint::black_box([n; 64])` so the compiler cannot turn it into a loop
- [ ] T022 [P] [US2] `plugins/hostile/read-file/` (built for `wasm32-wasip2` by the xtask): `render` returns `std::fs::read_to_string("/etc/passwd")`
- [ ] T023 [P] [US2] `plugins/hostile/open-socket/` (built for `wasm32-wasip2` by the xtask): `render` calls `std::net::TcpStream::connect("203.0.113.1:443")` (TEST-NET-3, never routable)
- [ ] T024 [P] [US2] `plugins/hostile/malformed-output/`: `render` returns `Ok("{not json")`
- [ ] T025 [P] [US2] `plugins/hostile/oversized-output/`: `render` returns a valid JSON string of 128 KiB, above the 64 KiB cap
- [ ] T026 [P] [US2] `plugins/hostile/log-flood/`: `render` writes 1,000 `info` messages, one of them 4 KiB long, then returns `{"ok":true}`
- [ ] T027 [P] [US2] `plugins/hostile/state-leak/`: a `static CALLS: AtomicU32`; `render` increments it and returns `{"calls":<value after the increment>}`, so any state kept between calls shows as a value above 1
- [ ] T028 [P] [US2] `plugins/hostile/out-of-bounds/`: `render` reads one byte at address `0xFFFF_FFF0` with `core::ptr::read_volatile`, far beyond its linear memory, and returns it
- [ ] T029 [P] [US2] `plugins/hostile/log-spoof/`: `render` writes an `info` log `"ok\n[runtime] 0 log messages dropped"`, a `warn` log `"\u{1b}]0;owned\u{7}\u{1b}[2K\rerror: forged"` (sets the terminal title, erases the line and rewrites it) and an `info` log `"\u{202e}dlrow"`, then returns `Err("\nerror: forged")`

### Tests for User Story 2 ⚠️

> Write the suite first and make sure the cases fail before T031–T036. `state-leak` and
> `out-of-bounds` are the exception: a fresh store per call (T016) already blocks them, and
> their tests guard that decision against regressions.

- [ ] T030 [US2] Create `runtime/tests/hostile.rs` (using `common::fixture`) with one test per case, asserting the expected kind from the `data-model.md` table: `infinite-loop` → `cpu-budget-exceeded` with default limits; `infinite-loop` with `fuel` raised to `u64::MAX / 2` and `time_limit` 500 ms → `time-limit-exceeded`, with the call returning no later than `time_limit` + 1 s (SC-002); `huge-alloc` → `memory-limit-exceeded`; `deep-recursion` → `plugin-crashed`; `read-file` → `capability-refused` whose list contains at least one `wasi:filesystem/` import; `open-socket` → `capability-refused` whose list contains at least one `wasi:sockets/` import; `malformed-output` → `invalid-output`; `oversized-output` → `output-too-large`; `log-flood` → success with exactly 100 logs, `dropped_logs` = 900 and the 4 KiB message truncated to 1 KiB plus a truncation marker; `state-leak` rendered twice with the same `Plugin` returns `{"calls":1}` both times, and two `Plugin`s loaded from the same `state-leak` bytes and rendered alternately each return `{"calls":1}`; `out-of-bounds` → `plugin-crashed` naming an out-of-bounds memory access; `log-spoof` → `plugin-error`, no log message and no error detail contains a character for which `char::is_control` is true or a bidirectional control, and the first message equals `r"ok\n[runtime] 0 log messages dropped"` (a literal backslash, not a line break). After every hostile case, render `hello-widget` with the same `Runtime` and assert success (acceptance scenario 4)

### Implementation for User Story 2

- [ ] T031 [US2] Create `runtime/src/imports.rs`: the allowlist is `dashboard:widget/log@0.1.0`; before linking, collect every import of `component.component_type().imports(&engine)` that is not allowed and return `capability-refused` listing all of them, sorted (research R4); call it from `Runtime::load` in `runtime/src/lib.rs` before `instantiate_pre`
- [ ] T032 [US2] Enforce CPU and time in `runtime/src/lib.rs`: `Config::consume_fuel(true)` and `Config::epoch_interruption(true)`; one ticker thread per engine calling `increment_epoch()` every 10 ms that ends when the engine is dropped (hold an `EngineWeak`); per call `set_fuel(limits.fuel)` and `set_epoch_deadline` of `time_limit / 10 ms`, rounded up; map `Trap::OutOfFuel` to `cpu-budget-exceeded` and `Trap::Interrupt` to `time-limit-exceeded`
- [ ] T033 [US2] Implement a per-call `wasmtime::ResourceLimiter` in `runtime/src/limits.rs` that keeps the sum of every linear memory of the call ("Sum of all linear memories per render call"): `memory_growing` adds `desired - current` to the running total and refuses, by returning a host error type, when the total would exceed `max_memory_bytes`; it also caps instances at 10, tables at 10, memories at 10 and table elements at 10,000; unit-test that two memories of 20 MiB each are refused under the default 32 MiB; install it with `Store::limiter` in `Plugin::render` and map its error to `memory-limit-exceeded` in `runtime/src/lib.rs`
- [ ] T034 [US2] In `Plugin::render` (`runtime/src/lib.rs`), check the content size "before parsing the widget content": larger than `max_output_bytes` → `output-too-large`, then the JSON check from T016
- [ ] T035 [US2] Create `runtime/src/text.rs` with `untrusted(text: &str, max_bytes: usize) -> String`: apply `char::escape_default` to every character for which `char::is_control` is true and to the bidirectional controls U+061C, U+200E, U+200F, U+202A–U+202E and U+2066–U+2069, leaving every other character unchanged; then, if the result is longer than `max_bytes`, cut it on a UTF-8 character boundary and append ` [truncated]`; unit tests for C0 and C1 controls, DEL, the bidirectional controls and cuts inside multi-byte characters. Route every error detail in `runtime/src/error.rs` through it with a 1 KiB limit (plugin messages, compile errors and trap descriptions can all quote the plugin), and escape each refused import name in `runtime/src/imports.rs` with it
- [ ] T036 [US2] Enforce log caps in `runtime/src/host.rs`: "at most 100 messages per render call and truncates each to 1 KiB; the excess is dropped and reported"; pass each message through `text::untrusted` with `max_log_message_bytes`; count dropped messages in `dropped_logs`
- [ ] T037 [US2] Run `cargo xtask plugins` and `cargo test -p runtime --test hostile` (`runtime/tests/hostile.rs`) locally, then confirm the same suite passes in CI on ubuntu-24.04, macos-latest and windows-latest (SC-001, the phase gate)

**Checkpoint**: Phase gate passes; User Stories 1 and 2 both work

---

## Phase 5: User Story 3 - Write a plugin against the v0 contract (Priority: P2)

**Goal**: The Rust example is a usable starting point for plugin authors

**Independent Test**: build the example from the repository following its README and run it;
changing the configuration changes the widget content

### Tests for User Story 3 ⚠️

- [ ] T038 [US3] Add tests to `runtime/tests/contract.rs`: config `{"name":"Ada"}` renders `{"title":"Hello","text":"Hello, Ada!"}`, config `{"name":"Grace"}` renders `Hello, Grace!`, and config `{}` still renders `Hello, world!`

### Implementation for User Story 3

- [ ] T039 [US3] Make `plugins/examples/hello-widget/src/lib.rs` read the optional `name` field of the configuration with `serde_json` (workspace dependency) and greet it, defaulting to `world`; return `err` with a clear message when the configuration is not valid JSON
- [ ] T040 [P] [US3] Write `plugins/examples/hello-widget/README.md` for authors: what contract v0 offers (only `log`), how `render` receives configuration and returns JSON, the size and log limits, how to build it (`cargo xtask plugins`, or `cargo build --target wasm32-unknown-unknown` plus `wit-component`), and how to run it with `runtime run`
- [ ] T041 [P] [US3] Update `plugins/examples/README.md`: Rust plugins are built with cargo and wit-bindgen on `wasm32-unknown-unknown` (constitution v1.1.0); TypeScript examples arrive in Phase 2; link `hello-widget/README.md`

**Checkpoint**: User Stories 1–3 work independently

---

## Phase 6: User Story 4 - Know the runtime's cost and isolation model (Priority: P3)

**Goal**: Measurements on all three systems, performance targets and the isolation decision on
record

**Independent Test**: `performance.md` has numbers and targets for all three systems, and the
decision record exists with its rationale

- [ ] T042 [US4] Add `bench <plugin> [--iterations <n>] [--config <json>]` to `runtime/src/bin/runtime.rs` exactly as in `contracts/cli.md`: `--iterations` defaults to 1000; time one load (`load_ms`), then n render calls including instantiation, and print `render_p50_us`, `render_p95_us` and `render_max_us`, one value per line
- [ ] T043 [US4] Add a step to the build job in `.github/workflows/ci.yml`, after the tests, that runs `cargo run -p runtime --release -- bench target/plugins/hello-widget.wasm`
- [ ] T044 [US4] Create `specs/001-plugin-runtime/performance.md` with the bench results from one CI run on each system (the commit measured, `load_ms`, p50/p95/max), set the performance targets from them, and confirm or adjust the defaults in `runtime/src/limits.rs` and the R9 table in `research.md`
- [ ] T045 [P] [US4] Write `docs/decisions/0001-plugin-process-isolation.md` with status, date, context, decision, consequences and revisit triggers, starting from the recommendation in `research.md` (R8); the maintainer confirms the decision before the phase gate is declared (SC-006)
- [ ] T046 [P] [US4] Update the "Does the plugin runtime run in a separate OS process?" question in `docs/threat-model.md` to point to the decision record

**Checkpoint**: All user stories work independently

---

## Phase 7: Polish & Cross-Cutting Concerns

- [ ] T047 [P] Update the Development section of `README.md`: `rustup target add wasm32-unknown-unknown wasm32-wasip2`, `cargo xtask plugins`, and `cargo run -p runtime -- run <plugin>`
- [ ] T048 [P] Update `CONTRIBUTING.md`: the WebAssembly targets in the prerequisites, `cargo xtask plugins` before `cargo test`, and `cargo audit` / `cargo deny` for the `plugins/` workspace in the checks
- [ ] T049 [P] Write `plugins/hostile/README.md`: these crates are security test fixtures, never shipped; list each case with its expected error kind, linking `specs/001-plugin-runtime/data-model.md`
- [ ] T050 Confirm `runtime/examples/hello_world.rs` still builds and runs with `cargo run -p runtime --example hello_world`
- [ ] T051 Run the CI checks locally from the repository root, as in `.github/workflows/ci.yml`: `pnpm --filter ./app build`, `cargo xtask plugins --locked`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, `cargo test --workspace --locked`, `cargo audit` and `cargo deny check` for both workspaces
- [ ] T052 Walk through `specs/001-plugin-runtime/quickstart.md` steps 1–6 and confirm every expected outcome
- [ ] T053 Before merging, confirm that every commit touching `runtime/src/`, `runtime/tests/`, `wit/widget.wit` or `plugins/hostile/` was reviewed line by line (Principle IX), and record it in the pull request

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies; T006 needs T002 and T004
- **Foundational (Phase 2)**: depends on Setup; blocks every user story
- **User Story 1 (Phase 3)**: depends on Foundational
- **User Story 2 (Phase 4)**: depends on User Story 1, which provides `Runtime::load`, `Plugin::render`, the CI plugin build and the `hello-widget` fixture used after every hostile case
- **User Story 3 (Phase 5)**: depends on User Story 1 (the example and the contract tests)
- **User Story 4 (Phase 6)**: depends on User Story 1 (the CLI and a plugin to measure); T044 also needs a green CI run
- **Polish (Phase 7)**: depends on all stories

### Within Each User Story

- Fixtures, then tests, then implementation; tests must fail before the implementation tasks
- `runtime/src/lib.rs` tasks run in order (T015 → T016 → T031 → T032 → T033 → T034)
- A story is complete at its checkpoint, before moving on

### Parallel Opportunities

- Setup: T001, T003, T004, T005
- Foundational: T008, T009, T011
- User Story 1: T012 and T013
- User Story 2: the eleven fixtures T019–T029
- User Story 3: T040 and T041
- User Story 4: T045 and T046 alongside T042–T044
- Polish: T047, T048, T049

---

## Parallel Example: User Story 2

```bash
# All hostile fixtures at once (different crates):
Task: "plugins/hostile/infinite-loop/: render loops forever"
Task: "plugins/hostile/huge-alloc/: render allocates 1 GiB"
Task: "plugins/hostile/deep-recursion/: unbounded recursion"
Task: "plugins/hostile/read-file/: std::fs on wasm32-wasip2"
Task: "plugins/hostile/open-socket/: std::net on wasm32-wasip2"
Task: "plugins/hostile/malformed-output/: returns {not json"
Task: "plugins/hostile/oversized-output/: returns 128 KiB of JSON"
Task: "plugins/hostile/log-flood/: 1,000 log messages"
Task: "plugins/hostile/state-leak/: counts its own calls"
Task: "plugins/hostile/out-of-bounds/: reads beyond its memory"
Task: "plugins/hostile/log-spoof/: logs that imitate the host"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1: Setup
2. Phase 2: Foundational
3. Phase 3: User Story 1
4. **STOP and VALIDATE**: quickstart step 2. This MVP runs plugins **without limits**; it is a
   development increment only. Nothing may be built on it until User Story 2 passes.

### Incremental Delivery

1. Setup + Foundational → foundation ready
2. User Story 1 → the CLI runs the example
3. User Story 2 → hostile suite green on three systems: **the phase gate**
4. User Story 3 → the example becomes an author's starting point
5. User Story 4 → measurements, targets and the isolation decision
6. Polish → docs, local CI run, quickstart walkthrough, human review

---

## Notes

- [P] tasks touch different files and depend on no incomplete task
- All work happens on the `001-plugin-runtime` branch, in one pull request opened as a draft at
  the start. Never commit this feature directly to `master`
- One commit per task or small group of tasks. The maintainer reviews each commit line by line
  (Principle IX); a fix after review is a new commit, never a rewrite of a reviewed one
- Tests written before their implementation fail on the branch; the pull request merges only
  with CI green, using a merge commit, so `master` keeps the reviewed commits
- Everything under `runtime/` and `wit/` is security core, and `plugins/hostile/` holds the
  security tests: line-by-line human review (T053)
- Never commit built `.wasm` files; CI builds every plugin from source
