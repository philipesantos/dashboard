---

description: "Task list for plugin installation and the installation dialog (feature 010)"
---

# Tasks: Plugin Installation and the Installation Dialog

**Input**: Design documents from `specs/010-plugin-installation/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R16 and the review's outcomes),
data-model.md, contracts/, quickstart.md

**Tests**: Required (FR-026, Principle VIII). Every rule gets its test or probe case before the
code that enforces it, and that test fails until the code lands. Research R11 lists the cases
below the page and the probe cases, and what only quickstart §6's review reaches.

**Security core** (constitution v2.7.0, Principle IX): `runtime/src/install/`, `compile/`,
`flows.rs`, `network/`, `grants.rs`, `permissions.rs`, `lib.rs` (`engine_config`,
`load_compiled`); `app/src-tauri/src/main.rs`, `install.rs`, `widgets.rs`, `commands.rs`,
`capabilities/`, `window-surface.toml`, `build.rs`; `app/src/host/`,
`app/src/components/ui/dialog.tsx`, `app/src/widgets/Frame.tsx`, `app/src/probe/`; the record
formats; the new CI steps. The pull request gets line-by-line human review.

**Heavy builds**: run `cargo build` and `cargo test` locally inside
`systemd-run --user --scope -p MemoryMax=8G -p MemorySwapMax=1G`; run `cargo xtask plugins`
before any build of the app or the tests.

**Organization**:
- **Setup:** changes that alter no behaviour: HS-02 first, the engine config, the permission enum,
  error kinds, fixtures.
- **Foundation:** the compile child's protocol, loading a compiled form, the host GET, the release
  source, the records, the warning rule, the dialog model, and the app's requests and commands.
- **The stories:** US1 installing from a repository; US2 what the dialog says; US3 updates and
  grant changes; US4 a hostile package (limits and confinement); US5 the host dialog's form.
- **Polish:** threat model, decision 0003, author docs, CONTRIBUTING, checks, walks, review, merge.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1–US5)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: everything later tasks build on, with no change in behaviour.

- [X] T001 [P] Change criterion HS-02 in `docs/design/host-surfaces.md` first, as feature 007's rule
  allows (research R14): the header of every installed widget shows its repository owner
  (`github.com/<owner>`) after the name, drawn by the host as escaped text in the frame header's
  host-only roles; bundled widgets show none; the id beside a shared name stays. Its check names
  the probe case `frame-owner-shown` and the feature "Instalação de plugin a partir do repositório
  do autor". Run `pnpm --filter ./design check`.
- [X] T002 [P] Factor the engine's configuration out of `Runtime::start` into one public
  `runtime::engine_config(&Limits) -> wasmtime::Config` in `runtime/src/lib.rs` (today
  `lib.rs:606-665`), used by `Runtime::start`; no behaviour change. `cargo test -p runtime --test it`
  passes.
- [X] T003 [P] Replace the permission string registry in `runtime/src/permissions.rs:132-166` by an
  enum (`Permission::Log`, `Permission::Storage`) with the same names, `v0`, `v1` and `since`;
  parsing, errors and the CLI's output unchanged. `cargo test -p runtime --test it` passes.
- [X] T004 [P] Add the runtime error kinds of data-model.md to the `error_kinds!` macro in
  `runtime/src/error.rs` (`identity-mismatch`, `tag-mismatch`, `changed-release`,
  `repository-changed`, `not-newer`, `source-unreachable`, `rate-limited`, `no-package-asset`,
  `compile-time-limit`, `compile-memory-limit`, `compile-failed`, `package-changed`), update the
  count asserted at `error.rs:271`, and add each to `docs/authors/reference/errors.md` with when it
  is answered. The docs reference test passes.
- [X] T005 Move the code generator of `bench/src/bin/compile_cost.rs` (`many`, `huge`) into a
  function `cargo xtask plugins` can call, keep the bench using it, and have `xtask/src/main.rs`
  build `target/plugins/compile-heavy.wasm`: a real widget (manifest id
  `github.com/dashboard-fixtures/compile-heavy`, a `renderer` export returning a one-line text) padded
  with generated functions to just under the 10 MiB package cap (research R11). `cargo xtask plugins`
  builds it; `cargo run -p dashboard-bench --bin compile_cost many` still runs.
- [X] T006 [P] Add the install fixtures under `plugins/fixtures/`, each with a `github.com/dashboard-fixtures/<name>`
  id, built by `cargo xtask plugins`: `install-basic` (`log` required, `storage` with a quota, the
  catalog GitHub connector optional), `install-account` (an account connector with a credential and
  a second catalog destination optional), `install-more` (`install-basic` at a higher version with the
  connector required), `install-repo-connector` and `install-local-connector` (a repository and a
  `local/` connector, optional). Record each in `plugins/fixtures/README.md`.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the pieces every story uses. Tests first, each failing until its code lands.

- [X] T007 Write `runtime/tests/compile.rs` (listed in `runtime/tests/it.rs`): the child spawned
  through the runtime's CLI binary with `--compile-package` returns a form that
  `Runtime::load_compiled` loads and renders for the `install-basic` fixture; it refuses WAT text
  even with the `wat` feature on; a fingerprint mismatch (a test hook) is `compile-failed`, never
  retried; `compiled-tampered` (one flipped byte: recompiled once, never deserialized) and
  `package-tampered` (`package-changed`).
- [X] T008 Implement `runtime/src/compile/mod.rs` and `compile/child.rs` without limits or
  confinement yet (research R2 steps 1, 2, 4, 5): `child_main()` prints the fingerprint
  (`Engine::precompile_compatibility_hash`), builds the engine from `engine_config()` and rayon's
  pool, reads at most `max_package_bytes` from standard input, checks the component preamble as
  `inspect` does (`lib.rs:922-934`), calls `Engine::precompile_component`, writes the bytes; the
  parent `Compiler::compile(package)` starts `/proc/self/exe` (Linux) or `current_exe` with only
  `--compile-package`, a cleared environment except `RAYON_NUM_THREADS`, `SystemRoot` (Windows) and
  `LD_LIBRARY_PATH` when set, compares the fingerprint, reads standard error up to 64 KiB into the
  log, caps the output at 64 MiB, and always waits for the child. Wire `--compile-package` before
  anything else in `runtime/src/bin/` (the CLI) and in `app/src-tauri/src/main.rs` before Tauri
  starts.
- [X] T009 Add `Runtime::load_compiled(package, compiled, origin, grants)` in `runtime/src/lib.rs`
  (research R3): reads nothing from disk; checks both digests against the caller's record on the
  bytes it was given; `unsafe { Component::deserialize(&compiled) }` (never `deserialize_file`); then
  the same `inspect` steps as `load` (`lib.rs:910-958`) with that component. T007's load cases pass.
- [X] T010 [P] Write `runtime/tests/install_github.rs` against a local HTTPS test server under the
  existing `test-network` feature: the host GET follows at most 3 redirects, only to
  `release-assets.githubusercontent.com` and `objects.githubusercontent.com` over HTTPS
  (`redirect-elsewhere` refused); API answers capped at 256 KiB and 10 s, the package at the package
  cap and 60 s; `403` or `429` with `x-ratelimit-remaining: 0` or `retry-after` is `rate-limited`;
  the request headers are `Accept: application/vnd.github+json`, `X-GitHub-Api-Version: 2022-11-28`
  and the `dashboard/<version>` user agent, with no credential.
- [X] T011 Implement the host GET in `runtime/src/network/mod.rs` (public to the crate's `install`
  module only; `ureq` stays inside `network/`, `clippy.toml:16-60`) with the existing TLS setup and
  public-only resolver, and the installer's redirect rule; `runtime::install::FetchLimits` with the
  data-model's defaults. T010 passes.
- [X] T012 Implement `runtime/src/install/github.rs`: the `ReleaseSource` trait and the GitHub
  source (research R4): identity by `GET /repos/{owner}/{repo}` (a `301` refused naming the move;
  `full_name` lowercased must equal the id), release and asset by `/repositories/{id}/…`, with the
  documented fallback (`/repos/{owner}/{repo}/…` then `/repositories/{id}` checked again after the
  download); `tag_name` `v<version>` or `<version>`; exactly one `.wasm` asset (`no-package-asset`
  otherwise), its `size` within the cap, its `digest` checked when present. The in-memory source for
  tests lives behind `#[cfg(test)]` or `test-network` (research R16).
- [X] T013 [P] Write `runtime/tests/install_records.rs`: `installed.json` round-trips, refuses
  unknown fields, writes atomically under its lock; the grant record reads versions 1 to 4 with
  `account_sources` empty and writes version 5; `GrantStore::install` builds a fresh entry (required
  items plus the given optionals) and ignores optionals on in an earlier entry of the same origin
  (`stale-optional`).
- [X] T014 Implement `runtime/src/install/record.rs` (`installed.json`, data-model.md: "key: a
  repository plugin id … never `local/`", `repository_id`, `tag`, `version`, `package_digest`,
  `compiled_digest`, `installed_at`), grant record version 5 with `account_sources` ("Sorted
  connector ids … Never shrinks while the store exists"), and `GrantStore::install` in
  `runtime/src/grants.rs`. T013 passes.
- [X] T015 [P] Write `runtime/tests/flows.rs`: `ask-little-then-more` (every subset of
  `install-account`'s optionals: a flow exactly when an account source and another destination are
  both held), `no-false-warning`, `account-then-elsewhere` (v1 account plus storage, v2 drops the
  account and adds a destination: still a flow, through `account_sources`), and `flow-not-hidden`
  (FR-018: a manifest that names the other destination under another case or with a trailing dot,
  or declares the account connector required beside an optional destination, still gives the
  flow).
- [X] T016 Implement `runtime/src/flows.rs` (`judge(entry_after, remembered) -> Vec<Flow>`,
  research R7) and keep `account_sources` up to date wherever a grant is written in
  `runtime/src/grants.rs`. Replace the CLI's `data flow:` lines (`cli/mod.rs:1214-1248`,
  `:1329-1352`) with `judge`. T015 passes; the CLI's existing output tests are updated where the
  wording changes.
- [X] T017 [P] Write `runtime/tests/dialog_model.rs`: for every fixture, the model holds every
  permission and connector property the manifest declares (`plain-language`); names and authors
  with markup, bidirectional controls and 500 characters come out sanitised and bounded, the author
  labelled as declared by the plugin, the id in full; the choice digest changes with `optional_on`
  and with the warning; the CLI's `describe` prints the same model.
- [X] T018 Implement `runtime/src/install/dialog.rs`: `DialogModel` (data-model.md), the Portuguese
  sentences of research R6 in one exhaustive `match` on the permission enum, the author through the
  same sanitising as the name (moved from `app/src-tauri/src/name.rs` into `runtime/src/text.rs`
  so both use it; `name.rs` calls it), the approval and choice digests; make `runtime/src/cli/`
  `describe` print it. T017 passes.

  Done on 2026-11-12, with one change: the CLI's `describe` keeps its own text, which many CLI
  tests pin, and shares with the model the functions that read an operation (`sends`, `fixed`,
  the URL) and the warning rule; `the_development_tool_shows_the_operations_the_dialog_shows`
  checks that each operation and destination of the model is in the CLI's output.
- [X] T019 In `app/src-tauri/src/widgets.rs`: the app's grant record moves to
  `<app data dir>/host/grants.json` (`GrantStore::open`); `Widgets.shown` becomes a lock-guarded
  list (`widgets.rs:135-138`); at start, installed widgets load from `installed.json` through
  `load_compiled` with origin `github:<repository_id>`, after the cleanup of research R5, which runs
  only when both records were read; `FrameInfo` gains `source` for installed widgets. Tests in
  `app/src-tauri/tests/install.rs`, with records the test writes itself: the start-up cleanup
  removes package directories no entry names, entries with no grant and `github:` grants with no
  entry; an unreadable record removes nothing. (`crash-mid-install`, which needs the installer's
  write path, is T022–T023.)
- [X] T020 Add `app/src-tauri/src/install.rs` with the request state machine of data-model.md and
  the five commands of `contracts/window-commands.md` (`install_start`, `update_start`,
  `request_judge`, `request_approve`, `request_cancel`), each listed in
  `app/src-tauri/window-surface.toml` with its reason, granted in `capabilities/default.json` and
  registered in `src/commands.rs`; the app refusals `request-pending`, `unknown-request`,
  `already-installed`, `bad-address`, `bad-choice`; pending requests dropped and any compile child
  killed at exit. `app/src-tauri/tests/surface.rs` passes; tests in `tests/install.rs`:
  `one-request`, `token-replayed`, `optional-bounds`, `bad-address`, `approval-changed` (the
  approval digest; the choice digest is T029–T030), and the safe-choice cases of FR-006 and FR-023:
  `cancel-keeps-nothing` (after `request_cancel`, no package, compiled form, grant or entry, and the
  temporary directory gone), `window-closed-mid-dialog` (the window's close drops the pending
  request: approving its token afterwards is `unknown-request`, nothing granted) and
  `quit-mid-compile` (stopping the host while a request compiles kills the child, leaves no process
  and keeps nothing).
- [X] T021 Add `app/src/host/api.ts` (the five commands, typed from the contract) and the first
  `HostDialog.tsx` on the Radix dialog, in the host-only `dialog` roles and elevation instead of
  `popover` (fix `app/src/components/ui/dialog.tsx` too), with the safe choice first and focused,
  and Escape taking it. The full form of the criteria is US5.

**Checkpoint**: an installer, a model and the app's commands exist; the stories wire them up.

---

## Phase 3: User Story 1 - The person installs a widget from its author's repository (Priority: P1) 🎯 MVP

**Goal**: from a repository address to a running widget, through one dialog.

**Independent Test**: spec US1; quickstart §3 steps 1 to 4 and §4's address refusals.

- [X] T022 [US1] Write the installation cases in `runtime/tests/install.rs` (in-memory source):
  `identity-mismatch`, `tag-mismatch`, `no-asset`, `two-assets`, `asset-digest-mismatch`,
  `oversized-asset` (declared size, then a body that grows past the cap: refused before compiling),
  `renamed-at-install`, `name-swapped-mid-install`, `repository-connector` and `local-connector`
  (refused even optional, the message naming the later item), `connector-major-unavailable` (a
  catalog connector major the host does not ship: `connector-unavailable` before any dialog), the
  happy path for `install-basic`, and `crash-mid-install` (a kill after each write step of research
  R5: the next start keeps nothing usable); each refusal keeps nothing.
- [X] T023 [US1] Implement the installer in `runtime/src/install/mod.rs`: address → identity →
  release → package → `Compiler::compile` → `inspect` with development mode off → `DialogModel`,
  in a temporary directory; on approval the write order of research R5 (package and compiled form
  renamed into `packages/<digest hex>/`, then `GrantStore::install`, then the installed record).
  T022 passes, `crash-mid-install` included.

  Done on 2026-11-12, beside T019 and T020, which need it: `runtime::install::installer`
  (`Installer`, `Layout`, `Prepared`), with `Runtime::check_compiled` so a package the app did
  not build is never compiled in its own process. The release is held in memory until the
  approval, not in a temporary directory: a cancel then has nothing to remove. `no-asset`,
  `two-assets`, `asset-digest-mismatch`, `renamed-at-install` and the declared-size half of
  `oversized-asset` are cases of the GitHub source, in `runtime/tests/install_github.rs` (T010).
- [X] T024 [US1] Connect `install_start` and `request_approve` in `app/src-tauri/src/install.rs`
  to the installer; the address forms of the contract (`github.com/owner/repo`,
  `https://github.com/owner/repo`, case-insensitive, trailing `/` or `.git`, at most 200
  characters). Test `install-from-file`: no command takes a path, and `file:`, URLs and other hosts
  are `bad-address`.
- [X] T025 [P] [US1] The page: `app/src/host/AddWidget.tsx` in the chrome (an address field and a
  button, progress shown in the chrome while the host fetches and compiles), the opener recorded
  for focus return, `InstallDialog.tsx` drawing who is asking and the permission list on
  `HostDialog`, and `app/src/widgets/Widgets.tsx` refreshing the list after an approval.
- [X] T026 [P] [US1] `app/src/widgets/Frame.tsx` draws `source` after the name for installed
  widgets (FR-030, HS-02 as changed in T001).
- [X] T027 [US1] The probe's release source in `app/src-tauri/src/probe.rs` or a module beside it,
  only under the `shell-probe` feature and answering only with `DASHBOARD_PROBE=1`, serving the
  install fixtures, `compile-heavy` and the hostile releases; extend the build test that lists embedded packages
  (feature 009, research R1) to check that a build without `shell-probe` has GitHub's source alone
  (research R16). Probe cases in `app/src/probe/dialogs.ts`: `frame-owner-shown`, and
  `install-on-time` (SC-001): from `install_start` for `install-basic` on the probe source to its
  first rendered content, after an approval sent as soon as the input delay allows; it fails above
  10 s and reports the time.
- [X] T028 [US1] Run quickstart §3 steps 1 to 4 against the probe source on Linux (a real
  repository waits for T052) and §4's address refusals; fix any drift.

  Done on 2026-11-12 on Linux: the probe installs `install-basic` through the chrome and the
  dialog and draws it in about 300 ms (three runs, 69 cases held each); a restart loads it again
  from its compiled form (`a_widget_installed_through_the_requests_shows_its_owner_and_loads_again_at_start`);
  the address refusals are `an_address_that_is_not_a_repository_is_refused`. `install-basic`'s
  component is a test fixture whose render is not widget content, so `install-on-time` measures to
  the frame's first drawn state and names it. The first probe run after the build broke two
  cases of feature 009 (`window-responsive`, `frame-header-hittable`); with `master`'s page and
  in four later runs of this one they held, so it reads as the first run's cold start; T054
  watches them on CI.

**Checkpoint**: a widget installs from a repository and shows its owner.

---

## Phase 4: User Story 2 - The dialog tells the person what they are deciding (Priority: P1)

**Goal**: plain language, every connector detail, the warning, and an approval of exactly that.

**Independent Test**: spec US2.

- [X] T029 [US2] Write `app/src-tauri/tests/install.rs` cases: `request_judge` returns the warning
  and choice digest for each optional set of `install-account`; `request_approve` with a choice
  digest from another set is `choice-changed` (`approval-changed`); the grant written holds exactly
  the required items and the optionals approved; `account-not-connected-in-app` (FR-024):
  `install-account` installs, and its call through the account connector answers
  `account-not-connected`, and the app reads no development record or vault (the test points the
  development paths at a record granting that connector and checks nothing of it is used).
- [X] T030 [US2] Implement `request_judge` and the choice check in `request_approve`
  (`app/src-tauri/src/install.rs`, research R7). T029 passes.
- [X] T031 [P] [US2] `app/src/host/InstallDialog.tsx`: each permission's sentence; each connector
  with destinations, operations (kind, "revisado" or "declarado", method, URL, sends, fixed), the
  account line saying no account is connected in the app; optionals as switches that call
  `request_judge` on every change and redraw the warning; the warning naming the source and every
  destination; approve sends the last choice digest.
- [X] T032 [P] [US2] The development tool: `runtime optional <connector> on` runs `judge` and prints
  the flows it completes (`runtime/src/cli/mod.rs:1525-1550`); test `cli-optional-flow` in
  `runtime/tests/`.

  Done on 2026-11-12. T030's code came with T020; T031's page with T025 (`InstallDialog.tsx`
  draws connectors, the account line, the optionals' switches and the warning). The tool's
  existing case `optional_turns_a_connector_on_and_off_and_off_discards_the_kept_data` now expects
  the flow its local connector completes beside GitHub.

**Checkpoint**: the dialog says what is decided, and the approval covers exactly that.

---

## Phase 5: User Story 3 - Updates and later grant changes never widen access silently (Priority: P1)

**Goal**: updates only from the same repository, newer, with every grant change judged.

**Independent Test**: spec US3; quickstart §3 step 5 and §4's repository case.

- [X] T033 [US3] Write the update cases in `runtime/tests/install.rs`: `changed-release`,
  `repo-jacked`, `renamed-at-update`, `downgrade`, the fallback path of research R4, an update that
  asks for nothing new (`update-confirm`), `install-basic` → `install-more`
  (`update-lists-everything`: every permission, connector and destination, the connector marked
  `changed`), and the old version still loaded until approval.
- [X] T034 [US3] Implement updates in `runtime/src/install/mod.rs`: `GET /repositories/{id}` first
  (`repository-changed` on `404` or another `full_name`), then the release by id; `not-newer` by
  Semantic Versioning; change marks in the model; on approval, the host's kept data is discarded
  when a source goes away (as `cli/mod.rs:1531-1546` does) and `account_sources` kept. T033 passes.

  Done on 2026-11-12 with T022–T023's installer. The host's kept data needs no discard here: its
  key holds the package, the approval and the connectors on (`lib.rs`, `data_key`), so a new
  version, or an account source gone, leaves it unread and load step 7a removes it.
  `account_sources` is kept at every write of the grant record (T014).
- [X] T035 [US3] Connect `update_start` (`app/src-tauri/src/install.rs`): `{ none: true }` when no
  newer release; `update` or `update-confirm` models; the new version replaces the old only after
  `request_approve`.
- [X] T036 [P] [US3] The page: an update control in the frame header of installed widgets (a host
  control, `app/src/widgets/Frame.tsx`), and the update and confirm dialogs in
  `app/src/host/InstallDialog.tsx` with the change marks.

**Checkpoint**: updates cannot take over a name or widen access unseen.

---

## Phase 6: User Story 4 - A hostile package cannot hurt the host while it is installed (Priority: P1)

**Goal**: the compile child limited and confined on the three systems.

**Independent Test**: spec US4; quickstart §5.

- [X] T037 [US4] Extend `runtime/tests/compile.rs` with `compile-slow`, `compile-cpu`,
  `compile-memory` and `compile-output` (the `compile-heavy` fixture against lowered limits, one per
  limit, each refused naming its limit, no child left, the test process's memory unchanged) and
  `compile-confined` (a test-only hook makes the child, after confining itself, try to open a file
  and a socket: both fail and the child exits non-zero), on the three systems.
- [X] T038 [US4] Implement the limits and their attribution in `runtime/src/compile/mod.rs`
  (research R2, data-model "Compile limits": `wall_time` 20 s, `cpu_time` 60 s, `memory_bytes`
  1.5 GiB, `threads` max(1, cores / 2), `max_compiled_bytes` 64 MiB, `max_stderr_bytes` 64 KiB):
  `RLIMIT_CPU`, `RLIMIT_DATA`, `nice(10)` and `PR_SET_PDEATHSIG(SIGKILL)` in `pre_exec` on Unix; the
  child's allocator wrapper exiting 86; the rayon spawn failure caught and mapped to 86; `SIGXCPU`
  and the wall-clock kill to `compile-time-limit`. No setting changes the limits in a release build.

  Done on 2026-11-12 for Unix; Windows's job object is T041, macOS's watchdog T040. The child
  builds rayon's pool itself (`rayon`, already in the lock, becomes a direct dependency), sized by
  `RAYON_NUM_THREADS`, before it confines itself. An allocation that fails is told apart by the
  child's own error text (the standard library's message, or a thread it could not start), not
  by an allocator wrapper: a global allocator in the runtime would have been the app's too.
- [X] T039 [P] [US4] Linux confinement in `runtime/src/compile/confine_linux.rs` (research R13):
  `PR_SET_NO_NEW_PRIVS`, a Landlock ruleset handling every file-system right and granting none when
  supported, a `seccompiler` filter with `SECCOMP_FILTER_FLAG_TSYNC` allowing only the calls R13
  lists. Add `seccompiler` and `landlock` to `runtime/Cargo.toml` for Linux; `cargo deny check` and
  `cargo audit` pass.

  Done on 2026-11-12. Two filters, stacked: the first answers opening a file with `EACCES` (glibc
  opens a file of its own from a compile thread in some runs, under resource limits, and handles
  the refusal), the last one kills the process on any call not on the list; installed in that
  order, since installing a filter takes `prctl` and `seccomp`, which the last one kills. The list
  was taken with strace on a compile, and found complete by 25 compiles under the limits and five
  runs of the tests.
- [X] T040 [P] [US4] macOS confinement and limits in `runtime/src/compile/confine_macos.rs`:
  `sandbox_init` with `kSBXProfilePureComputation`; the watchdog thread sampling the child's own
  physical footprint every 50 ms and exiting with 86 above `memory_bytes`, and exiting when
  `getppid()` changes.

  Written on 2026-11-12 (`runtime/src/compile/confine_macos.rs`), not compiled or run yet: this
  session had no macOS. The watchdog reads the peak resident memory with `getrusage`, which the
  sandbox allows, rather than `proc_pid_rusage`; the profile is passed by name. Open until CI on
  macOS runs `compile-memory` and `compile-confined` green (T054).

  Done on 2026-11-13: CI on macOS runs every compile case green. Two fixes on the way: clippy's
  needless `return` in a branch compiled only there, now one confinement function per system; and
  `compile-memory`'s limit, 48 MiB on macOS, since the watchdog reads the peak every 50 ms and
  `compile-heavy`, peaking at 104 MiB, passed 96 MiB only near its end, which one run missed.
- [X] T041 [P] [US4] Windows confinement and limits in `runtime/src/compile/confine_windows.rs`
  (`windows-sys` as a direct Windows dependency): an AppContainer with no capabilities, read and
  execute granted on the app's install directory when it is per-user; a job object with the memory
  and process-time limits, kill-on-job-close, no child processes and no desktop access, the child
  assigned before the first byte is written; `BELOW_NORMAL_PRIORITY_CLASS`; attribution from the
  job's accounting. If AppContainer cannot start the binary, the restricted low-integrity fallback,
  and the residual recorded in research R13.

  Written on 2026-11-12 (`runtime/src/compile/confine_windows.rs`): the process created by hand,
  since `CommandExt::raw_attribute` is still unstable in Rust 1.100; it is checked with clippy for
  `x86_64-pc-windows-gnu` here, not run. The parent drives either kind of child through one
  `Running` trait. No fallback to a restricted token is written: if CI shows the AppContainer
  cannot start the binary, the fallback comes then. Open until CI on Windows runs `compile-cpu`,
  `compile-memory` and `compile-confined` green (T054).

  Done on 2026-11-13: CI on Windows runs every compile case green, the AppContainer starting the
  binary with the grant on the executable file alone, so no fallback was needed. What it took
  (research R2, "What Windows taught on CI"): the system's and the profile's folder variables in
  the child's environment (`ERROR_ENVVAR_NOT_FOUND` without them); one compile starting at a time,
  since two raced the AppContainer's profile (`ERROR_FILE_NOT_FOUND`); `STATUS_QUOTA_EXCEEDED` read
  as the CPU limit; `compile-memory` at 48 MiB, the job counting committed memory; and
  `compile-cpu` on a child that computes for 30 s once confined (`burn-cpu`), since the job checks
  its time only now and then. A throwaway branch with a Windows-only workflow found these in
  minutes, and was deleted.
- [X] T042 [US4] CI in `.github/workflows/ci.yml`: a step on the three systems compiles
  `compile-heavy` through the real child under the default limits and prints its wall time and peak
  memory as each system counts it; `compile-confined` runs in the test step. Record the numbers in
  research R2 and adjust a limit if a legitimate package comes within half of it.

  Done on 2026-11-12: the step runs `compile::measure_compile_heavy_under_the_default_limits`, an
  ignored test, alone. Here (Linux, 12 threads, 6 for the compile): 1.1 s and 106 MiB resident at
  most, against 20 s and 1.5 GiB. `compile-confined` is now the same test on the three systems: a
  file of the user's and a port on loopback, which every system's confinement refuses; turned off
  on Linux, the test breaks ("escaped: file true, network true"), which it did not before the CLI
  recognised the hook (the CLI's own check of the invocation had not been replaced in T008).

**Checkpoint**: a hostile package is refused by limit, and the compiler cannot reach the user's data.

---

## Phase 7: User Story 5 - Host dialogs cannot be imitated, hijacked or rushed (Priority: P2)

**Goal**: HS-10 to HS-15 and HS-17 to HS-23 on every host dialog.

**Independent Test**: spec US5; the probe; quickstart §6.

- [X] T043 [US5] Write the probe cases of research R11's table in `app/src/probe/dialogs.ts`, run
  with the showcase and a hostile widget on screen and a dialog opened from the probe release
  source; add the five commands to `foreign-actions-refused` in `app/src/probe/widgets.ts`; list the
  new case ids in `PAGE_CASES` (`app/src-tauri/src/probe.rs`). Each fails until T044. Also
  `window-responsive-while-compiling` (SC-003): the page starts installing `compile-heavy` from the
  probe source and records the gaps between `requestAnimationFrame` callbacks and between 10 ms
  timers until the dialog opens; the case fails if any gap exceeds 100 ms, and reports the largest.
- [X] T044 [US5] Complete `app/src/host/HostDialog.tsx` (research R8): `inert` on `#root` while open,
  removed in the step that closes; outside pointer events prevented; its own `keydown` handler for
  Enter and Escape, not filtering on `isTrusted`; the 1000 ms input delay restarted on window
  `focus` and on input during it, the approve button disabled meanwhile; `onCloseAutoFocus` to the
  recorded opener or the window; `aria-labelledby` and `aria-describedby`; fixed header and footer
  with a scrolling body; a `forced-colors:` border in `CanvasText`; animation only under
  `motion-safe:`. The window's close event sends `request_cancel`. T043 passes.
- [X] T045 [US5] On Windows, call `CallDevToolsProtocolMethod("Accessibility.getFullAXTree")` from
  the probe build with developer tools left off (`probe.rs` already reads WebView2's settings) and,
  when it answers, assert in the HS-11 and HS-22 cases that the tree outside the dialog is empty.
  Done either way after one attempt on the Windows CI runner: if the call fails or needs developer
  tools on, record that in research R11 and leave the check to quickstart §6.

  Done on 2026-11-13: the probe's `shell_probe_tree` answers, on the Windows runner with developer
  tools off, and `dialog-makes-window-inert` and `dialog-hides-the-rest` report that the engine's
  accessibility tree names nothing outside the dialog; on Linux and macOS they say it was not read.
  Not mutation-tested on Windows (one attempt, as planned): the page's own checks of those cases
  break without the inertness, and quickstart §6 still covers screen readers by hand.
- [X] T046 [US5] Run the probe locally on Linux: every case of features 008, 009 and this one holds.

  Done on 2026-11-12: 82 cases held in three runs. `window-responsive-while-compiling` counts the
  gaps until the dialog appears, not the interval in which the page draws it (119 ms with it,
  34 to 40 ms without); `compile-heavy` takes 0.6 s from the add button to the dialog here. With
  the dialog's inertness removed and its delay at zero, eight cases broke, among them
  `dialog-makes-window-inert`, `dialog-input-delay` and `dialog-hides-the-rest`. T045 is open: it
  is one attempt on the Windows CI runner, and the review step of quickstart §6 covers it until
  then.

**Checkpoint**: the dialog's form holds in the page; the review covers what untrusted events cannot.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [X] T047 Update `docs/threat-model.md` (research R12, FR-027): rows `:113`, `:152`, `:153`,
  `:155`, `:157`, `:158`, `:175` cite their tests; B6 (`:90`) and the diagram line (`:51`) state
  what this feature provides and what waits for Phase 6; new residual rows (a name trusted at first
  installation; an author account taken over shipping an update that asks for nothing new; no
  revocation yet; the compile child's output trusted as code; the host's requests to GitHub); the
  open questions at `:192-194` and `:205-213` closed. `pnpm --filter ./design check` passes.
- [X] T048 [P] Write `docs/decisions/0003-compile-installed-plugins-out-of-process.md` (research R1:
  compile out of process and confined now; running out of process in the next feature, before the
  alpha, with the reason) and add the dated note to `docs/decisions/0001-plugin-process-isolation.md`.
- [X] T049 [P] Write `docs/authors/guides/publishing.md` from `contracts/release.md`; update
  `docs/authors/reference/manifest.md`'s `id` row (now checked against the repository) and the
  limits reference; the docs reference test passes.
- [X] T050 [P] Update `CONTRIBUTING.md`: the security core list as constitution v2.7.0 and this
  feature's files; a short section on installation (the compile child, the fixtures, the probe
  release source, the confinement test on each system).
- [X] T051 Run every local check under the memory cap: `cargo xtask plugins`, clippy on the
  workspace and the probe build, `cargo test --workspace`, `cargo deny`, `cargo audit`,
  `pnpm audit`, the design checks and licenses, the docs reference test, and the probe on Linux.
- [X] T052 Walk quickstart §1 to §5 on Linux and fix any drift between it and the code. §3 needs a
  public test repository: ask the maintainer to create it, or walk it against the probe source and
  say so.

  Done on 2026-11-12 on Linux, without a public repository: §1 and §2 as T051 and T046 ran them
  (777 tests, 82 probe cases); §3 against the probe's source and the app's tests; §4's four breaks
  are tests (`an_address_that_is_not_a_repository_is_refused`,
  `a_changed_compiled_form_is_compiled_again_and_a_changed_package_refused`,
  `a_name_taken_by_another_repository_serves_no_update`); §5 by the probe, and the confinement by
  hand (`Seccomp: 2`, `NoNewPrivs: 1` in the compile process's status). Drift fixed: §3 and §5 asked
  to publish fixtures whose id is `github.com/dashboard-fixtures/...`, which a tester's repository
  cannot be; they now say how to make a copy with one's own id. T051 also found a comment naming
  the engine in `dialogs.ts`, which `webkitgtk_register` refuses.
- [X] T053 Review the security core independently: a fresh agent with none of this feature's
  context reads `runtime/src/{install,compile,flows}`, `network/`, `grants.rs`, the app's
  `install.rs`, `widgets.rs`, `main.rs`, commands and `window-surface.toml`, and `app/src/host/`,
  against the spec and contracts, and looks for any way to grant without the dialog, install from
  elsewhere, take over a name, escape the compile child, run a form the host did not produce, or
  rush or imitate the dialog. Check each finding at the source before changing anything; bring real
  trade-offs to the maintainer. Done: eight findings, all confirmed; seven fixed with a test that
  breaks without the fix, one (a fresh form deserialized before the decision) recorded as a
  residual in the threat model and decision 0003 until the next feature (research, "The
  implementation's independent review"). The Windows grant's change waits for CI with T041.
- [X] T054 Only once the maintainer has pushed and opened the pull request, as they ask: follow CI
  on the three systems and record per system the `compile-heavy` time and peak (research R2), the
  largest gap of `window-responsive-while-compiling` (SC-003), the time of `install-on-time`
  (SC-001, without the network) and the probe's time.

  Done on 2026-11-13, PR #23 green on the three systems at `e0c8935` (CI run 43424334857):

  | System | `compile-heavy` | Peak | Largest gap (SC-003, limit 100 ms) | `install-on-time` (SC-001) | Probe |
  | --- | --- | --- | --- | --- | --- |
  | Linux | 1675 ms, 2 threads | 105 MiB | 16 ms | 1335 ms | 82 held, 28 s |
  | macOS | 1507 ms, 1 thread | 105 MiB | 19 ms | 1389 ms | 82 held, 30 s |
  | Windows | 1354 ms, 2 threads | not read | 16 ms | 1344 ms | 86 held, 27 s |

  Over every CI run that reached it, `compile-heavy` took 0.8 to 2.3 s, under a tenth of the 20 s limit, and
  peaked at 105 MiB, under a tenth of 1.5 GiB: no limit moves. CI changed twice on the way: the
  compile cost step reuses the workspace tests' build, and the build job has 55 minutes, a slow
  Windows runner without the cache having passed 40. Two runs failed for causes outside this
  feature, each passing when run again: a download from crates.io on macOS, and feature 003's
  `hostile::connectors::log_flood` past its 1 s limit on a slow Windows runner.
- [X] T055 The maintainer's part, before the merge: the line-by-line review of the security core
  (Principle IX); quickstart §6 (keyboard, screen reader, high contrast, reduced motion) and §5's
  confinement check on Windows, macOS and Linux. Done: the maintainer reported the review done on
  2026-11-12.
- [X] T056 Before the merge, and only when the maintainer asks, update the development guide: tick
  the Phase 3 items "Diálogo de instalação com permissões em linguagem clara" and "Instalação de
  plugin a partir do repositório do autor", noting constitution v2.7.0 and decision 0003. Done at
  the maintainer's request on 2026-11-12 (guide rev 153), each item with what PR #23 did.
- [ ] T057 Merge:
  - The maintainer reviewed the pull request line by line.
  - CI is green on the three systems.
  - `grep -n '^- \[ \]' specs/010-plugin-installation/tasks.md` shows only this task.
  - Tick this box in the pull request's last commit, wait for green CI on that head, and merge
    with `gh pr merge --merge --match-head-commit <sha>`.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: none. T001 before any frame change (T026); T005 and T006 before any test
  that uses a fixture.
- **Foundational (Phase 2)**: after Phase 1.
  - T007 → T008 → T009 (T002 first).
  - T010 → T011 → T012.
  - T013 → T014; T015 → T016 (after T014); T017 → T018 (after T003, T016).
  - T019 after T009 and T014; T020 after T019 and T018; T021 after T020.
- **US1 (Phase 3)**: after Phase 2. T022 → T023 → T024; T025, T026 beside T024; T027 after T024;
  T028 last. The MVP.
- **US2 (Phase 4)**: after T024. T029 → T030; T031 after T025 and T030; T032 after T016.
- **US3 (Phase 5)**: after T023. T033 → T034 → T035 → T036.
- **US4 (Phase 6)**: after T008. T037 → T038; T039, T040, T041 in parallel after T038; T042 after
  them.
- **US5 (Phase 7)**: after T025 and T027. T043 → T044 → T045 → T046.
- **Polish (Phase 8)**: T047 to T050 after the stories; T051 to T053 after everything; T054 after
  the PR opens; T055 and T056 at the maintainer's hand; T057 last.

### Parallel Opportunities

- Phase 1: T001, T002, T003, T004 and T006 together; T005 beside them.
- Phase 2: the compile child (T007–T009), the network and source (T010–T012), the records
  (T013–T014) and the flows (T015–T016) in parallel; then the model (T017–T018) and the app
  (T019–T021).
- Stories: US4 (limits and confinement) can run beside US1 to US3 once T008 lands; T039, T040 and
  T041 are one file per system; page tasks (T025, T026, T031, T036) beside their Rust tasks.

### Parallel Example: User Story 4

```text
T039 Linux confinement in runtime/src/compile/confine_linux.rs
T040 macOS confinement in runtime/src/compile/confine_macos.rs
T041 Windows confinement in runtime/src/compile/confine_windows.rs
```

## Implementation Strategy

### MVP First (User Story 1)

1. Phases 1 and 2.
2. Phase 3: a widget installs from a repository through the dialog.
3. Stop and validate with quickstart §3 against the probe source.

The MVP is not shippable on its own: until US4 lands, the compile child has no limits or
confinement, so nothing is merged before every story is done.

### Incremental Delivery

1. US1, then US2 (what the dialog says) and US3 (updates), then US4 (limits and confinement), then
   US5 (the dialog's form).
2. Every story keeps features 008 and 009's probe green.
3. One commit per task; push only when the maintainer asks.
