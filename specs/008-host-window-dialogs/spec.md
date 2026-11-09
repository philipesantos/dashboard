# Feature Specification: The Host Window, Host Dialogs and the Rules That Replace the Webview's CSP

**Feature Branch**: `008-host-window-dialogs`

**Created**: 2026-11-09

**Status**: Draft

**Input**: User description: "Feature 008 (Phase 3): the host window in GPUI, the host dialog
component, its accessibility, and the rules that replace the webview's CSP. Four items of the
development guide in one feature: 'Host em GPUI', 'Diálogos do host desenhados só com os
componentes do próprio host', 'Acessibilidade dos diálogos' and 'Regras que substituem a CSP da
webview'. 1) Host in GPUI: a new `host` crate (the only one deny.toml lets depend on
`host-surfaces`), with the gpui-unofficial crates pinned with `=` and updated at most once a
month; parts of gpui-base (Apache-2.0) copied and adapted where behavior is hard (focus trap,
text field, accessible table), with the license notice. The window focuses its root when it opens
so the first Tab works. It draws, from feature 007's tokens, the chrome, a widget frame with its
monogram and label, and the panel a frame opens, attached to it. Until the kit and the grid
arrive, frames show the installed widgets with simple placeholder content. 2) Host dialogs: drawn
only with the host's own components, in the form no widget can produce: they cover the chrome,
block every host window while open, hold keyboard focus, ignore pointer input outside them, and
are the only thing assistive technology sees; their buttons ignore clicks and keys for an instant
after the dialog appears; the system's native prompt is never used for trust decisions. 3) Dialog
accessibility: GPUI does not mark a dialog modal in the accessibility tree (no aria_modal);
contribute the marking upstream or compensate. Accessibility-tree checks in CI; screen-reader
walks (VoiceOver, Narrator, Orca) before a release; interface tests with real keys in a real
window. 4) Rules that replace the webview's CSP, each with a test in the security suite: widget
content only becomes host components; images arrive only as bytes; no plugin string reaches
GPUI's img() or open_url; GPUI's HTTP client stays null. Constraints: constitution v5.0.1; meet
the criteria of docs/design/host-surfaces.md tested by 'host window' or 'host dialogs'; decision
0001: the plan decides whether the runtime and decoding move to a separate process, and until
then no plugin image or icon. Out of scope: Linux without hardware Vulkan, the dashboard grid,
the UI kit, declared actions and settings, the install dialog, official widgets."

## Clarifications

### Session 2026-11-09

- Q: Feature 007 left the screen where the person picks the appearance (gray family, host color,
  widget color, corners, theme, with the 30° refusal) to "the host window". Does it ship in 008?
  → A: Yes. It is a host settings screen opened from the chrome, not a trust dialog; it applies
  the choice at once and keeps it between runs.
- Q: No trust decision happens in the window yet. Which real host dialog does 008 bring, so a
  person can open it and walk it with a screen reader? → A: "Remove from dashboard", from the
  frame's menu: confirmed, it removes the plugin's approval and every grant with it, as the
  command-line tool's `remove` does; refused, nothing changes.
- Q: In what language does the host write its own text (chrome, frame status, menus, appearance
  settings, dialogs)? → A: English for now, every string from one message catalog, so other
  languages can be added later without touching the screens.
- Q: The window and the development command-line tool share the approval record. When does the
  window show a change the tool made while it was open? → A: When the window regains focus, and
  when the person chooses "Reload" in the chrome; no file watching.

## User Scenarios & Testing *(mandatory)*

The people in these stories are the **person** who uses the dashboard; a **keyboard or
screen-reader user**, the same person working without a pointer or without sight; the
**maintainer**, who owns the host and its security core; and a **plugin author**, whose widget the
host loads and whose strings the host must never trust. "The window" is the dashboard's main host
window. "The chrome" is the host's own bar at the top of the window (constitution, Principle V).
"A frame" is the host-drawn border around one widget, with the plugin's monogram and label. "A
host dialog" is a dialog the host draws for a decision, in the form Principle VII describes.
"The criteria" are those of `docs/design/host-surfaces.md` (feature 007) that name *host window*
or *host dialogs* as what tests them: HS-01, HS-02, HS-10, HS-11, HS-12 and HS-16 to HS-19, HS-24.

### User Story 1 - The dashboard opens as a native window in the host's look (Priority: P1)

The person starts the dashboard and a native window opens, with no web view. At the top is the
chrome, the host's own bar. Below it, every widget the host has approved for loading sits in its
own frame, drawn by the host from feature 007's visual system: the plugin's monogram and name in
the frame's header, the host's lines, type and corners. The widget's own content is not drawn
yet (the widget kit is a later feature); in its place the frame shows the host's own words: that
the widget loaded and rendered, or that it failed and why, in the host's status tones. Keyboard
focus starts on the window's content, so the first Tab reaches the first control.

**Why this priority**: every other Phase 3 feature (kit, grid, actions, installation) draws into
this window; without it nothing of Phase 3 can be seen or tested in a real window.

**Independent Test**: start the dashboard with two approved development plugins, one whose render
succeeds and one whose render fails; check the window, the chrome, two frames with monograms and
labels, the status of each, and that one Tab moves focus to the first control.

**Acceptance Scenarios**:

1. **Given** the host has approved plugins for loading, **When** the person starts the dashboard,
   **Then** one window opens with the chrome at the top and one frame per plugin below it, each
   with the plugin's monogram and name as feature 007's criteria for the label require.
2. **Given** a plugin whose render succeeds, **When** its frame is drawn, **Then** the frame says,
   in the host's words and with the ok tone's icon and words, that the widget rendered, and shows
   none of the plugin's returned content.
3. **Given** a plugin whose load or render fails, **When** its frame is drawn, **Then** the frame
   says so in the host's words with the error tone's icon and words, and the rest of the
   dashboard keeps working.
4. **Given** the window has just opened, **When** the person presses Tab once, **Then** focus
   lands on the first control in the window and shows the host's focus ring (HS-24).
5. **Given** the person picks a theme or appearance that feature 007 allows, **When** the window
   draws, **Then** every color, type size, radius and line comes from the generated tokens for
   that appearance; nothing is styled outside them.

---

### User Story 2 - A frame opens its panel, attached to it and under the chrome (Priority: P1)

The person opens a widget's panel from its frame. The panel opens beside the frame, attached to
it, without dimming or blocking the rest of the window, and never covers the chrome. It closes
again from the frame, with Escape, or by clicking outside it.

**Why this priority**: the panel is where declared actions and settings will live (a later
feature); the host must already keep it attached and under the chrome, since Principle V forbids
any frame or panel from covering the chrome (HS-01).

**Independent Test**: open a frame's panel at several window sizes and scale factors, move the
frame toward the window's top edge, and check the panel stays attached and the chrome uncovered.

**Acceptance Scenarios**:

1. **Given** a frame, **When** the person opens its panel, **Then** the panel appears attached to
   that frame, drawn with the host's panel tokens, and the rest of the window still takes input.
2. **Given** a frame near the top of the window, **When** its panel opens, **Then** neither the
   frame nor the panel intersects the chrome, at window sizes 800 × 600, 1280 × 800 and
   2560 × 1440 and at scale factors 1, 1.5 and 2 (HS-01).
3. **Given** an open panel, **When** the person presses Escape or clicks outside it, **Then** it
   closes and focus returns to the frame control that opened it.

---

### User Story 3 - A host dialog takes the form no widget can produce (Priority: P1)

When the host asks the person to decide something, it shows a host dialog: a scrim covers the
whole of every host window, chrome included; the dialog sits above it; nothing outside the
dialog takes pointer or keyboard input until it is answered; and its buttons ignore input for its
first second, so a click or key meant for something else cannot answer it. Closing the window or
quitting while it is open answers it as a refusal. The host never asks through the operating
system's native prompt.

**Why this priority**: Principle VII makes this form the way a person tells a host dialog from
anything a widget draws. Installation, permissions, accounts and destinations (later features)
all depend on this component behaving exactly so.

**Independent Test**: from a frame's menu, choose "Remove from dashboard" over a dashboard with
frames, an open panel and a second host window; send clicks and keys to everything outside the
dialog and to its buttons at timed moments; check what each one reaches and how the dialog is
answered.

**Acceptance Scenarios**:

1. **Given** a dashboard with frames and an open panel, **When** a host dialog opens, **Then** its
   scrim covers every host window entirely, chrome included, and the dialog's surface sits above
   it (HS-02).
2. **Given** an open dialog, **When** clicks and keys are sent to the chrome, the frames, the
   panel and another host window, **Then** none reaches its target, and a click outside the dialog
   neither closes nor answers it (HS-10).
3. **Given** a dialog that has just appeared, **When** its buttons are clicked or Enter, Space or
   Escape is pressed at 0, 500 and 999 ms, **Then** nothing answers it; at 1000 ms input does; a
   key or button held from before it appeared answers nothing until released and pressed again;
   the second starts over when the window regains focus or the dialog's content changes (HS-11).
4. **Given** an open dialog, **When** the person closes the window or quits, **Then** the dialog is
   answered as a refusal (HS-10).
5. **Given** the host's code and dependencies, **When** they are checked, **Then** no path that
   grants, installs, connects or chooses a destination calls a native prompt, and the native
   dialog crates stay banned (HS-12).
6. **Given** the person chose "Remove from dashboard" on a frame, **When** they confirm the dialog,
   **Then** the plugin's approval and every grant with it are removed and its frame leaves the
   window; **When** they refuse it, close the window or quit, **Then** nothing changes.

---

### User Story 4 - Keyboard and screen-reader users can answer a dialog safely (Priority: P2)

A keyboard user finds focus inside a newly opened dialog, on a control that does not confirm it;
Tab and Shift+Tab cycle through the dialog's controls only; Escape refuses it and puts focus back
where it was. A screen-reader user hears the dialog announced as the host's, with the host's name
for the decision and its question in the host's words, and while it is open cannot reach anything
outside it, even with the screen reader's own navigation.

**Why this priority**: the dialog's form is a security property only if it holds for every way a
person reaches the dialog. GPUI does not mark a dialog modal for assistive technology, so this
needs work beyond focus handling.

**Independent Test**: in a real window, open a dialog from the keyboard and from the pointer;
record focus after opening, after many Tabs and Shift+Tabs and after Escape; read the
accessibility tree of every host window while the dialog is open.

**Acceptance Scenarios**:

1. **Given** a dialog opened from the keyboard or the pointer, **When** it appears, **Then** focus
   is on its first control that does not confirm it (Cancel or the first field), and Enter there
   never confirms it (HS-16).
2. **Given** an open dialog, **When** Tab or Shift+Tab is pressed more times than it has controls,
   **Then** focus cycles through its controls only; **When** Escape is pressed, **Then** it closes
   as a refusal and focus returns to where it was (HS-17).
3. **Given** an open dialog, **When** the accessibility tree of every host window is read, **Then**
   only the dialog and its contents are exposed; no chrome, frame, panel or widget node can be
   reached (HS-18).
4. **Given** a dialog opens, **When** its accessibility node is read, **Then** it has the dialog
   role, is labelled with the host's name for the decision and described by its question in the
   host's words, with any plugin name in a child node of its own after the host's words (HS-19).
5. **Given** a release is being prepared, **When** the maintainer walks every host dialog with
   VoiceOver, Narrator and Orca, **Then** each walk confirms scenarios 1 to 4, and its result is
   recorded with the release.

---

### User Story 5 - Nothing a plugin supplies makes the window fetch, open or decode on its own (Priority: P2)

A plugin author tries to make the window reach outside the sandbox: an image address, a link, a
string shaped like markup, an enormous or malformed value. The window turns plugin content only
into the host's own components, accepts images only as bytes it has already been given (and, until
decision 0001 is revisited, no plugin image at all), never passes a plugin's string to GPUI's image
loader or to the function that opens a URL, and its HTTP client is null, so nothing the window
draws can make a network request.

**Why this priority**: these rules replace the content security policy a web view would have
enforced; each one closes a path to the network or to a decoder in the host process, beside the
secrets vault.

**Independent Test**: load hostile plugins whose content carries image addresses, links, file
paths and markup, and check in the security suite that no request leaves, no file is read, no URL
is opened and no decoder runs on plugin bytes.

**Acceptance Scenarios**:

1. **Given** a plugin whose output contains an image address, a `file:` path or a link, **When**
   the window draws its frame, **Then** no network request is made, no file is read and no URL
   is opened.
2. **Given** the host's code, **When** it is checked, **Then** no plugin-supplied string reaches
   GPUI's image loader or its URL opener, and a change that adds such a path fails the build.
3. **Given** the running window, **When** anything it draws asks GPUI's HTTP client for a
   resource, **Then** the request fails without reaching the network.
4. **Given** a plugin's output, **When** the window draws, **Then** it becomes only host components
   (in this feature, the host's status words in its frame); no markup, style or script in it has
   any effect.

---

### User Story 6 - The maintainer can update GPUI safely (Priority: P3)

The maintainer updates the GPUI crates at most once a month, to a version published from a Zed
stable release tag, after reviewing the republisher's patch list, and knows from CI on Windows,
macOS and Linux whether the window, the dialog and the security rules still hold.

**Why this priority**: GPUI is a large dependency outside the project's control; its upgrades
must be deliberate and checked, but the window works without an upgrade path in place.

**Independent Test**: try to build with a GPUI version that is not pinned with `=`, or from a
`-pre` release, and check that the checks refuse it; run the real-window tests on the three
systems.

**Acceptance Scenarios**:

1. **Given** the host's manifest, **When** a GPUI crate is not pinned with `=` or comes from a
   `-pre` release, **Then** a check fails the build.
2. **Given** an upgrade, **When** CI runs, **Then** the real-window tests and the security suite run
   on Windows, macOS and Linux.

---

### User Story 7 - The person picks the dashboard's appearance (Priority: P2)

From the chrome, the person opens the host's appearance settings and picks a gray family, a host
color (or monochrome), a widget color, the corners and the theme (light, dark or the system's).
The window takes the new look at once. A widget color whose hue sits less than 30° from the host
color or from a status tone is refused, and the host says why in its own words, naming the colors.
The choice is kept for the next run.

**Why this priority**: feature 007 built the picks and the rule and left their screen to the host
window; without it the person can only ever see the default look.

**Independent Test**: open the appearance settings, pick each option, check the window redraws in
the new look; pick a widget color too close to the host color and check the refusal and its words;
restart and check the choice is kept.

**Acceptance Scenarios**:

1. **Given** the window, **When** the person opens the appearance settings from the chrome,
   **Then** they see every gray family, the host color (monochrome or a chromatic scale), the
   widget color, the corners and the theme, with the current choice marked.
2. **Given** a valid choice, **When** the person applies it, **Then** the window redraws at once in
   that appearance, chrome, frames, panels and dialogs alike.
3. **Given** a widget color less than 30° from the host color or a status tone, **When** the person
   picks it, **Then** the host refuses it and names, in its own words, the colors that are too
   close; the previous choice stays.
4. **Given** a saved choice, **When** the dashboard starts again, **Then** it opens in that
   appearance.

---

### Edge Cases

- **No approved plugins**: the window opens with the chrome and an empty state in the host's words
  that says how a plugin is added at this stage (the development command-line tool).
- **A plugin's name is long, empty, mixes scripts or carries control or bidirectional
  characters**: the frame's label and monogram follow feature 007's rules for the label (HS-06,
  HS-07): made safe and capped, never trusted as markup.
- **A render that hangs, traps or exceeds its limits**: the runtime's limits end it; the frame
  shows the failure in the host's words and the window stays responsive.
- **The window is resized smaller than a frame**: frames and panels move or clip, but never cover
  the chrome (HS-01).
- **The command-line tool changes the approval record while the window is open**: the window
  shows the change when it regains focus or on "Reload" (FR-040); if the tool removed a plugin
  whose "Remove from dashboard" dialog is open, confirming it removes nothing more and the frame
  leaves the window.
- **Two dialogs are requested at once**: one shows at a time; the second waits, and when it
  appears its first-second delay starts over (HS-11).
- **A dialog opens while a key or pointer button is held**: that press answers nothing until
  released and pressed again (HS-11).
- **The window loses focus while a dialog is open**: when it regains focus, the first-second delay
  starts over (HS-11).
- **The machine has no GPU driver GPUI can use**: the window does not open and the host says why
  in a message a person can act on; making Linux without hardware Vulkan work well is a later
  feature.
- **A screen reader is running when the dashboard starts**: the tree is correct from the first
  frame; nothing depends on the screen reader starting first.
- **A second host window exists** (for example a test window): a dialog blocks it too (HS-10).
- **The saved appearance is no longer valid** (a hand-edited or damaged settings file, or a rule
  that changed): the host opens in the default appearance and says, in its own words, that the
  saved one could not be used.
- **The theme is the system's and the system switches between light and dark**: the window follows
  without a restart.

## Requirements *(mandatory)*

### Functional Requirements

**The window**

- **FR-001**: The host MUST open the dashboard as a native window drawn by the host's own
  components, with no web view, HTML, script or style sheet anywhere in the window.
- **FR-002**: The host MUST be a crate of its own, the only crate the dependency checks let depend
  on the host-surface tokens of feature 007.
- **FR-003**: Every GPUI crate the host uses MUST be a direct dependency pinned with `=` to a
  version published from a Zed stable release tag, never a `-pre` one, under Apache-2.0, never
  GPL-3.0-or-later (constitution, fixed decisions); a check MUST fail the build otherwise. The
  republisher's patch list MUST be reviewed on every upgrade, and upgrades happen at most once a
  month unless a security advisory requires one.
- **FR-004**: Code copied and adapted from gpui-base (Apache-2.0) MUST keep its license notice and
  record where it came from, at which version and what was changed.
- **FR-005**: When the window opens, it MUST focus its root so that the first Tab reaches the
  first control.
- **FR-006**: The window MUST draw the chrome, frames, panels and dialogs only from the tokens
  feature 007 generates from `DESIGN.md`, for the person's appearance and theme; nothing in the
  host MAY hard-code a color, type size, radius, line or shadow.
- **FR-007**: The window MUST show one frame per plugin the host has approved for loading (at this
  stage, the development grant record of the command-line tool), each with the monogram and
  label feature 007 defines, made safe as its criteria require.
- **FR-008**: Until the widget kit exists, a frame MUST show, in place of the widget's content and
  in the host's words with the host's status tones, whether the widget loaded and rendered or
  why it failed; it MUST NOT show any part of what the plugin's render returned.
- **FR-009**: A failure in one plugin MUST NOT stop the window from drawing or taking input for the
  others.
- **FR-040**: The window MUST read the approval record again when it regains focus and when the
  person chooses "Reload" in the chrome, adding frames for newly approved plugins and removing
  those no longer approved; it MUST NOT watch the file. A change of the record while a host dialog
  is open MUST wait until the dialog is answered.
- **FR-010**: A frame MUST be able to open its panel, attached to it, drawn with the host's panel
  tokens; the panel MUST NOT dim anything or block input to the rest of the window, and MUST
  close on Escape and on a click outside it, returning focus to the control that opened it.
- **FR-011**: No frame or panel MAY intersect the chrome, at window sizes 800 × 600, 1280 × 800
  and 2560 × 1440, at scale factors 1, 1.5 and 2, and while frames move toward or past the
  window's edges (HS-01).
- **FR-012**: Every control the host draws MUST show the host's focus ring when it has keyboard
  focus: the `focus` role, 2 px thick and 2 px outside the control, never hidden (HS-24).
- **FR-013**: When no GPU driver GPUI can use is present, the host MUST NOT crash silently; it MUST
  say, in words a person can act on, why the window cannot open.

**Appearance**

- **FR-034**: The chrome MUST open the host's appearance settings, a host surface drawn with host
  components, where the person picks the gray family, the host color (monochrome or a chromatic
  scale), the widget color, the corner radius and the theme (light, dark or the system's), from
  the sets feature 007 defines.
- **FR-035**: The host MUST refuse a widget color less than 30° of hue from the host color or a
  status tone, using feature 007's rule, and MUST name the colors involved in its own words; a
  refused choice MUST leave the previous one in place.
- **FR-036**: A valid choice MUST apply at once to every host surface and MUST be kept, in a host
  file no plugin can reach, for the next run; a saved choice the host cannot use MUST fall back to
  the default with a notice in the host's words.
- **FR-037**: With the system's theme picked, the window MUST follow the system's switches between
  light and dark without a restart.

**Host dialogs**

- **FR-014**: The host MUST provide one dialog component, drawn only with the host's own
  components and host-surface tokens, through which every host decision is asked.
- **FR-015**: While a host dialog is open, its scrim MUST cover the whole of every host window,
  chrome included, and the dialog's surface MUST sit above it (HS-02).
- **FR-016**: While a host dialog is open, no pointer or keyboard input outside it MAY reach
  anything in any host window; a click outside it MUST NOT close or answer it; closing a window
  or quitting MUST answer it as a refusal (HS-10).
- **FR-017**: A host dialog's controls MUST ignore pointer and keyboard input for its first 1000 ms,
  counted from the first frame that shows it; a key or button held from before MUST answer
  nothing until released and pressed again; the 1000 ms MUST start over when the window regains
  focus and when the dialog's content changes or another dialog replaces it (HS-11).
- **FR-018**: Only one host dialog MAY be shown at a time; further requests wait in order.
- **FR-038**: Each frame MUST offer, from a menu the host draws in its header, "Remove from
  dashboard", which asks in a host dialog named with the host's words for the decision (the
  plugin's name in a child node of its own). Confirmed, it MUST remove the plugin's approval and
  every grant with it, exactly as the command-line tool's `remove` does, and the frame leaves the
  window; refused (including by closing or quitting), nothing changes.
- **FR-019**: The host MUST NOT ask for any trust decision (granting a permission, installing a
  plugin or connector, connecting an account, choosing a destination) through GPUI's native
  prompt or any operating-system dialog; the native dialog crates MUST stay banned, and a check
  MUST fail the build if a host path calls a native prompt (HS-12).
- **FR-020**: When a dialog opens, focus MUST land on its first control that does not confirm it;
  a dialog MUST have no default button, so Enter in its initial focus or in a field never confirms
  it (HS-16).
- **FR-021**: Tab and Shift+Tab MUST cycle focus through a dialog's controls only; Escape MUST close
  it as a refusal and return focus to where it was before it opened (HS-17).

**Accessibility**

- **FR-022**: While a host dialog is open, the accessibility tree of every host window MUST expose
  only the dialog and its contents; no chrome, frame, panel or widget node MAY be reachable, by
  focus or by a screen reader's own navigation, until it closes (HS-18). Because GPUI does not mark
  a dialog modal, the host MUST achieve this either through a marking contributed upstream or by
  compensating in the host; the plan records which.
- **FR-023**: A host dialog's accessibility node MUST have the dialog role, be labelled with the
  host's name for the decision and described by its question in the host's words; any plugin name
  MUST be in a child node of its own, after the host's words (HS-19).
- **FR-024**: Everything a frame shows of a plugin (its label, its status) MUST be exposed to
  assistive technology as that widget's content, under the label the host shows (Principle V).
- **FR-025**: CI MUST check the accessibility tree (FR-022 to FR-024) on Windows, macOS and Linux,
  and the keyboard behavior (FR-005, FR-010, FR-020, FR-021) with real key events in a real window,
  not only headless.
- **FR-026**: Before every release, the maintainer MUST walk every host dialog with VoiceOver,
  Narrator and Orca, following a written script, and record the result; this walk adds to the
  automated checks and never replaces them.

**Rules that replace the content security policy**

- **FR-027**: A plugin's output MUST become only host components; no part of it MAY be interpreted
  as markup, style, script, a path or an address.
- **FR-028**: Images MAY reach GPUI only as bytes the host already holds; until decision 0001 is
  revisited, no image, SVG or icon a plugin supplies MAY be decoded, and frames show the monogram
  only. Fonts are only the host's own and, for glyphs they lack, the system's.
- **FR-029**: No string a plugin supplies MAY reach GPUI's image loader or its URL opener; a check
  in the security suite MUST fail the build when a host code path could pass one.
- **FR-030**: GPUI's HTTP client MUST be null in the host: any request made through it fails
  without reaching the network, and a test in the security suite proves it.
- **FR-031**: Each rule of FR-027 to FR-030, and each form rule of FR-015 to FR-017 and FR-022,
  MUST have a hostile case in the security suite that fails the build when the rule breaks, on
  Windows, macOS and Linux (Principle VIII).

**Process isolation**

- **FR-032**: The plan MUST revisit decision 0001 for this feature's scope: whether the runtime,
  and the decoding of images, SVG and fonts, move to a separate process; its outcome MUST be
  recorded in `docs/decisions/`. Until it allows them, FR-028 holds.

**The host's words**

- **FR-039**: Every text the host writes in the window (chrome, frame status, menus, appearance
  settings, dialog names, questions and buttons, and their accessible names) MUST come from one
  message catalog, in English in this feature; no host text MAY be written inline in a screen, and
  a check MUST fail the build when one is.

**Scope**

- **FR-033**: This feature MUST NOT change the behavior of the plugin runtime, the permission
  engine, the connector engine, the vault or the WIT contract; it uses them as they are, except for
  one read-only addition: a way to list the approvals in the grant record, which the window needs
  to know which plugins to load (research R5), reviewed line by line as security core.

### Key Entities

- **Window**: the dashboard's native host window, holding the chrome and the frames; the only
  window in normal use, with a second one only in tests.
- **Chrome**: the host's own bar at the top of every host window; only a host dialog covers it.
- **Frame**: the host-drawn border of one widget, with the plugin's monogram and safe label, its
  status in the host's words, and the control that opens its panel.
- **Panel**: the surface a frame opens, attached to it, for the plugin's controls in later
  features; never dims or blocks.
- **Host dialog**: a request for a decision, with the host's name for it, its question in the
  host's words, its controls (a refusing control first, the confirming one never default), and its
  answer (confirmed or refused; closing and quitting refuse).
- **Approved plugin**: a plugin the host may load; at this stage, one in the development grant
  record.
- **Appearance choice**: the person's gray family, host color, widget color, radius and theme,
  valid only under feature 007's hue rule, kept by the host between runs.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On the reference machine, the window is drawn with every approved plugin's frame
  within 2 s of starting, with 5 plugins approved.
- **SC-002**: 100% of the criteria tested by *host window* or *host dialogs* (HS-01, HS-02,
  HS-10, HS-11, HS-12, HS-16 to HS-19, HS-24) have an automated test that runs in CI on Windows,
  macOS and Linux and fails when the criterion breaks.
- **SC-003**: Every rule that replaces the content security policy (FR-027 to FR-030) has at least
  one hostile case in the security suite, and the suite only grows (Principle VIII).
- **SC-004**: In the timing tests, 0 inputs answer a dialog before 1000 ms and 100% of inputs at or
  after 1000 ms do.
- **SC-005**: While a dialog is open, 0 accessibility nodes outside it are reachable in any host
  window, on all three systems.
- **SC-006**: A host dialog appears within 100 ms of the host asking for it, so the delay a person
  feels is the deliberate 1000 ms and not more.
- **SC-007**: The pre-release screen-reader walks pass on VoiceOver, Narrator and Orca for every
  host dialog, with the results recorded.
- **SC-008**: A change that adds an unpinned or `-pre` GPUI crate, a native prompt, a plugin string
  reaching the image loader or URL opener, or a working HTTP client fails CI.

## Assumptions

- The window loads the plugins the development command-line tool approved, from its development
  grant record; installing plugins from repositories, and the install dialog, are later features.
  No decision that grants anything is asked in the window yet; its first real host dialog is
  "Remove from dashboard" (FR-038), which only takes away, and the tests and screen-reader walks
  use it.
- The widget kit, which draws widget content, is a later feature; until then frames show status,
  not content, and the rule of FR-027 is met by drawing none of the returned content.
- The dashboard grid (adding, moving, resizing and saving layouts) is a later feature; this one
  lays frames out simply, in a fixed order, enough to test HS-01 and the panel.
- Declared actions and settings, and their fields (HS-13 to HS-15, HS-20 to HS-22), are a later
  feature; the panel exists here empty or with host text only.
- The 1000 ms input delay is feature 007's decision (research R11 there).
- Linux without hardware Vulkan (software rendering, a hardware matrix, Wayland and X11) is a
  later feature; this one only refuses clearly when no usable driver exists.
- CI runners can open real windows: macOS and Windows in their desktop sessions, Linux under a
  virtual display with a software renderer; the plan confirms how.
- "Reference machine" is the one feature 001 measured on (8 cores).
