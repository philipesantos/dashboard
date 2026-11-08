---

description: "Task list for the host visual system and the design of the host frame and dialogs (feature 007)"
---

# Tasks: Host Visual System and the Design of the Host Frame and Dialogs

**Input**: Design documents from `specs/007-host-visual-design/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Required. The checks are the feature: the format and project-layer checker, the drift
test, the contrast lint, the hue rule, the host-surface split and the criteria coverage are all
tests. The feature adds no hostile case and no security rule (FR-022): the security suite must
pass unchanged. Every test task comes before the implementation it checks and fails until that
implementation lands. No test reaches the network; every pin and drift comparison reads LF.

**Organization**: the shared pieces every check needs come first: the vendored data, the three
crates and `design-gen`'s readers and color math. Then the stories in priority order: one source
and its generation (US1), contrast over every appearance (US2), the host-surface split (US3), the
appearance picks (US4, before US2, whose lint
enumerates what it accepts), the frame and dialog criteria (US5) and their keyboard and screen-reader
part (US6).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1–US6)

## Path Conventions

Paths follow plan.md:
- **Visual system:** `DESIGN.md` at the root; vendored data, fonts, the three crates and the
  generated DTCG file under `design/`.
- **Criteria:** `docs/design/host-surfaces.md`.
- **Repository task:** `xtask/src/main.rs`.
- **CI:** `.github/workflows/ci.yml`.

Where the details live:
- decisions and measured numbers: `research.md` (R1–R13);
- the two layers of `DESIGN.md` and the checker's rules 1–14: `contracts/design-md.md`;
- the crates' API, `cargo xtask design` and the DTCG file: `contracts/tokens-api.md`;
- the criteria file and its coverage rules: `contracts/host-surfaces-criteria.md`;
- entities and their rules: `data-model.md`.

Long builds and test runs go under the memory cap:
`systemd-run --user --scope -p MemoryMax=8G <command>`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: the vendored data, the crates' skeletons, line endings and the dependency bans. The
workspace builds and every existing test passes at the end of this phase.

- [ ] T001 Vendor the upstream data under `design/` (FR-005, FR-007; research R1, R2, R9), each
  file unmodified,
  with its license beside it:
  - `design/radix/3.0.0/{light.ts,dark.ts,blackA.ts,LICENSE}` from `radix-ui/colors` at commit
    `c4d0e50006c71ec51c4d6ff349062ab261b57e67`;
  - `design/format/0.4.0/{spec.md,LICENSE}` from `google-labs-code/design.md` at tag `0.4.0`
    (commit `9bf8eae67128b6cc55ad9bf86665767deb4c11cd`);
  - `design/fonts/inter/` with Inter's upstream variable WOFF2 and TTF and `OFL.txt`, from the
    latest Inter release at least two weeks old, no subsetting;
  - `design/README.md`: one line per directory saying what it holds, where it came from (URL and
    commit or release) and its license.
  - Record each file's SHA-256 (after turning `\r\n` into `\n` for text) in the README; T007
    pins them.
- [ ] T002 [P] Add `.gitattributes` at the root (research R7): `DESIGN.md`, `docs/design/*.md`
  and the text files under `design/` (`*.rs`, `*.ts`, `*.md`, `*.json`, `*.txt`, `*.toml`) as
  `text eol=lf`; `*.woff2` and `*.ttf` as `binary`. Run `git add --renormalize .` and confirm it
  changes no existing file.
- [ ] T003 Create the three crates and add them to the root `Cargo.toml`'s workspace members
  (plan, Project Structure), each `edition = "2024"`, `publish = false`, Apache-2.0:
  - `design/gen` (package `design-gen`, lib) with `yaml-rust2 = "=0.13.0"` and a one-line
    comment saying why (research R4);
  - `design/tokens` (package `design-tokens`, lib, no dependencies), `src/lib.rs` including
    `generated.rs`, which starts as an empty module with the "generated from DESIGN.md, do not
    edit" line;
  - `design/host-surfaces` (package `host-surfaces`, lib) depending on `design-tokens`, the same
    shape.
  - Confirm `cargo deny check` passes with no new license and `cargo tree -d` shows no new
    duplicate.
- [ ] T004 Update `deny.toml` (research R6): under `[bans] deny`, add
  `{ name = "host-surfaces", wrappers = ["host"] }` and `{ name = "design-gen", wrappers =
  ["xtask"] }`, each with a comment pointing to `specs/007-host-visual-design/research.md` R6.
  This hunk sits beside the network stack's bans and gets line-by-line review. Then run
  `cargo deny check bans` on the workspace and confirm it passes: `host-surfaces` has no
  dependent and `design-gen` only `xtask` (after T020). If cargo-deny flags a banned crate just
  for being a workspace member, stop: move `host-surfaces` out of the workspace (as the fixture
  is), or pick another enforcement, bring the choice to the maintainer and record it in
  research R6 before going on.
- [ ] T005 [P] Create the ban fixture `design/ban-fixture/` (research R6): a crate named
  `ban-fixture`, outside the workspace (add it to the root `Cargo.toml`'s `exclude` with a
  comment), depending on `host-surfaces` by path, with its own committed `Cargo.lock`. Confirm
  the flag placement against `cargo deny --help` first, then confirm
  `cargo deny --manifest-path design/ban-fixture/Cargo.toml check --config deny.toml bans` fails
  and names the `host-surfaces` ban.

**Checkpoint**: data vendored and hashed, crates build empty, bans in place and shown to bite.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: what every check reads and computes with. No user story starts before T011.

- [ ] T006 [P] Write `design/gen/tests/math.rs` first, failing:
  - WCAG 2.2 contrast of known pairs: white on black is 21; `#767676` on white is 4.54 (cut off
    to two decimals, never rounded up); a pair equal to itself is 1;
  - compositing: `blackA` step 7 (`rgba(0, 0, 0, 0.5)`) over white is `#808080` within one unit;
  - OKLCH of Radix light step 9 for `blue`, `red`, `green`, `amber` and `gray` within 0.1° of
    hue and 0.001 of chroma of research R3's table (gray: chroma under 0.001);
  - hue distance wraps: 350° and 10° are 20° apart.
- [ ] T007 [P] Write `design/gen/tests/pins.rs` first, failing: the SHA-256 of every vendored
  text file of T001 (after `\r\n` → `\n`) and of each font file equals the hash recorded in
  `design/README.md`; a changed byte fails naming the file.
- [ ] T008 [P] Write `design/gen/tests/radix_reader.rs` first, failing (research R2): the reader
  of T009 returns 31 solid scales plus `blackA`, each with exactly 12 steps, light and dark; and
  refuses, naming the line: an unexpected line, a scale with 11 or 13 steps, a step out of
  order, a missing or extra scale name, and a value that is neither `#rrggbb` nor
  `rgba(r, g, b, a)`. Fixtures are small inline strings, not copies of the vendored files.
- [ ] T009 Implement `design/gen/src/color.rs` (sRGB, relative luminance, contrast cut off to two
  decimals, compositing over an opaque color, OKLab/OKLCH, hue distance) and
  `design/gen/src/radix.rs` (the strict line reader of research R2, the `Scale` entity of
  `data-model.md` with `kind` derived from the gray-family list and "chroma under 0.08 means
  neutral"). T006 and T008 pass.
- [ ] T010 Implement `design/gen/src/source.rs`: read `DESIGN.md` into its two layers
  (contracts/design-md.md): the front matter as the format's 0.4.0 schema, the `##` sections in
  order, and every fenced `design-roles` block as YAML, keeping line numbers for messages. Parse
  only; the rules come in US1. Unit tests with inline fixtures: front matter not first, a
  duplicate section, a `design-roles` key in two blocks, YAML that does not parse, each refused
  with its line.
- [ ] T011 Implement `design/gen/src/model.rs`: the `Role`, `Pairing`, `Tone`, `Token` and
  `Criterion` entities of `data-model.md`, with their fields and rules verbatim (for example
  `kind` of a pairing is `text` \| `large-text` \| `non-text` \| `decorative` \| `inactive`, and
  `why` is required for `decorative` and `inactive`). No behaviour beyond construction. T007
  passes.

**Checkpoint**: the data reads, the math is right, `DESIGN.md` parses. Every existing test still
passes.

---

## Phase 3: User Story 1 - One source for the whole visual system (Priority: P1) 🎯 MVP

**Goal**: `DESIGN.md` is the only hand-edited source; tokens, the DTCG export and the preview
follow from it; drift and format errors fail CI; a `DESIGN.md`-only change runs every job.

**Independent Test**: quickstart §1, §2, §7 and §8.

### Tests for User Story 1

- [ ] T012 [P] [US1] Write `design/gen/tests/checker.rs` first, failing: one inline fixture per
  checker rule 1–9, 12 and 14 of `contracts/design-md.md` that breaks it, each refused naming its
  line or token path; and one valid minimal fixture that passes. Rules 10, 11 and 13 are US2's.
- [ ] T013 [P] [US1] Write `design/gen/tests/drift.rs` first, failing: generating from the
  committed `DESIGN.md` reproduces `design/tokens/src/generated.rs`,
  `design/host-surfaces/src/generated.rs` and `design/tokens.dtcg.json` exactly (LF); with one
  value changed in memory, the comparison fails naming the token and the file (FR-004).
- [ ] T014 [P] [US1] Write `design/gen/tests/dtcg.rs` first, failing: the export has `$type` and
  `$value` for every token, the groups of `contracts/tokens-api.md`, the rationale in
  `$description`, and `$extensions."dev.dashboard".hostSurface: true` exactly on the host-surface
  tokens.
- [ ] T015 [P] [US1] Write `design/gen/tests/preview.rs` first, failing (research R8): the
  preview writes the five files of quickstart §7; each parses as XML, has no `<script>`, no
  `on*` attribute, no `href` or `url(` outside the directory except the embedded `data:` font,
  and embeds the Inter WOFF2 from `design/fonts/inter/`.

### Implementation for User Story 1

- [ ] T016 [US1] Implement the checker rules 1–9, 12 and 14 in `design/gen/src/check.rs` (FR-005,
  FR-023), with
  messages as `contracts/tokens-api.md` says (file and line or token path, text escaped). T012
  passes for those rules.
- [ ] T017 [US1] Write `DESIGN.md` (FR-001, FR-002, FR-012, FR-024; research R1, R3, R5, R9,
  R13; contracts/design-md.md):
  - the front matter and the eight sections in the format's order, each with prose saying what
    the section decides and why;
  - the `design-roles` blocks: `scales` (Radix 3.0.0, status green, amber and red, `solid-text`
    per chromatic scale), `picks` (the six gray families, 30°, 0.08, the default appearance of
    FR-012), `roles` with a `why` each, `pairings`, `tones`, `elevation` (`flat`, `frame`,
    `panel`, `dialog`, shadows and scrim from `blackA`), `radius-picks` (`none` 0, `small` 3 px,
    `medium` 6 px, `large` 10 px, `full`), `icons` (check, alert, cross on a 16×16 grid) and
    `host-surfaces`, plus `rationale` for every format-layer token;
  - typography in Inter: at most nine levels from 12 px to 32 px, body 14 px; spacing on a 4 px
    base;
  - the design choices R5 forces: no normal-size text on steps 9–10 unless the scale's
    `solid-text` reaches 4.5:1; status text in gray 12 beside a status icon; soft badges with
    step-12 text; control boundaries at a step that reaches 3:1; the host's focus style as a
    host surface.
  `cargo xtask design --check` (T020) may fail on contrast until US2.
- [ ] T018 [US1] Implement generation in `design/gen/src/generate.rs` (FR-003, FR-004): the Rust tokens split by
  the `host-surface` marks into the two crates' `generated.rs` (contracts/tokens-api.md), the
  DTCG file, and the hue and chroma per scale for US4. Output is deterministic (sorted, LF, no
  timestamps). T013 and T014 pass.
- [ ] T019 [US1] Implement the preview in `design/gen/src/preview.rs` (FR-026, research R8): the five SVG
  files, drawn from the default appearance's tokens, Inter embedded as WOFF2 `@font-face`, no
  script, `index.svg` linking only its siblings. T015 passes.
- [ ] T020 [US1] (SC-001) Add `design`, `design --check` and `design --preview <dir>` to
  `xtask/src/main.rs` as `contracts/tokens-api.md` says, calling `design-gen` (add it as an
  `xtask` dependency; the T004 ban allows only `xtask`). Update `xtask/Cargo.toml`'s
  `description`. Run `cargo xtask design` and commit the generated files.
- [ ] T021 [US1] Update `.github/workflows/ci.yml` (FR-006, FR-026; research R12):
  - the `changes` job's first test becomes
    `'^(docs/authors|docs/design|wit|design)/|^DESIGN\.md$|^docs/threat-model\.md$'`; its
    comment names these paths as Markdown the build uses as code and cites constitution v5.0.0
    instead of v2.4.1;
  - in the Linux build job: `cargo xtask design --check`; the ban fixture step, which passes only
    when `cargo deny --manifest-path design/ban-fixture/Cargo.toml check --config deny.toml
    bans` fails; `cargo xtask design --preview target/design-preview`; and
    `actions/upload-artifact` pinned to a commit SHA with its tag in a comment, artifact
    `design-preview`, kept 14 days.

**Checkpoint**: quickstart §1, §2 and §7 pass locally; §8 is checked on the pull request's run.

---

## Phase 4: User Story 4 - The person's appearance picks are checked (Priority: P2)

**Goal**: the 30° hue rule, the neutral-scale rule and the default (FR-010–FR-013).

**Independent Test**: quickstart §4.

### Tests for User Story 4

- [ ] T022 [P] [US4] Write `design/tokens/tests/appearance.rs` first, failing:
  `Appearance::DEFAULT` validates; blue with an indigo host is `TooClose` at 15°; bronze, gold and
  brown as the widget color are `NotChromatic`; with a monochrome host the accepted widget colors
  are exactly lime, sky, cyan, blue, indigo, iris, violet, purple, plum and pink; and, for every
  pair of scales, the verdict equals one computed independently in the test from the vendored
  Radix data through `design-gen`'s math (SC-004).

### Implementation for User Story 4

- [ ] T023 [US4] Implement `Appearance`, `HostColor`, `Radius`, `ThemePick`, `GrayFamily`,
  `Refusal` and `validate()` in `design/tokens/src/lib.rs` over the generated hue and chroma
  table (contracts/tokens-api.md, data-model.md: "hue ≥ 30° from `host` (if chromatic) and from
  green, amber and red"; FR-011, FR-013). T022 passes. It comes before US2 because the lint
  enumerates the appearances this rule accepts.

**Checkpoint**: quickstart §4 passes.

---

## Phase 5: User Story 2 - Text stays readable in every appearance a person can pick (Priority: P1)

**Goal**: every pairing passes WCAG's contrast minimums in every appearance the host accepts, in
both themes, and an undeclared or failing pairing fails CI (constitution v5.0.0, FR-008, FR-009).

**Independent Test**: quickstart §3.

### Tests for User Story 2

- [ ] T024 [P] [US2] Extend `design/gen/tests/checker.rs` first, failing, for rules 10, 11 and
  13: a fixture whose control boundary uses gray step 8 fails on `slate`, light, near 1.86 (the
  message names pairing, kind, theme, picks and ratio); a foreground role with no pairing on a
  background it is drawn on fails; `focus` not paired on accent step 9 fails; `solid-text: white`
  on `cyan` fails, and `none` on `blue` fails because dark text reaches 4.5:1.
- [ ] T025 [P] [US2] Write `design/gen/tests/lint.rs` first, failing: the lint over the committed
  `DESIGN.md` covers every appearance the host accepts in both themes (count them and assert the
  count matches an independent enumeration from the picks, at most about 2,760), and passes;
  translucent roles are measured after compositing over each background.

### Implementation for User Story 2

- [ ] T026 [US2] Implement the contrast lint and rules 10, 11 and 13 in `design/gen/src/lint.rs`
  (research R5): enumerate the appearances `Appearance::validate` (T023) accepts, resolve every
  role per theme, composite translucent ones, apply the
  thresholds by `kind`, skip `decorative` and `inactive` only with a `why`. T024 and T025's
  failing parts pass except where `DESIGN.md` itself fails.
- [ ] T027 [US2] Fix `DESIGN.md` until the lint passes, changing the step a role uses, never a
  threshold. Record in the Colors section each pairing that needed a step other than Radix's
  usual one for its role, with its measured worst case. Regenerate with `cargo xtask design`.
  T024–T025 pass; quickstart §3 behaves as written.

**Checkpoint**: `cargo xtask design --check` passes; every accepted appearance is AA.

---

## Phase 6: User Story 3 - A widget cannot build the host's surfaces (Priority: P1)

**Goal**: the host-surface tokens are only in `host-surfaces`, which only the future host may
use; nothing includes their source (FR-014–FR-017).

**Independent Test**: quickstart §5.

### Tests for User Story 3

- [ ] T028 [P] [US3] Write `design/gen/tests/host_surfaces.rs` first, failing (research R6):
  every token `DESIGN.md` marks `host-surface` is absent from
  `design/tokens/src/generated.rs` and present in `design/host-surfaces/src/generated.rs`; no
  token outside the marks is in `host-surfaces`; and a scan of every `.rs` file of the workspace
  outside `design/host-surfaces/` finds no `include!`, `include_str!` or `#[path]` naming
  `host-surfaces/src`.
- [ ] T029 [P] [US3] Write `design/tokens/tests/tone.rs` first, failing (FR-015, FR-017,
  FR-024, FR-025): `Tone::parse` accepts the five names and returns `UnknownTone` with the name
  for anything else; `tone()` resolves default from the gray family, accent from the widget
  color, ok, warning and error from green, amber and red; each status tone has its icon.

### Implementation for User Story 3

- [ ] T030 [US3] Implement `design/tokens/src/lib.rs`'s hand-written part (`Tone`, `UnknownTone`,
  `ToneColors`, `tone()`, `Rgb`, `contrast()`, `Icon`) over the generated tables, and
  `design/host-surfaces/src/lib.rs` (`chrome`, `dialog`, `panel_attachment`, `focus`), as
  `contracts/tokens-api.md` says. T028 and T029 pass.

**Checkpoint**: quickstart §5 passes, the fixture step fails as it must.

---

## Phase 7: User Story 5 - The frame and the dialogs are designed to be checked (Priority: P2)

**Goal**: numbered criteria for the chrome, frame, panel, dialogs and field note, each answering a
constitution rule and a threat row, with coverage checked against the threat model (FR-018,
FR-019, FR-021).

**Independent Test**: quickstart §6.

### Tests for User Story 5

- [ ] T031 [P] [US5] Write `design/gen/tests/criteria.rs` first, failing
  (contracts/host-surfaces-criteria.md, rules 1–4): reading `docs/design/host-surfaces.md` and
  `docs/threat-model.md`, every `HS-NN` is unique with four non-empty fields; every quoted row
  matches a threat row's first cell exactly; every Phase 3 row on boundary B4 is quoted by a
  criterion or under "Decided elsewhere"; every **Tested by** is one of the six allowed names.
  Self-tests on inline copies: a missing field, a misquoted row and an uncovered B4 row each
  fail with the row's title.

### Implementation for User Story 5

- [ ] T032 [US5] Write `docs/design/host-surfaces.md`, sections Chrome, Widget frame, Frame panel,
  Host dialogs and Fields the host draws for a plugin, with every criterion FR-019 lists
  (contracts/host-surfaces-criteria.md, "Criteria the file must hold"), the 1000 ms delay of
  research R11, the frame's label and monogram rules of the spec's edge cases (the label capped
  at 40 visible characters, then an ellipsis), and "Decided
  elsewhere" with the rows other features own, the fake dialog from declared actions first
  (FR-021). Each criterion's **Observe** is a bound, a focus position, an accessibility-tree
  property or an input outcome, never a judgement. T031 passes for these sections.

**Checkpoint**: quickstart §6 passes.

---

## Phase 8: User Story 6 - Host dialogs work for keyboard and screen-reader users (Priority: P3)

**Goal**: the keyboard and screen-reader criteria of host dialogs (FR-020).

**Independent Test**: each criterion in the section states an observable result on the focus
position or the accessibility tree and names its screen readers and systems.

- [ ] T033 [US6] Add the section Keyboard and screen readers to `docs/design/host-surfaces.md`:
  initial focus on the first control that is not the one that confirms; Tab and Shift+Tab never
  leave the dialog; Escape refuses; what is announced (the host's name for the dialog, then its
  question); only the dialog exposed while open; no plugin-supplied node with a dialog role; how
  the host makes up for GPUI's missing modal state (every node outside the dialog removed from the
  tree, or hidden, while it is open, the choice left to the host-dialog feature with both
  options named); tests with VoiceOver on macOS, Narrator on Windows and Orca on Linux, and real
  key presses in a real window. Extend T031's self-test with a criterion lacking **Tested by**.
  T031 passes.

**Checkpoint**: the criteria file is complete.

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: docs for contributors, checks, reviews and merge

- [ ] T034 [P] Update `CONTRIBUTING.md`: how to change `DESIGN.md` (edit, `cargo xtask design`,
  commit the generated files with it), what each check means when it fails, how to move the
  Radix or format pin (its own commit, reviewed like a dependency upgrade), and that the future
  host must ship Radix's MIT notice and Inter's OFL.
- [ ] T035 [P] Update `.github/pull_request_template.md`'s security-core comment to name the
  `deny.toml` bans of `host-surfaces` and `design-gen` among what needs line-by-line review.
- [ ] T036 Run every local check, under the memory cap:
  - `cargo fmt --all --check`;
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`;
  - `cargo test --workspace --locked`, on Linux inside a `dbus-run-session` with gnome-keyring;
  - `cargo xtask design --check`, timed: record the time in plan.md's Performance Goals (the goal
    is under 5 s on the reference machine; over it, bring it to the maintainer); and
    `cargo xtask design --preview target/design-preview`;
  - `cargo deny check`, the ban fixture failing as it must, and `cargo audit`.
- [ ] T037 Confirm FR-022: `git diff master -- runtime/ plugins/ wit/ connectors/` is empty, and
  the security suite passes unchanged.
- [ ] T038 Walk `specs/007-host-visual-design/quickstart.md` §1–§7, including each mutation
  (a value in `DESIGN.md`, gray step 8 for a control boundary, a criterion's **Answers** line, a
  new B4 row), and fix any drift between it, the contracts and the code.
- [ ] T039 Independent review: a fresh agent with none of this feature's context reads
  `DESIGN.md` and `docs/design/host-surfaces.md` against constitution v5.0.0,
  `docs/threat-model.md` and `research.md`, and returns each rationale that does not hold, each
  criterion that is a judgement rather than an observation, and each threat row a criterion
  claims to answer but does not. Check each finding at the source before changing anything;
  record them and their outcomes at the end of `research.md` under "Review of DESIGN.md and the
  criteria".
- [ ] T040 Once the maintainer has pushed and the pull request's run is green, open its
  `design-preview` artifact and bring the five SVG files to the maintainer for approval of the
  look (SC-007). Changes they ask for go into `DESIGN.md` as new commits. On the same run,
  confirm quickstart §8 (SC-006): when the pushed commits touch `DESIGN.md`, the `changes` job's
  log shows `code=true` and every job ran on the three systems; if no pushed commit changed only
  `DESIGN.md`, check the job's decision on that path with its regex locally and record both.
- [ ] T041 Before the merge, and only when the maintainer asks: mark the development guide's
  Phase 3 items "Sistema visual em `DESIGN.md`" and "Spec de design da moldura e dos diálogos do
  host" as done.
- [ ] T042 Human review (Principle IX) and merge:
  - The maintainer reviews every commit line by line, the `deny.toml` hunk with the care of the
    network stack's bans.
  - The maintainer ticks the pull request's review box with their handle.
  - Tick this task's box in the pull request's last commit; check
    `grep -n '^- \[ \]' specs/007-host-visual-design/tasks.md` shows nothing on that head.
  - CI is green on the three systems on that head.
  - Merge with `gh pr merge 13 --merge --match-head-commit <sha> --author-email
    8193616+philipesantos@users.noreply.github.com`.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies; T001 → T003 → T004 → T005; T002 with any.
- **Foundational (Phase 2)**: after Setup; blocks every story.
- **US1 (Phase 3)**: after Foundational. MVP.
- **US4 (Phase 4)**: after T018 (the hue table is generated). It runs before US2, out of priority
  order, because the lint enumerates the appearances its rule accepts.
- **US2 (Phase 5)**: after T023 and T016–T018 (the checker and generation exist, `DESIGN.md` has
  roles).
- **US3 (Phase 6)**: after T018 (the split crates are generated).
- **US5 (Phase 7)**: after Foundational only; it reads Markdown, not the tokens.
- **US6 (Phase 8)**: after T032 (same file).
- **Polish (Phase 9)**: after every story; T040 needs the pushed branch; T041 only on request;
  T042 last.

### Within Each Phase

- Tests before their implementation; each fails first.
- Foundational: T006–T008 in parallel → T009; T010 and T011 after T009.
- US1: T012–T015 in parallel → T016 → T017 → T018 → T019 → T020 → T021.
- US4: T022 → T023.
- US2: T024–T025 → T026 → T027.
- Tasks that edit `DESIGN.md` (T017, T027) and `docs/design/host-surfaces.md` (T032, T033) run in
  order; T027 regenerates, so US3 and US4 rebase their generated files on it.

### Parallel Opportunities

- Setup: T002 with T001; T005 after T004.
- Foundational: T006, T007, T008.
- US1: T012, T013, T014, T015.
- US5 can run beside US4, US2 and US3 from the end of Foundational.
- Polish: T034 with T035.

---

## Parallel Example: User Story 1

```bash
# Four test files, each its own file, all failing first:
Task: "design/gen/tests/checker.rs: one fixture per checker rule 1–9, 12, 14"
Task: "design/gen/tests/drift.rs: generated files equal what DESIGN.md produces"
Task: "design/gen/tests/dtcg.rs: the DTCG export's shape and host-surface marks"
Task: "design/gen/tests/preview.rs: five SVG files, no script, Inter embedded"
```

---

## Implementation Strategy

### MVP First (User Story 1)

1. Setup and Foundational.
2. US1: `DESIGN.md`, the checker, generation, the preview and CI.
3. **Stop and validate**: quickstart §1, §2, §7; push when the maintainer asks and check §8 on
   the run.

### Incremental Delivery

1. US4 adds the appearance rule; US2 makes the lint pass over every appearance it accepts,
   adjusting `DESIGN.md`.
2. US3 adds the tones and the host surfaces' API over the generated tables.
3. US5 and US6 write the criteria, which can proceed in parallel from Foundational.
4. Polish: reviews, the maintainer's look at the preview, merge.

One commit per task or small group of tasks; a fix after review is a new commit.
