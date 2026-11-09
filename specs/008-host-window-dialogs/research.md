# Research: The Host Window, Host Dialogs and the Rules That Replace the Webview's CSP

Facts below come from two reviews made on 2026-11-09, read only: the published sources of
gpui-unofficial 1.25.3 and of the AccessKit 0.24 crates and adapters it resolves to, and the
GitHub runner images and the projects that run real windows on them (AccessKit, enigo, Vello).
Nothing was built or run for them; what they could not confirm is listed in R15 and turned into
the first tasks.

## R1. Which GPUI, and how it is pinned

**Decision**: gpui-unofficial **1.25.3**, generated from Zed tag `v1.25.3` (commit
`8dd255c3eafb5231056fe4e6f601cedd011f1c93`), published on 2026-10-23, the newest stable version at
least two weeks old (v1.26.2 is from 2026-10-28, v1.27.0 from 2026-11-04). All **19** crates of the
line are direct dependencies of `host`, each pinned with `=1.25.3`: `gpui-unofficial` (its library
is `gpui`), `gpui-platform-gpui-unofficial`, `collections-`, `derive-refineable-`, `refineable-`,
`gpui-macros-`, `gpui-shared-string-`, `gpui-util-`, `http-client-`, `scheduler-`, `sum-tree-`,
`ztracing-`, `ztracing-macro-`, and the platform crates `gpui-linux-`, `gpui-wgpu-`, `gpui-macos-`,
`gpui-apple-`, `gpui-windows-`, `gpui-web-` (each `*-gpui-unofficial`), the platform ones under
`[target.'cfg(...)'.dependencies]`. A test in `host` reads `host/Cargo.toml` and `Cargo.lock` and
fails when any `*gpui-unofficial*` package is not a direct `=` dependency, is a pre-release, or is
not the version `host/README.md` records with the patch list it was reviewed against.

**Rationale**: the constitution asks for every GPUI crate direct, pinned with `=`, from a stable
tag. The siblings depend on each other with caret requirements: pinning only `gpui-unofficial` and
the platform crate resolved eleven siblings to 1.27.0 in a resolution probe; pinning all nineteen
kept every one at 1.25.3. The republisher's patch list is `docs/upstream-patches.md` in
`gpui-archipelago/gpui-unofficial`, at commit `58b5a3814e` when 1.25.3 was published (30 entries;
the ones to read on every upgrade: SRC-1, wgpu mapped from a git revision to crates.io 30.0.1;
PUB-1 and MAN-4, git dependencies swapped for crates.io forks such as `zed-font-kit`; PKG-3,
crates published with `--no-verify`). Every crate of the line is Apache-2.0; the forks are MIT or
MIT/Apache-2.0; no GPL-only crate is in the normal or build graph on the three systems (one,
`self_cell`, is "Apache-2.0 OR GPL-2.0-only", taken under Apache-2.0).

**Alternatives considered**: `gpui-pre` (a weekly snapshot of Zed's main branch by another
publisher, the base of gpui-base; not from stable tags); Zed's own `gpui` on crates.io (last
published 0.2.2 in 2025-10); v1.26.2 or v1.27.0 (under two weeks old).

**Upgrades**: at most once a month, or at once for a security advisory: read the patch list's
diff since the recorded commit, move all nineteen pins together, update `host/README.md`, and let
CI run the real-window and security tests on the three systems (US6).

## R2. Decision 0001, revisited for this feature

**Decision**: plugins keep running in the host process during this feature, and no byte a plugin
supplies reaches a decoder: frames show no content (FR-008), no image or icon from a plugin
exists (FR-028), and the only fonts are the host's own, loaded from its binary, with the system's
for glyphs they lack. The plugin's name is shaped as text by those fonts; no font, image or SVG
comes from a plugin. Recorded as a revisit section in `docs/decisions/0001-plugin-process-isolation.md`,
whose next trigger stays as written: before the app installs a plugin it did not build (the
installation feature), and on any Wasmtime escape advisory.

**Rationale**: 0001 asks for its revisit in Phase 3's plan "at the latest" before the app can
install a plugin it did not build. This feature installs nothing: it loads only packages the
maintainer approved with the development tool, the same trust 0001 already accepts. GPUI moving
decoding into the host process matters once plugin bytes can be decoded, and this feature keeps
them out.

**Alternatives considered**: moving the runtime out of process now (an inter-process protocol to
design and secure before any plugin can be installed; the cost lands where the risk arrives, at
installation); a separate decoding process now (nothing to decode yet).

## R3. The `host` crate

**Decision**: a new workspace member `host/`, library `host` and binary `dashboard`, the only crate
`deny.toml` lets depend on `host-surfaces` (already a wrapper rule since feature 007). It depends on
`runtime`, `design-tokens`, `host-surfaces` and the nineteen GPUI crates, and on `serde_json` for
its settings file. Modules: `app` (start, fonts, HTTP client, prompt builder), `theme` (tokens to
GPUI values), `text` (the message catalog, R7), `label` (a plugin's name made safe, R8), `chrome`,
`frame`, `panel`, `dialog` (the modal layer, the input gate, the queue), `a11y` (the modal marking,
R10), `appearance` (the settings screen and its file, R11), `plugins` (loading and rendering through
the runtime, R5), `media` (the only path to GPUI's image element, R12), and `vendor/` (code adapted
from gpui-base, R9).

**Rationale**: one crate keeps the host-surface tokens behind the existing ban, and modules map
one to one to the spec's surfaces, so a review can follow a rule to the module that keeps it.

## R4. Fonts in the window

**Decision**: the window registers Barlow (Regular, Medium, SemiBold) and IBM Plex Mono (Regular,
Medium) from `design/fonts/` with `include_bytes!` and GPUI's text system at start, uses them by
the families `DESIGN.md` names, and loads no other font file; GPUI's system fallback covers glyphs
they lack (scripts outside Latin, emoji). The host ships the fonts' licenses.

**Rationale**: the fonts are the host's (feature 007, R9 and R14); embedding them makes the window
look the same on every system and leaves no font path a plugin could name.

## R5. Which plugins the window shows, and the one runtime change

**Decision**: the window lists the approvals of the development grant record and loads each from
the origin it was approved from (the package path), with `Runtime::load` and the record's
`GrantStore`, as `runtime run` does; it renders each once after load and again on reload (FR-040),
on GPUI's background executor, never on the UI thread. The runtime gains one **read-only** call,
`GrantStore::approvals()`, listing every approval's plugin id and origin; nothing else in the
runtime changes. "Remove from dashboard" calls `GrantStore::remove` and `Runtime::discard_kept_data`,
exactly the calls of `runtime remove`, and keeps the plugin's store, as the tool does.

**Rationale**: the record has no listing today (`entry(id)` reads one), and reading its file from
the host would duplicate the parser of a security file. A read-only listing is the smallest
change; it is in the permission engine, so it gets line-by-line review (Principle IX). FR-033 is
narrowed to allow it.

**Alternatives considered**: a list of package paths on the command line (approvals and the
window could disagree); parsing the record in the host (two parsers of one security file).

## R6. Layout before the grid, and the chrome kept uncovered

**Decision**: the window is a column: the chrome, a fixed band, then the content area below it,
which clips everything it holds, frames and panels included. Frames flow in a wrap of fixed-width
columns in approval order. A panel is drawn inside the content area, attached to its frame's side,
flipping to the other side or below when there is no room, never in a layer above the chrome.
GPUI's floating layers (`deferred`, `anchored`) are not used for panels.

**Rationale**: HS-01 then holds by construction (nothing in the content area can draw over a
sibling above it), and the test still measures it at the three sizes and scale factors.

## R7. The host's words: one catalog, enforced by type

**Decision**: every host string is a variant of an enum `Msg` in `host::text`, with its English
text in one `match`; screens take `HostText`, a type only `text` can build from a `Msg`, or
`PluginText`, a type only `label` can build from a plugin's string (R8). GPUI's `IntoElement` for
`&str` and `String` is still there, so a test scans `host/src/` and fails on a string literal
passed to `child`, `aria_label`, `aria_description` or a text element outside `text.rs`.

**Rationale**: FR-039; the types make the right path the easy one, the scan catches the shortcut.

## R8. A plugin's name in the frame

**Decision**: the label follows feature 007's rules for the label and monogram (HS-06, HS-07):
the manifest's name in Unicode NFC, control, format and bidirectional characters removed, capped
at 40 visible characters with an ellipsis, the plugin id when nothing is left; the monogram is its
first grapheme cluster, uppercased, from the host's own font. It reaches the screen only as
`PluginText`, drawn as plain text, and for assistive technology in a child node of its own after
the host's words (HS-19).

**Rationale**: the frame's header is the first place a plugin's string appears in a window, so the
rules 007 wrote for the kit apply here first; HS-06 and HS-07 stay tested by the kit as well.

## R9. The dialog's form: one modal layer over every host window

**Decision**: a global `DialogHost` holds the queue (FR-018) and the open dialog. Every host window
has the same root view, which, while a dialog is open, draws over its whole surface, chrome
included, a scrim in the `scrim` role that takes every pointer event (`occlude`), and routes every
key to the dialog; the dialog itself is drawn in the window that asked, above its scrim. The
answer is a `Confirmed` or `Refused`; a window's close request and the app's quit answer the open
dialog `Refused` before they proceed (HS-10). Focus: on open, the dialog focuses its first control
that does not confirm; it registers no default button; Tab and Shift+Tab cycle inside it through a
focus trap adapted from gpui-base 0.9.0's `src/focus_trap.rs` (about 250 lines, Apache-2.0, kept
under `host/src/vendor/gpui_base/` with its notice and a provenance file: crate, version, file,
SHA-256 and what changed), and Escape answers `Refused` and gives focus back to the handle that
held it before (HS-16, HS-17).

**The input gate** (HS-11, FR-017): a `Clock` trait, the system clock in the app and a manual one
in tests; the gate counts 1000 ms from the first frame that shows the dialog, restarts on window
activation and on a change of the dialog's content or a new dialog, and remembers the keys and
pointer buttons down when it started, which answer nothing until released.

**Native prompts** (HS-12, FR-019): `clippy.toml` disallows `gpui::Window::prompt`,
`gpui::AsyncWindowContext::prompt`, `gpui::PlatformWindow::prompt`, `gpui::App::open_url`,
`gpui::Platform::open_url`, `gpui::App::prompt_for_paths`, `gpui::App::prompt_for_new_path`,
`gpui::App::open_with_system`, `gpui::App::reveal_path` and GPUI's credential calls; the host also
replaces GPUI's prompt builder with one that refuses at once, so a prompt reached through a path
clippy misses still never shows. `deny.toml` keeps banning `rfd` and `native-dialog`.

**Rationale**: one layer in every window is the simplest way to make "blocks every host window"
true by construction; the gpui-base trap is small, depends only on GPUI, and every API it uses
exists in 1.25.3; its dialog (910 lines) sets no modal state and would be rewritten anyway.

**Alternatives considered**: a separate OS window for the dialog (other windows would need their
own blocking, and the window manager decides stacking); gpui-base's input (about 26 000 lines with
`lsp_types` and a beta `ropey`: not needed until a field exists).

## R10. Modality for assistive technology

**Decision**: compensate in the host, with no upstream change needed: GPUI exposes no `aria_modal`
or `aria_hidden`, but a custom element can write the raw AccessKit node
(`Element::write_a11y_info`). While a dialog is open, each window's background root (the node that
holds the chrome and the content area, given the role `Pane` so it becomes a node) is marked
`set_hidden()`, which AccessKit's shared filter excludes with its subtree on every platform; the
dialog's node has role `Dialog`, `set_modal()`, the host's name for the decision as its label, the
question as its description, and any plugin name in a child node after them (HS-18, HS-19). Focus
inside the dialog keeps the focused node exposed, as the filter requires. An issue proposing
`aria_modal` and `aria_hidden` builders goes upstream, for information.

**Rationale**: no AccessKit adapter hides nodes outside a modal dialog; hiding is the app's job.
`set_modal` is what Orca needs to root its navigation in the dialog (it reads a dialog role with
STATE_MODAL), what UI Automation reports as `WindowIsModal`, and what macOS reports as
`isAccessibilityModal`. `set_hidden` removes the rest even from flat review.

## R11. Appearance settings and their file

**Decision**: a host screen opened from the chrome, laid over the content area like a panel of the
host (not a dialog: it decides nothing about trust), with one group per pick and the current one
marked; a widget color the rule refuses stays selectable but shows the host's refusal naming the
colors (feature 007's `Refusal`) and is not applied. The choice is saved as JSON
(`{"version": 1, "gray", "host", "widget", "radius", "theme"}`) in
`<data dir>/dev.philsantos.dashboard/host/appearance.json`, outside every plugin's store; a file that
does not parse or validate is ignored with a notice and the default is used. With the system theme
picked, the window observes the platform's appearance and redraws on change.

**Rationale**: FR-034 to FR-037; JSON matches the host's other records, and the directory is the
one the development tool already uses.

## R12. The rules that replace the content security policy

**Decision**:
- **Content becomes only host components** (FR-027): this feature draws none of what a render
  returns; the frame's body is a host status built from `Msg` and the render's outcome. The hostile
  case renders output full of markup, addresses and paths and checks the frame's elements hold
  only host text and the safe label.
- **Images only as bytes, none from plugins yet** (FR-028): `media::image_from_bytes(Arc<gpui::Image>)`
  is the only caller of `gpui::img`, which `clippy.toml` otherwise disallows; nothing calls it in
  this feature.
- **No plugin string to the image loader or the URL opener** (FR-029): `gpui::img` is reachable only
  through `media` with bytes, and `open_url` is disallowed everywhere; a test reads `clippy.toml`
  and fails if those entries go.
- **A null HTTP client** (FR-030): the app is built with `with_http_client` and GPUI's
  `BlockedHttpClient`; GPUI's own default is a null client too, but the host sets one explicitly
  so a GPUI upgrade cannot change it. A test asks the app's client for a URL and expects the
  refusal; another draws an image element from a URI string in a test harness and counts no
  request reaching a recording client.
- Each rule has a hostile case under `host/tests/hostile/` (FR-031), alongside the runtime's suite.

**Rationale**: GPUI's `img("…")` turns a string into a URI fetched through the app's HTTP client, a
path read from disk, or an embedded asset; `file://` URIs also go to the client. With the client
blocked and `img` reachable only with bytes, no string can become a fetch or a read.

## R13. Tests: four layers

**Decision**:
1. **Logic, headless, all three systems**: GPUI's `TestAppContext` with its fake platform: the
   dialog queue, the input gate with a manual clock (0, 500, 999 and 1000 ms, held keys, refocus,
   content change), focus order with simulated keystrokes, the panel's attachment, HS-01's bounds
   at 800 × 600, 1280 × 800 and 2560 × 1440 and scale factors 1, 1.5 and 2, reload, the appearance
   file, the label rules.
2. **Real window, real keys, three systems**: a test target `window` with `harness = false` (GPUI
   needs the main thread) opens the real window and sends OS key events with `enigo` 0.6.1 (XTEST
   under Xvfb on Linux, `CGEventPost` on macOS, `SendInput` on Windows): first Tab, the dialog's
   initial focus, Tab and Shift+Tab cycling, Escape refusing, the gate at real time.
3. **Accessibility tree, three systems**: in the same target, with the adapter activated, the tree
   is read through the platform (AT-SPI with the `atspi` crate on Linux, UI Automation with the
   `windows` crate on Windows from an MTA thread, AX on macOS from another thread) and also from
   GPUI's `debug_a11y_tree_json` through `accesskit_consumer`'s `common_filter`, the filter every
   adapter uses: only the dialog and its contents reachable while it is open (HS-18), and the
   dialog's role, label, description and child node (HS-19).
4. **Screen-reader walk, before each release**: a written script in `docs/accessibility/` for
   VoiceOver, Narrator and Orca over every host dialog, its results recorded with the release
   (FR-026).

**CI setup**: Linux inside `dbus-run-session`, with `xvfb`, `mesa-vulkan-drivers`, `libegl1`,
`libgl1-mesa-dri`, `at-spi2-core` and GPUI's build libraries installed; the AT-SPI bus started by
its first `GetAddress` and switched on with `busctl --user set-property org.a11y.Bus /org/a11y/bus
org.a11y.Status IsEnabled b true`, which also activates GPUI's adapter; X11 under Xvfb with an
explicit focus of the window, `WAYLAND_DISPLAY` unset. Windows and macOS in their desktop
sessions. The real-window target runs after the other tests, in the existing build job.

**Rationale**: GPUI's tests never send OS events and its fake window has no accessibility tree, so
layers 2 and 3 need a real window; AccessKit's own CI proves UI Automation on `windows-latest`, and
enigo's proves XTEST under Xvfb.

## R14. Performance goals and what is measured

**Decision**: the real-window target measures, on the three systems, the time from start to the
first frame with every frame drawn (5 approved fixtures; SC-001, under 2 s on the reference
machine) and from a dialog request to the first frame that shows it (SC-006, under 100 ms). The
bench numbers are recorded in this feature's `performance.md`, as earlier features did.

## R15. What the reviews could not confirm, and how the tasks settle it

1. **Whether 1.25.3 compiles as published** (some crates went out with `--no-verify`): the first
   task builds the window on the three systems before anything else is written.
2. **Whether macOS runners render a GPUI window** (Metal under virtualization is unconfirmed) and
   **whether `CGEventPost` keys reach it**: a CI spike task opens a window, sends Tab and reads the
   AX tree on `macos-latest`. If rendering or input fails there, the macOS real-window steps run
   what they can (the accessibility tree needs no rendering) and the gap is brought to the
   maintainer before the plan changes, not silently skipped.
3. **Xvfb with lavapipe**: the same spike on Linux.
4. **The exact `disallowed-methods` paths** for GPUI's re-exported types: the spike runs clippy with
   the entries and a file that calls each, and confirms each is caught.
5. **The MSRV**: the workspace's is 1.97 and Zed builds 1.25.3 with 1.98.1; the spike builds with
   1.97 and, if it fails, the plan's MSRV moves to the oldest stable that builds, as a reviewed
   change.
6. **Narrator and VoiceOver confining their cursor to a modal AccessKit dialog**: the release walk
   (FR-026) records it; `set_hidden` on the background makes it hold even if a reader ignores
   `set_modal`.
7. **The licenses the GPUI graph adds to `deny.toml`'s list** (for example BSD-2-Clause, BSL-1.0,
   CC0-1.0 or 0BSD, depending on features): the spike runs `cargo deny check licenses`, and each
   addition is reviewed as part of `deny.toml`, which is security core.
