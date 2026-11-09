---

description: "Task list for the host window, host dialogs and the rules that replace the webview's CSP (feature 008)"
---

# Tasks: The Host Window, Host Dialogs and the Rules That Replace the Webview's CSP

**Input**: Design documents from `specs/008-host-window-dialogs/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Required. Every security rule gets a hostile case (Principle VIII, FR-031) and every
criterion the feature owns (HS-01, HS-02, HS-10 to HS-12, HS-16 to HS-19, HS-24) an automated test
on the three systems (SC-002). Each test task comes before the code it checks and fails until that
code lands. The runtime's security suite must keep passing unchanged, apart from the one read-only
addition of R5.

**Organization**: a CI spike first, because research R15 lists what nobody has confirmed (that
gpui-unofficial 1.25.3 builds, that runners draw a window and take real keys, the clippy paths, the
MSRV, the licenses). Then the crate and the pieces every story needs, then the stories in priority
order: the window (US1), the panel (US2), the dialog's form with "Remove from dashboard" (US3),
keyboard and screen readers (US4), the CSP rules (US5), the appearance settings (US7), GPUI upgrades
(US6).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1–US7)

## Path Conventions

- **Host:** `host/` (library `host`, binary `dashboard`); tests in `host/tests/`.
- **Runtime:** `runtime/src/grants.rs` (one read-only addition).
- **Fixtures:** `plugins/hostile/`.
- **Bans:** `clippy.toml`, `deny.toml` at the root.
- **CI:** `.github/workflows/ci.yml`.

Where the details live: decisions R1–R16 in `research.md`; the binary, its files and runtime calls in
`contracts/host-binary.md`; the dialog in `contracts/dialog.md`; bans, checks and hostile cases in
`contracts/bans-and-checks.md`; entities in `data-model.md`.

Long builds and test runs go under the memory cap:
`systemd-run --user --scope -p MemoryMax=8G <command>`.

---

## Phase 1: Setup and the CI spike

**Purpose**: settle R15 before anything is built on it. Nothing in later phases starts until T004 is
done and the maintainer has seen its result.

- [X] T001 Create `host/` as a workspace member (root `Cargo.toml` `members`), library `host` and
  binary `dashboard`, with the nineteen gpui-unofficial crates at `=1.25.3` exactly as research R1
  lists them (platform crates under `[target.'cfg(...)'.dependencies]`), `runtime`,
  `design-tokens`, `host-surfaces` and `serde_json`; `host/src/main.rs` opens one empty window and
  quits on close. `host/README.md` records the pin: version 1.25.3, Zed tag `v1.25.3` (commit
  `8dd255c3eafb5231056fe4e6f601cedd011f1c93`), patch list `docs/upstream-patches.md` at commit
  `58b5a3814e`, and the upgrade rule of research R1.
- [X] T002 Build `host` on Linux under the memory cap with the workspace's MSRV and with stable;
  run `cargo deny check` and `cargo audit`. Record in research R15: whether 1.25.3 builds as
  published, the MSRV that builds (if 1.97 fails, the oldest stable that builds, brought to the
  maintainer before the workspace's MSRV moves), and each license `cargo deny check licenses`
  refuses.
- [X] T003 Spike in CI on a throwaway branch `008-spike` (never merged; pushed only once the
  maintainer agrees, and deleted after T004): a job on `ubuntu-24.04`,
  `macos-latest` and `windows-latest` that builds `host`, opens its window (Linux: inside
  `dbus-run-session`, with `xvfb`, `mesa-vulkan-drivers`, `libegl1`, `libgl1-mesa-dri`,
  `at-spi2-core` and GPUI's build libraries; the AT-SPI bus started and `IsEnabled` set as research
  R13 says), sends one Tab with `enigo` 0.6.1, reads one node of the window's accessibility tree
  through the platform (AT-SPI, UI Automation, AX), and runs clippy on a file that calls every
  entry of `contracts/bans-and-checks.md`'s clippy list, expecting each to be refused. Also,
  headless on each system: open a window of GPUI's test platform at 800 × 600 with scale factors
  1, 1.5 and 2, and read an element's bounds at each.
- [X] T004 Record T003's outcome in research R15, per system: the window drew or not, the key
  arrived or not, the tree was read or not, each clippy path caught or corrected. If macOS cannot
  draw or take keys, bring it to the maintainer with the options before going on (research R15,
  item 2). Record whether the test platform honours the scale factor; if it does not, T017 and T024
  run their scale-factor cases in `host/tests/window.rs` instead, and research R13 says so.

---

## Phase 2: Foundational (blocking every story)

**Purpose**: the crate's modules and checks every story uses.

### Tests first

- [X] T005 [P] `host/tests/checks.rs`: the checks of `contracts/bans-and-checks.md` (GPUI pins
  against `Cargo.lock` and `host/README.md`; every clippy entry present; no
  `allow(clippy::disallowed_*)` outside `host/src/media.rs`; no `deferred(` or `anchored(`; no
  `development(`; the host-words scan over `child`, `aria_label`, `aria_description`, tooltips,
  `TitlebarOptions` and text elements outside `host/src/text.rs`; `deny.toml` keeps `host-surfaces`
  wrapped by `host` and bans `rfd` and `native-dialog`).
- [X] T006 [P] `host/tests/logic.rs` part 1: `label` follows HS-06 and HS-07 exactly (research R8):
  Cc, Cf, Zl, Zp removed; 40 grapheme clusters and an ellipsis; "Unnamed widget"; host-imitating
  names unchanged; the monogram's first letter or digit, upper-cased, or the host glyph, for names
  starting with an emoji, a digit, a symbol and a non-Latin letter.
- [X] T007 [P] `runtime/src/grants.rs` tests: `GrantStore::approvals()` lists every approval's id and
  origin, reads nothing else into the result, changes no file, and grants nothing to a package from
  another origin.

### Implementation

- [X] T008 `runtime/src/grants.rs`: `GrantStore::approvals()`, read-only, under the record's lock as
  `entry` reads (research R5). Security core: the PR marks it for line-by-line review (Principle
  IX).
- [X] T009 [P] `clippy.toml`: the entries of `contracts/bans-and-checks.md` with the paths T004
  confirmed, each with its reason; `deny.toml`: each license T002 found, with a comment naming the
  crates that bring it (both security core).
- [X] T010 [P] `host/src/text.rs`: `Msg`, `HostText`, the English catalog (data-model "Host text").
- [X] T011 [P] `host/src/label.rs`: `PluginText`, the label and monogram rules (T006).
- [X] T012 [P] `host/src/theme.rs`: `design-tokens` and `host-surfaces` values to GPUI colors, sizes,
  radii and lines for an `Appearance` and theme; nothing else in `host` names a color or size.
- [X] T013 [P] `host/src/media.rs`: the only module allowed to call `gpui::img` and `gpui::svg`, with
  an inner `#[allow]` and its reason; `image_from_bytes(Arc<gpui::Image>)` and the three status icons
  from `DESIGN.md`'s embedded paths (research R12).
- [X] T014 `host/src/app.rs`: fonts from `design/fonts/` with `include_bytes!` (research R4), the app
  built with GPUI's `BlockedHttpClient`, GPUI's prompt builder replaced by one that refuses at once
  (research R9, R12); `host/src/main.rs` parses `--grants` and `--data` (no `--connectors`), and exits
  with the codes of `contracts/host-binary.md`, code 3 with a message when no GPU driver is usable
  (FR-013).
- [X] T015 `host/src/vendor/gpui_base/`: the focus trap adapted from gpui-base 0.9.0's
  `src/focus_trap.rs` and the Tab handling of its `src/root.rs`, with the Apache-2.0 notice and a
  `PROVENANCE.md` (crate, version, files, SHA-256, what changed); its test ported to `host`'s test
  setup (research R9).

**Checkpoint**: `cargo test -p host`, `cargo test -p runtime` and `cargo clippy --workspace` pass.

---

## Phase 3: User Story 1 - The dashboard opens as a native window (Priority: P1) 🎯 MVP

**Goal**: one window, the chrome, one frame per approved plugin with its label, monogram and
status, focus ready for the first Tab.

**Independent Test**: spec US1.

### Tests first

- [X] T016 [P] [US1] `host/tests/logic.rs` part 2: a window with two approved fixtures (one renders,
  one traps) shows two frames with label, monogram and status ("Rendered" in the ok tone; "Did not
  render" with the host's words for the error kind in the error tone); nothing of what `render`
  returned is in the frame's elements; a third failing plugin does not stop the others (FR-007 to
  FR-009); the window's root is focused on open (FR-005); reload on window activation and from the
  chrome adds and removes frames (FR-040); waiting while a dialog is open is tested with the
  dialog, in T026.
- [X] T017 [P] [US1] `host/tests/logic.rs` part 3: HS-01: no frame intersects the chrome at 800 × 600,
  1280 × 800 and 2560 × 1440, scale factors 1, 1.5 and 2, with the content scrolled so frames sit
  past the top, bottom and sides (FR-011, research R6).
- [X] T018 [P] [US1] `host/tests/hostile.rs` (`content`): the fixtures of T019 whose output is markup and
  addresses, whose names imitate the host and whose render error imitates host text: the frame holds
  only host text and the safe label; no request, no file read (FR-008, FR-027, FR-029).
- [X] T019 [P] [US1] `plugins/hostile/`: fixtures `window-addresses` (render output of markup,
  `https://`, `file://` and paths), `window-names` (long, empty, bidi, control, mixed-script and
  host-imitating names), `window-error-imitation` (a render error and names reading "Rendered",
  "Remove a widget") and `window-local-connector` (declares a `local/` connector); built by
  `cargo xtask plugins`.
  Done without `window-names`: a plugin's only name is its id, which the runtime limits to
  lowercase ASCII (`PluginId::parse`), so no package can carry bidi or control characters in it;
  the label rules stay tested on arbitrary strings (T006), and the frame shows the id's last
  segment (research R8). `window-local-connector` comes with US5 (T040).

### Implementation

- [X] T020 [US1] `host/src/plugins.rs`: approvals through `GrantStore::approvals()`, each package read
  capped at the package size limit, loaded and rendered on the background executor with the runtime
  started without development mode (research R5); each outcome mapped to `FrameStatus`, failures
  by `ErrorKind` to a `Msg`; compile time and peak memory measured per load (research R14).
- [X] T021 [US1] `host/src/chrome.rs`: the chrome band from `theme`, with "Reload" and the entries
  later stories add.
- [X] T022 [US1] `host/src/frame.rs`: the header (monogram, label, menu button), the status body with
  the status icon from `media`; frames in a wrap of fixed-width columns inside a scrolling content
  area that clips everything it holds, with 4 px inside its edges (research R6).
- [X] T023 [US1] `host/src/lib.rs` and `host/src/main.rs`: the root view (chrome above the content
  area), the empty state in host words when nothing is approved, reload on activation and from the
  chrome (FR-040).

**Checkpoint**: quickstart §1 and §2 work; T016–T018 pass.

---

## Phase 4: User Story 2 - A frame opens its panel (Priority: P1)

**Goal**: an attached panel, under the chrome, closing on Escape and on a click outside.

### Tests first

- [X] T024 [P] [US2] `host/tests/logic.rs` part 4: the panel opens attached to its frame, flips when
  there is no room, never intersects the chrome at the sizes, scales and scroll positions of T017;
  the rest of the window takes input while it is open; Escape and a click outside close it, the
  click still reaches its target, and focus returns to the frame's control (FR-010, FR-011).

### Implementation

- [X] T025 [US2] `host/src/panel.rs`: the panel inside the content area, host panel tokens, no
  floating layer; the frame's menu button opens a menu drawn the same way (research R6).

---

## Phase 5: User Story 3 - A host dialog takes the form no widget can produce (Priority: P1)

**Goal**: the dialog component, its gate and "Remove from dashboard".

### Tests first

- [X] T026 [P] [US3] `host/tests/logic.rs` part 5: the dialog's form, every row of
  `contracts/dialog.md` that needs no real window: the scrim over every host window, chrome
  included, in the element tree (HS-02); a reload asked while it is open waits for its answer (FR-040); input to the chrome, frames, a panel and a second window
  reaches nothing; a click outside neither closes nor answers; other windows drop keys and lose
  focus, restored after; the close request and quit answer `Refused` (HS-10); the queue shows one
  at a time (FR-018).
- [X] T027 [P] [US3] `host/tests/hostile.rs` (`gate`): with the manual clock, input at 0, 500 and 999 ms
  answers nothing and at 1000 ms does; a key and a button held from before answer nothing until
  released; the 1000 ms restart on activation, content change and a new dialog (HS-11, FR-017).
- [X] T028 [P] [US3] `host/tests/logic.rs` part 6: "Remove from dashboard": confirmed, the approval
  and kept data go (as `runtime remove`) and the frame leaves; refused, closed or quit, nothing
  changes; focus returns to the menu button (FR-038); if the tool removed the plugin meanwhile,
  confirming removes nothing more.

### Implementation

- [X] T029 [US3] `host/src/dialog/`: `DialogHost`, `Request`, `Answer`, the queue, the modal layer in
  every window's root view, other windows dropping input and clearing focus, close and quit
  answering `Refused` (research R9, `contracts/dialog.md`).
- [X] T030 [US3] `host/src/dialog/gate.rs`: `Clock`, the input gate (data-model "Input gate").
- [X] T031 [US3] `host/src/frame.rs`: "Remove from dashboard" in the frame's menu, asking through
  `DialogHost` with the texts of `contracts/dialog.md`, then `GrantStore::remove` and
  `Runtime::discard_kept_data` on the background executor.

**Checkpoint**: quickstart §4 works.

---

## Phase 6: User Story 4 - Keyboard and screen-reader users answer safely (Priority: P2)

**Goal**: HS-16 to HS-19 and HS-24, proved in a real window on the three systems.

### Tests first

- [X] T032 [P] [US4] `host/tests/logic.rs` part 7: initial focus on the refusing control, no default
  button, Enter there never confirms (HS-16); Tab and Shift+Tab cycle inside; Escape refuses and
  restores focus (HS-17); every focusable host control shows the `focus` ring 2 px wide, 2 px out
  (HS-24).
- [ ] T033 [US4] `host/tests/window.rs` (`harness = false`, `test = false` in `host/Cargo.toml`):
  the real window with real OS input through `enigo`: the first Tab; the panel's Escape and click
  outside; the dialog opened from the keyboard and from the pointer; initial focus, Tab, Shift+Tab,
  Escape; Enter in a second window after 1000 ms answering nothing; the gate in real time; every
  focusable control's ring unclipped; the pixels under the scrim in every window (FR-015); the
  start and dialog times (SC-001, SC-006).
- [ ] T034 [US4] `host/tests/window.rs`, accessibility part: with the adapter active, the platform
  tree (AT-SPI, UI Automation, AX) and `debug_a11y_tree_json` through `accesskit_consumer`'s
  `common_filter`, in the dialog's window and in a second window: only the dialog and its children
  while it is open (HS-18, FR-022); its role, label, description and the subject's child node
  (HS-19, FR-023); each frame's label and status under its frame (FR-024).

### Implementation

- [X] T035 [US4] `host/src/a11y.rs`: the element that writes `set_hidden` on each window's background
  root (role `Pane`) and `set_modal` with role `Dialog`, label and description on the dialog
  (research R10); frames expose their label and status as the widget's content.
- [X] T036 [US4] `host/src/dialog/`: initial focus, no default button, the vendored focus trap,
  Escape and focus restoration (research R9).
- [ ] T037 [US4] `docs/accessibility/screen-reader-walk.md`: the release walk script for VoiceOver,
  Narrator and Orca over every host dialog, what to record and where (FR-026).

---

## Phase 7: User Story 5 - Nothing a plugin supplies makes the window fetch, open or decode (Priority: P2)

**Goal**: each CSP rule with its hostile case (FR-027 to FR-031).

### Tests first

- [ ] T038 [P] [US5] `host/tests/hostile.rs` (`network`): the app's HTTP client refuses a request; an
  image element from a URI string, drawn in a test harness with a recording client, sends nothing;
  an SVG from an external path reads no file (FR-028 to FR-030).
- [ ] T039 [P] [US5] `host/tests/hostile.rs` (`prompts`): GPUI's prompt builder is the host's and
  refuses at once (FR-019). `host/ban-fixture/`, a crate outside the workspace (root `Cargo.toml`
  `exclude`, like `design/ban-fixture`), with the workspace's `clippy.toml` and the same GPUI pins,
  holds one function per banned method of `contracts/bans-and-checks.md`; CI runs
  `cargo clippy --manifest-path host/ban-fixture/Cargo.toml -- -D warnings` and requires it to fail
  with one `disallowed_methods` error per function, each named (FR-019, FR-029).
- [ ] T040 [P] [US5] `host/tests/hostile.rs` (`content`) addition: `window-local-connector` does not load
  and its frame says why in host words (FR-041).

### Implementation

- [ ] T041 [US5] Close whatever T038–T040 find open in `app.rs`, `media.rs` and `plugins.rs`; no new
  path from a plugin string to GPUI's loaders.

---

## Phase 8: User Story 7 - The person picks the appearance (Priority: P2)

**Goal**: the appearance settings from the chrome (FR-034 to FR-037).

### Tests first

- [ ] T042 [P] [US7] `host/tests/logic.rs` part 8: the settings list every gray family, host color,
  widget color, radius and theme with the current one marked; a valid pick redraws every surface;
  a widget color within 30° is refused naming the colors and leaves the previous choice; the
  choice is saved and read back; an unreadable or invalid file falls back to the default with a
  notice; with the system theme, a theme switch redraws without restart.

### Implementation

- [ ] T043 [US7] `host/src/appearance.rs`: the settings screen inside the content area, opened from
  the chrome; `appearance.json` in `<data dir>/dev.philsantos.dashboard/host/` (research R11).

---

## Phase 9: User Story 6 - The maintainer can update GPUI safely (Priority: P3)

- [ ] T044 [P] [US6] `host/tests/checks.rs`: an unpinned, caret or `-pre` GPUI crate, or one that
  differs from `host/README.md`, fails the pin check (extends T005 with the mutations).
- [ ] T045 [US6] `CONTRIBUTING.md`: how to upgrade GPUI: at most monthly or for an advisory, read the
  patch list's diff since the recorded commit, move all nineteen pins together, update
  `host/README.md`, and let the "Host window" job run on the three systems.

---

## Phase 10: Polish & Cross-Cutting Concerns

- [ ] T046 `.github/workflows/ci.yml`: the "Host window" job on the three systems (Linux packages,
  `dbus-run-session`, the AT-SPI bus, Xvfb, `WAYLAND_DISPLAY` unset), running
  `cargo test -p host --test window` with its own timeout; the build job's apt list gains GPUI's
  build libraries; `host/` already counts as code for the changes filter. The "Host window" job also
  runs the `host/ban-fixture` clippy step of T039, expecting the failure, as the deny job does for
  `design/ban-fixture`.
- [ ] T047 [P] `docs/decisions/0001-plugin-process-isolation.md`: the revisit section of research R2
  (four triggers kept, the compile-out option weighed, the stale line about local files corrected,
  next owners named).
- [ ] T048 [P] `docs/threat-model.md` and `docs/design/host-surfaces.md`: research R16 (the decoder
  row's mitigation, the process question, the "Decided elsewhere" owners), the row's first cell
  unchanged; `design-gen`'s criteria test still passes.
- [ ] T049 [P] `specs/008-host-window-dialogs/performance.md`: the start, dialog, compile and memory
  numbers T033 and T020 measured on the three systems and the reference machine (SC-001, SC-006).
- [ ] T050 Every local check: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets
  --locked -- -D warnings`, `cargo test --workspace --locked` (on the maintainer's machine the
  tests that write to the real keychain wait on its password prompt; they are skipped there and
  checked in CI, as in feature 007's T047),
  `cargo test -p host --test window` under Xvfb as quickstart §6, `cargo xtask design --check`,
  `cargo deny check`, `cargo audit`; `git diff master -- runtime/` shows only `approvals()` and its
  tests, and the runtime's security suite passes unchanged.
- [ ] T051 Walk `quickstart.md` §1–§8 and fix any drift between it, the contracts and the code.
- [ ] T052 Independent review: a fresh agent reads `host/src/dialog/`, `a11y.rs`, `media.rs`,
  `app.rs`, `plugins.rs`, the `approvals()` hunk, `clippy.toml` and `deny.toml` against constitution
  v5.0.1, the criteria and the threat model; check each finding at the source; record findings and
  outcomes at the end of `research.md` under "Review of the implementation".
- [ ] T053 Once the maintainer has pushed and the pull request's run is green on the three systems,
  including "Host window", bring them the screenshots of the window and the dialog for approval of
  the look; changes go in as new commits. The maintainer also runs
  `docs/accessibility/screen-reader-walk.md` once with Orca on Linux; record the result in that
  file's log (SC-007), and fix what it finds as new commits.
- [ ] T054 Before the merge, and only when the maintainer asks: mark the development guide's Phase 3
  items "Host em GPUI", "Acessibilidade dos diálogos", "Regras que substituem a CSP da webview" and
  "Diálogos do host desenhados só com os componentes do próprio host" as done.
- [ ] T055 Human review (Principle IX) and merge:
  - The maintainer reviews every commit line by line: `approvals()`, `clippy.toml` and `deny.toml`
    with the care of the security core, and the dialog, a11y, media and app modules.
  - The maintainer ticks the pull request's review box with their handle.
  - Tick this task's box in the pull request's last commit; check
    `grep -n '^- \[ \]' specs/008-host-window-dialogs/tasks.md` shows nothing on that head.
  - CI is green on the three systems on that head.
  - Merge with `gh pr merge <number> --merge --match-head-commit <sha> --author-email
    8193616+philipesantos@users.noreply.github.com`.

---

## Dependencies & Execution Order

- **Setup (Phase 1)**: T001 → T002 → T003 → T004; nothing else starts before T004 and the
  maintainer has seen it.
- **Foundational (Phase 2)**: after T004; tests T005–T007 first; T008 after T007; T009 after T004;
  T010–T013 in parallel; T014 after T010 and T013; T015 after T001.
- **US1 (Phase 3)**: after Phase 2; T019 before T018; T020–T023 after their tests. MVP.
- **US2 (Phase 4)**: after T022.
- **US3 (Phase 5)**: after T023 and T015.
- **US4 (Phase 6)**: after T029; T033 and T034 need T046's job to run in CI, and run locally first.
- **US5 (Phase 7)**: after T014 and T020.
- **US7 (Phase 8)**: after T021.
- **US6 (Phase 9)**: after T005.
- **Polish (Phase 10)**: T046 can start after T004; T047–T049 after the stories; T050–T052 after
  everything else; T053 needs the pushed branch; T054 only on request; T055 last.

### Parallel examples

- Phase 2 tests: T005, T006 and T007 together; then T010, T011, T012 and T013 together.
- US1 tests: T016, T017 and T018 (with T019) together.
- US5 tests: T038, T039 and T040 together.

## Implementation Strategy

1. The spike (Phase 1) decides whether the plan stands as written; a red result goes to the
   maintainer, not around them.
2. MVP: Phases 2 and 3, the window with its frames, checked on the three systems.
3. Then the panel, the dialog and its accessibility (US2–US4), which make the window usable for
   every later Phase 3 feature; then the CSP cases, the appearance settings and the upgrade path.
4. Commit per task on the feature branch; push only when the maintainer asks.
