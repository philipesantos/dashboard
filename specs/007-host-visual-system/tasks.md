---

description: "Task list for the host visual system and the design of the host frame and dialogs (feature 007)"
---

# Tasks: Host Visual System and the Design of the Host Frame and Dialogs

**Input**: Design documents from `specs/007-host-visual-system/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Required. The checks are the feature, so each one has `node --test` tests in
`design/test/`, with failing fixtures that show the check fails where it must (FR-005, FR-010,
FR-015, SC-002, SC-003, SC-005). Every test task comes before the code it checks and fails until
that code lands.

The feature adds no hostile case, so the security suite must pass unchanged (FR-021). No test
reaches the network, and none reads `specs/`.

**Organization**:
- **Foundation:** the tooling package, the color math, the parser and a first `DESIGN.md` that
  holds today's values.
- **The stories:**
  - US1: make `DESIGN.md` the one source;
  - US2: add contrast and fix what it finds;
  - US3: split host-only roles from what widget content can select;
  - US4: write the frame criteria and their coverage check;
  - US5: write the dialog-form criteria;
  - US6: write the dialog accessibility criteria.
- **Polish:** the preview, the npm audit and licenses, the docs, the walks, the review and the
  merge.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1–US6)

## Path Conventions

Paths follow plan.md:
- **Tooling:** `design/`, run with Node.js 24 type stripping. Sources are in `design/src/`, tests
  in `design/test/` and fixtures in `design/test/fixtures/`.
- **The visual system:** `DESIGN.md` at the root.
- **Generated files:** `app/src/theme.generated.css` and `design/widget-map.generated.json`.
- **Criteria:** `docs/design/host-surfaces.md`.

Where the details live:
- decisions and measurements: `research.md` (R1–R13);
- `DESIGN.md`'s layers: `contracts/design-md.md`;
- commands, outputs and error lines: `contracts/outputs.md`;
- entities and validation rules: `data-model.md`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: the tooling package, line endings, and a clean `pnpm audit` to build on

- [X] T001 Create the `design/` workspace package:
  - `design/package.json`: private, `"type": "module"`, `"engines": { "node": ">=24" }`.
  - Scripts:
    - `gen`: `node src/cli.ts gen`;
    - `check`: `node src/cli.ts check`;
    - `preview`: `node src/cli.ts preview`;
    - `licenses`: `node src/cli.ts licenses`;
    - `test`: `node --test test/`.
  - `devDependencies`, exact: `"@google/design.md": "0.4.0"` and `"yaml": "2.9.1"`.
  - Add `design` to `pnpm-workspace.yaml`.
  - Run `pnpm install`, and commit `pnpm-lock.yaml`.
  - The scripts run from the repository root through `pnpm --filter ./design <script>`, so
    `cli.ts` resolves paths from the repository root (two levels up from `src/`), never from
    the working directory.
- [X] T002 [P] Add `.gitattributes` entries with `eol=lf` for:
  - `DESIGN.md`;
  - `app/src/theme.generated.css`;
  - `design/widget-map.generated.json`;
  - `docs/design/*.md`;
  - `docs/threat-model.md`;
  - `design/test/fixtures/**`.

  Add `design/preview/` to `.gitignore` (research R3, R8).
- [X] T003 Clear the high npm advisory already on master (research R13). It is `body-parser`
  <2.4.0, reached through `app` → `shadcn` → `@modelcontextprotocol/sdk` → `express`.
  - Update `shadcn` in `app/package.json` if a release pulls `body-parser` 2.4.0 or later.
  - Otherwise, add `"pnpm": { "overrides": { "body-parser": ">=2.4.0" } }` to the root
    `package.json`.
  - Confirm that `pnpm audit --audit-level high` exits 0, and that `pnpm --filter ./app build`
    still passes.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: color math, parsing, the CLI's shape and a `DESIGN.md` that holds today's values.
Every story builds on these.

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

### Tests for the foundation

- [ ] T004 [P] Write `design/test/color.test.ts`.
  - **Conversion:** `oklch()` to sRGB against reference values:
    - `oklch(1 0 0)` → white;
    - `oklch(0 0 0)` → black;
    - `oklch(0.628 0.2577 29.23)` → `#ff0000` within one 8-bit step.
  - **Alpha:** `oklch(1 0 0 / 9%)` composited over `oklch(0.17 0.012 255)`.
  - **WCAG ratios** that research R4 measured, within 0.01:
    - `foreground` on `background`: 17.94 light, 17.03 dark;
    - `input` oklch(0.60 0.02 250) on `card`: 3.94;
    - `chart-2` oklch(0.68 0.1 200) on `card`: 2.76;
    - `ring` at 50% on `background`: 1.79 light.
  - **Layer stacks:** `destructive` on `[card, destructive/10]`: 4.42 with the starting value.
- [ ] T005 [P] Write `design/test/parse.test.ts` with fixtures in `design/test/fixtures/parse/`:
  - **Front matter:** the colors with `-dark` twins, typography, rounded and spacing.
  - **`design-rules` blocks:** read per section. A key appearing in two blocks is an error.
  - **Rationale:** each token's bold-named rationale is found under its section. A token without
    one is reported.
  - **Sections:** a section out of the format's order, and a duplicate `##` heading, are errors
    (research R1).
  - **Line endings:** the same fixture with `\r\n` line endings parses identically.

### Implementation for the foundation

- [ ] T006 [P] Implement `design/src/color.ts`:
  - OKLCH to linear sRGB to sRGB, clamped;
  - compositing a `role/opacity` layer over an opaque color;
  - WCAG 2.2 relative luminance and ratio.

  No dependency. Pass T004.
- [ ] T007 Implement `design/src/parse.ts`. It uses `yaml` 2.9.1 for the front matter and for the
  ` ```design-rules ` blocks, and splits the body into the format's `##` sections. It returns:
  - the tokens by group;
  - each color's light and dark value;
  - the merged `design-rules`;
  - a map from token to rationale;
  - errors as `DESIGN.md: <what>` lines (`contracts/design-md.md`, "Errors").

  It accepts `\r\n`. Pass T005.
- [ ] T008 Implement `design/src/cli.ts`, with the subcommands `gen`, `check`, `preview` and
  `licenses`.
  - Each check returns failure lines prefixed with their file. `check` runs them all, prints
    every line and exits 1 if any failed (`contracts/outputs.md`, "Commands").
  - `gen` writes nothing when the project checker reports an error.
  - Wire only the parser for now. Later tasks add their checks to the list.
- [ ] T009 Write the first `DESIGN.md` at the root, in format 0.4.0 (`contracts/design-md.md`):
  - **Front matter:** `version: alpha`, `name: Dashboard`, `description`, and the colors.
    - Every role in `app/src/index.css` today, with today's light value and its dark value as
      `<role>-dark`. Values are not fixed yet: US2 fixes them.
    - The new roles of research R5, with their values: `ok`, `warning`, `frame`,
      `frame-foreground`, `frame-border`, `frame-header`, `frame-header-foreground`, `dialog`,
      `dialog-foreground` and `scrim`. Use `scrim` oklch(0 0 0 / 10%) in light, the value of
      today's `bg-black/10`, and a value of your choice in dark.
    - 41 roles in all.
  - **Typography:** `heading` in Bricolage Grotesque Variable 600, the `body-*` sizes in Figtree
    Variable, and `mono` in IBM Plex Mono, each with a fallback stack. Add a type scale whose
    sizes and line heights equal Tailwind's defaults for the sizes the app uses (`text-xs` to
    `text-2xl`), so nothing moves yet.
  - **Rounded and spacing:** `rounded.base: 0.625rem` and `spacing.unit: 0.25rem`.
  - **Body:** the eight sections in order. Every token is named in bold in its section's prose,
    with its rationale (FR-002).
  - **Typography section:** records the OFL-1.1 license of each font (research R7).

  With only the parser wired, `pnpm --filter ./design check` must report no error.

**Checkpoint**: the parser reads `DESIGN.md` with no error, and the color math matches research
R4.

---

## Phase 3: User Story 1 - One source for the whole visual system (Priority: P1) 🎯 MVP

**Goal**: the app's theme is generated from `DESIGN.md`, drift and format errors fail, and a
`DESIGN.md`-only change runs all of CI.

**Independent Test**: quickstart §3 and §4. Change a color, a spacing step and `rounded.base` in
`DESIGN.md` only, run `gen`, and see all three in the app. A hand edit to the generated file
fails `check`, naming the token.

### Tests for User Story 1

- [ ] T010 [P] [US1] Write `design/test/generate.test.ts`, with fixtures in
  `design/test/fixtures/generate/`.
  - The generated CSS has the three blocks of `contracts/outputs.md`, in `DESIGN.md`'s role
    order:
    - `@theme inline`, with `--font-heading`, `--font-sans`, `--font-mono`, one `--color-<role>`
      per role, `--radius-sm` to `--radius-4xl`, `--spacing`, `--text-*` with their line
      heights, and one `--shadow-*` per elevation level;
    - `:root`;
    - `.dark`.
  - The radius steps use the multipliers 0.6, 0.8, 1, 1.4, 1.8, 2.2 and 2.6 of `--radius`.
  - Changing `rounded.base` in a fixture changes only `--radius`.
- [ ] T011 [P] [US1] Write `design/test/drift.test.ts`. A committed file that differs in two
  values reports one line for each, as
  `app/src/theme.generated.css: --<role> differs from DESIGN.md (run pnpm --filter ./design gen)`.
  A file that differs only in `\r\n` passes.
- [ ] T012 [P] [US1] Write `design/test/format.test.ts`.
  - The pinned checker's JSON report is mapped to lines: errors fail, warnings print with
    `warning:` and do not fail.
  - A fixture with a broken `{rounded.nope}` reference fails.
  - The fixture of research R1's probe passes with warnings only.
- [ ] T013 [P] [US1] Write `design/test/fonts.test.ts`. It fails when a family in a fixture's
  typography has no license file in a fixture `licenses/fonts/` directory, naming the family
  (research R7).

### Implementation for User Story 1

- [ ] T014 [US1] Implement `design/src/generate.ts`, writing `app/src/theme.generated.css` as in
  `contracts/outputs.md`, with the "Generated from DESIGN.md … Do not edit." first line. Pass
  T010.
- [ ] T015 [US1] Implement `design/src/checks/drift.ts`: generate in memory and compare with the
  committed file after normalizing `\r\n`. Report every differing variable, one line each
  (FR-005). Pass T011.
- [ ] T016 [US1] Implement `design/src/checks/format.ts`.
  - Run the pinned checker as `node node_modules/@google/design.md/dist/index.js lint DESIGN.md`,
    resolved from the `design` package with `import.meta.resolve`, never `npx`.
  - Parse its JSON report, then fail on its errors and print its warnings. Pass T012.
- [ ] T017 [US1] Copy the OFL-1.1 license text of each font into `app/public/licenses/fonts/`:
  - `figtree.txt`, from `app/node_modules/@fontsource-variable/figtree/LICENSE`;
  - `bricolage-grotesque.txt`;
  - `ibm-plex-mono.txt`.

  Implement `design/src/checks/fonts.ts`, which maps each typography family to its file. Pass
  T013. Confirm the files land in `app/dist/licenses/fonts/` after `pnpm --filter ./app build`.
- [ ] T018 [US1] Wire the format, schema, drift and font checks into `check`.
  - Run `pnpm --filter ./design gen`.
  - Replace the hand-written `@theme inline`, `:root` and `.dark` blocks of `app/src/index.css`
    with `@import "./theme.generated.css";`. Keep the Tailwind and font imports, the
    `@custom-variant` and the `@layer base` rules.
  - Confirm that `pnpm --filter ./app build` passes, and that the built CSS holds the same
    values as before for every unchanged role.
- [ ] T019 [US1] Silence the format checker's `orphaned-tokens` warnings where a component
  exists. Give each `components` entry a `-dark` twin that references the dark roles, such as
  `button-primary-dark` (research R1). Run `check`, and record the checker's remaining warnings
  and their count in `research.md` R1 (quickstart §2).
- [ ] T020 [US1] Update `.github/workflows/ci.yml` (research R9).
  - **The filter:** the first `grep` becomes
    `^(design|docs/authors|docs/design|wit)/|^(DESIGN|docs/threat-model)\.md$`. Update the
    comment above it to name the new paths and cite `specs/007-host-visual-system/research.md`,
    R9.
  - **The build job:** after `pnpm install --frozen-lockfile`, add
    `pnpm --filter ./design check` and `pnpm --filter ./design test`.

**Checkpoint**: `DESIGN.md` is the only place token values are written, and `check` and `test`
pass (SC-001 and SC-004 can be walked).

---

## Phase 4: User Story 2 - Text stays readable in both themes (Priority: P1)

**Goal**: every declared pairing meets WCAG AA in both themes, every color role is covered, and
every color the app's code draws is declared.

**Independent Test**: quickstart §5. `chart-2` back at oklch(0.68 0.1 200) fails by name, and
`ring-ring/50` in a component fails the usage scan.

### Tests for User Story 2

- [ ] T021 [P] [US2] Write `design/test/contrast.test.ts`, with fixtures in
  `design/test/fixtures/contrast/`. For each kind of `data-model.md`'s Pairing, check that "the
  number is the minimum ratio":
  - `text` 4.5, `large-text` 3, `boundary` 3, `focus` 3, `chart` 3.

  It covers these failures:
  - a failing pairing reports
    `DESIGN.md: pairing chart-2 on card (chart, light): 2.76 < 3.00`;
  - a role in no pairing and not decorative fails ("Every color role appears in at least one
    pairing, as `fg` or in `on`, or is listed in `decorative` with a reason");
  - a decorative role used as a `boundary` or `focus` `fg` fails;
  - an `on` stack whose first layer is translucent fails.
- [ ] T022 [P] [US2] Write `design/test/usage.test.ts`, with fixture `.tsx` and `.css` files in
  `design/test/fixtures/usage/`.
  - `ring-ring/50` fails when `ring` is declared only at 100.
  - `outline-ring/50` inside an `@apply` fails the same way.
  - `bg-black/10` fails as a color that names no role.
  - `dark:hover:bg-input/50` is read as role `input` at 50.
  - `disabled:opacity-50` is ignored.
  - `bg-transparent`, `border-transparent` and `text-current` are ignored.
  - `text-sm`, `ring-3`, `shadow-md`, `outline-none`, `border-0` and `text-right` are not colors.
  - `text-red-500` and `bg-[#fff]` fail as colors that name no role.
  - Each failure line names the file and the utility.

### Implementation for User Story 2

- [ ] T023 [US2] Implement `design/src/checks/contrast.ts` (both themes, layer compositing) and the
  coverage rule, and wire them into `check`. Pass T021.
- [ ] T024 [US2] Implement `design/src/checks/usage.ts`. It scans `app/src/**/*.tsx` and the
  `@apply` lines of `app/src/*.css` for Tailwind color utilities: `bg`, `text`, `border`,
  `ring`, `outline`, `fill`, `stroke`, `divide`, `placeholder`, `caret`, `accent`, `shadow`,
  `from`, `via` and `to`, with any variant prefix. A utility is a color only when what follows
  the prefix, without `/opacity`, is a role, a palette color, `black`, `white` or `[…]`;
  `transparent`, `current` and `inherit` are skipped (research R4). Wire it into `check`. Pass
  T022.
- [ ] T025 [US2] Add the Colors section's `design-rules` `pairings` and `decorative` to
  `DESIGN.md`, covering every role.
  - Include every layer the components use: `input`/30, /50 and /80 in dark, `muted`/50,
    `destructive`/10, /20 and /30, `primary`/80, `secondary`/80 and `foreground`/10.
  - Declare `border` decorative, with the reason in research R4.
  - Declare the `aria-invalid` rings `destructive`/20 and /40 as layers, not `focus` pairings
    (research R4).
  - Run `check`. It must fail exactly on research R4's list. Record any other failure in
    `research.md` R4 before fixing it.
- [ ] T026 [US2] Fix the values in `DESIGN.md` (research R4), each with its rationale in the
  prose:
  - `input` oklch(0.60 0.02 250), and `input-dark` oklch(1 0 0 / 35%);
  - `chart-2` oklch(0.58 0.10 200) and `chart-3` oklch(0.60 0.13 70);
  - `destructive` oklch(0.54 0.2 27).

  Run `gen`.
- [ ] T027 [P] [US2] Make the app draw only declared colors:
  - in `badge.tsx`, `button.tsx`, `checkbox.tsx`, `input.tsx`, `select.tsx`, `switch.tsx` and
    `tabs.tsx`, under `app/src/components/ui/`, change `ring-ring/50` to `ring-ring`;
  - in `app/src/index.css`, change `outline-ring/50` to `outline-ring`;
  - in `app/src/components/ui/dialog.tsx`, change the overlay's `bg-black/10` to `bg-scrim`;
  - in `select.tsx` and `dropdown-menu.tsx`, change `shadow-md` and `shadow-lg` to
    `shadow-raised` (research R5, "Elevation").

  Run `check` until it passes with zero failures (SC-002).

**Checkpoint**: contrast, coverage and usage pass, and quickstart §5 fails as written.

---

## Phase 5: User Story 3 - Widget content cannot use the host's surfaces (Priority: P1)

**Goal**: tones and variants resolve only to shared roles, and the widget map holds nothing
host-only.

**Independent Test**: quickstart §6. Mapping `button.secondary`'s fill to `frame-header` fails
by name, and `design/widget-map.generated.json` holds no host-only role.

### Tests for User Story 3

- [ ] T028 [P] [US3] Write `design/test/hostonly.test.ts`, with fixtures in
  `design/test/fixtures/hostonly/`:
  - **Prefix rule:** `hostOnly` entries cover their prefix, so `sidebar` covers
    `sidebar-border`.
  - **A variant reaching a host-only role** reports
    `DESIGN.md: variant button.secondary maps fill to host-only role "frame-header"`.
  - **A tone reaching a host-only role** fails the same way.
  - **`ring` in a variant** fails: "A variant never names the focus style (`ring`)".
  - **Missing status icon:** a status tone (`ok`, `warning` or `error`) without an `icon` fails
    (FR-012).
  - **The generated widget map** contains `unknownTone: "default"`, role names only and no
    value.
  - **Elevation:** a level marked `hostOnly` never appears in the map.

### Implementation for User Story 3

- [ ] T029 [US3] Add the remaining `design-rules` blocks to `DESIGN.md`:
  - **Colors:** `hostOnly`, listing `sidebar`, `frame`, `dialog`, `scrim` and `ring`, with the
    reason in the prose. The prose says the split is by role, not color (research R6).
  - **Elevation & Depth:** `flat`, then `frame`, `raised` and `dialog`, the last three host-only,
    at Tailwind's `shadow-sm`, `shadow-md` and `shadow-lg` values (research R5).
  - **Components:**
    - the tones `default`, `muted`, `accent`, `ok`, `warning` and `error`, with icons
      `circle-check`, `triangle-alert` and `circle-x` for the three statuses;
    - `unknownTone: default`;
    - the variants of `text`, `list`, `table`, `chart`, `button` and `form`, each mapping to
      shared roles only, and to `rounded` steps, spacing and typography.
- [ ] T030 [US3] Implement `design/src/checks/hostonly.ts`, and extend `design/src/generate.ts`
  to write `design/widget-map.generated.json` (`contracts/outputs.md`). Extend the drift check
  to cover it. Pass T028.
- [ ] T031 [US3] Run `gen` and `check`, and confirm the map lists every tone and variant and no
  host-only role (SC-003).

**Checkpoint**: the host-only split is enforced on what widget content can select.

---

## Phase 6: User Story 4 - The frame and the dialogs are designed to be checked (Priority: P2)

**Goal**: the chrome and frame criteria exist with their checks and features, and every B4 row
of Phase 3 in the threat model cites criteria that a test or hostile case checks.

**Independent Test**: quickstart §8. A criterion without `Feature:`, a B4 row without `HS-`, or
a cited criterion checked only by review, each fails by name.

### Tests for User Story 4

- [ ] T032 [P] [US4] Write `design/test/criteria.test.ts`, with fixture criteria and threat-model
  files in `design/test/fixtures/criteria/`. Each of these fails:
  - a criterion without `Check:` or `Feature:`;
  - a `Check:` that starts with something other than `test`, `hostile case` or `review`;
  - a B4 row in Phase 3 with no `HS-` citation;
  - a citation of an unknown id;
  - a cited criterion whose `Check:` is `review` ("A cited criterion's `check` is `test` or
    `hostile case`, never `review`").

  Rows on other boundaries or phases are ignored. `\r\n` files parse.

### Implementation for User Story 4

- [ ] T033 [US4] Implement `design/src/checks/criteria.ts`, which reads
  `docs/design/host-surfaces.md` and `docs/threat-model.md`'s threats table, and wire it into
  `check`. Pass T032.
- [ ] T034 [US4] Write `docs/design/host-surfaces.md`, starting with what only the host draws and
  why. Write the chrome and frame criteria in the format of `contracts/outputs.md`, each with
  `Check:` and `Feature:` lines. Cover:
  - the frame's header and its actions belong to the host;
  - the name and icon come from the manifest, escaped, with a length limit and an empty-name
    placeholder;
  - widget content is clipped to the body, which is a containing block with paint containment
    (research R11);
  - widget content cannot cover the header;
  - a control in widget content triggers only that widget's declared actions;
  - widget content has no field for a color, font, size, class or style, and an unknown tone
    draws as the default tone and is recorded in the plugin's log (FR-014), with
    `Feature: Biblioteca de UI declarativa`;
  - the form has no password or masked field, and its inputs are never offered for autofill;
  - no plugin string reaches the opener or anything that opens a link, a file or another app;
  - status is never shown by color alone.
- [ ] T035 [US4] Update `docs/threat-model.md`.
  - Add the Phase 3 row "Widget content imitates the host's styling" on B4. Its mitigation is
    the dialog form, and it names the declarative UI feature's hostile widget as its test
    (FR-020).
  - End every B4 row in Phase 3 with its `HS-` citations, choosing criteria whose `Check:` is a
    test or hostile case.
  - Run `check` until it passes.

**Checkpoint**: the frame is designed as checkable criteria, and the threat model points to
them.

---

## Phase 7: User Story 5 - The person recognizes a real host dialog (Priority: P2)

**Goal**: the dialog's form, which widget content cannot produce, is written as criteria.

**Independent Test**: from `docs/design/host-surfaces.md`, each property in FR-019 maps to a
criterion with a test or hostile case and a feature, and each holds in forced-colors mode.

- [ ] T036 [US5] Add the dialog-form criteria to `docs/design/host-surfaces.md`:
  - a host dialog covers the chrome and every frame;
  - the rest of the window is inert for pointer, keyboard and assistive technology;
  - clicks and keys are ignored for a short moment after it appears;
  - it names the plugin or connector by name and repository id, escaped, with a length limit;
  - the safe choice is the default;
  - long content scrolls inside the dialog;
  - widget content cannot produce a layer above the frames, a scrim or an inert window;
  - none of these depends on color alone, so each holds in forced-colors mode.
- [ ] T037 [US5] Cite the dialog-form criteria from the threat model's B4 rows on fake logins and
  on widgets opening dialogs. Run `check`.

---

## Phase 8: User Story 6 - Host dialogs work with the keyboard and a screen reader (Priority: P3)

**Goal**: keyboard and screen-reader behavior of host dialogs, as criteria with checks.

**Independent Test**: each acceptance scenario of User Story 6 maps to a criterion whose
`Check:` names a keyboard-only or screen-reader check on each of the three systems.

- [ ] T038 [US6] Add the accessibility criteria to `docs/design/host-surfaces.md` (FR-018):
  - focus moves into the dialog and is trapped;
  - Escape takes the safe choice and never approves;
  - focus returns to the opener, or to the window if the opener is gone;
  - the dialog is announced with its name and description;
  - nothing behind it is reachable by assistive technology;
  - dialog and frame transitions follow reduced motion.

  Run `check`.

**Checkpoint**: every story's criteria exist, and the coverage check passes.

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: preview, npm audit and licenses, docs, walks, review and merge

- [ ] T039 [P] Write `design/test/preview.test.ts`. The page contains no `<script`, no
  `http://`, no `https://` and no `@import`. It shows both themes, each role with its pairings'
  ratios, the scales, the tones with their icon names, every variant, and a chrome, frame and
  dialog sketch.
- [ ] T040 Implement `design/src/preview.ts`, writing `design/preview/index.html` (research R8).
  Pass T039.
- [ ] T041 [P] Write `design/test/licenses.test.ts`, with a fixture `node_modules` tree. A package
  whose `license` is outside the allowlist fails, naming it. The allowlist is `deny.toml`'s list
  plus `BSD-2-Clause`, `0BSD`, `BlueOak-1.0.0`, `CC0-1.0` and `OFL-1.1` (research R13). SPDX
  `OR` expressions pass when one side is allowed.
- [ ] T042 Implement `design/src/licenses.ts` over the installed packages of every workspace
  package. Run it, and bring any package that fails to the maintainer before adding a license
  to the list. Pass T041.
- [ ] T043 Update the CI workflows (research R8, R13).
  - **The build job** in `.github/workflows/ci.yml`:
    - add `pnpm --filter ./design licenses`;
    - on Linux only, add `pnpm --filter ./design preview`, then
      `actions/upload-artifact@cf430e030ddbb5b0abf93d22962f4752f3646cd9 # v7.0.2`, with name
      `design-preview` and path `design/preview/`.
  - **A new `pnpm-audit` job** in `ci.yml`, gated on code changes, which runs
    `pnpm audit --audit-level high`. Add it to the "CI result" job's `needs`.
  - **A weekly `pnpm audit` job** in `.github/workflows/audit.yml`.
  - The new actions are pinned to a commit SHA, like the rest.
- [ ] T044 [P] Add a "The visual system" section to `CONTRIBUTING.md`:
  - edit `DESIGN.md` only, then run `pnpm --filter ./design gen` and `check`;
  - what each check means;
  - how to open the preview;
  - that new frame or dialog work must pass `docs/design/host-surfaces.md`.
- [ ] T045 Run every local check, and record the times of `check` and `test` (target: under 10
  seconds, plan's Performance Goals):
  - `pnpm --filter ./design check`, `test` and `licenses`;
  - `pnpm audit --audit-level high`;
  - `pnpm --filter ./app build`.

  Nothing under `runtime/`, `wit/` or `plugins/` changed, so `git diff master --stat` confirms
  the Rust checks are unaffected. Confirm FR-021: no file under `runtime/tests/hostile*` or
  `plugins/hostile/` changed.
- [ ] T046 Walk `specs/007-host-visual-system/quickstart.md` §1–§10, including each mutation, and
  fix any drift between it and the tooling. §7 and §10 need the pull request's CI, so walk them
  in T049.
- [ ] T047 Review `DESIGN.md` and `docs/design/host-surfaces.md` independently.
  - A fresh agent with none of this feature's context reads both against the spec's FRs and the
    constitution's Principles V, VII and VIII. It also checks the WCAG claims against
    `design/src/color.ts`'s output.
  - It returns each claim that is wrong, unsupported or uncheckable, with file and line.
  - Check each finding at the source before changing anything, and bring choices with real
    trade-offs to the maintainer.
- [ ] T048 Before the merge, and only when the maintainer asks, update the development guide.
  - Check the Phase 3 items "Sistema visual em `DESIGN.md`…" and "Spec de design da moldura e
    dos diálogos do host…".
  - In the first item, note that the `@theme` is generated, the contrast lint runs in CI and
    `DESIGN.md` left the docs filter.
- [ ] T049 Only once the maintainer has pushed and opened the pull request, as they ask:
  - walk quickstart §7, with a `DESIGN.md`-only commit and a fixture-only commit, each running
    every job on the three systems;
  - walk §10, downloading the `design-preview` artifact;
  - record both in `research.md` R9.
- [ ] T050 Merge:
  - The maintainer reviews the pull request.
  - CI is green on the three systems.
  - `grep -n '^- \[ \]' specs/007-host-visual-system/tasks.md` shows only this task.
  - Tick this box in the pull request's last commit, wait for green CI on that head, and merge
    with `gh pr merge --merge --match-head-commit <sha>`.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies. T003 can run beside T001 and T002.
- **Foundational (Phase 2)**: depends on T001. T004 to T008 before T009, which needs the parser
  to check it. Blocks every story.
- **US1 (Phase 3)**: depends on Phase 2. It is the MVP, and the other stories add checks to its
  `check`.
- **US2 (Phase 4)**: depends on US1, because the fixes are regenerated into the theme.
- **US3 (Phase 5)**: depends on US1. It can run beside US2, but T029 and T025 both edit
  `DESIGN.md`, so do them one after the other.
- **US4 (Phase 6)**: depends on Phase 2 only, since the criteria check is independent of the
  theme. It can run beside US1–US3.
- **US5 and US6 (Phases 7, 8)**: depend on US4, because they write into the same document and
  rely on its check.
- **Polish (Phase 9)**: T039–T044 after US3. T045–T047 after every story. T048 at the
  maintainer's request. T049 after the PR opens. T050 last.

### Within Each Story

- Tests are written first and fail.
- Then the check or generator that makes them pass.
- Then the `DESIGN.md` or docs content, until `check` passes.

### Parallel Opportunities

- Phase 1: T002 and T003, beside T001's install.
- Phase 2: T004 and T005 (tests), then T006 (color) beside T007 (parser).
- US1: T010–T013, the four test files, together.
- US2: T021 and T022 together. T027 (the component edits) beside T026 once T024 lands.
- US4 can proceed in parallel with US1–US3 by another contributor.
- Polish: T039, T041 and T044 together.

## Parallel Example: User Story 1

```text
Task: "T010 [P] [US1] Write design/test/generate.test.ts"
Task: "T011 [P] [US1] Write design/test/drift.test.ts"
Task: "T012 [P] [US1] Write design/test/format.test.ts"
Task: "T013 [P] [US1] Write design/test/fonts.test.ts"
```

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1 and Phase 2.
2. Phase 3 (US1): `DESIGN.md` is the source, the theme is generated, and CI runs on it.
3. **Stop and validate**: quickstart §3 and §4.

### Incremental Delivery

1. US1, then US2: contrast holds, and the app draws only declared colors.
2. US3: widget content's reach is fixed.
3. US4, then US5 and US6: the criteria, with their coverage in CI.
4. Polish: the preview, npm audit and licenses, docs, walks, review and merge.

Each story is committed task by task on `007-host-visual-system`, and pushed only when the
maintainer asks.
