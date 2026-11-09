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

**Decision**: plugins keep running, and compiling, in the host process during this feature, and
no image, SVG or font bytes a plugin supplies reach a decoder: frames show no content (FR-008), no
image or icon from a plugin exists (FR-028), and the only fonts are the host's own, loaded from its
binary, with the system's for glyphs they lack. A plugin's name does reach text shaping, as text
in the host's fonts. Recorded as a revisit section in
`docs/decisions/0001-plugin-process-isolation.md` that keeps all four of its triggers (before the
app installs a plugin it did not build; a Wasmtime or Cranelift escape advisory; loading a package
near the cap taking more than 2 s or 1 GiB on any CI system; before AI-generated modules are
compiled on the user's machine), corrects its stale line about installing from a local file
(installation is only from repositories, threat model), and names the next owners: installation
for the runtime and compilation, the widget kit before any image block or plugin icon.

The window now compiles packages, at start and on reload, which 0001's accepted risk described as
the maintainer loading from the command-line tool. It is still only packages the maintainer
approved with that tool; the window measures the compile cost of each load (R14) so the third
trigger can be seen from the window too.

**Rationale**: 0001 asks for its revisit in Phase 3's plan "at the latest" before the app can
install a plugin it did not build. This feature installs nothing: it loads only packages the
maintainer approved with the development tool, the same trust 0001 already accepts. GPUI moving
decoding into the host process matters once plugin bytes can be decoded, and this feature keeps
them out.

**Alternatives considered**: moving the runtime out of process now (an inter-process protocol to
design and secure before any plugin can be installed; the cost lands where the risk arrives, at
installation); compiling in a separate process and running in the host, the option 0001 names as a
Phase 3 candidate (deferred to installation, the first time a package the maintainer did not build
is compiled; until then the window compiles only what the maintainer already compiles with the
tool); a separate decoding process now (nothing to decode yet).

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

GPUI 1.25.3 has no letter spacing in its text style, so `label-mono` is drawn in capitals without
its 0.06 em; the preview and `DESIGN.md` keep the value for when GPUI can draw it.

The host reads the roles of `design-tokens` by name through `design_tokens::role_color`, a small
public function added for it (the kit keeps asking for tones), and the host surfaces through
`host-surfaces`.

## R5. Which plugins the window shows, and the one runtime change

**Decision**: the window lists the approvals of the development grant record and loads each from
the origin it was approved from (the package path), with `Runtime::load` and the record's
`GrantStore`; it renders each once after load and again on reload (FR-040). Reading the record
(which takes its file lock), reading each package (capped at the package size limit, as the
tool's `read_capped`), loading and rendering all run on GPUI's background executor, never on the
UI thread.

**Development mode stays off.** The runtime documents that only the development tool turns it on
and that "the host app of Phase 3 never does", and feature 003 (FR-018) lets local connectors load
only in that mode. The window starts the runtime with `RuntimeOptions::new(data_dir)` and no
`development(true)`: it takes no `--connectors` option, and a plugin that declares a `local/`
connector fails to load with the runtime's refusal, which its frame shows as "Did not render" with
the host's reason. Reading the development grant record is not granting: the constitution keeps
granting to the tool (Principle VII), and the window only reads approvals and, through "Remove
from dashboard", withdraws one.

**A failure's reason** is the host's words for the runtime's `ErrorKind` (a `Msg` per kind),
never the error's detail or names, which can quote the plugin (the threat-model row "Log or error
text imitates the host's own output"). The runtime gains one **read-only** call,
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
which scrolls vertically and clips everything it holds: frames, panels, a frame's menu and the
appearance settings. Frames flow in a wrap of fixed-width columns in approval order. A panel or a
frame's menu is drawn inside the content area, attached to its frame, flipping to the other side
or below when there is no room. GPUI's floating layers (`deferred`, `anchored`) are not used
anywhere in the host. Every clipping edge keeps at least 4 px of padding inside it, so a control's
2 px focus ring, 2 px outside the control, is never cut (HS-24).

**Window decorations**: the operating system's own title bar and window controls stay, on every
system; the chrome is the first band of the window's content, right under them. The constitution's
"chrome ... that holds its window controls and menus" is met by the chrome holding the host's
menus; the window controls belong to the system, outside what a widget or the host draws. While a
dialog is open the scrim covers the whole content area, chrome included (HS-02); the system's
close control answers the dialog `Refused` before the window closes (HS-10), and minimize and
maximize change nothing about it.

**Rationale**: HS-01 then holds by construction (nothing in the content area can draw over a
sibling above it, scrolled or not), and the test still measures it at the three sizes and scale
factors and with the content scrolled so frames sit past the top, bottom and sides. Client-side
decorations would mean drawing window controls three ways for no gain in what the criteria
observe.

## R7. The host's words: one catalog, enforced by type

**Decision**: every host string is a variant of an enum `Msg` in `host::text`, with its English
text in one `match`; screens take `HostText`, a type only `text` can build from a `Msg`, or
`PluginText`, a type only `label` can build from a plugin's string (R8). GPUI's `IntoElement` for
`&str` and `String` is still there, so a test scans `host/src/` and fails on a string literal
passed to `child`, `aria_label`, `aria_description` or a text element outside `text.rs`.

**Rationale**: FR-039; the types make the right path the easy one, the scan catches the shortcut.

## R8. A plugin's name in the frame

**Decision**: the label follows HS-06 and HS-07 exactly: every Cc, Cf, Zl and Zp character
removed (bidirectional overrides and zero-width characters included); capped at 40 grapheme
clusters (UAX #29) followed by an ellipsis; an empty result shown as the host's "Unnamed widget"
(a `Msg`); a name imitating the host drawn like any other. A manifest has no display name:
a plugin's only name is its id (`local/<name>` or `github.com/<owner>/<repository>`, lowercase
ASCII, `PluginId::parse`), so the frame's label is the id's last segment, made safe the same way,
and the full id is the frame's accessible description. The monogram is the first letter or
digit of the cleaned name, upper-cased where the script has case, or a fixed host glyph when there
is none, in the monogram component's roles. It reaches the screen only as
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

**Other windows**: while a dialog is open, every host window other than the dialog's drops every
key and pointer event (they never reach the dialog, so a key typed elsewhere cannot answer it),
and its focused element is cleared and remembered, then restored when the dialog is answered, so
no focused node in another window stays exposed (R10).

**The input gate** (HS-11, FR-017): a `Clock` trait, the system clock in the app and a manual one
in tests; the gate counts 1000 ms from the first frame that shows the dialog, restarts on window
activation and on a change of the dialog's content or a new dialog, and remembers the keys and
pointer buttons down when it started, which answer nothing until released.

**Native prompts** (HS-12, FR-019): `clippy.toml` disallows `gpui::Window::prompt`,
`gpui::AsyncWindowContext::prompt`, `gpui::PlatformWindow::prompt`, `gpui::App::open_url`,
`gpui::Platform::open_url`, `gpui::App::prompt_for_paths`, `gpui::App::prompt_for_new_path`,
`gpui::App::open_with_system`, `gpui::App::reveal_path` and GPUI's credential calls (and the asset
and image paths of R12); the host also
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
question as its description, and any plugin name in a child node after them (HS-18, HS-19). AccessKit's filter always keeps a focused node, even inside a
hidden subtree, so focus must be inside the dialog in its window and cleared in every other host
window while it is open (R9). An issue proposing
`aria_modal` and `aria_hidden` builders goes upstream, for information.

**Rationale**: no AccessKit adapter hides nodes outside a modal dialog; hiding is the app's job.
`set_modal` is what Orca needs to root its navigation in the dialog (it reads a dialog role with
STATE_MODAL), what UI Automation reports as `WindowIsModal`, and what macOS reports as
`isAccessibilityModal`. `set_hidden` removes the rest even from flat review.

## R11. Appearance settings and their file

**Decision**: a host screen opened from the chrome, drawn in the content area in place of the
frames like a panel of the host (no floating layer, R6; not a dialog: it decides nothing about
trust), with one group per pick and the current one marked; a widget color the rule refuses stays selectable but shows the host's refusal naming the
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
- **Images only as bytes, none from plugins yet** (FR-028): `media` is the only module that may
  call `gpui::img` and `gpui::svg`, which `clippy.toml` otherwise disallows, together with
  `Svg::external_path` (reads a file), `Window::use_asset` and `App::fetch_asset`. `media` takes
  only bytes the host holds: `image_from_bytes(Arc<gpui::Image>)`, unused in this feature, and the
  three status icons of feature 007, drawn from their SVG paths in `DESIGN.md` embedded in the
  binary (host bytes, decoded in the host process). No `ImageSource` is ever built from a string or
  a path.
- **No plugin string to the image loader or the URL opener** (FR-029): `gpui::img` is reachable only
  through `media` with bytes, and `open_url` is disallowed everywhere; a test reads `clippy.toml`
  and fails if those entries go.
- **A null HTTP client** (FR-030): the app is built with `with_http_client` and GPUI's
  `BlockedHttpClient`; GPUI's own default is a null client too, but the host sets one explicitly
  so a GPUI upgrade cannot change it. A test asks the app's client for a URL and expects the
  refusal; another draws an image element from a URI string in a test harness and counts no
  request reaching a recording client.
- **Nothing slips the bans**: a check fails on any `allow(clippy::disallowed_methods)` or
  `allow(clippy::disallowed_types)` in `host/src/` outside `media.rs`.
- Each rule has a hostile case (FR-031): the headless ones under `host/tests/hostile.rs`, the ones
  that need a real window (the accessibility tree, the scrim's cover in pixels) in the `window`
  target, all counted in the security suite.

**Rationale**: GPUI's `img("…")` turns a string into a URI fetched through the app's HTTP client, a
path read from disk, or an embedded asset; `file://` URIs also go to the client; `svg()` can read a
file by path, and assets can be loaded by name. With the client blocked and every one of those
reachable only from `media` with host bytes, no plugin string can become a fetch, a read or a
decode.

## R13. Tests: four layers

**Decision**:
1. **Logic, headless, all three systems**: GPUI's `TestAppContext` with its fake platform: the
   dialog queue, the input gate with a manual clock (0, 500, 999 and 1000 ms, held keys, refocus,
   content change), focus order with simulated keystrokes, the panel's attachment, HS-01's bounds
   at 800 × 600, 1280 × 800 and 2560 × 1440 and scale factors 1, 1.5 and 2, reload, the appearance
   file, the label rules.
2. **Real window, real keys, three systems**: a test target `window` with `harness = false` (GPUI
   needs the main thread, and `test = false` so `cargo test --workspace` never runs it) opens the
   real window and sends OS key and pointer events with `enigo` 0.6.1 (XTEST under Xvfb on Linux,
   `CGEventPost` on macOS, `SendInput` on Windows): first Tab; the panel closing on Escape and on a
   click outside, with focus back on its control and the outside click reaching its target; the
   dialog opened from the keyboard and from the pointer, its initial focus, Tab and Shift+Tab
   cycling, Escape refusing; Enter in a second window after 1000 ms not answering; the gate at real
   time; every focusable host control showing the ring unclipped (HS-24).
3. **Accessibility tree, three systems**: in the same target, with the adapter activated, the tree
   is read through the platform (AT-SPI with the `atspi` crate on Linux, UI Automation with the
   `windows` crate on Windows from an MTA thread, AX on macOS from another thread) and also from
   GPUI's `debug_a11y_tree_json` through `accesskit_consumer`'s `common_filter`, the filter every
   adapter uses: only the dialog and its contents reachable while it is open, in the dialog's
   window and in a second host window (HS-18); the dialog's role, label, description and child
   node (HS-19); each frame's label and status exposed under the frame (FR-024).
4. **Screen-reader walk, before each release**: a written script in `docs/accessibility/` for
   VoiceOver, Narrator and Orca over every host dialog, its results recorded with the release
   (FR-026).

**CI setup**: Linux inside `dbus-run-session`, with `xvfb`, `mesa-vulkan-drivers`, `libegl1`,
`libgl1-mesa-dri`, `at-spi2-core` and GPUI's build libraries installed; the AT-SPI bus started by
its first `GetAddress` and switched on with `busctl --user set-property org.a11y.Bus /org/a11y/bus
org.a11y.Status IsEnabled b true`, which also activates GPUI's adapter; X11 under Xvfb with an
explicit focus of the window, `WAYLAND_DISPLAY` unset. Windows and macOS in their desktop
sessions. The real-window target runs in a job of its own, "Host window", on the three systems,
with its own time budget, so the build job's 30 minutes are not shared with a cold GPUI build and
window tests.

**Rationale**: GPUI's tests never send OS events and its fake window has no accessibility tree, so
layers 2 and 3 need a real window; AccessKit's own CI proves UI Automation on `windows-latest`, and
enigo's proves XTEST under Xvfb.

### How the layers were built (T033, T034)

- **Pixels** (`host/tests/pixels.rs`): GPUI's headless renderer draws the dashboard on the GPU
  without a display; the scrim's colour over the chrome, the content and every corner, and the
  focus ring around a focused control, are checked in pixels. Linux (Mesa's software Vulkan in CI)
  and macOS (Metal) have the renderer; Windows has none and skips, and its real window covers the
  rest.
- **Real window** (`host/tests/window.rs`): two dashboard windows, real keys and clicks from the
  system's own tools (`xdotool` under Xvfb, `osascript` and a CGEvent click on macOS, `SendKeys`
  and `SendInput` on Windows), the same ones the spike proved, instead of `enigo`; the
  accessibility tree read through AT-SPI (Python's Atspi), AX (AppleScript) and UI Automation
  (PowerShell), which also give the screen positions to click. GPUI's `debug_a11y_tree_json` is
  GPUI's own format, not AccessKit's `TreeUpdate`, so the platform reading, after each adapter's
  own `common_filter`, is the check of HS-18.

## R14. Performance goals and what is measured

**Decision**: the real-window target measures, on the three systems, the time from start to the
first frame with every frame drawn (5 approved fixtures; SC-001, under 2 s on the reference
machine), from a dialog request to the first frame that shows it (SC-006, under 100 ms), and the
compile time and peak memory of each package the window loads (decision 0001's third trigger,
R2). The
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
4. **The exact `disallowed-methods` paths** for GPUI's re-exported types (prompts, URL and system
   openers, credentials, `img`, `svg`, `Svg::external_path`, `use_asset`, `fetch_asset`): the spike
   runs clippy with the entries and a file that calls each, and confirms each is caught.
5. **The MSRV**: settled by T002 below: the workspace declares none and CI builds with stable.
6. **Narrator and VoiceOver confining their cursor to a modal AccessKit dialog**: the release walk
   (FR-026) records it; `set_hidden` on the background makes it hold even if a reader ignores
   `set_modal`.
7. **The licenses the GPUI graph adds to `deny.toml`'s list** (for example BSD-2-Clause, BSL-1.0,
   CC0-1.0 or 0BSD, depending on features): the spike runs `cargo deny check licenses`, and each
   addition is reviewed as part of `deny.toml`, which is security core.

### What T002 found (2026-11-09, Linux, rustc 1.99.1)

- **It compiles as published**: every gpui-unofficial 1.25.3 crate, the wgpu 30.0.1 mapping and
  the forks built in 4 minutes in a cold debug build under the 8 GiB cap. Linking the binary needs
  the development symlinks of `libxkbcommon` and `libxkbcommon-x11` (Debian and Ubuntu packages
  `libxkbcommon-dev` and `libxkbcommon-x11-dev`); CI installs them with GPUI's other build
  libraries (T046). With those libraries linked, the binary opened its empty window on the
  maintainer's Wayland session and ran until stopped, with nothing on standard error.
- **The MSRV**: the workspace declares none (`rust-version` appears in no manifest, and CI builds
  with stable); the "1.97" in this plan came from feature 007's plan, not from a declared value.
  The host builds with the stable toolchain CI uses, so nothing moves; declaring an MSRV stays out
  of this feature.
- **Licenses**: `cargo deny check licenses` refuses two, both permissive and compatible with the
  client's Apache-2.0: BSD-2-Clause (`arrayref`, through `tiny-skia` and `resvg`) and CC0-1.0
  (`tiny-keccak`, through `const-random` and `ahash`). The bans pass: nothing in GPUI's graph hits
  the TLS or native-dialog bans.
- **Advisories**: `cargo audit` passes; GPUI adds six "unmaintained" warnings (`paste`,
  `rustybuzz`, `ttf-parser`, `zbus_names`, `zvariant`, `zvariant_derive`), which the audit allows
  by default and none of which is a vulnerability.

### What the spike found (T003, T004, 2026-11-09)

The throwaway branch `008-spike` (deleted afterwards) ran an example that opens a GPUI window
titled "Dashboard spike" with one focusable button, on GitHub's `ubuntu-24.04`, `macos-latest` and
`windows-latest` (runs 42642457786 and 42649459520):

| System | Window drew | A real Tab arrived and moved focus | The platform read the tree |
| --- | --- | --- | --- |
| Linux: Xvfb, Mesa's software Vulkan, `dbus-run-session`, AT-SPI bus with `IsEnabled` set | yes | yes (`xdotool windowfocus … key Tab`) | yes: AT-SPI listed "push button: Spike button" |
| macOS 26 arm64 | yes | yes (`osascript` key code 48, CGEvent) | yes: AX listed "button Spike button" with role `AXButton` |
| Windows Server 2025 | yes | yes (`SendKeys`, SendInput) | yes: UI Automation listed "ControlType.Button: Spike button" |

On every system GPUI built its accessibility tree only after a platform client asked for it
(`is_a11y_active` turned true after the first query), and then `debug_a11y_tree_json` held the
button too, so the `window` target reads the platform first and GPUI's tree after. Research R15's
items 1–4 are settled: 1.25.3 builds and runs on the three runners, macOS draws and takes keys,
Xvfb with lavapipe works. The `window` target may use `enigo` or the same system tools the spike
used; both send through XTEST, CGEvent and SendInput.

Run locally, outside CI:
- **clippy**: all seventeen `disallowed-methods` paths of `contracts/bans-and-checks.md` are
  caught as written (a crate calling each, clippy 1.99), none reported as an invalid path.
- **The scale factor**: GPUI's test platform honours `simulate_scale_factor_change` at 1, 1.5 and
  2 and lays out in logical pixels, so HS-01's scale cases stay in the headless layer (T017, T024).

## R16. The documents this feature updates

**Decision**: besides 0001 (R2), the threat model's decoder row keeps its first cell (the criteria
test quotes it) and has its mitigation say that decoding stays in the host process with no plugin
image, SVG or font decoded until the widget kit brings images; its open question on the process
gets the outcome of R2. `docs/design/host-surfaces.md` moves the decoder entry of "Decided
elsewhere" to its next owners (installation for the runtime, the widget kit for images and icons).
Editing the threat model runs the full CI, as feature 007 set up.

## Review of the plan

An independent review on 2026-11-09 read the spec, this research, the plan, the data model, the
contracts and the quickstart against constitution v5.0.1, `docs/design/host-surfaces.md`, the
threat model and decision 0001. Each finding was checked at the source before anything changed.

| Finding | Held? | Outcome |
| --- | --- | --- |
| The window would run the runtime in development mode, which only the tool may (runtime docs, 003 FR-018) | Yes | Development mode off, no `--connectors`, FR-041, a hostile case (R5) |
| `svg().external_path`, assets and paths are other ways from a string to a read or a decode | Yes | Banned outside `media`, which takes host bytes only; status icons from embedded paths (R12) |
| An `allow(clippy::disallowed_methods)` anywhere would slip the bans | Yes | A check fails on it outside `media.rs` (R12) |
| A focused node in a second window stays exposed under `set_hidden` | Yes | Focus cleared in other windows and restored (R9, R10) |
| A key in another window could answer the dialog | Yes | Other windows drop every key; a test (R9) |
| Window decorations unspecified | Yes | The system's title bar stays; close refuses the dialog (R6) |
| Menus and the appearance screen could float over the chrome; scrolling and moved frames untested | Yes | No floating layers anywhere, a check; the content area scrolls; HS-01 tested scrolled past every edge (R6) |
| HS-24 untested beyond the dialog; clipping could cut the ring | Yes | Every focusable host control walked in the window target; 4 px inside clipping edges (R6, R13) |
| Label and monogram rules differed from HS-06 and HS-07 | Yes | Copied exactly (R8) |
| Hostile cases missed FR-015 and FR-028; the tree case cannot run headless | Yes | Added; real-window cases in the `window` target, counted in the suite |
| Real-window layers missed the panel, the pointer-opened dialog and FR-024 | Yes | Added (R13) |
| The window target would run inside `cargo test --workspace`; CI time | Yes | `test = false`; a "Host window" job of its own (R13) |
| Threat model and "Decided elsewhere" not updated with R2 | Yes | R16 |
| The 0001 revisit dropped two triggers, the compile-out option and a stale line | Yes | R2 rewritten |
| A failure's detail could show a plugin's text as host text | Yes | Host words per error kind only; a hostile case (R5) |
| `clippy.toml` not named for review; panel click-through; focus after the menu closes; "no byte"; UI-thread reads; window title and tooltips | Yes | Each corrected in the plan, spec and contracts |

## Review of the implementation

An independent review on 2026-11-09 (T052) read `host/src/dialog/`, `a11y.rs`, `media.rs`,
`app.rs`, `plugins.rs`, `appearance.rs`, `dashboard.rs`, the `approvals()` and `APP_DIR` hunks of
`runtime/src/grants.rs`, `clippy.toml`, `deny.toml`, `host/ban-fixture/` and the "Host window" job
against constitution v5.0.1, the criteria, the threat model and decision 0001. Each finding was
checked at the source (GPUI 1.25.3 included) before anything changed.

| Finding | Held? | Outcome |
| --- | --- | --- |
| A panel left open behind a dialog: GPUI's `on_mouse_down_out` runs in the capture phase whatever the scrim occludes, so a click on the scrim closed the panel and moved focus to the menu button behind it, out of the trap | Yes | A dialog closes every window's menus and panels; closing them moves no focus while a dialog is open; the dialog's window puts focus back on Cancel whenever it is anywhere else; a test (`a_panel_behind_the_dialog_cannot_take_focus`), which fails without the fix |
| The bans missed `Window::get_asset`, `ImageCacheItem::new`, the image caches' `load` and `Platform`'s file, prompt and credential methods (reachable through `gpui_platform::current_platform()`) | Yes | Twelve more paths in `clippy.toml` (29 in all), each called in `host/ban-fixture`, which CI now requires to fail 29 times; `image_cache()` and `retain_all()` only hand a cache to `img` children, which stay banned, so they are not |
| "Remove" ignored a failed `GrantStore::remove` and discarded the kept data anyway, unlike the tool's `remove` | Yes | The approval first, the kept data only once it is gone, a host notice when either fails; a test |
| A slower, older reload could replace a newer list and write statuses into the wrong frames | Yes | The epoch is checked before the list is replaced too |
| Every focus regain started another reload, the old ones compiling on | Yes | The running reload is kept as a task; a new reload drops it, which stops its loads |
| A record that cannot be read (locked or damaged) showed the empty dashboard and its advice | Yes | The frames stay and a host notice says the approvals could not be read; a test |
| Closing a window refused the open dialog, then showed the next one, possibly in the closing window | Yes | The dialogs waiting for that window are refused first; quitting refuses every one; a test |
| A dialog answered before its deferred focus save ran left every window blurred | Yes | The save does nothing once its dialog is gone |
| The appearance file's partial file had a fixed name, followed links and was not flushed before the rename | Yes | A new file named for the process and moment, created only if absent, flushed, then renamed; removed on failure; a test that nothing is left |
| (Possible) GPUI turns Enter's and Space's release into a click, which reached `click()` without the gate's held-key rule, and pressed host controls twice | Yes | Keyboard clicks are ignored: keys press controls and answer dialogs only through their key handlers; a test that Enter presses once, which fails without the fix |
| (Possible) The dialog names a plugin by its id's last segment, so two owners' `notes` read alike | Partly | Kept as R8 decided: only the development tool approves plugins in this feature; the frame's description carries the full id. Installation, which brings plugins from different owners, decides how the dialog names the owner |
