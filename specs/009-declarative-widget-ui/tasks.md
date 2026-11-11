---

description: "Task list for the declarative widget UI and the host frame (feature 009)"
---

# Tasks: Declarative Widget UI and the Host Frame

**Input**: Design documents from `specs/009-declarative-widget-ui/`

**Prerequisites**: plan.md, spec.md, research.md (R1–R16), data-model.md, contracts/,
quickstart.md

**Tests**: Required (FR-020, Principle VIII). Every rule gets its test or probe case before the
code that enforces it, and that test fails until the code lands. Research R13 lists the tests
below the page, R14 the probe cases in the page.

**Security core** (constitution v2.6.3, Principle IX): `wit/`, the runtime's grants, phases,
vault and network changes, `app/src-tauri/` (`widgets.rs`, `name.rs`, `commands.rs`,
`capabilities/`, `window-surface.toml`, `build.rs`), `widget-content/` and
`wit/content.schema.json`, `app/src/widgets/` and `app/src/probe/`, and the probe step in `ci.yml`. The pull request gets line-by-line human review.

**Heavy builds**: run `cargo build` and `cargo test` locally inside
`systemd-run --user --scope -p MemoryMax=8G -p MemorySwapMax=1G`; run `cargo xtask plugins`
before any build of the app (research R1).

**Organization**:
- **Setup:** what must move before the contract changes.
- **Foundation:** contract 1.4.0, the manifest's new fields, actions in the runtime, the content
  crate, and the app's widget host with its commands.
- **The stories:** US1 the showcase in its frame; US2 the format for authors and invalid content;
  US3 content stays in its frame; US4 controls; US5 status; US6 the window's surface.
- **Polish:** the threat model, decision 0001, CONTRIBUTING, checks, walks, review and merge.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1–US6)

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: clear the way for contract 1.4.0 and for the app embedding plugins.

- [X] T001 Repoint the plugins that build against `wit/v1` (1.3.0) to `wit/history/1.3.0`:
  `plugins/examples/private-notes/src/lib.rs:6`, `plugins/fixtures/account-probe/src/lib.rs:10`,
  `plugins/hostile/token-hunt/src/lib.rs:9`. Change every use of 1.4.0 as "newer than the host"
  to 1.5.0: `runtime/src/permissions.rs` 695, 976, 1011, 1102-1109, 1453;
  `runtime/tests/contract.rs:252-258`; `docs/authors/getting-started.md:254-274` (the `sed` and
  its expected message). Run `cargo xtask plugins` and `cargo test -p runtime --test it` (research
  R7).
- [X] T002 [P] Put the runtime's CLI behind a `cli` feature in `runtime/Cargo.toml`, enabled by
  the `runtime` binary and the tests that need it, not by default for library users (research
  R3). `cargo build -p runtime --locked` and `cargo test -p runtime --test it` still pass.
- [X] T003 [P] Change criterion HS-02's check in `docs/design/host-surfaces.md` first, as feature
  007's rule allows (research R6): "An SVG icon, or any image that is not a PNG within the bounds,
  is refused when the plugin loads, and the frame draws the host's monogram; nothing of it runs or
  is fetched." Run `pnpm --filter ./design check`.
- [X] T004 [P] Move `cargo xtask plugins --locked` above the probe build in
  `.github/workflows/ci.yml` (today `:157` builds the app before `:179` builds the plugins), and
  list it first in `CONTRIBUTING.md`'s commands (research R1).

**Checkpoint**: everything builds and tests as before, with nothing left on `wit/v1` 1.3.0.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the contract, the manifest, the runtime's actions, the content crate and the app's
widget host, before any story draws.

**⚠️ CRITICAL**: no story can begin until this phase is complete.

### Contract 1.4.0

- [X] T005 Write the 1.4.0 tests first in `runtime/tests/contract_v1.rs`: a new only-adds test,
  1.3.0 (`history/1.3.0`) against `wit/v1`, asserting `actor.act` and the six `interactive-*`
  worlds exist; the 1.3.0 pair test moves to history against history; `actor` joins the skip list
  of the not-granted check (`:11-12,53`); a SHA-256 pin for `history/1.4.0` in
  `frozen_contracts`. They fail until T006.
- [X] T006 Write contract 1.4.0 in `wit/v1/widget.wit` (contracts/actions.md):
  - first correct the doc comments 1.4.0 makes untrue: "Calls work only while the host runs
    `update`" (`:36`), storage writes "`update` only", and render's document "for a plugin that
    also exports `updater`" (research R7);
  - raise the package to 1.4.0; add `interface actor` with `act`, documented as in the contract;
    add the six worlds, each the imports of its 1.3.0 counterpart plus `export actor`;
  - copy it to `wit/history/1.4.0/widget.wit`, pin its SHA-256, and update `wit/README.md` (1.4.0,
    the six worlds, the world the host binds).
  T005 passes.

### The manifest's new fields

- [X] T007 [P] Write the manifest tests first in `runtime/tests/manifest_schema.rs`: agreement
  cases for `name` (1 to 60 characters, no control characters), `icon` (base64) and `actions`
  (up to 16, unique, `^[a-z][a-z0-9-]{0,31}$`); host-only cases in
  `the_host_also_refuses_what_a_schema_cannot_see`: an SVG, a JPEG, a PNG over 8 KiB, not square,
  under 16 or over 64 px, animated, malformed; actions declared without `actor`, `actor` without
  actions (contracts/actions.md). They fail until T008.
- [X] T008 Add `name`, `icon` and `actions` to `wit/manifest.schema.json` and to `RawManifest` in
  `runtime/src/manifest.rs` (research R6, R8). Decode the icon at parse time with `png` 0.18.1
  (already in the lock), keeping RGBA pixels for `Manifest::icon_rgba()`; refuse anything else
  with `invalid-manifest`. The check of `actions` against the `actor` export at load
  needs the runtime to know `actor`, so it lands in T010 (`runtime/src/permissions.rs`, as
  `updater` is checked for connectors at `:358-364`). T007 passes.

### Actions in the runtime

- [X] T009 [P] Write the action tests first, `runtime/tests/actions.rs` (listed in `tests/it.rs`),
  with two new packages: `plugins/fixtures/action-probe` (`interactive-connected-stored-widget`,
  1.4.0: actions that write the store, call a read operation, and fail) and
  `plugins/fixtures/action-steps` (configurable like `store-probe`: store writes, a read and a
  change operation, then ok, an error, a trap or a loop; a fixture rather than a hostile crate,
  since `hostile/` holds one broken rule per crate). `action-probe` is the actor-only
  `interactive-widget`, and `action-steps` the `interactive-connected-stored-widget`.
  Cases (research R13):
  - an undeclared name is refused with `undeclared-action` before instantiation;
  - a 1.3.0 plugin has no actions;
  - the store is written all or nothing; a read operation works; a change operation is refused
    with `change-needs-user-action`; connectors stay refused in render;
  - time, fuel, memory and output limits as for `update`;
  - the action's data reaches the next render and is never written to `widget-data/`; the newer
    of update data and action data wins; an actor-only plugin sees its data.
  They fail until T010.
- [X] T010 Implement actions in the runtime (research R7, R8):
  - `V1_VERSION` 1.4.0 and `ACTIONS_SINCE` in `permissions.rs:27-35`; `renderer@1.4.0` (`:61`);
    the `actor` export found like `updater` (`:182-196`);
  - host bindings on `interactive-account-stored-widget` (`lib.rs:78-85`); `Prepared`/`V1Pre`
    gain `act` (`:471-493`, `:1332-1349`);
  - `Phase::Action`, and the phase checks become `Phase::may_call_connectors()` and
    `Phase::writes_storage()` at every site (`calls.rs:464,563`; `lib.rs:1415,1462,1549,1562,1581`;
    `storage/mod.rs:517,597,703,716,775`);
  - `Plugin::act(&self, action, input) -> ActionReport`, refusing an undeclared name first with the
    new `ErrorKind::UndeclaredAction` (`error.rs`, `ErrorKind::ALL`);
  - the last action's data kept in the `Plugin` in memory, and render's document chosen on
    `updater()` or `actor()` (`lib.rs:1392`), with the newer data.
  T009 passes; `cargo test -p runtime --test it` passes.
- [X] T011 [P] Write, then implement, the app vault and the secret check (research R3, R11):
  - `RuntimeOptions::vault_service(&'static str)` used by `Vault::new` (`vault/mod.rs:168,205-210`);
  - `Runtime::holds_secret_in(&self, text) -> Result<bool, RuntimeError>`: every connected
    account's secrets, in the encodings of `network/mod.rs:245-286` made a shared function;
  - tests in `runtime/tests/accounts.rs`: each secret in each encoding is found in a form value;
    an unconnected vault touches no keychain; `test_vault_fail_next` makes it an error.

### The content crate

- [X] T012 [P] Create the workspace crate `widget-content/` and write its tests first:
  - `tests/schema.rs`: compile `wit/content.schema.json` with `boon` and run the corpus of
    contracts/content-format.md: schema-valid ⇒ accepted with no log entry; host-refused ⇒
    schema-invalid, except the listed host-only rules (500 components, 64 KiB, row length, labels
    per value); the lenient cases (unknown fields, tones, types) listed apart;
  - unit tests: every bound of research R5, one over each; HS-09 (style fields draw as without
    them, unknown tone → `default` and one log entry); HS-08 (every status tone carries its icon
    from `design/widget-map.generated.json`); HS-06 (no password, masked or credential field kind,
    name or hint survives); an unknown `type` → placeholder and one log entry.
  They fail until T013.
- [X] T013 Write `wit/content.schema.json` and implement `widget-content` (`parse(&Value) ->
  Result<(Content, Vec<LogEntry>), ContentError>`), with the bounds as constants and the tones
  and status icons read from the widget map at build time (research R4, R5). T012 passes.
- [X] T014 [P] Teach the CLI (research R4, contracts/actions.md): `runtime run` reports the
  content check (drawn, log entries, or the error with its path); new `runtime act <package>
  <action> [--value id=text]…` with the development record. Update `cli::USAGE`; `docs_reference.rs` then asks for its doc,
  written in T023.

### The app's widget host

- [X] T015 Write the app's widget tests first, `app/src-tauri/tests/widgets.rs` (research R13):
  - the app's `RuntimeOptions`: no path or vault service equals the development one, development
    mode off; a scan of `app/src-tauri/src` finds no `default_development_path` or
    `default_development_data_dir`;
  - a release build embeds exactly `showcase`; with `shell-probe`, the seven `ui-*` fixtures too;
  - a bundled package declaring a permission, a connector or storage is refused, one case each;
  - `widget_render` refuses a handle not shown or malformed (`unknown-widget`) and a widget whose
    action is running (`widget-busy`);
  - `widget_act` refuses, in order and without calling the plugin: a handle not shown or
    malformed (`unknown-widget`), a busy widget (`widget-busy`), an undeclared action, bad values
    (`bad-values`: not strings, over 20 entries, over 500 characters), a value holding a held
    secret (`secret-in-form`, using a mock vault);
  - the drawn name (`name.rs`): markup, bidi overrides, format characters, 60 characters,
    nothing left → the id's last part (HS-02).
  They fail until T016.
- [X] T016 Build the widget host (research R1, R2, R9, R12; contracts/window-commands.md):
  - `app/src-tauri/build.rs`: embed `target/plugins/showcase.wasm`, and with `shell-probe` the
    `ui-*` fixtures; fail with "run `cargo xtask plugins` first" when one is missing; one
    `rerun-if-changed` per package; write the embedded list for T015;
  - `app/src-tauri/Cargo.toml`: depend on `runtime` (without its `cli` feature) and
    `widget-content`;
  - `src/widgets.rs`: the runtime at `<app data dir>/host` with the app vault service and
    development off; the in-memory approvals of bundled packages that declare nothing; handles;
    one lock per widget; the per-widget log of 100 entries; render and act on Tauri's blocking
    pool; a probe hook counting actions run (`probe::Refusal`'s sibling counter `actions`);
  - `src/name.rs`; `src/commands.rs`: `widgets_list`, `widget_render`, `widget_act`;
    `capabilities/default.json`: their three `allow-` permissions; `window-surface.toml`: the
    three permissions and three commands with the reasons of contracts/window-commands.md.
  T015 and `tests/surface.rs` pass.
- [X] T017 Measure the probe's release build now that it links Wasmtime under LTO: time and peak
  memory, inside the memory cap (research R1). Record them in research R1, and raise the build
  job's timeout in `ci.yml` only if the Windows estimate needs it.

**Checkpoint**: the runtime runs actions, the content crate checks content, and the app serves
widgets through three reviewed commands; nothing is drawn yet.

---

## Phase 3: User Story 1 - The person sees a real widget on the dashboard (Priority: P1) 🎯 MVP

**Goal**: the showcase widget in its frame, every component drawn, in both themes.

**Independent Test**: quickstart §1.

- [X] T018 [US1] Write `plugins/examples/showcase` (`local/showcase`, world `interactive-widget`
  of 1.4.0, no permission): manifest `name` "Vitrine", a 32×32 PNG `icon`, actions `increment`
  and `add`; render shows every component, tone and variant (a heading, the three status texts,
  a list, a table, a bar and a line chart, every button variant, a form with each field kind,
  a stack); `increment` counts, `add` appends the form's title to a list, both in the data they
  return. `cargo xtask plugins` builds it; `runtime run` and `runtime act` on it pass.
- [X] T019 [US1] Draw widgets in the page (research R10):
  - `app/src/widgets/api.ts`: the three commands, typed;
  - `Frame.tsx`: header in the `frame-header` roles with the icon drawn on a `<canvas>` by
    `putImageData` (or the monogram) and the drawn name, plus the host's reload button; body in
    the `frame` roles with `contain: layout paint`, `overflow: auto`, a fixed maximum height,
    `isolation: isolate`; states `loading`, `content`, `error`, `busy`, `not-loaded`;
  - `content/*.tsx`: text, list, table, chart (inline SVG, series `chart-1`…`chart-5`, a text
    table of the values), button, form (`noValidate`; text and number as `type="text"`, a native
    checkbox, the host listbox inside the body; `autocomplete="off"`; inline errors), stack, and
    the placeholder; every string a text child inside `<bdi>`; no `title`, `href` or `src` from
    content; the tone and variant class map in `content/styles.tsx`;
  - `Widgets.tsx`: the stack of frames, shown in `App.tsx` above the demo's tabs.
  `pnpm --filter ./app build` and `pnpm --filter ./design check` pass.
- [ ] T020 [US1] Walk quickstart §1 on Linux: the widget appears within a second, follows the
  theme, "Contar" counts and starts over after a restart. Capture the development build on a
  virtual display, as feature 008 did.

**Checkpoint**: the showcase is on the dashboard.

---

## Phase 4: User Story 2 - An author writes widget content from a published format (Priority: P1)

**Goal**: the format published and checked, and invalid content shown as the host's error.

**Independent Test**: quickstart §2 and §3.

- [X] T021 [P] [US2] Write three fixtures (1.4.0, no permission), embedded only in the probe
  build: `plugins/hostile/ui-invalid` (render returns content over a bound; action `wrong-type`
  makes the next render return a wrong type; action `spin` loops until the time limit),
  `ui-crash` (render traps), `ui-badmanifest` (a manifest name over 60 characters). Add to a new
  `app/src/probe/widgets.ts`, registered in `run.ts` and `PAGE_CASES` (`probe.rs:26-81`), the
  cases `content-error-shown`, `window-responsive` and `showcase-on-time` of research R14. They
  fail until T022.
- [ ] T022 [US2] Draw the content error and the not-loaded state in `Frame.tsx` (FR-005): the
  host's message with the error's kind and path, as text; the plugin's log gets the entry
  (`widgets.rs`). T021's case holds.
- [ ] T023 [US2] Write the author docs (research R16): `docs/authors/reference/content.md` (each
  component, field kind, tone, variant and bound, with `<!-- ref: content <path> -->`); the kind
  `content` in `runtime/tests/docs_reference.rs` sourced from `wit/content.schema.json`;
  `widget-content/tests/docs.rs` validating every example of the page; `limits.md` (the content
  bounds), `manifest.md` (`name`, `icon`, `actions`), `contract.md` (1.4.0, `actor`, the six
  worlds), `errors.md` (`undeclared-action`), `cli.md` (`act`, the content report),
  `docs/authors/README.md` and `getting-started.md` (the contract version). The tutorial and the
  examples `hello-widget`, `github-releases`, `update-counter` and `private-notes` render content
  in format 1, so the app draws them, and the tutorial runs `runtime run --check-content`.
  `cargo test -p runtime --test it docs_reference::` and `cargo test -p widget-content` pass.

**Checkpoint**: an author can write and check content from the docs alone.

---

## Phase 5: User Story 3 - Widget content stays inside its frame and cannot pass for the host (Priority: P1)

**Goal**: HS-01 to HS-04, HS-07, HS-16, HS-24 and HS-25 held in the real page on three systems.

**Independent Test**: quickstart §4.

- [ ] T024 [P] [US3] Write the hostile fixtures (1.4.0, no permission), embedded only in the
  probe build: `plugins/hostile/ui-header` (manifest name with markup and bidi overrides; content
  shaped like a header with another plugin's name), `ui-overflow` (wider and taller than any
  frame, toward the header in every direction), `ui-strings` (HTML, script, image tags, bidi
  overrides, URLs, file paths and custom schemes in every component; a long select; an invalid
  form; status tones), `ui-dialog` (every tone and variant shaped like a host dialog; buttons
  naming host commands and other widgets' actions).
- [ ] T025 [US3] Add the probe cases of research R14 for this story to `app/src/probe/widgets.ts`
  and `PAGE_CASES`: `frame-header-kept`, `frame-clips`, `frame-header-hittable`,
  `content-is-text`, `strings-isolated`, `dialog-shaped-widget`, `nothing-outside-frame`. Run the
  probe until each holds; then break each protection once (both `contain` and `overflow`; the
  `<bdi>`; the header's place) and see its case break. Revert.

**Checkpoint**: no hostile widget leaves its frame or passes for the host.

---

## Phase 6: User Story 4 - A widget's controls do only what that widget declared (Priority: P2)

**Goal**: HS-05 and HS-06 held, the form's secret check in place.

**Independent Test**: quickstart §4 and §6.

- [ ] T026 [US4] Add `foreign-actions-refused` and `form-no-credentials` to
  `app/src/probe/widgets.ts` and `PAGE_CASES` (research R14): the probe calls `widget_act` with
  host command names, another widget's action and undeclared names, each refused, the `actions`
  counter unchanged; every text-entry input is `type="text"`, every input `autocomplete="off"`
  with no credential name or hint. Run until both hold; remove the declared-name check once and
  see the first break. Revert.

---

## Phase 7: User Story 5 - Status reads without color (Priority: P2)

**Goal**: HS-08 in the page.

- [ ] T027 [US5] Add `status-icons` to `app/src/probe/widgets.ts` and `PAGE_CASES`: each status
  text of `ui-strings` and the showcase has its icon element. Run until it holds.

---

## Phase 8: User Story 6 - The window's surface grows only by reviewed entries (Priority: P3)

**Goal**: the three commands are the window's only new reach, and feature 008's protections hold.

- [ ] T028 [US6] Run quickstart §5: `cargo test -p dashboard` passes; removing `widget_act`'s
  entry from `window-surface.toml` fails the surface test naming it (revert). Run the full probe
  locally: every case of feature 008 and of this feature holds.

---

## Phase 9: Polish & Cross-Cutting Concerns

- [ ] T029 Update `docs/threat-model.md` (research R16, FR-021):
  - B4 rows cite the probe cases and tests of each criterion;
  - new rows: B1, an action reaching beyond its grants; B1, an undeclared action; B7, a window
    command given a forged handle; a residual row for text typed into a form (Principle IV,
    v2.6.2);
  - B1's "all or nothing in update" and B2's "calls only in `update`" read "in update or an
    action"; B1 says an action's data stays in memory and durable state needs `storage`.
  `pnpm --filter ./design check` passes (its criteria step reads B4 rows).
- [ ] T030 [P] Add the dated note to `docs/decisions/0001-plugin-process-isolation.md`: its
  "During Phase 1" scope now covers the app's own bundled plugins, with no change to its risk
  (research R15).
- [ ] T031 [P] Update `CONTRIBUTING.md`: the security core gains `app/src-tauri/src/widgets.rs`,
  `name.rs`, the three commands, `app/src/widgets/`, `widget-content/` and the content schema;
  a short section on widgets (the content format, the bundled-plugin rule, the hostile fixtures
  and probe cases).
- [ ] T032 Run every local check under the memory cap: `cargo xtask plugins`, clippy on the
  workspace and the probe build, `cargo test --workspace`, `cargo test -p widget-content`,
  `cargo deny`, `cargo audit`, `pnpm audit`, the design checks and licenses, the docs reference
  test, and the probe on Linux. Record the times in research R1.
- [ ] T033 Walk quickstart §1 to §6 on Linux and fix any drift between it and the code.
- [ ] T034 Review the security core independently: a fresh agent with none of this feature's
  context reads `wit/`, the runtime's changes, `app/src-tauri/src/widgets.rs`, `name.rs`,
  `commands.rs`, `window-surface.toml` and `app/src/widgets/` against the spec and contracts, and
  looks for any way widget content or an action could leave its frame, reach the host, act for
  another widget, outlive its permission, or see a secret. Check each finding at the source
  before changing anything; bring real trade-offs to the maintainer.
- [ ] T035 Only once the maintainer has pushed and opened the pull request, as they ask: follow
  CI on the three systems and record, per system, the probe's time and the showcase's
  first-render time from `showcase-on-time` (SC-001) in research R1.
- [ ] T036 The maintainer's part, before the merge: the line-by-line review of the security core
  (Principle IX); quickstart §7 (keyboard, screen reader, narrow frames, forced colors and
  reduced motion) on Windows, macOS and Linux.
- [ ] T037 Before the merge, and only when the maintainer asks, update the development guide:
  tick the Phase 3 items "Biblioteca de UI declarativa" and "Moldura do host em cada widget",
  noting contract 1.4.0 and constitution v2.6.2 and v2.6.3.
- [ ] T038 Merge:
  - The maintainer reviewed the pull request line by line.
  - CI is green on the three systems.
  - `grep -n '^- \[ \]' specs/009-declarative-widget-ui/tasks.md` shows only this task.
  - Tick this box in the pull request's last commit, wait for green CI on that head, and merge
    with `gh pr merge --merge --match-head-commit <sha>`.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: none. T001 before anything that raises the contract.
- **Foundational (Phase 2)**: depends on Phase 1.
  - T005 → T006; T007 → T008; T009 → T010 (after T006 and T008); T011 alongside.
  - T012 → T013; T014 after T010 and T013.
  - T015 → T016, after T010, T011 and T013; T017 after T016.
- **US1 (Phase 3)**: after Phase 2. T018 → T019 → T020. The MVP.
- **US2 (Phase 4)**: after T019. T021 → T022; T023 after T013 and T014.
- **US3 (Phase 5)**: after T019 and T021 (the probe module). T024 → T025.
- **US4, US5 (Phases 6, 7)**: after T024 and T025.
- **US6 (Phase 8)**: after every story.
- **Polish (Phase 9)**: T029 to T031 after the stories; T032 to T034 after everything; T035 after
  the PR opens; T036 and T037 at the maintainer's hand; T038 last.

### Parallel Opportunities

- Phase 1: T002, T003 and T004 beside T001.
- Phase 2: the contract (T005–T006), the manifest (T007–T008), the vault (T011) and the content
  crate (T012–T013) in parallel; then actions (T009–T010), the CLI (T014) and the app (T015–T016).
- Stories: T021 and T024 (fixtures) in parallel; T023 (docs) beside the probe work.

## Implementation Strategy

### MVP First (User Story 1)

1. Phases 1 and 2.
2. Phase 3: the showcase on the dashboard.
3. Stop and validate with quickstart §1.

### Incremental Delivery

1. US1, then US2 (authors and invalid content), then US3 to US5 (the hostile cases), then US6.
2. Every story keeps feature 008's probe green.
3. One commit per task; push only when the maintainer asks.
