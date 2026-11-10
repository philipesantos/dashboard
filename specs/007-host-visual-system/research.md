# Research: Host Visual System and the Design of the Host Frame and Dialogs

All measurements below were taken on 2026-11-10, against `app/src/index.css` as merged in PR #16.
Contrast is WCAG 2.2 relative luminance on sRGB. Translucent colors are composited over the
surface beneath before measuring.

## R1. The DESIGN.md format: version, and how the project fits in it

**Decision**: pin the format at version `0.4.0` of `google-labs-code/design.md`. That is tag
`0.4.0`, commit `9bf8eae67128b6cc55ad9bf86665767deb4c11cd` (2026-07-27, Apache-2.0), published
to npm as `@google/design.md@0.4.0`, whose CLI is the format's own checker. `DESIGN.md` has two
layers:

1. **The format layer**: the YAML front matter the format defines (`version: alpha`, `name`,
   `description`, `colors`, `typography`, `rounded`, `spacing`, `components`), and the `##`
   sections in the format's order:
   1. Overview
   2. Colors
   3. Typography
   4. Layout
   5. Elevation & Depth
   6. Shapes
   7. Components
   8. Do's and Don'ts

   Color values stay in `oklch()`, which 0.4.0 accepts. Each light value is named by its role,
   such as `primary`, and each dark value takes the same name with a `-dark` suffix, such as
   `primary-dark`. The format accepts unknown color names, so every tool reading `DESIGN.md`
   sees both themes.
2. **The project layer**: fenced blocks with the info string `design-rules`, placed in the
   section they belong to. They hold what the format has no place for:
   - the host-only marks;
   - the pairings and their kinds;
   - the tones and component variants that widget content can select;
   - the elevation levels;
   - the decorative marks.

   Their schema is in [contracts/design-md.md](contracts/design-md.md).

The rationale for each token is prose. Each token's name appears in bold in its section's prose,
followed by why it exists. The project's checker fails when a token is not named that way.

**Rationale**:
- 0.4.0 is the latest release and is about 3½ months old.
- The format's spec says that unknown section headings are kept, unknown color names are
  accepted and unknown component properties only warn. So the two layers coexist without errors.
- The fenced blocks use their own info string, so that a future version of the format that reads
  ` ```yaml ` blocks as tokens will not read them twice.
- Rationale as bold-named prose is what agents and people already read. A rationale field in
  YAML would repeat it away from the text.

**Alternatives considered**:
- Converting to hex, the format's recommended default. Rejected: it loses the OKLCH hue and
  lightness that the design reasons in, and 0.4.0 does not require it.
- Unknown top-level keys in the front matter. Rejected: the tool's `unknown-key` rule warns on
  them, and a later version may reject them.
- A second file beside `DESIGN.md`. Rejected: FR-001 asks for one source.

## R2. Tooling language: Node.js in the pnpm workspace

**Decision**: the generator and checks are TypeScript in a new workspace package, `design/`, run
directly by Node.js 24 with its built-in type stripping, so there is no compile step. The tests
use `node --test`. Its development dependencies are pinned to exact versions in
`design/package.json` and locked in `pnpm-lock.yaml`:

- `@google/design.md` `0.4.0`, the format checker;
- `yaml` `2.9.1` (ISC, 2026-09-11), which parses the front matter and the `design-rules` blocks.

The OKLCH to sRGB conversion and the WCAG math are written in the project, in about 40 lines,
and tested against reference values. No color library is added.

**Rationale**:
- CI already sets up Node.js 24 and pnpm in the build job, to build the app, so this adds no
  toolchain.
- The output is CSS for the app, and the format's checker is an npm package. A Rust generator
  would still need Node.js for the checker.
- Type stripping keeps the source typed without a build step. The code avoids enums and other
  syntax that stripping cannot erase.

**Alternatives considered**:
- A Rust `cargo xtask design`. Rejected: it would add a YAML crate to `deny.toml`'s scope and
  still need Node.js for the checker.
- `culori` for color math. Rejected: one more dependency for one conversion.
- Running the checker with `npx` at an unpinned version. Rejected: FR-001 asks for a pinned
  version.

**Observed by running the 0.4.0 checker** (2026-11-10, on probe files shaped like
[contracts/design-md.md](contracts/design-md.md)):

- **Accepted with no error, exit 0:** `oklch()` values with and without alpha, `-dark` color
  names and a `design-rules` block.
- **Errors, exit 1:** a broken reference (`broken-ref`).
- **Contrast in `components` is only a warning**, even at 1.52:1. The checker converts oklch to
  sRGB for this (`textColor (#4886b8) on backgroundColor (#0068a7)`), but the project's own check
  is still the guarantee (R4).
- **Sections out of order are only a warning.** A duplicate `## Colors` heading drew no finding
  at all, although the format's spec says a duplicate is an error. The project checker
  therefore enforces the section order and rejects duplicate headings itself.
- **Every `-dark` color draws an `orphaned-tokens` warning** unless a component references it.
  Light roles that no component references drew none on the probe. These warnings print in CI
  and do not fail it. Making each component's dark twin (`button-primary-dark`) reference the
  dark roles would silence them and also make the format's own contrast warning cover dark;
  the implementation does this where a component exists.

If the checker reports an error on the real `DESIGN.md` that the format's text does not explain,
it is brought to the maintainer before working around it.

**On the real `DESIGN.md`** (T019, 2026-11-10): zero errors, and one kind of warning,
`orphaned-tokens`, for the 66 color names (33 roles, light and dark) that no format component
references. Fourteen components with `-dark` twins reference the other 8 roles in both themes.
The 66 are the surfaces (`card`, `popover`, `muted`, `accent`), the edges, the status and chart
colors, the chrome, the frame, the dialog and the scrim. They are used through `design-rules`,
which the format does not read, so the warnings are expected. The project checker prints them as
one line. The format's contrast rule drew no warning.

## R3. What the app reads: one generated CSS file, committed

**Decision**: `pnpm --filter ./design gen` writes `app/src/theme.generated.css`. The file opens
with a "generated from DESIGN.md, do not edit" line and holds three blocks:

- `@theme inline`, mapping Tailwind's color, radius and font variables to the roles, and
  setting the spacing unit (`--spacing`), the type scale (`--text-*`) and the elevation shadows
  (`--shadow-*`). This is how a spacing or size change in `DESIGN.md` reaches every Tailwind
  utility;
- `:root`, with the light values;
- `.dark`, with the dark values.

`app/src/index.css` keeps the Tailwind and font imports and the `@layer base` rules, and imports
the generated file in place of today's hand-written blocks. `pnpm --filter ./design check`
generates in memory and compares with the committed file, naming every token that differs,
one line each (FR-005). `.gitattributes` sets `eol=lf` on every file the checks read or write:
- `DESIGN.md`;
- the generated files;
- `docs/design/*.md` and `docs/threat-model.md`;
- `design/test/fixtures/**`.

Every parser also accepts `\r\n`, so the comparison holds on Windows.

**Rationale**:
- A committed file shows token changes in review, and the app builds without running the
  generator.
- The drift check keeps the file honest on all three systems.
- shadcn/ui components read the same CSS variable names they read today, so no component
  changes for the colors.

**Alternatives considered**:
- A Vite plugin that generates at build time. Rejected: it hides the values from review and puts
  YAML parsing in every build.
- The format tool's own `export --format tailwind`. Rejected: it knows nothing of dark values,
  host-only marks or shadcn's variable names.

## R4. Contrast: what is checked, and what the starting theme fails

**Decision**: the check measures every pairing declared in `design-rules`, in both themes. A
pairing is a foreground role drawn on a stack of layers: a base surface, then translucent fills
composited in order. For example, `destructive` text on `card` with `destructive` at 10% on top.
Each pairing has a kind:

| Kind | Minimum | WCAG |
| --- | --- | --- |
| `text` | 4.5:1 | 1.4.3 |
| `large-text` (at least 24 px, or 18.66 px bold) | 3:1 | 1.4.3 |
| `boundary` (a control's edge that identifies it) | 3:1 | 1.4.11 |
| `focus` | 3:1 | 1.4.11 |
| `chart` (a mark that carries data) | 3:1 | 1.4.11 |

A role may instead be marked `decorative`, with a reason, when WCAG's exemptions apply: card
edges and dividers whose meaning something else carries.

**Measured on the starting theme** (fails only, ratio found → required):

| Pairing | Light | Dark | Fix in `DESIGN.md` |
| --- | --- | --- | --- |
| `input` boundary on `background` / `card` | 1.39 / 1.43 | 1.47 / 1.52 | `input` to oklch(0.60 0.02 250) light (3.83 / 3.94), and to oklch(1 0 0 / 35%) dark (3.19 / 3.22), which stays translucent like today's 14% |
| `ring` at 50% (`ring-ring/50`) on `background` | 1.79 | 2.10 | draw the focus ring solid: `ring` alone reaches 3.58 light, 4.57 dark on `card` |
| `chart-2` on `card` | 2.76 | ok | oklch(0.58 0.10 200), 4.08 |
| `chart-3` on `card` | 2.34 | ok | oklch(0.60 0.13 70), 4.06 |
| `destructive` text on `destructive`/10 over `card` (soft badge and button) | 4.42 | 4.58 | `destructive` light to oklch(0.54 0.2 27) |
| `border` on `background` / `card` | 1.27 / 1.31 | 1.25 / 1.29 | none: marked `decorative` (card edges and dividers), since controls carry their own `input` boundary |

`input` is also a fill: `bg-input` for the unchecked switch, and in dark mode `bg-input/30`,
`/50` and `/80` for fields, selects, tabs and the switch. With the dark value at 35% white, those
fills become a little lighter than today's. Text on them stays readable, for example
`foreground` on `input`/30 over `background` at 13.05. They are declared as layers in pairings,
and SC-006 lists the change.

Every other declared pairing passes. For example, `foreground` on `background` is 17.94 light
and 17.03 dark, and `muted-foreground` on `muted` is 4.83 light and 5.65 dark.

**The usage scan.** A pairing only proves contrast for what is declared. The check also scans
`app/src/**/*.tsx` and the `@apply` rules of `app/src/*.css` for Tailwind color utilities, such
as `text-destructive`, `ring-ring/50` and `outline-ring/50` in `index.css`'s base layer. A color
utility that names no role, such as Tailwind's palette, `black` or `white`, fails the scan, unless
it is declared as a layer or pairing in its own right. For example, the demo dialog's
`bg-black/10` overlay becomes the `scrim` role. `opacity-*` on disabled controls is not scanned:
WCAG exempts inactive components from contrast, and the spec makes no promise for them.
`transparent`, `current` and `inherit` are not scanned either. They add no color of their own:
`transparent` shows the layer beneath, and `current` and `inherit` repeat a text color that is
already checked. The components use them 15 times today, as in `bg-transparent` and
`border-transparent`.

A utility counts as a color only when what follows its prefix, without the `/opacity`, is a
color role, a Tailwind palette color (`red-500`), `black`, `white` or an arbitrary `[…]` value.
The same prefixes also name sizes and styles, as in `text-sm`, `ring-3`, `shadow-md`,
`outline-none`, `border-0` and `text-right`, and those are not colors.
Each role and opacity the app uses must appear in a declared pairing, as a layer in one, or as
decorative. Otherwise the check fails, naming the file and the utility. This is how the solid
focus ring is enforced: once `ring` is declared only at full opacity, `ring-ring/50` fails until
the seven components that use it are changed. The `aria-invalid` rings at `destructive`/20 and
/40 sit beside the solid focus ring and mark an error that the field's text also states. They are
declared as layers (decorative tint), not as `focus` pairings. The components keep every other
opacity they use, and `DESIGN.md` declares each one.

**Rationale**: the measurements show the default shadcn look fails WCAG 1.4.11 in both themes
(input edges, focus ring). The lint has to run on what the components really draw, opacity
included, not only on the token table.

**Alternatives considered**:
- The format tool's contrast rule. Rejected: it only checks `components` with a background and
  a text color, and only warns.
- APCA. Rejected: it is not yet a WCAG requirement.
- Rewriting the components to drop opacity. Rejected: that is a larger change than declaring
  what they use.

## R5. New roles: status tones, frame and dialog surfaces

**Decision**: `DESIGN.md` adds the following roles, each with a `-dark` value:

- `ok` and `warning`, as text and icon colors. Measured:
  - `ok` oklch(0.50 0.12 150): 5.52 on `background` light;
  - `warning` oklch(0.52 0.12 65): 5.53 light;
  - dark values oklch(0.76 0.14 150) and oklch(0.80 0.14 80): 9.40 and 10.09.
- `error`, which the tones map to the existing `destructive` (no new value).
- `frame`, `frame-foreground`, `frame-border`, `frame-header` and `frame-header-foreground`, for
  the widget frame.
- `dialog`, `dialog-foreground` and `scrim`, for host dialogs.

The app's chrome uses shadcn's existing `sidebar-*` roles.

**Elevation.** The levels `frame`, `raised` and `dialog` start at Tailwind's `shadow-sm`,
`shadow-md` and `shadow-lg` values, so nothing moves. The generator writes them as
`--shadow-frame`, `--shadow-raised` and `--shadow-dialog`. The demo's menus (`select.tsx` and
`dropdown-menu.tsx`) draw `shadow-raised` in place of `shadow-md` and `shadow-lg`, so the
elevation `DESIGN.md` documents is the one the app draws. The submenu's shadow becomes the
menu's, a little smaller than today. The active tab's `shadow-sm` is a control's detail, not a
level, and stays.

**Rationale**:
- The kit needs ok and warning tones (FR-012), and shadcn has only `destructive`.
- The frame and dialog get roles of their own, so that they can be host-only without making
  `card` and `popover` host-only. The demo's cards and menus use those.

**Alternatives considered**: reusing `card` for the frame. Rejected: `card` must stay shared for
widget content's own panels.

## R6. Host-only tokens and the widget map

**Decision**: the `design-rules` block in the Colors section lists the host-only roles:

- `sidebar` and every `sidebar-*` role (the chrome);
- the five `frame*` roles;
- `dialog`, `dialog-foreground` and `scrim`;
- `ring` (the host's focus style).

The elevation levels above `frame` are also host-only. The Components section declares the tones
(default, muted, accent, ok, warning, error) and each declarative UI component's variants (text,
list, table, chart, button, simple form). Each tone or variant is a set of roles for its text,
fill and border.

The generator writes `design/widget-map.generated.json`, holding the tones and variants resolved
to role names, and never a host-only one. The future kit reads only this file. The check fails
when a tone or variant resolves to a host-only role, naming both (FR-015).

**The focus ring is not part of any variant.** The host draws its focus style on every control,
including the controls in widget content. Widget content cannot choose it, and cannot draw it
anywhere else. That is why `ring` is host-only while widget buttons still show focus.

**Rationale**: the boundary is the only path from widget content to styling. The declarative UI
has no field for a color, class or style (FR-014), so the map is the whole surface the kit can
reach. This check is an ordinary test (FR-021). The security rule it supports, the dialog's form,
is tested with the features that draw dialogs.

**The split is by role, not by color.** Today `sidebar-primary`, `sidebar-accent` and the other
`sidebar-*` roles hold the same values as `primary`, `accent` and so on. Widget content can
therefore reach identical colors through shared roles. That is intended: the spec's clarification
shares the accent, and FR-019 makes the dialog's form, never a color, what tells the host apart.
The split keeps the host's surfaces, elevation and focus style out of what widget content can
select. It is not a color-distance control, and the plan does not claim one.

**Alternatives considered**:
- A separate CSS file for host-only variables. Rejected: in a single webview, CSS variables are
  global, so splitting files would prove nothing. The boundary that matters is what widget
  content can select, and that is the map.

## R7. Fonts

**Decision**: keep the three `@fontsource` packages added in PR #16, all OFL-1.1 by their
`package.json`. `DESIGN.md`'s typography tokens name the families as the packages declare them:
`Figtree Variable`, `Bricolage Grotesque Variable` and `IBM Plex Mono`. Each has a fallback
stack. The packages bundle the font files into `app/dist`, so nothing is fetched at run time and
the coming CSP needs no font origin (FR-007). `DESIGN.md`'s Typography section records the
license of each.

OFL-1.1 requires every distributed copy to carry the copyright notice and the license text, and
Vite bundles only the font files. So the license texts are copied from the three packages into
`app/public/licenses/fonts/` and land in `app/dist`, which the Tauri crate embeds in every
installer. `check` fails when a family named in `DESIGN.md`'s typography has no license file
there.

**Rationale**: OFL-1.1 allows bundling in an Apache-2.0 app as long as the fonts keep their own
license. The files are already bundled.

**Alternatives considered**: system fonts only. Rejected: they lose the chosen identity, and the
look would vary by system.

## R8. The preview page

**Decision**: `pnpm --filter ./design preview` writes `design/preview/index.html`, which is not
committed (`.gitignore`). It is a static page with no script and inline CSS, rendering:

- both themes side by side;
- the color roles with their pairings and ratios;
- the type scale;
- the spacing and radius scales;
- the tones with their status icons;
- each declarative UI component's variants;
- sketches of the chrome, a widget frame and a host dialog over its scrim.

The Linux build job uploads it with `actions/upload-artifact` v7.0.2, pinned to commit
`cf430e030ddbb5b0abf93d22962f4752f3646cd9` like the workflow's other actions, as the
`design-preview` artifact (SC-007).

**Rationale**: the maintainer reviews the visual system without building the app, as with the
theme comparison page in the conversation that preceded this feature.

**Alternatives considered**:
- Committing the preview. Rejected: generated noise in every diff.
- Screenshots of the app. Rejected: they need a display and a browser in CI.

## R9. CI

**Decision**: in `.github/workflows/ci.yml`:

1. The filter treats as code:
   - `DESIGN.md`;
   - `docs/design/`, where the criteria live (R10);
   - `docs/threat-model.md`, which the criteria coverage check reads.

   - `design/`, whose test fixtures are Markdown files the tests read as code.

   The first `grep` becomes
   `^(design|docs/authors|docs/design|wit)/|^(DESIGN|docs/threat-model)\.md$`, and the comment
   above it names the new paths.
2. The build job runs `pnpm --filter ./design check` and `pnpm --filter ./design test` after
   `pnpm install`, on all three systems. The `check` script covers:
   - the format checker at 0.4.0, failing on its errors and printing its warnings;
   - the project-layer schema and the bold-named rationale;
   - contrast;
   - the usage scan;
   - the host-only map;
   - drift of the generated files;
   - criteria coverage.
3. On Linux, the job runs `pnpm --filter ./design preview` and uploads the page.

The constitution needs no change: its rule already says Markdown the build uses as code is not
documentation.

**Rationale**: FR-006 and SC-004. The checks take seconds, so running them on the three systems
costs little and catches line-ending issues on Windows.

**Alternatives considered**: a separate workflow. Rejected: the required "CI result" check
aggregates the jobs of `ci.yml`.

## R10. Where the criteria live and how coverage is checked

**Decision**: the frame and dialog design is written as `docs/design/host-surfaces.md`, outside
`specs/`, because later features cite it as a standing reference. Each criterion has:

- an id (`HS-01`, `HS-02` and so on);
- a statement;
- `Check:` (automated test, hostile case or review step);
- `Feature:` (the later Phase 3 item that runs it, named as in the development guide).

The check parses the file and fails in each of these cases:
- a criterion lacks either field;
- a threat-model row selected below cites no criterion, or cites an id that does not exist
  (SC-005);
- a criterion that some row cites has a `Check:` of `review` (Principle VIII: the threat model's
  rows are cases that "must fail in CI").

The selected rows are every row of `docs/threat-model.md` whose boundary column includes B4 and
whose phase is Phase 3. Selecting by boundary and phase is mechanical, so no row is missed by
wording. Each gains `HS-` references in its mitigation cell.

The `Feature:` line names an item of the development guide, which lives outside the repository.
So CI can check that the line exists, not that the item does. The review of each later feature
checks that its criteria were run.

**Rationale**: a durable document that later specs link to. The coverage check turns the threat
model's promise into something CI can verify.

**Alternatives considered**: keeping the criteria in this feature's `contracts/`. Rejected: specs
are history, and the criteria are a standing rule.

## R11. Dialog structure: what the later features build on

**Decision**: the criteria fix the following, implementation-neutral, for the dialog feature:

- **The dialog covers everything.** A host dialog is drawn in a layer above the app's own chrome
  and every frame. Today that chrome is the in-webview header and sidebar, since the window
  title bar belongs to the operating system.
- **The rest is inert.** Everything else becomes inert for pointer, keyboard and assistive
  technology until the dialog closes.
- **Focus.** Focus moves to the dialog's safe choice on open and returns to the opener on close.
- **Escape** triggers the safe choice.
- **Announcement.** The dialog has an accessible name and description.
- **Input delay.** Clicks and keys are ignored for a short moment after the dialog appears, so
  input aimed at what was under the pointer or the caret cannot answer it. The dialog feature
  measures the length.
- **Who is asking.** The dialog names the plugin by its manifest name and its repository id
  (`github.com/owner/repo`), drawn by the host as escaped text with a length limit, as the frame
  does.
- **Links.** Today the app holds the `opener:default` capability
  (`app/src-tauri/capabilities/default.json`). The declarative UI has no link component. The
  criteria require that no plugin string reach the opener or any call that opens a link, a file
  or another app.
- **Forms.** The form component has no password or masked field. Its inputs set
  `autocomplete="off"` and are never marked as credentials, so no webview offers to fill or save
  them.

Radix Dialog, already used by the demo's `dialog.tsx`, provides the inert background (by hiding
its siblings from assistive technology and blocking pointer events), the focus trap and Escape
handling, in every engine the app ships: WebView2 on Windows, WKWebView on macOS and an
embedded Chromium on Linux. The criteria name the behavior, not the library.

For the frame, the criteria require that the frame body be a containing block for everything in
it, with paint containment and clipping. Then even a positioned element in widget content cannot
leave the frame. Widget content has no style field, so this is defense in depth.

**Rationale**: FR-017 to FR-019 describe what a host dialog does. These are the web mechanisms
that make it true, recorded so that the later features do not rediscover them.

## R12. Light or dark by default

**Decision**: no change. The demo already starts in the theme the system prefers
(`app/src/App.tsx`, `useState(() => matchMedia("(prefers-color-scheme: dark)").matches)`), and
its switch toggles it. That matches the spec's assumption.

## R13. npm dependencies: advisories and licenses

**Decision** (the maintainer's choice, 2026-11-10): npm gets the same treatment as Rust.

- **Advisories.** `pnpm audit --audit-level high` runs:
  - in `ci.yml`, as its own job gated on code changes, beside `cargo audit`;
  - in `audit.yml`, every week, so new advisories surface without a change.
- **Licenses.** `pnpm --filter ./design licenses` reads the `license` field of every installed
  package and fails on one outside an allowlist. The allowlist is `deny.toml`'s list plus the
  licenses common in npm and compatible with an Apache-2.0 client:
  - `BSD-2-Clause`, `0BSD`, `BlueOak-1.0.0` and `CC0-1.0`;
  - `OFL-1.1`, for the fonts, which ship under their own license (R7).

  It runs in CI with `check`. `pnpm licenses` printed nothing on pnpm 12.14.0, so the package
  reads `package.json` files itself.

**Measured on master today**, before this feature: `pnpm audit` reports 1 high, 1 moderate and 1
low advisory, all reached through the `shadcn` CLI package in `app/`'s dependencies:

| Severity | Package | Path | Fixed in |
| --- | --- | --- | --- |
| high | `body-parser` <2.4.0 (denial of service) | `shadcn` → `@modelcontextprotocol/sdk` → `express` | 2.4.0 |
| moderate | `minimatch` <11.0.1 | `shadcn` → `ts-morph` | 11.0.1 |
| low | `deepmerge` ≤4.3.1 | `shadcn` | none |

None of them runs in the app: the `shadcn` package is a CLI, and the app only imports its
`tailwind.css`. The implementation still has to clear the high one before the audit job can
pass, by updating `shadcn` or with a `pnpm.overrides` entry for `body-parser` `>=2.4.0`. The audit
fails only at high and above, so the moderate and low ones are left as they are. That is
recorded with the job.

**Done in T003** (2026-11-10): `pnpm update -r body-parser` moved it to 2.4.0 inside the
existing ranges, with no override. Installing `@google/design.md` 0.4.0 then brought a new high
advisory, `zod` <4.6.6 (GHSA-mrhx-c7xc-m3w5, memory exhaustion on oversized constrained
collections). The checker's CLI bundles its own copy of `zod` 3.25.76 inside `dist/index.js`, so
an override would silence the audit without changing the code that runs. That copy only reads
this repository's `DESIGN.md` and fixtures at build time, and never ships in the app.
`pnpm-workspace.yaml` therefore lists the advisory under `auditConfig.ignoreGhsas`, with this
reason and the condition for dropping it: a design.md release on `zod` 4.6.6 or later.

**Rationale**:
- `cargo audit` and `cargo deny` see only Rust, while npm packages run in CI on three systems
  and on contributors' machines (threat T4).
- The constitution's MUST names the cargo tools. This extends the same intent to npm without an
  amendment.
- The weekly run matches how `audit.yml` already handles Rust.

**Alternatives considered**:
- Weekly only, which never blocks a pull request.
- Recording the gap. Rejected by the maintainer.
