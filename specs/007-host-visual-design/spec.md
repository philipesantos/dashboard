# Feature Specification: Host Visual System and the Design of the Host Frame and Dialogs

**Feature Branch**: `007-host-visual-design`

**Created**: 2026-11-08

**Status**: Draft

**Input**: User description: "Feature 007 (first of Phase 3): the host's visual system and the
design of the host frame and dialogs. Two items of the development guide, in one feature.
1) Visual system in DESIGN.md, in Google's open DESIGN.md format (still alpha: pin the tool
version). Color tokens for light and dark themes, typography, spacing, shapes and the declarative
UI's components, each with its rationale. Rust tokens are generated from it, with Radix Colors
scales for color; typography, spacing, radius and shadow are the project's own scales; tokens are
also exported as W3C Design Tokens Community Group JSON for design tools. Default is neutral. The
person picks the gray family, the host color, the widget color and the corner radius. Widgets
only ask for tones (default, accent, ok, warning, error), never colors, and the kit never
receives the host colors. A WCAG AA contrast lint runs in CI. Because the build uses DESIGN.md as
code, it must leave the docs-only filter of ci.yml. 2) Design spec of the host frame and the host
dialogs: what only the host draws, tokens reserved to the host that plugin UI cannot use, dialog
accessibility (keyboard and screen reader), and testable criteria that a widget cannot imitate
the host. Constraints: constitution v4.0.0 (GPUI host, own Rust components, Radix Colors,
Apache-2.0); no plugin string reaches GPUI img() or open_url; no image block or plugin-supplied
icon until decision 0001 is revisited (monogram only)." (Updated the same day: the personal
visual secret and the secret color were dropped by constitution v4.0.0.)

## Clarifications

### Session 2026-11-08

- Q: The guide lists the `DESIGN.md` visual system and the frame and dialog design spec as two
  Phase 3 items, and calls only the second a Spec Kit feature. One feature or two? → A: One
  feature, 007. The tokens reserved to the host and the dialog's look depend on the color choices
  in `DESIGN.md`.
- Q: There is no host app in the repository yet (only `runtime`, `bench` and `xtask`), and "Host
  in GPUI" is its own Phase 3 item. How far does 007 go? → A: No window. 007 delivers the visual
  system, the generated tokens, the checks that can run without a window (contrast, the hue rule,
  the split between host and widget tokens), and the frame and dialog design as testable
  criteria. The hostile cases that need a window (a dialog that is not the host's, a widget
  imitating the host) arrive with the host features, which must pass these criteria.
- Q: What is the visual secret made of? → A: Nothing: it was dropped. Constitution v4.0.0
  (2026-11-08) replaced it with structural rules: a host dialog takes a form that neither widget
  content nor a declared control can produce. It covers the host's chrome, blocks every host
  window, keeps keyboard focus and is all that assistive technology sees. With it went the secret
  color, so the person picks one color fewer and the 30° rule now keeps the widget color away from
  the host color and the status tones.
- Q: A widget can still draw a fake dialog inside its frame and use its own declared action as
  the button. Is that solved here? → A: No. It is recorded as an open question in the threat
  model and decided in the Phase 3 spec for declared actions and settings: whether the host writes
  the effect and destination on every control of an action that changes data, and whether a
  destructive action always asks for a host dialog. 007 stays simple.
- Q: How is the split between host and widget checked: by color value or by the token's role? →
  A: By role only, and no rule on color values. Since v4.0.0 a host dialog is told apart by its
  form, not its colors; the 30° rule already keeps the widget color away from the host color, and
  shared grays and status tones let nothing pass for the host. What stays is a split in the code:
  the kit cannot reach the tokens of the host's surfaces (chrome, dialog, scrim, elevation above
  the frame, the host's focus style), so no widget can build one. It is an ordinary test; the
  security rule is the dialog's form, tested with the host features.
- Q: Are the colors of the status tones (ok, warning, error) fixed, or does the person pick them?
  → A: Fixed: green for ok, amber for warning, red for error. And a status is never shown by color
  alone: every use carries a host icon or host text naming it (WCAG 1.4.1), which serves people
  with color vision deficiencies without making the scales a pick.
- Q: With no window in 007, how is the visual system seen and reviewed? → A: The command that
  generates the tokens also draws a static preview (SVG, no script): both themes in the default
  appearance, a grid of the scales, sketches of the frame, a panel and a dialog, and the status
  tones with their icons. CI publishes it as a build artifact on every run; it is not committed.
- Q: The plan measured that Radix's border and focus-ring steps miss WCAG 1.4.11's 3:1 and that
  some on-role text pairings miss 4.5:1, against the constitution's "each step used for its
  role". Replace Radix, or amend? → A: Keep Radix and amend: constitution v5.0.0 lets `DESIGN.md`
  choose the step for each role and makes the CI contrast check the guarantee.

### Session 2026-11-09

- Q: Reviewing the preview, the maintainer found the edges soft and asked for something sharper and
  cleaner. Which visual direction? → A: "Painel de instrumento", chosen over eight others mocked
  in HTML: the sand gray family by default, flat (no shadows; a frame reads by its tone and a firm
  line), 2 px corners, Barlow for text and IBM Plex Mono in capitals for labels and controls, and
  the host's own marks (the monogram, the actions in a frame's header, the focus ring) in step 12
  of the host color, its ink. A bright host color was tried and rejected: on a dashboard of many
  widgets it would dominate.

## User Scenarios & Testing *(mandatory)*

The people in these stories are the **maintainer**, who owns the host and its look; a
**contributor**, who changes the visual system; the **person** who uses the dashboard and picks
its appearance; and a **plugin author**, whose widget content the host draws. "The visual system"
is `DESIGN.md` and everything generated from it. "The kit" is the closed set of components the
host draws widget content with. "Host-surface tokens" are the tokens only the host's own surfaces
use: the chrome, dialogs, their scrim, elevation above the frame, the host's ink (the monogram and
the actions in a frame's header) and the host's focus style.

### User Story 1 - One source for the whole visual system (Priority: P1)

A contributor changes a color role, a spacing step or a radius in `DESIGN.md`, and nowhere
else. The tokens the host compiles and the design-tool export follow from that one file. If the
generated tokens no longer match `DESIGN.md`, or the file breaks the format, CI fails and says
which token or rule is wrong. A change to `DESIGN.md` alone runs the full CI, since the build
uses it as code.

**Why this priority**: every later Phase 3 feature (host window, kit, frame, dialogs) draws with
these tokens. Two sources of truth would drift, and a design rule that only lives in prose cannot
be checked.

**Independent Test**: change one value in `DESIGN.md` without regenerating and see CI fail with
the token named; regenerate and see it pass; make a change to `DESIGN.md` only and see every CI
job run.

**Acceptance Scenarios**:

1. **Given** `DESIGN.md` and the generated tokens in step, **When** the generator runs again,
   **Then** nothing changes.
2. **Given** a value changed in `DESIGN.md` only, **When** CI runs, **Then** the drift check
   fails and names the token that differs.
3. **Given** a `DESIGN.md` that breaks the format (an unknown section, a token without a value,
   a reference to a token that does not exist), **When** the format check runs against the pinned
   specification, **Then** it fails and names the line or token.
4. **Given** a pull request that changes only `DESIGN.md`, **When** CI decides which jobs run,
   **Then** it treats the change as code and runs every job.
5. **Given** a token in `DESIGN.md`, **When** a reader looks it up, **Then** it has a stated
   role and a one-line rationale.
6. **Given** a pull request that changes `DESIGN.md`, **When** CI runs, **Then** its run carries
   the generated preview, so the change can be reviewed by eye.

---

### User Story 2 - Text stays readable in every appearance a person can pick (Priority: P1)

Whatever gray family, host color, widget color, corner radius and theme (light or dark) the
person picks, every text and control the visual system defines meets WCAG AA contrast. A
contributor who adds a pairing that fails in any allowed combination sees CI fail with the
combination and the measured ratio.

**Why this priority**: the person chooses the colors, so contrast cannot be checked on the
default only; a pick that makes host text unreadable would hide what a dialog asks.

**Independent Test**: run the contrast lint over every allowed combination; then add a pairing
known to fail (for example light text on a light solid step) and see the lint fail naming it.

**Acceptance Scenarios**:

1. **Given** every combination of the allowed picks in both themes, **When** the contrast lint
   runs, **Then** every text pairing reaches at least 4.5:1 (3:1 for large text) and every
   control boundary and focus indicator at least 3:1 against what it sits on.
2. **Given** a scale whose solid step needs dark text on it, **When** a pairing places text on
   that step, **Then** the visual system takes the text color declared for that scale, and the
   lint checks it.
3. **Given** a failing pairing, **When** the lint runs, **Then** it fails and reports the
   pairing, the theme, the picks and the measured ratio.

---

### User Story 3 - A widget cannot build the host's surfaces (Priority: P1)

A plugin author can only ask the kit for tones (default, accent, ok, warning, error) and for
the kit's own components. The kit resolves each tone from the person's picks: default from the
gray family, accent from the widget color, ok, warning and error from the status scales. It never
holds the tokens of the host's surfaces, so no widget can build the chrome, a dialog's surface,
its scrim, an elevation above the frame or the host's focus style. Colors themselves may be
shared: a red "error" means the same in a widget and in the host.

**Why this priority**: every later feature that adds kit components draws from these tokens; if
the kit could reach a dialog's surface, a future component could build one by mistake.

**Independent Test**: check that no host-surface token is in the crate the kit depends on, and
that a crate depending on the host-surface crate fails the merge checks.

**Acceptance Scenarios**:

1. **Given** the generated tokens, **When** a crate other than the host depends on the
   host-surface tokens or includes their source, **Then** the merge checks fail.
2. **Given** a widget asking for a tone the kit does not have, **When** the host draws it,
   **Then** the host uses the default tone and logs the unknown name, escaped.

---

### User Story 4 - The person's appearance picks are checked (Priority: P2)

The person can pick a gray family, a host color (or the monochrome default), a widget color and
a corner radius, and a light, dark or system theme. The host refuses a widget color whose hue is
less than 30° from the host color or from any status tone, and explains why in its own words.
The default is neutral: the warm sand gray family, a monochrome host and blue widgets. 007 provides the
rule and its check; the screen where the person picks arrives with the host window.

**Why this priority**: a widget color close to the host color lets widget content look like the
host; one close to a status tone makes "accent" indistinguishable from "ok", "warning" or
"error".

**Independent Test**: run the check over every pair of allowed scales and compare with the hue
distances computed from the scales themselves; check that the default passes.

**Acceptance Scenarios**:

1. **Given** a widget color 29° of hue from the host color, **When** the pick is checked,
   **Then** it is refused, naming both colors.
2. **Given** a widget color 30° or more from the host color and from every status tone,
   **When** the pick is checked, **Then** it is accepted.
3. **Given** the monochrome host, **When** a widget color is checked, **Then** only the status
   tones constrain it.
4. **Given** the default appearance, **When** it is checked, **Then** it is accepted and passes
   the contrast lint.

---

### User Story 5 - The frame and the dialogs are designed to be checked (Priority: P2)

The maintainer gets a design for what only the host draws: the chrome, the widget frame, the
panel a frame opens, the host dialogs and the field note on plugin fields. Every rule is written
as a numbered criterion that a later feature turns into a test, and every criterion names the
constitution rule and the threat-model row it answers. The criteria say how a host dialog looks
and behaves, to the eye, to the keyboard and to assistive technology, in a way no widget content
or declared control can produce.

**Why this priority**: the host window, the kit, the dialogs and the declared actions are later
features; written criteria let each one be checked against the same design, and the Phase 3 gate
needs these rows tested.

**Independent Test**: for every Phase 3 row of the threat model about host imitation, dialogs and
plugin fields, find at least one criterion that answers it; for every criterion, check that it
states a pass/fail observation (a bound, a focus position, an accessibility-tree property, an
input outcome), not a judgement.

**Acceptance Scenarios**:

1. **Given** the design, **When** a reviewer reads a criterion, **Then** it names what is
   observed, the expected result, the constitution rule and the threat row, and the feature that
   will test it.
2. **Given** the Phase 3 threat rows on host imitation, fake dialogs, dialogs under the user's
   input, plugin settings panels and plugin text fields, **When** they are matched against the
   criteria, **Then** each row has at least one criterion, or is marked as decided elsewhere
   (the fake dialog built from declared actions, left to the declared-actions spec).
3. **Given** the design, **When** it describes the widget frame, **Then** it defines the label
   (the plugin's name), the monogram, and how both are made safe from the name a plugin chooses.

---

### User Story 6 - Host dialogs work for keyboard and screen-reader users (Priority: P3)

The design says how a host dialog behaves for someone who uses only the keyboard or a screen
reader: where focus goes when it opens, how it stays inside, how it closes, what is announced,
and that nothing behind it can be reached. Because GPUI does not mark a dialog as modal in its
accessibility tree, the design says how the host makes up for it, and which screen readers
(VoiceOver, Narrator, Orca) and real-window key tests verify it once the host window exists.

**Why this priority**: the trust decisions happen in these dialogs; someone who cannot operate
them cannot make the decision safely. It is P3 only because its tests need the window.

**Independent Test**: check that each dialog behaviour in the design is a criterion with an
observable result on the accessibility tree or the focus position, and that each names the
screen readers and systems it is tested on.

**Acceptance Scenarios**:

1. **Given** a host dialog opens, **When** focus is observed, **Then** it is inside the dialog,
   on the first control that is not the one that confirms.
2. **Given** a host dialog is open, **When** Tab and Shift+Tab are pressed repeatedly, **Then**
   focus never leaves the dialog, and Escape closes it as a refusal.
3. **Given** a host dialog is open, **When** the accessibility tree is read, **Then** only the
   dialog and its contents are exposed, with the host's name for the dialog, and no node
   supplied by a plugin has a dialog role at any time.

### Edge Cases

- **Monochrome host**: the host color has no hue, so the 30° rule applies only against the status
  tones; the host's surfaces stay apart from widget content by their form, not by their colors.
- **Low-chroma scales** (the gray families and scales whose hue is unstable at low chroma): they
  count as neutral and cannot be the widget color, since an accent with no hue reads as default.
- **Scales that need dark text on their solid step** (Radix's light scales such as amber,
  yellow, lime, mint, sky): the visual system declares the text color per scale, and the lint
  checks it in both themes.
- **Two scales of nearly the same hue** (for example red and tomato): the hue rule measures the
  hue, not the scale's name, so the pair is refused when it is under 30°.
- **A theme the person did not pick** (the system switches between light and dark): the tokens
  for both themes exist and pass the lint, so the switch never breaks contrast.
- **The plugin's name in the frame**: control characters and bidirectional overrides removed,
  capped at 40 visible characters with an ellipsis, drawn in the host's font; the monogram is the first letter or
  digit of the name as displayed, upper-cased, and a fixed host glyph when there is none; its
  colors come from the neutral scale, never from a plugin.
- **A name that imitates the host** ("Dashboard", "System", "Security check"): the label always
  sits in the frame, under the host's chrome, so it reads as a widget's name; refusing such names
  belongs to installation, not to this feature.
- **Several host windows**: a dialog blocks all of them, not only the one it opens in.
- **A dialog that appears under the pointer or while a key is held**: its buttons ignore input
  for a short time after it appears, and no key that was already down answers it.
- **`DESIGN.md` format changes upstream** (the format is alpha): the specification is pinned to
  one commit, and moving it is a change of its own, reviewed like a dependency upgrade.

## Requirements *(mandatory)*

### Functional Requirements

**Visual system and generation**

- **FR-001**: The repository MUST hold the visual system in one `DESIGN.md` at the root, in
  Google's open DESIGN.md format, defining color roles for light and dark themes, typography,
  spacing, radius, shadow and elevation, and the kit's components, each token with its role and a
  one-line rationale.
- **FR-002**: Colors MUST come from Radix Colors scales, plus white and black, with the step for
  each role chosen in `DESIGN.md` (constitution v5.0.0);
  typography, spacing, radius and shadow MUST be the project's own scales, defined in
  `DESIGN.md`.
- **FR-003**: The host's Rust tokens MUST be generated from `DESIGN.md`, and so MUST a W3C Design
  Tokens Community Group JSON export; neither MAY be edited by hand.
- **FR-004**: CI MUST fail when the generated tokens or the export differ from what `DESIGN.md`
  produces, naming the first token that differs.
- **FR-005**: CI MUST check `DESIGN.md` against the format's specification, pinned to one commit
  of Google's repository, with the project's own checker; the format's npm tool is not part of CI
  and MAY be run by hand to compare readings.
- **FR-006**: The CI filter that skips documentation-only changes MUST treat `DESIGN.md` as code,
  so a change to it alone runs every job.
- **FR-026**: The command that generates the tokens MUST also draw a static preview, as SVG files
  with no script: both themes in the default appearance, a grid of every scale with its roles,
  sketches of the chrome, a widget frame, a panel and a host dialog drawn from the tokens, and the
  status tones with their icons. CI MUST publish it as a build artifact of every run on Linux; it
  MUST NOT be committed.
- **FR-007**: The fonts the visual system names MUST be built into the host, under licenses
  compatible with Apache-2.0; no font MAY come from a plugin or the network.

**Contrast**

- **FR-008**: `DESIGN.md` MUST pair every foreground role with every background it can be drawn
  on, and CI MUST run a contrast lint over every appearance the host accepts, in both themes,
  failing when a text pairing is under 4.5:1 (3:1 for large text as WCAG defines it, WCAG 1.4.3)
  or a control boundary, focus indicator or meaningful icon is under 3:1 (WCAG 1.4.11), with
  translucent colors measured after compositing, and reporting the pairing, theme, picks and
  ratio. A pairing MAY go unchecked only under a WCAG exemption (inactive component, decoration,
  incidental text, logotype), with its reason in `DESIGN.md`.
- **FR-009**: For each scale, `DESIGN.md` MUST declare the text color used on its solid steps, and
  the lint MUST use it.

**Appearance picks**

- **FR-010**: The allowed picks MUST be: a gray family from the Radix gray families; a host color,
  monochrome by default, or a chromatic Radix scale; a widget color from the chromatic scales; a
  corner radius from a small set defined in `DESIGN.md`; and a theme (light, dark or follow the
  system).
- **FR-011**: The host MUST refuse a widget color whose hue is less than 30° from the host
  color's or any status tone's, measured on the scales' solid step in one perceptual color space,
  and the refusal MUST name the colors in the host's own words.
- **FR-012**: The default appearance MUST be the sand gray family, a monochrome host, blue widgets,
  the small radius and the system theme, and MUST pass FR-008 and FR-011.
- **FR-013**: Low-chroma scales MUST count as neutral and MUST NOT be accepted as the widget
  color or the host color.

**Split between host and widget**

- **FR-014**: `DESIGN.md` MUST mark the tokens of the host's surfaces: at least the chrome's
  surface, a dialog's surface, its scrim and its elevation, any elevation above the frame, the
  panel's attachment to its frame, and the host's focus style.
- **FR-015**: The kit MUST resolve widget content only from tones (default from the gray family,
  accent from the widget color, ok, warning and error from the status scales) and its own
  component tokens, and MUST NOT be able to reach a host-surface token: a crate other than the
  host that depends on them, or includes their source, MUST fail the checks that gate a merge,
  and a test MUST show it.
- **FR-016**: No rule compares color values between the kit and the host: the gray family and the
  status scales are shared, and the 30° rule (FR-011) keeps the widget color away from the host
  color.
- **FR-017**: An unknown tone MUST resolve to the default tone, and the host MUST log the name,
  escaped.

**Status tones**

- **FR-024**: The status tones MUST use fixed Radix scales: green for ok, amber for warning and
  red for error. The person does not pick them, and the hue rule (FR-011) and the contrast lint
  (FR-008) measure against these three.
- **FR-025**: The kit and the host MUST never convey a status by color alone: every use of ok,
  warning or error MUST carry an icon built into the host, or host text, naming the status, and a
  test MUST check that each status component has one.

**Frame and dialog design (criteria for later features)**

- **FR-018**: The feature MUST write the design of the host's chrome, widget frame, frame panel,
  host dialogs and the note on plugin fields as numbered criteria, each with what is observed,
  the expected result, the constitution rule, the threat-model row and the feature that will
  test it.
- **FR-019**: The criteria MUST cover, at least:
  - the chrome is never covered by a frame or panel, and is covered by every host dialog;
  - a host dialog blocks every host window, keeps keyboard focus, ignores pointer input outside
    it and is the only part exposed to assistive technology while open;
  - no plugin-supplied content has a dialog role or draws outside its frame;
  - a panel stays attached to its frame and never dims or blocks the window;
  - every field the host draws for a plugin that takes typed text or digits carries the host's
    note, bound to the field for assistive technology, naming the widget that receives it and
    saying it is not for passwords, tokens or codes, and no such field is masked;
  - a new dialog's buttons ignore input for at least 500 ms after it appears, and no key held
    before it appeared answers it;
  - the frame's label and monogram rules of the edge cases above;
  - native prompts are never used for trust decisions.
- **FR-020**: The criteria MUST state the keyboard and screen-reader behaviour of host dialogs
  (initial focus, focus kept inside, Escape as refusal, what is announced), how the host makes up
  for GPUI's missing modal state, and that each is tested with VoiceOver, Narrator and Orca and
  with real key presses in a real window.
- **FR-021**: The criteria MUST record the fake dialog built from a widget's own declared actions
  as decided in the declared-actions spec, pointing to the threat model's open question.

**Hostile cases and existing suite**

- **FR-022**: The feature adds no hostile case: the security rule is the host dialog's form, whose
  cases arrive with the host features (FR-019). The existing security suite MUST pass unchanged.
- **FR-023**: No plugin string MAY reach an image or URL-opening call in anything this feature
  adds, and the visual system MUST NOT define an image block or a plugin-supplied icon (decision
  0001 stands).

### Key Entities

- **Visual system**: `DESIGN.md`, the single source; holds tokens, roles, rationales, the
  host-surface marks and the kit's components.
- **Token**: a named value (color, size, radius, shadow, font), with its role, its value per
  theme, its rationale and whether it is a host-surface token.
- **Tone**: one of default, accent, ok, warning, error; the only color a widget can ask for. Ok,
  warning and error use fixed scales and always come with the host's icon or text.
- **Appearance**: the person's picks (gray family, host color, widget color, radius, theme), valid
  only if it passes the hue rule.
- **Host-surface tokens**: the tokens of the chrome, dialogs, scrim, elevation above the frame, the
  host's ink and the host's focus style; the kit never reaches them.
- **Criterion**: a numbered, observable rule for the chrome, frame, panel, dialogs or field note,
  with its constitution rule, threat row and the feature that will test it.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A contributor changes one color role, one spacing step and one radius by editing
  `DESIGN.md` only, and the generated tokens and export follow with one command; no other file
  is edited by hand.
- **SC-002**: The contrast lint covers 100% of the allowed appearance combinations in both
  themes, and every pairing passes WCAG AA.
- **SC-003**: 100% of host-surface tokens are absent from what the kit can depend on, and a crate
  that reaches for them fails the merge checks.
- **SC-004**: The hue rule's accepted and refused pairs match an independent computation of hue
  distances for every pair of allowed scales.
- **SC-005**: Every Phase 3 threat-model row about host imitation, dialogs, panels and plugin
  fields maps to at least one criterion or to its documented owner, and every criterion states a
  pass/fail observation.
- **SC-006**: A change to `DESIGN.md` alone runs every CI job on the three systems, and the
  existing security suite passes unchanged.
- **SC-007**: Every CI run that builds carries a preview the maintainer can open in a browser to
  approve the look, with nothing to install.

## Assumptions

- Google's DESIGN.md format is Markdown with YAML front matter, specified in `docs/spec.md` of
  `google-labs-code/design.md` (Apache-2.0). Its tool, `@google/design.md`, is an npm package;
  CI does not install Node.js (it left with Tauri), and the tool's contrast check covers only
  declared pairs, as a warning. So the project's checker, generator, DTCG export and contrast lint
  are its own, in Rust, and the npm tool is only a manual cross-check (decided 2026-11-08).
- The DTCG export follows the format's current draft; design tools that read an older draft are
  not a goal.
- Radix Colors is used as plain color data (MIT), never through its React packages.
- Hue is measured in a perceptual space (for example OKLCH) on step 9 of each scale; the plan
  fixes the space and the chroma threshold for "low-chroma".
- The 500 ms input delay on new dialogs follows what browsers use for permission and download
  prompts; the plan may adjust it with a reason.
- The radius set is small (for example none, small, medium, large, full); the exact steps are
  chosen in `DESIGN.md`.
- Out of scope: the host window and GPUI dependency (the "Host in GPUI" item), the screen where
  the person picks the appearance, the kit's components beyond their tokens, the widget content
  format, the dashboard grid, the dialogs themselves, installation, and declared actions. Each
  arrives with its own Phase 3 feature and must pass this feature's criteria.
- Dependencies: constitution v5.0.0 (Principles V, VII, VIII; Fixed Technical Decisions for UI;
  the contrast rule of the Development Workflow);
  the threat model's Phase 3 rows and open questions; decision 0001.
