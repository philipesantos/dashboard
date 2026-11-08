# Research: Host Visual System and the Design of the Host Frame and Dialogs

Decisions for [plan.md](plan.md). Measurements were made on 2026-11-08 with the Radix data and
the format specification pinned below, by a throwaway script outside the repository.

## R1. Which version of the DESIGN.md format, and how the project fits in it

**Decision**: pin the format specification at tag `0.4.0` of `google-labs-code/design.md`
(commit `9bf8eae67128b6cc55ad9bf86665767deb4c11cd`, 2026-07-27, `docs/spec.md` SHA-256
`8f297a7b…894e691a`, Apache-2.0), vendored unmodified with its license under
`design/format/0.4.0/` and pinned by that hash, so the checker's reading can be reviewed against
the text it follows. `DESIGN.md` has two layers:

1. **The format layer**: the YAML front matter the format defines (`version`, `name`,
   `description`, `colors`, `typography`, `rounded`, `spacing`, `components`), and the eight `##`
   sections in the format's order. `typography`, `rounded`, `spacing` and `components` are the
   source for those tokens. `colors` holds the default appearance resolved for both themes (light
   names as roles, dark ones with a `-dark` suffix), so any tool or agent reading the format sees a
   coherent palette.
2. **The project layer**: fenced blocks whose info string is `design-roles`, inside the matching
   sections, holding what the format has no place for: each color role as a Radix step per theme,
   the elevation levels and their shadows, the radius set a person can pick, the status scales,
   the host-surface marks and a rationale for every token of either layer
   ([contracts/design-md.md](contracts/design-md.md)).

The checker validates the format layer against 0.4.0 (required keys, types, `{path}` references
that resolve, section order, no duplicate section) and the project layer against its own schema,
and fails when the front matter's `colors` differ from the roles resolved at the default
appearance.

**Rationale**: 0.4.0 parses only the front matter. `main` (commit `1ed38aa`, 2026-11-06) also
merges fenced ` ```yaml ` blocks, two days old and not yet tagged; pinning a release avoids
building on it. The project blocks use their own info string, so neither version reads them as
tokens and no top-level key is defined twice. The format has no themes, no shadows, no notion
of a step in a scale and no per-token rationale, so these live in the project layer, which the
format allows (unknown sections are kept, unknown content is not an error).

**Alternatives considered**: everything in the front matter under unknown keys (the format's
tool warns on possible typos of top-level keys, and a future version may reject them); a second
file next to `DESIGN.md` (two sources, which FR-001 forbids); the 744 Radix values as color
tokens (unreadable, and they would be copied from data the project already pins, R2).

## R2. Radix Colors as data

**Decision**: vendor `src/light.ts`, `src/dark.ts` and `src/blackA.ts` of `radix-ui/colors` at the commit of the
npm release 3.0.0 (`c4d0e50006c71ec51c4d6ff349062ab261b57e67`, 2023-10-02; identical to `main` on
2026-11-08), with its MIT `LICENSE`, under `design/radix/3.0.0/`. A test pins each file by
SHA-256 (`light.ts` `1a972548…8329acbd`, `dark.ts` `d8599fe6…f03826c68`, `blackA.ts` recorded
when vendored), after turning `\r\n` into `\n` as the contract pins do, and `.gitattributes` keeps
`design/` text files LF on every checkout. The generator reads the solid scales (`gray`,
`grayDark`, …) and `blackA`, Radix's overlay scale, which the scrim and every shadow use in both
themes (gray step 12 is near white in the dark theme, `#eeeeee`, so it cannot darken anything);
never the per-scale alpha (`A`) or P3 (`P`) exports.

The files are read by a strict line reader in `design-gen`, not a TypeScript parser: it accepts
only `export const <name> = {`, `  <name><step>: "<#rrggbb or rgba(r, g, b, a)>",`, `};` and
blank lines; refuses any other line, a scale without exactly 12 steps, a step out of order, and a
set of scale names other than the expected 31 solid scales plus `blackA`.

**Rationale**: the constitution uses Radix as plain color data; the files are the upstream
source, so a reviewer can diff an upgrade line by line. No npm, no Node.js.

**Alternatives considered**: the npm package (needs Node.js, R4); a JSON copy made by hand
(a second, unreviewable source).

## R3. Measuring hue, and which colors a person can pick

**Decision**: hue and chroma in OKLCH, on step 9 of the light scale (step 9 is nearly the same in
both themes). A scale with chroma under 0.08 counts as neutral. Measured:

| Scale | Chroma | Hue | Scale | Chroma | Hue |
| --- | --- | --- | --- | --- | --- |
| crimson | 0.213 | 1.3° | mint | 0.100 | 178.0° |
| ruby | 0.195 | 13.2° | teal | 0.114 | 182.0° |
| red | 0.193 | 23.0° | sky | 0.103 | 217.8° |
| tomato | 0.194 | 33.3° | cyan | 0.122 | 221.7° |
| bronze | 0.046 | 44.2° | blue | 0.193 | 251.8° |
| orange | 0.191 | 45.0° | indigo | 0.191 | 267.0° |
| brown | 0.078 | 61.0° | iris | 0.184 | 278.3° |
| gold | 0.049 | 77.7° | violet | 0.179 | 288.0° |
| amber | 0.157 | 84.1° | purple | 0.183 | 305.9° |
| yellow | 0.184 | 100.9° | plum | 0.188 | 322.1° |
| lime | 0.175 | 126.1° | pink | 0.208 | 346.0° |
| grass | 0.147 | 147.4° | | | |
| green | 0.133 | 157.7° | | | |
| jade | 0.115 | 170.7° | | | |

The six gray families have chroma 0 to 0.019. Bronze, gold and brown fall under 0.08, so they
count as neutral and are not offered as the widget color or the host color (FR-013). With the
status scales fixed at green (157.7°), amber (84.1°) and red (23.0°), the widget colors 30° or
more from all three are **lime, sky, cyan, blue, indigo, iris, violet, purple, plum and pink**
(ten). A chromatic host color removes those within 30° of it. The default (monochrome host, blue
widgets) passes. The generator writes hue and chroma per scale into the tokens; the hue rule's
test recomputes them from the vendored data with an independent implementation and compares
every pair (SC-004).

**Rationale**: OKLCH is perceptually uniform in hue, unlike HSL, where Radix's yellows and greens
bunch together. The 0.08 threshold sits in the widest gap of the measured chromas (0.078 brown,
0.100 mint).

**Alternatives considered**: hue of step 11 (darker steps shift hue toward the text color);
CIELCh (works too, but OKLCH is what the format already accepts for colors).

## R4. No Node.js: the project's own checker, generator and lint

**Decision**: everything runs in Rust: a library crate `design-gen` (format and project-layer
checker, generator of the Rust tokens, the DTCG export and the preview, contrast lint, hue rule),
used by `cargo xtask design` and by its own tests. YAML is read with `yaml-rust2` `=0.13.0`
(MIT or Apache-2.0, pure Rust). The format's npm tool `@google/design.md` may be run by hand to
compare readings (decided in the spec, 2026-11-08).

**Rationale**: CI left Node.js with Tauri; the npm tool's contrast check covers only declared
pairs, as a warning, while FR-008 needs every allowed combination and a failure. `serde_yaml` is
unmaintained (`unmaintained = "workspace"` in `deny.toml` would flag it); `yaml-rust2` is the
maintained successor of `yaml-rust`.

**Alternatives considered**: `serde-saphyr` 2.1.1 (published 2026-11-05, too fresh to pin);
running the npm tool in CI (Node.js and its packages back in the build).

## R5. Contrast: what the lint checks, and what the measurements force

**Decision**: the lint checks every pairing the project layer declares, over every allowed
appearance (6 gray families × the monochrome host and 22 chromatic host colors × the widget
colors left by R3 × 2 themes; the radius does not affect contrast), with WCAG 2.2 relative
luminance on sRGB. A pairing declares its kind: `text` (4.5:1), `large-text` (3:1, at least 24 px,
or 18.66 px bold), `non-text` (3:1, WCAG 1.4.11: control boundaries, focus indicators, icons
that carry meaning), `decorative` or `inactive` (not checked, only under WCAG's exemptions for
decoration and inactive components, with the reason written; a decorative line is allowed only where something else carries
the meaning). Measured worst cases on Radix 3.0.0, which `DESIGN.md` must design around:

| Pairing | Worst light | Worst dark | Consequence for `DESIGN.md` |
| --- | --- | --- | --- |
| step 12 on step 1 or 2 (gray) | 15.48 | 15.12 | body text: passes |
| step 11 on step 1–3 (gray) | 5.17 | 7.58 | secondary text: passes |
| step 11 of a status scale on gray step 2 | 4.36 (amber on sage) | 8.29 | status text is gray step 12 beside a status icon in step 11 (non-text, 3:1) |
| step 11 on step 3 of the same scale | 4.21 (green) | 7.08 | soft badges set text in step 12 (9.82 worst) |
| white or dark text on step 9 | neither reaches 4.5 on red, tomato, ruby, crimson, pink, gold | the same | solid fills never carry normal-size text; text on step 9 only as `large-text`, or the soft style |
| gray step 8 border on step 1 | 1.86 | 3.00 | step 8 is `decorative` (dividers); a control's boundary needs another step: 9 reaches 3.22 light and 3.64 dark, but Radix assigns 9 to solid backgrounds (open, below) |
| widget step 9 as a focus ring on gray step 1 | 1.31 (lime) | 3.48 | focus is the host's own style (host-surface, R6); Radix's focus-ring steps (7, 8) fail too (accent 8: 2.26; gray 8: 1.86), so it needs gray 12 or the host color's step 11 (open, below) |

**Resolved by constitution v5.0.0: steps chosen per role.** Until v4.0.0 the UI row said each
Radix step is "used for its role", and its reason said "text contrast holds for any Radix scale
used by role". Radix assigns steps 1–2 to backgrounds, 3–5 to component backgrounds, 6–8 to
borders and focus rings, 9–10 to solid backgrounds and 11–12 to text. The measurements
contradicted both (ratios cut off, not rounded):

- no border or focus-ring step reaches WCAG 1.4.11's 3:1 on step 1 in the light theme, so a
  control's boundary and the focus ring need step 9, 11 or 12;
- two on-role text pairings fail 4.5:1 (amber 11 on sage 2, 4.36; green 11 on green 3, 4.21), so
  "holds for any scale used by role" is false; the design avoids them (status text in gray 12,
  soft badges in step 12), and the lint catches the rest.

Keeping borders decorative would not have avoided it, since the focus ring needed another step
either way. The maintainer kept Radix (replacing it would trade this for weaker dark themes or
fewer hues) and amended the constitution: `DESIGN.md` chooses the step for each role, every
foreground role is paired with every background it can be drawn on, and CI shows each pairing
meets WCAG 2.2's contrast minimums (1.4.3, 1.4.11) in every appearance the host accepts, in both
themes, translucent colors after compositing; a pairing goes unchecked only under a WCAG
exemption with its reason. So no step needs a mark for leaving its Radix role.

`DESIGN.md` also declares, per scale, the text color used on its solid steps (`white`, `dark` or
`none`, FR-009); the checker fails when the declared color does not reach 4.5:1 there, or when
`none` is declared for a scale where one does.

**Rationale**: the person picks the colors, so only the whole product of picks proves FR-008;
the table shows the defaults Radix users usually reach for (white on step 9, step 8 borders,
accent focus rings) fail AA for some scales, so the lint has to run before anything draws.

**Alternatives considered**: APCA (not yet a WCAG requirement); checking only the default
appearance (misses every failure above but one).

## R6. Keeping the host's surfaces out of the kit's reach

**Decision**: two generated crates. `design-tokens` holds the scales, tones, typography,
spacing, radius, elevation levels up to the frame's, the kit's component tokens, the appearance
type and its hue rule, and the contrast functions. `host-surfaces` holds the tokens `DESIGN.md`
marks as host surfaces: the chrome, a dialog's surface, scrim and elevation, any elevation above
the frame, the panel's attachment, and the host's focus style. `deny.toml` bans
`host-surfaces` with `wrappers = ["host"]`, so only the future host crate may depend on it, and
bans `design-gen`, which holds every value, with `wrappers = ["xtask"]`; `cargo deny` includes
dev- and build-dependencies and already runs on every change. Three checks show the boundary:

- a fixture crate outside the workspace, `design/ban-fixture/`, depends on `host-surfaces`, and a
  CI step runs `cargo deny check bans` on it with the root `deny.toml`, expecting it to fail;
- a `design-gen` test checks that every token marked `host-surface` is absent from
  `design/tokens/src/generated.rs` and present in `design/host-surfaces/src/generated.rs`;
- a `design-gen` test scans the workspace's `.rs` files and fails when one outside
  `design/host-surfaces/` and the future `host` names `host-surfaces/src` in `include!`,
  `include_str!` or `#[path]`.

The boundary fails the checks that gate a merge (`cargo deny`, the tests), not `cargo build`
itself; the spec's FR-015 says so.

**Rationale**: a module boundary inside one crate can be crossed by any crate that depends on it;
a crate boundary plus a dependency ban is enforced by tools that already gate merges. The future
kit crate depends only on `design-tokens`.

**Alternatives considered**: one crate with a cargo feature (features are additive: the kit could
turn it on); `pub(crate)` items (the host would need to live in the same crate).

## R7. Generated files: committed, with a drift check

**Decision**: `cargo xtask design` writes `design/tokens/src/generated.rs`,
`design/host-surfaces/src/generated.rs` and `design/tokens.dtcg.json`, each starting with a
"generated from DESIGN.md, do not edit" line. A test in `design-gen` generates in memory and
compares with the committed files, after turning `\r\n` into `\n`, naming the first token that
differs (FR-004); `.gitattributes` sets `DESIGN.md`, `docs/design/*.md` and the text files under
`design/` to `eol=lf`, and the fonts to `binary`.
`cargo xtask design --check` runs the same checks without writing.

**Rationale**: committed output shows token changes in review and lets crates build without
running the generator; the drift test keeps them honest on three systems. A `build.rs` would hide
the generated code from review and run YAML parsing in every build.

**Alternatives considered**: `build.rs` generation; generating in CI only.

## R8. The SVG preview

**Decision**: `cargo xtask design --preview <dir>` writes static SVG files: `palette-light.svg`
and `palette-dark.svg` (every scale, its steps and roles, with the hue rule's verdict per widget
color), `default-light.svg` and `default-dark.svg` (the chrome, a widget frame with its label
and monogram, a panel, a host dialog over a scrim, and the status tones with their icons, drawn
from the default appearance's tokens), and `index.svg` linking them. Text is drawn with the
built-in font embedded as a WOFF2 `@font-face` in each file, so the preview looks the same on any
machine; no script, and no reference outside the preview directory (`index.svg` links only to
its siblings). The `<style>` inside each SVG is a style sheet, which is fine because the preview
is a review artifact and never reaches the host UI. The CI build job on Linux runs it and publishes the
directory with `actions/upload-artifact`, pinned to a commit SHA like the other actions, kept 14
days. Nothing is committed (FR-026).

**Rationale**: SVG opens in any browser, is reviewable as text when needed, and needs nothing
installed; the sketches use the same tokens the host will, so a change shows up in the PR's run.

**Alternatives considered**: PNG rendered with a rasterizer crate (a new dependency for no review
benefit); an HTML page (the constitution keeps HTML out of the host, and a static SVG is enough).

## R9. Fonts and icons

**Decision**: one built-in family, Inter (SIL OFL 1.1), its upstream variable WOFF2 and TTF
unmodified (no subsetting, so OFL's renaming clause never applies) under `design/fonts/inter/`
with its license, for the host's UI and the preview; its tabular figures
(`tnum`) for numbers in tables. A monospaced family is added when a feature first needs one.
Status icons are three glyphs drawn by the project (a check in a circle, an exclamation in a
triangle, a cross in a circle) as SVG paths in `DESIGN.md`'s project layer, so no icon set is a
dependency.

**Rationale**: OFL fonts may be bundled with Apache-2.0 software; Inter is designed for UI at
small sizes. Drawing three glyphs avoids vendoring an icon library for three shapes. The font is
data the host embeds; no font comes from a plugin or the network (FR-007). Its decoding runs in
the host process, which decision 0001's revisit in a later Phase 3 plan already covers for
built-in fonts.

**Alternatives considered**: IBM Plex Sans (also OFL; wider, less suited to dense UI); the system
font (differs per system, so the contrast and layout checks would not hold).

## R10. Where the frame and dialog criteria live, and how their coverage is checked

**Decision**: `docs/design/host-surfaces.md`, with one criterion per `### HS-NN` heading and four
labelled lines each: **Observe**, **Expect**, **Answers** (constitution rule and threat-model
row), **Tested by** (the later feature). The threat rows are named by their exact first cell. A
test in `design-gen` reads this file and `docs/threat-model.md`, and fails when a criterion lacks
a field, names a threat row that does not exist, or when a Phase 3 row on boundary B4 has no
criterion and is not listed under the file's "Decided elsewhere" section (SC-005). The fake dialog
built from a widget's own declared actions is listed there, pointing to the threat model's open
question (FR-021).

**Rationale**: the criteria outlive the feature, so they go in `docs/`, not `specs/`; a check that
reads the threat model keeps the two in step as rows are added.

**Alternatives considered**: inside `DESIGN.md` (it is about tokens; behaviour and focus rules
would be prose the checker cannot address); in the spec folder (lost to later features).

## R11. The input delay on new dialogs

**Decision**: 1000 ms. A host dialog's buttons and keys ignore input for 1000 ms after it appears,
and a key already held when it appears answers nothing until released.

**Rationale**: Firefox's `security.dialog_enable_delay` defaults to 1000 ms for its install and
download dialogs ([Mozilla bug 1717426](https://bugzilla.mozilla.org/show_bug.cgi?id=1717426),
[CIS Firefox 102 benchmark](https://tenable.com/audits/items/CIS_Mozilla_Firefox_102_ESR_v1.0.0_Linux_Level1.audit:2e575cc05c58b4e9442608abd4b2aa42));
trust dialogs are rare, so a second costs little. The spec asks for at least 500 ms.

**Alternatives considered**: 500 ms (the spec's floor; less margin for a double click).

## R12. CI

**Decision**:

- The `changes` job counts as code: `DESIGN.md`, `design/`, `docs/design/` and
  `docs/threat-model.md`, the files the checks read, as it already does for `docs/authors/` and
  `wit/` (constitution: Markdown the build uses as code is not documentation). Today `DESIGN.md`
  matches `\.md$` and the two `docs/` paths match `^docs/`; the first test becomes
  `'^(docs/authors|docs/design|wit|design)/|^DESIGN\.md$|^docs/threat-model\.md$'`. The comment in
  `ci.yml` names them and cites constitution v5.0.0 instead of v2.4.1.
- A step runs `cargo deny check bans` on `design/ban-fixture/` and passes only when it fails
  (R6).
- No new job: the checks are tests in `design-gen`, so the build job's `cargo test --workspace`
  runs them on three systems; Clippy, `cargo deny` and `cargo audit` cover the new crates.
- The Linux build job adds two steps: `cargo xtask design --check` and `cargo xtask design
  --preview target/design-preview`, then the upload.

**Rationale**: the existing jobs already run everything a change needs; a separate job would add
a runner start for checks that take seconds.

**Alternatives considered**: a `design` job per system (no gain for pure computation).

## R13. Typography, spacing, radius and elevation scales

**Decision**: written in `DESIGN.md` during implementation, within these limits: spacing on a
4 px base; a type scale of at most nine levels from 12 px to 32 px, body at 14 px; the radius set
a person picks is `none` (0), `small` (3 px), `medium` (6 px, the default), `large` (10 px) and
`full` (pills); elevation levels `flat`, `frame`, `panel` and `dialog`, of which `panel` and
`dialog` are host surfaces. A dialog's scrim is a `blackA` step per theme, also a host surface;
every shadow takes its color from `blackA` (R2).

**Rationale**: these are the values a host with dense widgets needs; the rationale of each lives
next to it in `DESIGN.md` (FR-001).
