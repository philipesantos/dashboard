---

description: "Task list for contract v1 closed and docs for plugin authors (feature 006)"
---

# Tasks: Contract v1 Closed and Docs for Plugin Authors

**Input**: Design documents from `specs/006-contract-v1-docs/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Required. The feature adds no security rule and no hostile case (FR-024): the hostile
suite must pass unchanged. It adds tests of its own:
- the 1.3.0 pin and the check that `wit/v1` equals its frozen copy (R13);
- `pack` on a core module: the same bytes as `xtask`, a refusal for a module without the world,
  and the encoder's cause escaped (R2);
- the real `runtime` ignoring the test network's variable (R9);
- self-tests of the drift test and the docs runner, which show that each fails where it must;
- the two checks themselves (R7, R8).

Every test task comes before the implementation it checks and fails until that implementation
lands. When a fixture or a built plugin is missing, the test helpers make a test **fail, never
skip**. No test reaches the internet or a real service, and no test reads `specs/`.

**Organization**: the review of the contract comes first, because everything the docs say depends
on its outcome (R12). The same goes for the CLI's move into `runtime::cli`, `ErrorKind::ALL`, and
the docs runner's core, which the walkthrough, the guides and the reference all run on; so those
are foundational. The stories then freeze the contract (US2), write and check the walkthrough
(US1), write the reference (US3), add the drift test and CI (US4), add the guides with the
test-only CLI (US5), and clear the leftovers (US6).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1–US6)

## Path Conventions

Paths follow plan.md:
- **Security core:** `runtime/` and `wit/`.
- **Guest crates:** `plugins/`.
- **Docs for authors:** `docs/authors/`.
- **Walkthrough crate:** `docs/authors/getting-started/`. It is kept for `audit` and `deny`, and
  excluded from the root workspace.
- **CI:** `.github/`.

Built plugins go to `target/plugins/<crate-name>.wasm`. Where the details live:
- decisions and numbers: `research.md` (R1–R17);
- marker and runnable-block formats: `contracts/docs-format.md`;
- `pack` and the test-only CLI: `contracts/cli.md`;
- review findings and walk records: `data-model.md`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: the new dependency, the workspace exclusion and the feature's records. The runtime
behaves as in feature 005 at the end of this phase.

- [X] T001 Update `runtime/Cargo.toml` (research R2):
  - Add `wit-component = "=0.262.0"`, with a one-line comment as the file has for its other
    dependencies: "Turns the core module cargo builds into a component in `runtime pack`; the
    release xtask uses and wit-bindgen 0.63.0 encodes with".
  - In the root `Cargo.toml`, add `"docs/authors/getting-started"` to the workspace's `exclude`,
    beside `"plugins"`, with a comment.
  - Confirm `cargo tree -d -p runtime -e normal` shows only the expected second copy of
    `wasmparser`/`wasm-encoder` (0.262 beside Wasmtime's 0.261) and no other new duplicate.
  - Confirm `cargo deny check` passes with no new license.
- [X] T002 [P] Create `specs/006-contract-v1-docs/review.md` and
  `specs/006-contract-v1-docs/walks.md` with the fields of `data-model.md`, sections only:
  - `review.md` has one table with the columns `id`, `item`, `what`, `checked`, `outcome` and
    `cost`. Outcomes are `docs`, `host`, `breaking` or `kept`, and a finding moves only from
    `open` to one of them.
  - `walks.md` has a section per walk: `who`, `system`, `date`, `docs commit`, `stuck points`
    (each with its fix) and `time` (the maintainer's walk only). It also has a section for the
    docs job's time per system.

**Checkpoint**: the dependency is vetted and the records exist. Every existing test still passes.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the reviewed contract, the CLI as a library module, the complete list of error
kinds, and the docs runner every page runs on.

**⚠️ CRITICAL**: No user story starts before T005. A breaking finding the maintainer chooses
(FR-006) adds tasks to this file before anything else proceeds.

### The review (R12)

- [ ] T003 Run the independent review of contract 1.3.0 (FR-004, research R12):
  - **Reviewer:** a fresh agent that did not write the contract and has none of this
    conversation's context.
  - **Inputs:** `wit/v1/widget.wit`, `wit/manifest.schema.json`, `wit/connector.schema.json`,
    `runtime/src/`, `runtime/tests/`, `docs/plugin-authors.md`, `docs/connectors.md` and
    `wit/README.md`.
  - **What it checks:** for every interface, function, type and error case, that a plugin can
    reach it, that the host behaves as its doc comment says, and that it is documented; for
    every world and name, that it promises nothing the host does not do.
  - **What it returns:** each finding with the file and line it rests on. It edits nothing.
- [ ] T004 Check each finding of T003 at the source, by code, a test or a small run, then record
  it in `specs/006-contract-v1-docs/review.md` with its outcome. By outcome:
  - **`docs`:** the fix goes to the docs (US3, US5) or to a doc comment of `wit/v1/widget.wit`
    (T005).
  - **`host`:** append a task here, after T005. It has a test that fails without the fix, comes
    before the fix, and gets line-by-line review. If the fix refuses something the host accepts
    today, it must close a security hole (FR-005); otherwise it is `breaking`.
  - **`breaking`:** **STOP** and give the maintainer the cost: what changes in the contract, the
    host, the examples, the fixtures and the docs. If they choose 2.0.0, append the tasks of
    FR-006 here (contract 2.0.0, v1 refused naming the versions the host runs, examples and
    fixtures moved, hostile cases rebuilt and none dropped, v0 kept) and re-check the plan's
    Constitution Check.
  - **`kept`:** record the reason.

  No finding may stay `open`.
- [ ] T005 Apply the doc-comment outcomes of T004 to `wit/v1/widget.wit`, changing comments only,
  never a definition (FR-001). Confirm `cargo test -p runtime --test contract_v1 --locked` still
  passes: the only-adds tests compare definitions, not comments.

### The CLI as a library module (R9)

- [ ] T006 Move the CLI unchanged into a library module, as its own commit:
  - Move the whole body of `runtime/src/bin/runtime.rs` into `runtime/src/cli/mod.rs`, declared
    as `#[doc(hidden)] pub mod cli;` in `runtime/src/lib.rs`, with one entry point
    `pub fn main() -> std::process::ExitCode` that holds what `main` holds today.
  - `runtime/src/bin/runtime.rs` becomes a `main` that calls `runtime::cli::main()`.
  - Allowed edits: visibility, `use` paths (`runtime::` becomes `crate::`) and the entry point's
    name.
  - `git diff --color-moved=dimmed-zebra HEAD~1` must show every other line as moved.
  - Confirm `cargo test -p runtime --test cli --locked` passes with `runtime/tests/cli.rs`
    unchanged, and that `cargo clippy --workspace --all-targets --locked -- -D warnings` passes.
- [ ] T007 Make the runtime, the opener and the default data locations parameters, as its own
  commit after T006 and reviewed line by line:
  - Add a `pub struct Host` in `runtime/src/cli/mod.rs` with four fields:
    - `start: fn(Limits, RuntimeOptions) -> wasmtime::Result<Runtime>`;
    - `open: fn(&str) -> std::io::Result<()>`;
    - `data_dir: fn() -> Option<PathBuf>`;
    - `grants_path: fn() -> Option<PathBuf>`.
  - Add `pub fn main_with(host: Host) -> ExitCode`, which every command uses: the five places
    that build a runtime (today `runtime.rs:528`, `640`, `1428`, `1459` and `1653`), the browser
    call (`runtime.rs:1749`), `fn data_dir` (`runtime.rs:1987`) and `fn grant_store`
    (`runtime.rs:2014`).
  - `cli::main()` calls `main_with` with `Runtime::new`, `open::that`,
    `RuntimeOptions::default_development_data_dir` and `GrantStore::default_development_path`.
  - Make `USAGE` `pub`, so the drift test can read the commands and options (T029).
  - Confirm `runtime/tests/cli.rs` passes unchanged.

### Error kinds (R7)

- [ ] T008 [P] Define `ErrorKind` through one macro in `runtime/src/error.rs` (research R7):
  - Each entry gives a variant's doc comment, its name and its `as_str` text. The macro declares
    the enum, `as_str` and `pub const ALL: [ErrorKind; 36]` together, so no kind can exist
    outside `ALL`.
  - Keep every variant's doc comment and every name as `as_str` returns it today.
  - Add a unit test that the 36 names in `ALL` are distinct.
  - Confirm `cargo test --workspace --locked` passes.

### The docs runner's core (R8)

- [ ] T009 Write the self-tests of the runner in `runtime/tests/docs_walk.rs`. They are not
  `#[ignore]`d and they fail until T010. Each runs on in-memory Markdown:
  - **Info string:** `check`, `os=linux,macos,windows`, `exit=<n>`, `file=<path>` and
    `output=any` are parsed. A block without `check` is not run.
  - **Prompts:** `$ ` runs on Linux and macOS and `PS> ` on Windows. A block without `os=` that
    mixes both kinds is refused.
  - **Expected output:** each command's expected output is every line up to the next prompt or
    the end of the block. Trailing spaces and CRLF are normalized.
  - **Placeholders:** each matches only its kind:
    - `<digest>`: 64 lowercase hex characters, not 63 and not uppercase;
    - `<n>`: an integer;
    - `<path>`: a path under the session's folder or the data directory, not one elsewhere;
    - `<time>`: a date-time or a duration as the CLI prints them;
    - `<version>`: a semver.

    Any other `<…>` is literal.
  - **`output=any`:** refused on a block that has a `runtime` command.
  - **Front matter:** `<!-- check: service=github,notes -->` is read, and `service=` on a block
    is refused.
  - **Failures:** a differing line fails, showing the expected and the actual lines.
  - **Starting state:** a page other than the walkthrough runs the session's `runtime`, never
    another one on `PATH`.
  - **Cargo variables:** a `cargo build` command does not see `CARGO_TARGET_DIR` or
    `CARGO_PROFILE_RELEASE_*`, and a `cargo install` command does.
- [ ] T010 Implement the runner in `runtime/tests/docs_walk.rs` (research R8,
  `contracts/docs-format.md`), so T009 passes:
  - **Entry point:** `#[ignore]`d `every_page_runs_as_written`, over every `.md` under
    `docs/authors/`, in path order, one session per page.
  - **Guard:** refuse to start unless `CI` or `DASHBOARD_DOCS_WALK_CLEARS_DEVELOPMENT_DATA=1` is
    set, naming the variable.
  - **Session:**
    - a fresh temporary folder as the working directory;
    - the development data directory and the development grant record, found by
      `runtime::RuntimeOptions::default_development_data_dir()` and
      `runtime::grants::GrantStore::default_development_path()`, cleared before each page;
    - `console` blocks run through `bash -c` on Linux and macOS and `pwsh -NoProfile -Command`
      on Windows, with `2>&1`;
    - file blocks are written to `file=<path>`;
    - blocks marked for other systems are skipped.
  - **Checks:**
    - a command must exit `exit=<n>`, or 0 if no `exit=` is given;
    - outputs are compared whole, except in `output=any` blocks;
    - the page stops at the first block that differs.
  - **Starting state for every page but `getting-started.md`** (`contracts/docs-format.md`):
    - a local clone at `<session>/dashboard`, made as the walkthrough makes it;
    - `CARGO_BIN_EXE_runtime` copied as `runtime` (or `runtime.exe`) into `<session>/bin`, first
      on `PATH`;
    - for pages under `reference/`, a copy of `docs/authors/getting-started/` at
      `<session>/my-widget`, with `wit/history/1.3.0/widget.wit` copied into its `wit/`.
  - **Environment for the whole session (R3):**
    - `GIT_CONFIG_COUNT`, `GIT_CONFIG_KEY_0` and `GIT_CONFIG_VALUE_0` set
      `url.<repository root>.insteadOf=https://github.com/philipesantos/dashboard`;
    - `CARGO_INSTALL_ROOT=<session>/install`, with `<session>/install/bin` first on `PATH`.
  - **Environment for a command that starts with `cargo install` only:** setting these for the
    whole session would move the author's own `cargo build` output away from the `target/` the
    walkthrough packs from.
    - `CARGO_TARGET_DIR`: `DASHBOARD_DOCS_TARGET_DIR` if set, else the workspace's `target/`;
    - `CARGO_PROFILE_RELEASE_LTO=false` and `CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16`.

**Checkpoint**: the reviewed contract has no finding `open`. `runtime::cli` is the CLI and
`ErrorKind::ALL` lists every kind. The runner's self-tests pass, and `cargo test --workspace` is
green.

---

## Phase 3: User Story 2 - What authors build against no longer moves (Priority: P1)

**Goal**: contract 1.3.0 (or 2.0.0, if chosen in T004) is frozen as published and pinned, and
the contributor docs say how v1 changes from now on.

**Independent Test**: `cargo test -p runtime --test contract_v1` passes. Editing one byte of any
`wit/history/*/widget.wit` makes it fail, naming the version, and so does editing one byte of
`wit/v1/widget.wit` (quickstart §1).

### Tests for User Story 2

- [ ] T011 [US2] Write the tests first, in `runtime/tests/contract_v1.rs`. Both fail until T012:
  - Add `("1.3.0", "<digest>")` to `frozen_contracts()`.
  - Add `v1_equals_its_frozen_copy`: read the version `wit/v1/widget.wit` declares (`package
    dashboard:widget@<version>;`), then assert that `wit/history/<version>/widget.wit` exists and
    that its bytes, line endings read as LF as `frozen_contracts` reads them, equal those of
    `wit/v1/widget.wit`. The message names the version.

### Implementation for User Story 2

- [ ] T012 [US2] Freeze the contract (research R13):
  - After T005, copy `wit/v1/widget.wit` unchanged to `wit/history/1.3.0/widget.wit`.
  - Put its SHA-256, LF-normalized, in place of the placeholder from T011.
  - `plugins/examples/private-notes` keeps building from `wit/v1`.
  - Confirm `contract_v1` passes, and that editing one byte of either file fails it, naming the
    version.
- [ ] T013 [P] [US2] Update `wit/README.md` (FR-003, FR-023):
  - 1.0.0 to 1.3.0 are frozen under `wit/history/`.
  - A new plugin binds its copy of `wit/history/1.3.0`.
  - How v1 changes from now on: freeze the current version first, add a new minor that only adds,
    and extend the only-adds tests in `runtime/tests/contract_v1.rs`; anything else is a v2.
  - For every item, point to `docs/authors/reference/contract.md` instead of describing it.

**Checkpoint**: what authors build against is frozen, and the docs can name a fixed version.

---

## Phase 4: User Story 1 - An author goes from an empty folder to a running widget (Priority: P1) 🎯 MVP

**Goal**: a walkthrough from an empty folder outside the repository to a widget built, packed,
approved and running, needing only cargo, the development CLI and the contract file. CI runs it
as written on three systems.

**Independent Test**: run `DASHBOARD_DOCS_WALK_CLEARS_DEVELOPMENT_DATA=1 cargo test -p runtime
--test docs_walk --locked -- --ignored` on `docs/authors/getting-started.md`: every block runs and
matches. Then follow the walkthrough by hand in an empty folder (quickstart §4).

### Tests for User Story 1

- [ ] T014 [US1] Write the `pack` tests first, in `runtime/tests/cli.rs` (`contracts/cli.md`).
  They fail until T015:
  - `pack_turns_a_core_module_into_the_component_xtask_builds`: for each example under
    `plugins/examples/`, run `pack` on the core module cargo built in the plugins' target folder
    (`wasm32-unknown-unknown/release/<crate>.wasm`) with the example's `manifest.json`. Assert
    the output equals `target/plugins/<crate>.wasm` byte for byte and that `pack` prints
    `packed <output>` and `digest: <sha-256>`. The test fails, never skips, when a core module is
    missing. It also checks that `pack` writes the manifest section as `xtask`'s
    `CustomSection::append_to_component` does (`xtask/src/main.rs:148`, `package.rs:104`). If
    only that differs, align `package.rs` with it, in a change reviewed line by line.
  - `pack_refuses_a_core_module_without_the_contracts_world`: a core module from `wat` with no
    `component-type` section gets exactly `error: invalid-package: <file> is a core module
    without the contract's world (build it with wit-bindgen)` and the exit code of the other
    `invalid-package` refusals.
  - `pack_escapes_the_encoders_cause`: a core module whose import name holds `\n`, `\r`, `\u{1b}`
    and `\u{7}` gets one line, `error: invalid-package: …`, with each character escaped as
    `runtime/tests/cli.rs:107` checks for plugin text.
  - `pack_checks_the_component_against_the_size_cap`: a core module under 10 MiB whose component
    is over it is refused as the size cap refuses today.
- [ ] T015 [US1] Implement it in `runtime/src/package.rs` and the `pack` command of
  `runtime/src/cli/mod.rs`, so T014 passes (research R2):
  - Tell a core module from a component by its header, with `wasmparser`.
  - Encode a core module with `wit_component::ComponentEncoder::default().validate(true)
    .module(&bytes)?.encode()`.
  - A missing component type, or any error the encoder returns, is `invalid-package`, with the
    cause escaped as every plugin text is.
  - Then apply today's path: the existing manifest section check (`manifest-mismatch`), the
    manifest check and the size cap.
  - A component input behaves exactly as today.

### Implementation for User Story 1

- [ ] T016 [US1] Create the walkthrough's crate, `docs/authors/getting-started/` (research R1, R4,
  R8):
  - **Files:** `Cargo.toml`, `src/lib.rs` and `manifest.json`.
  - **`Cargo.toml`:** the package `my-widget`, `crate-type = ["cdylib"]`,
    `wit-bindgen = "=0.63.0"` and `serde_json = "=1.0.154"`.
  - **`src/lib.rs`:** written from `plugins/examples/hello-widget/src/lib.rs`, with
    `wit_bindgen::generate!({ path: "wit", world: "widget" })`.
  - **`manifest.json`:** id `local/my-widget`, contract 1.3.0, a `log` permission.
  - **Lock file:** generate `Cargo.lock` with `cargo generate-lockfile`.
  - **No contract copy:** the crate holds no `wit/`, because the walkthrough copies it from the
    clone.
  - **Checks:** confirm `cargo audit --file docs/authors/getting-started/Cargo.lock` and
    `cargo deny --manifest-path docs/authors/getting-started/Cargo.toml check` pass.
- [ ] T017 [US1] Extend the runner in `runtime/tests/docs_walk.rs` (research R8) for a page with a
  folder of the same name beside it (`getting-started.md` and `getting-started/`):
  - each `file=` block of the page must equal the file of that path in the folder;
  - the folder's `Cargo.lock` is copied into the session's folder before the page;
  - after the page, it must be unchanged. Otherwise the run fails, naming the file.

  Add self-tests for the three rules beside T009's.
- [ ] T018 [US1] Write `docs/authors/getting-started.md` (FR-007, FR-008, FR-015,
  `contracts/docs-format.md`). Every command block is marked `check`, and the expected outputs
  come from real runs.
  1. The tools and the versions it was walked with: Rust 1.99.1 and the `wasm32-unknown-unknown`
     target; `libdbus-1-dev` and `pkg-config` on Linux; PowerShell 7 on Windows.
  2. `git clone https://github.com/philipesantos/dashboard` and `cargo install --path
     dashboard/runtime --locked`, both `output=any`. The page says this step builds the runtime
     and takes several minutes.
  3. An empty folder outside the clone; `Cargo.toml`, `src/lib.rs` and `manifest.json` as file
     blocks.
  4. Copy `dashboard/wit/history/1.3.0/widget.wit` into `wit/`, with a bash form and a
     PowerShell form.
  5. `cargo build --release --target wasm32-unknown-unknown` (`output=any`).
  6. `runtime pack target/wasm32-unknown-unknown/release/my_widget.wasm --manifest manifest.json
     --output my-widget.wasm`.
  7. `runtime run` before approval, refused (`exit=`).
  8. `runtime permissions` and `runtime approve`.
  9. `runtime run`, showing the widget's content.

  Then the mistakes (FR-008), each with its refusal as the CLI prints it and its fix:
  - building for `wasm32-wasip2`;
  - packing without a manifest, and with an invalid one;
  - running before approving;
  - a component built against a newer minor than the host runs: `package
    dashboard:widget@1.4.0;` in the copied `wit/widget.wit`, which the host refuses as
    `contract-mismatch`. The fix is to copy the version the installed CLI runs.

  End by pointing to the guides and the reference. Link a spec only as background.
- [ ] T019 [P] [US1] Write `docs/authors/README.md`:
  - where to start (the walkthrough, then the guides, then the reference);
  - the versions the docs were walked with;
  - what contract v0 still runs, and that new plugins use v1 (FR-016);
  - that render returns a JSON document of at most 64 KiB, that the content format the host frame
    draws comes in Phase 3 in a later minor, and that widgets adopt it then (FR-014);
  - that TypeScript plugins arrive in Phase 5, naming no toolchain or engine (FR-022);
  - that Go and Python get no docs yet.
- [ ] T020 [US1] Run `cargo xtask plugins --locked` and then
  `DASHBOARD_DOCS_WALK_CLEARS_DEVELOPMENT_DATA=1 cargo test -p runtime --test docs_walk --locked
  -- --ignored` on Linux until `getting-started.md` and `README.md` pass. Fix the command or the
  code; change an output the page shows only when the CLI's output is the right one.

**Checkpoint**: an author can reach a running widget from the docs alone. The runner proves it on
Linux; the docs job of US4 proves it on all three systems.

---

## Phase 5: User Story 3 - An author looks up any part of the contract (Priority: P2)

**Goal**: one reference with every contract item, host error, manifest and connector field,
limit and author-facing command. Each entry has a marker.

**Independent Test**: pick items at random from `wit/v1/widget.wit`, the two schemas,
`runtime/src/limits.rs`, `ErrorKind` and the CLI's usage, and find each in
`docs/authors/reference/` with what it is, when it happens and what to do.

### Implementation for User Story 3

- [ ] T021 [P] [US3] Write `docs/authors/reference/contract.md` (FR-010, R6):
  - **Coverage:** every world, interface, function, type and enum or variant case or record field
    of 1.3.0, each with a marker `<!-- ref: wit <id> -->` on the line before its heading or at the
    end of its table row.
  - **Each entry says:** what it is, the minor it arrived in, when each answer happens, and what a
    widget should do with it.
  - **Order:** `not-granted` comes first in every error type.
  - **Sources:** apply the `docs` outcomes of `review.md`.
- [ ] T022 [P] [US3] Write `docs/authors/reference/errors.md` (FR-011). For each of the 36 kinds
  of `ErrorKind::ALL`:
  - the marker `<!-- ref: error <name> -->`;
  - the line as the CLI prints it (`error: <name>: …`), its cause, and its fix;
  - a `console check` example where `my-widget/`, the copy of the walkthrough's crate the runner
    gives reference pages, can trigger it without a service.
- [ ] T023 [P] [US3] Write `docs/authors/reference/manifest.md` (FR-012): every field path of
  `wit/manifest.schema.json` (`[]` for array items), with the marker `<!-- ref: manifest <path> -->`,
  what it accepts, whether it is required, and what it changes at approval. `login_providers` is
  obsolete, accepted and grants nothing.
- [ ] T024 [P] [US3] Write `docs/authors/reference/connector-format.md` (FR-012): every field path
  of `wit/connector.schema.json`, `$defs` resolved where they are referenced, with the marker
  `<!-- ref: connector <path> -->`, what it accepts and whether it is required.
- [ ] T025 [P] [US3] Write `docs/authors/reference/limits.md` (FR-012):
  - One table row per field of `Limits` (44): the name, the default and the error past it.
  - Each row ends with `<!-- ref: limit <field> -->`.
  - The second cell is the default in the format of `contracts/docs-format.md` (`10 MiB`,
    `64 KiB`, `1 s`, `500 ms`, `500,000,000`, `30 per minute`).
- [ ] T026 [P] [US3] Write `docs/authors/reference/cli.md` (FR-013):
  - **Commands:** each command and option an author uses: `pack`, `permissions`, `approve`,
    `run`, `update`, `requests`, `accounts connect`, `accounts list`, `accounts disconnect`,
    `optional`, `remove`, `storage list`, `storage show`, `storage clear` and `check-connector`,
    as the CLI's usage lists them (`bench` is for the project's own measurements).
  - **Markers:** each command and option has `<!-- ref: cli <command> [<option>] -->`.
  - **Examples:** each command has a `console check` example of what it prints, run in
    `my-widget/`, the copy of the walkthrough's crate the runner gives reference pages.
  - **Commands that reach a service:** link to their checked example in the connectors or the
    accounts guide (US5).
  - **Escaping:** one example with a manifest whose `name` holds control characters shows the
    escaped text exactly as the CLI prints it.
- [ ] T027 [US3] Run the runner on `docs/authors/reference/` and fix every page until its checked
  blocks pass.

**Checkpoint**: every fact an author looks up is in the reference (FR-015). US4's drift test
proves the coverage.

---

## Phase 6: User Story 4 - The docs cannot drift from the contract or the host (Priority: P2)

**Goal**: the build fails, naming the item, when the reference and the sources disagree. The docs
job runs the runner on three systems, and the docs the checks read count as code.

**Independent Test**: delete a marker from `contract.md`, add `<!-- ref: error no-such-error -->`,
or change a default in `limits.md`: each time `cargo test -p runtime --test docs_reference`
fails, naming it (quickstart §2). Then change one command of `getting-started.md` so it fails: the
docs job fails (quickstart §3).

### Tests for User Story 4

- [ ] T028 [US4] Write the self-tests of the drift test first, in `runtime/tests/docs_reference.rs`.
  They fail until T029. Each runs the checker on an in-memory expected set and reference text:
  - a missing marker fails, naming the item;
  - a marker for an item that does not exist fails, naming it;
  - a repeated marker fails;
  - a limit row whose default differs fails, naming the limit and both values;
  - an unknown marker kind fails;
  - a WIT definition the checker cannot classify fails, naming it, and is never skipped;
  - an option in the usage that has no marker fails, naming it;
  - a link into `specs/` on a line that does not start with `Background:` fails, naming the page
    and the line.

### Implementation for User Story 4

- [ ] T029 [US4] Implement the drift test in `runtime/tests/docs_reference.rs` (research R7), so
  T028 passes:
  - **Expected set:**
    - parse `wit/v1` with `wit_parser::Resolve` and take every world, interface, function, named
      type, and case or field of enums, variants, records and flags; any other type definition
      kind fails, naming it;
    - walk `wit/manifest.schema.json` and `wit/connector.schema.json` through `properties`,
      `items` (`[]`) and local `$ref`s;
    - take every name in `runtime::ErrorKind::ALL`;
    - destructure `runtime::Limits::default()` field by field, with no `..`, and format each
      default in the reference's units;
    - take every command and option of `runtime::cli::USAGE` except `bench`; a usage line the
      parser cannot read fails, naming it.
  - **Reference set:** every `<!-- ref: … -->` under `docs/authors/reference/`.
  - **Tests:** `every_item_is_in_the_reference` and `no_page_needs_a_spec` (a link from
    `docs/authors/` into `specs/` fails unless its line starts with `Background:`; FR-015,
    SC-007), in every `cargo test`.
  - **Pass:** it passes against T021–T026. Fix the pages, never the expected set.
- [ ] T030 [US4] Update `.github/workflows/ci.yml` (research R11):
  - **`changes` job:** a changed path under `docs/authors/` or `wit/` sets `code=true` before
    the documentation-only test. Update the comment so it cites constitution v2.4.1 and says that
    those files are used as code (FR-019).
  - **New `docs` job:**
    - matrix `ubuntu-24.04`, `macos-latest` and `windows-latest`; `needs: changes`;
      `if: needs.changes.outputs.code == 'true'`; timeout 40 minutes;
    - steps: checkout; Rust 1.99.1 with `wasm32-unknown-unknown`; on Linux `libdbus-1-dev`,
      `pkg-config` and `gnome-keyring`; the cache pinned as the build job pins it; `cargo xtask
      plugins --locked`; `cargo test -p runtime --test docs_reference --locked`;
    - the runner, `cargo test -p runtime --test docs_walk --locked -- --ignored`, with
      `DASHBOARD_DOCS_TARGET_DIR` set to the cached target folder. On Linux it runs inside
      `dbus-run-session` with gnome-keyring unlocked, as the build job runs its tests; it runs
      as is on macOS and Windows.
  - **`result` job:** add `docs` to its `needs`.
  - **`audit` and `deny` jobs:** add `cargo audit --file docs/authors/getting-started/Cargo.lock`
    and `cargo deny --manifest-path docs/authors/getting-started/Cargo.toml check`.
- [ ] T031 [P] [US4] Update `CONTRIBUTING.md` (FR-020):
  - A change to the contract, a schema, a limit, a host error or an author-facing CLI command
    updates `docs/authors/reference/` in the same pull request, and `docs_reference` names what
    is missing.
  - A change to a walkthrough or guide command is checked by the docs job.
  - Running the runner locally needs `DASHBOARD_DOCS_WALK_CLEARS_DEVELOPMENT_DATA=1`, because it
    clears your development data. It needs a committed `HEAD`, and on Linux a Secret Service.
  - `docs/authors/` and `wit/` count as code in CI.
  - How v1 changes from now on: see `wit/README.md`.
  - A dependency of the walkthrough's crate moves only with its `Cargo.lock` and the page
    together.

**Checkpoint**: docs that lie fail the build on the three systems.

---

## Phase 7: User Story 5 - Each capability has a guide that starts from its example (Priority: P3)

**Goal**: guides for render and update, connectors, accounts, storage and writing connectors.
Each starts from its example and ends with the widget running. The guides that reach a service
run through the test-only CLI against the test services.

**Independent Test**: the runner passes on every page under `docs/authors/guides/`, including
`connectors.md` (`service=github`) and `accounts.md` (`service=notes`), and the real `runtime`
ignores `DASHBOARD_TEST_NETWORK`.

### Tests for User Story 5

- [ ] T032 [US5] Write `the_real_cli_ignores_the_test_network` first, in `runtime/tests/cli.rs`
  (`contracts/cli.md`):
  - Start a test HTTPS server of `runtime/tests/common/servers.rs` on `127.0.0.1:<port>`.
  - Use a development connector (`--connectors <dir>`) whose destination is
    `https://localhost:<port>`, and a widget that calls it, approved in `--grants <temp>`.
  - Write a `DASHBOARD_TEST_NETWORK` file that routes `localhost:<port>` to the server, lists it
    in `public_for_tests` and names a data directory.
  - Run the real `runtime update` with `--data <temp>`, once with the variable set and once
    without.
  - Assert the server got no request, the two outputs are equal, and both are the production
    refusal of a destination that is not public.

  `localhost` resolves on the machine, and production refuses loopback before connecting
  (`network/resolver.rs:64`), so nothing leaves the machine. This test passes today. It guards
  T034 and T035. If the connector schema already refuses `localhost`, use another name the
  system resolves locally to loopback, and record which.

### Implementation for User Story 5

- [ ] T033 [US5] Add an HTTP sign-in route to the test sign-in service in
  `runtime/tests/common/signin.rs`:
  - `GET /oauth/authorize` does what `SignInService::browser` does today, using the same script,
    codes and secrets, and answers `302` with `Location` set to the callback address and its
    query.
  - `browser()` stays as it is for the library tests.
  - Add a test in `runtime/tests/accounts.rs` that signs in through the route, with no
    `browser()`.
- [ ] T034 [US5] Add the test sign-in follower to `runtime/src/network/test_signin.rs`, declared
  under `#[cfg(feature = "test-network")]` in `runtime/src/network/mod.rs` (research R9):
  - `pub fn follow(test: &TestNetwork, address: &str) -> std::io::Result<()>` sends `GET` to the
    sign-in address through the seam's routes and test root, as connector requests do, reads the
    `Location` of the `302`, and sends `GET` to it on the loopback callback.
  - It is covered by the network module's existing `#![allow]`; add no new one.
  - Confirm `cargo clippy --workspace --all-targets --locked -- -D warnings` passes.
- [ ] T035 [US5] Create the test-only binary, `runtime/src/bin/runtime-test-network.rs`
  (`contracts/cli.md`):
  - **Declaration:** in `runtime/Cargo.toml`,
    `[[bin]] name = "runtime-test-network" required-features = ["test-network"]`, with a comment.
  - **Startup:** it reads the JSON file named by `DASHBOARD_TEST_NETWORK`, which holds `routes`,
    `public_for_tests`, `root` as a JSON array of the DER's bytes (the runtime has no base64 dependency, and a binary
    cannot use dev-dependencies), `catalog`, `data_dir` and `grants`. A missing or
    unreadable file is a usage error, and then the binary does nothing.
  - **CLI:** it calls `runtime::cli::main_with` with
    `Runtime::with_test_network_on_keychain` on that `TestNetwork`, an opener that calls
    `network::test_signin::follow`, and `data_dir` and `grants_path` from the file.
  - **Checks:** confirm `cargo install --path runtime --locked --root <temp>` installs only
    `runtime`, and that the CI step "The test seam never builds in release" still fails as it
    must.
- [ ] T036 [US5] Extend the runner in `runtime/tests/docs_walk.rs` for a page with a
  `service=<names>` front-matter comment:
  - **Before the page:**
    - start the named test servers from `runtime/tests/common`: `github` is the GitHub releases
      server, and `notes` is the sign-in service with the notes API;
    - write the `DASHBOARD_TEST_NETWORK` file with a data directory and a grant record of the
      session's own;
    - replace the `runtime` in `<session>/bin` with `CARGO_BIN_EXE_runtime-test-network`.
  - **After the page:** run `accounts disconnect` for every connected account, so the keychain
    keeps nothing.

  Add a self-test that a page with `service=` runs its `runtime` commands through the test-only
  binary.
Each guide of T037–T040 starts in `dashboard/plugins/examples/<example>` of the session's clone.
It builds the example with `cargo build --release --target wasm32-unknown-unknown`
(`output=any`) and packs it with `runtime pack` from
`../../target/wasm32-unknown-unknown/release/<crate>.wasm`, because the examples build in the
`plugins/` workspace.

- [ ] T037 [P] [US5] Write `docs/authors/guides/render-and-update.md`, from `hello-widget`
  (FR-009):
  - what `update` and `render` each receive and return;
  - that each call runs in a fresh instance;
  - that render never waits on the network and never writes;
  - the 64 KiB output and the content format in Phase 3.

  End with the widget running, in checked blocks.
- [ ] T038 [P] [US5] Write `docs/authors/guides/connectors.md`, from `github-releases`, with
  `<!-- check: service=github -->`:
  - the connector a widget declares, and what the maintainer approves (the data flow);
  - `runtime update` and `runtime requests`;
  - handling `not-granted`, `refused` and `failed`.

  End with `runtime run` showing the releases.
- [ ] T039 [P] [US5] Write `docs/authors/guides/accounts.md`, from `private-notes` and
  `local/notes`, with `<!-- check: service=notes -->`:
  - `runtime accounts connect local/notes`, then `update` and `run` with the notes;
  - handling `not-granted` and `account-not-connected`;
  - `accounts disconnect`;
  - registering an app with your own service for real use (no catalog connector has an account
    until Phase 6).
- [ ] T040 [P] [US5] Write `docs/authors/guides/storage.md`, from `update-counter`: what the store
  keeps, its limits, handling `not-granted`, and the widget running twice so it shows the count
  growing.
- [ ] T041 [P] [US5] Move the content of `docs/connectors.md` into
  `docs/authors/guides/writing-connectors.md`. Align it with the reference: link to
  `reference/connector-format.md` for each field instead of repeating it, and make its checked
  examples run.
- [ ] T042 [US5] Run the runner on `docs/authors/guides/` and fix every page until it passes on
  Linux.

**Checkpoint**: every guide runs as written against the test services, never GitHub.

---

## Phase 8: User Story 6 - Leftovers from earlier features are gone (Priority: P3)

**Goal**: no example to copy carries `login_providers`, no page promises TypeScript in 006, and
the older pages point to the new docs.

**Independent Test**: `git grep -n login_providers -- plugins/` finds nothing.
`git grep -n "TypeScript" -- ':!specs'` finds only the Phase 5 wording.

### Implementation for User Story 6

- [ ] T043 [P] [US6] Remove `login_providers` from the 16 manifests under `plugins/` that carry
  it (research R14). Keep the field in the tests that check it: `manifest_schema.rs`, `cli.rs` and
  `tests/common`. Run `cargo xtask plugins --locked` and `cargo test --workspace --locked`.
- [ ] T044 [P] [US6] Update `plugins/examples/README.md` (FR-022, FR-023):
  - Replace "TypeScript examples arrive in feature 006" with "TypeScript plugins arrive in
    Phase 5".
  - Point each example to its guide.

  Each example's own `README.md` and `plugins/hostile/README.md` link to the guides and the
  reference instead of repeating them.
- [ ] T045 [P] [US6] Turn `docs/plugin-authors.md` and `docs/connectors.md` into short pointers to
  `docs/authors/` and `docs/authors/guides/writing-connectors.md`. Check that every link to them
  from `README.md`, `SECURITY.md`, `CONTRIBUTING.md`, `connectors/README.md` and the READMEs under
  `plugins/` still leads to the right page.

**Checkpoint**: everything an author copies is current.

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: checks, walks, review and merge

- [ ] T046 [P] Update `.github/pull_request_template.md`: the security core comment names
  `runtime/src/cli/`, `runtime/src/bin/runtime-test-network.rs` and
  `runtime/src/network/test_signin.rs`; the `Cargo.lock` line names `wit-component`.
- [ ] T047 Run every local check:
  - `cargo fmt --all --check`;
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`, and the same for
    `plugins/` on `wasm32-unknown-unknown`;
  - `cargo xtask plugins --locked`;
  - `cargo test --workspace --locked`, on Linux inside a `dbus-run-session` with gnome-keyring;
  - the runner, with the opt-in variable, on a committed `HEAD`;
  - `cargo deny check`, and `cargo audit` on `Cargo.lock`, `plugins/Cargo.lock` and
    `docs/authors/getting-started/Cargo.lock`.
- [ ] T048 Confirm FR-024: `git diff master -- runtime/tests/hostile.rs runtime/tests/hostile/
  plugins/hostile/` shows only the `login_providers` removal and README links, and no hostile
  case was added or removed.
- [ ] T049 Walk through `specs/006-contract-v1-docs/quickstart.md` §1–§4, including each
  mutation (a byte of a frozen contract, a marker, a limit's default, a command), and fix any
  drift between it, the docs and the CLI.
- [ ] T050 The agent's walk (SC-001, research R16):
  - Run a fresh agent with no context in a disposable `rust:1.99.1` container, with the committed
    tree mounted and git's `insteadOf` sending the clone to it.
  - Tell it to follow only `docs/authors/` and to read no code or spec.
  - Record each stuck point and its fix in `walks.md`, and repeat until a walk has none.
- [ ] T051 Only once the maintainer has pushed and opened the pull request, as they ask: record in
  `walks.md` the docs job's time on each system from the first run. If any system goes over 25
  minutes, bring it to the maintainer before going on.
- [ ] T052 The maintainer's walk (SC-001, research R17): the maintainer follows
  `docs/authors/getting-started.md` by hand on Linux, timed, without the tool downloads. The time
  and any stuck point go in `walks.md`, and the goal is under 30 minutes. Record the `cargo
  install` time apart. If the walk goes over 30 minutes because of it, bring it to the
  maintainer, since SC-001 counts build time.
- [ ] T053 Human review (Principle IX):
  - The maintainer reviews every commit under `runtime/`, `wit/` and `plugins/`, and the
    `Cargo.lock` change that adds `wit-component`, line by line. T006 is reviewed as moved lines
    with `git diff --color-moved`.
  - The maintainer ticks the pull request's review box with their handle.
  - CI is green on the three systems.
  - Merge with `gh pr merge --merge --match-head-commit <sha> --author-email
    8193616+philipesantos@users.noreply.github.com`.
- [ ] T054 After the merge, and only when the maintainer asks, mark the development guide's
  Phase 2 item "Contrato WIT v1 com essas interfaces … e primeira documentação para autores" as
  done, and Phase 2 as complete.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies
- **Foundational (Phase 2)**: depends on Setup; blocks every user story. T003 → T004 → T005 must
  finish, with no finding `open`, before US2, US1 or US3 start.
- **User Stories (Phases 3–8)**: US2 first, then US1 (MVP), then US3, US4 and US5; US6 at any
  time after Foundational.
- **Polish (Phase 9)**: depends on every user story.

### User Story Dependencies

- **US2 (P1)**: after T005; freezes what the docs name
- **US1 (P1)**: after US2 (the walkthrough copies `wit/history/1.3.0`), T007 (pack in
  `runtime::cli`) and T010 (the runner)
- **US3 (P2)**: after T005, T008 and T010; the examples run in the copy of the walkthrough's
  crate (T016)
- **US4 (P2)**: T028–T029 after US3 (the drift test passes only against the reference); T030
  after US1 (the docs job runs the walkthrough)
- **US5 (P3)**: after T007 and T010; T038 and T039 after T033–T036
- **US6 (P3)**: after Foundational; T045 after T041 (the pointer's target exists)

### Within Each Phase

- Foundational: T003 → T004 → T005; T006 → T007 (two commits, in order); T008 with either; T009
  → T010
- US1: T014 → T015; T016 → T017 → T018 → T020; T019 with T018
- US4: T028 → T029; T030 and T031 after T029
- US5: T032 first; T033 → T034 → T035 → T036; T037–T041 after T036; T042 last
- The tasks of different stories that edit `runtime/tests/cli.rs` (T014, T032) or
  `runtime/tests/docs_walk.rs` (T009, T010, T017, T036) run in order.
- Tests before their implementation; they must fail first, except T032, which guards behaviour
  that exists.

### Parallel Opportunities

- Setup: T002 with T001
- Foundational: T008 with T006–T007 and with T009–T010
- US1: T019 with T018
- US3: T021–T026, six pages
- US4: T031 with T030
- US5: T037–T041, five pages
- US6: T043, T044 and T045 (T045 after T041)
- Polish: T046 with T047

---

## Parallel Example: User Story 3

```bash
# Six reference pages, each its own file:
Task: "docs/authors/reference/contract.md: every WIT item with its marker, not-granted first"
Task: "docs/authors/reference/errors.md: the 36 kinds of ErrorKind::ALL as the CLI prints them"
Task: "docs/authors/reference/manifest.md: every field of wit/manifest.schema.json"
Task: "docs/authors/reference/connector-format.md: every field of wit/connector.schema.json"
Task: "docs/authors/reference/limits.md: the 44 limits with their defaults"
Task: "docs/authors/reference/cli.md: the author's commands with checked examples"
```

---

## Implementation Strategy

### MVP First (User Story 1, after the contract is closed)

1. Phase 1: Setup
2. Phase 2: Foundational. The review comes first, and a breaking finding goes to the maintainer
   before anything else.
3. Phase 3: User Story 2 (the freeze, which the walkthrough copies)
4. Phase 4: User Story 1
5. **STOP and VALIDATE**: quickstart §1, §3 (the walkthrough) and §4. An author can reach a
   running widget.

### Incremental Delivery

1. Setup + Foundational → the reviewed contract, `runtime::cli`, `ErrorKind::ALL` and the
   runner's core, with every suite of features 001 to 005 green
2. User Story 2 → 1.3.0 frozen and pinned
3. User Story 1 → `pack` on a core module, and the walkthrough checked on Linux
4. User Story 3 → the reference
5. User Story 4 → the drift test, and the docs job on three systems
6. User Story 5 → the test-only CLI and the guides
7. User Story 6 → `login_providers`, TypeScript and the old pages
8. Polish → the template, local checks, the quickstart, the walks, human review

---

## Notes

- [P] tasks touch different files and depend on no incomplete task.
- All work happens on the `006-contract-v1-docs` branch, in one pull request opened as a draft
  when the maintainer asks to push. Never commit this feature directly to `master`.
- One commit per task or small group of tasks, without any co-author or tool attribution. T006
  and T007 are always separate commits. The maintainer reviews each commit line by line
  (Principle IX); a fix after review is a new commit, never a rewrite of a reviewed one.
- Push only when the maintainer asks.
- Line-by-line human review (T053) covers:
  - everything under `runtime/` and `wit/`, which is security core;
  - the `Cargo.lock` change that adds `wit-component`;
  - `plugins/hostile/`, `plugins/fixtures/` and `runtime/tests/hostile/`, which hold the security
    tests.
- Never commit built `.wasm` files; CI builds every plugin from source.
- No test reaches the internet or a real service. The runner's clone goes to the local checkout,
  and its services are the test servers.
- The runner clears the development data directory. Outside CI it runs only with
  `DASHBOARD_DOCS_WALK_CLEARS_DEVELOPMENT_DATA=1`.
