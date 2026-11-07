# Spike report: GPUI as the host UI instead of Tauri 2 + React

Branch `spike/gpui`, 2026-11-07. Code and how to run it: [README.md](README.md). Raw evidence:
[`evidence/`](evidence/) and the **Spike GPUI** workflow runs (`.github/workflows/spike-gpui.yml`;
run 42383088681 is in `evidence/ci-run-42383088681.txt`).

## 1. Recommendation

**Stay with Tauri 2 + React for Phase 3. Do not migrate to GPUI now.**

GPUI did better than expected. All five items to build work. Plugin content is drawn only by
host components. The accessibility tree that Orca reads is right after three fixes. It builds,
passes its UI tests and opens on Windows, macOS and Linux. Its memory is a fraction of a
webview's on Windows (66 MiB against 319 MiB).

But the rule was "only if every criterion is true". Three fail and one could not be verified
(§2):

1. **No dependency form is both stable and usable.**
   - gpui-kit is the only base that offers gpui-base, which owning the widget components needs.
     It runs on `gpui-pre`: weekly snapshots of Zed's development branch `main`, not of Zed's
     stable releases.
   - The exact pin keeps the build compiling indefinitely, but the pinned code is pre-release.
   - At least 4 of the last 6 weekly snapshots changed GPUI's API, so a monthly update means a
     monthly migration.
   - The form that follows Zed's stable tags (gpui-unofficial) cannot be combined with
     gpui-base.
   - None of the three distributions is affiliated with Zed Industries.
2. **On Linux without a modern GPU, GPUI falls back to the CPU and drags at 14–30 fps.**
   - GPUI's native OpenGL path needs compute shaders (OpenGL 4.3 / GLES 3.1). On this machine
     (VirtualBox, 3D acceleration on, OpenGL 4.1) it refused the accelerated GL adapter and ran
     on lavapipe, Mesa's CPU Vulkan.
   - It opens. With no Vulkan driver at all, it opens on X11 through llvmpipe's OpenGL (also on
     the CPU) and shows a clear message on Wayland, so the "no Vulkan" criterion passes.
   - Dragging a widget is not fluid: 14–30 fps (about 25 on a quiet machine) at 130–360 % CPU.
     WebKitGTK 4.1, the engine Tauri uses, draws the same dashboard on the same machine at
     48–57 fps and 35–47 % CPU, because it *can* use that OpenGL.
   - On this class of machine, GPUI makes the Linux problem it was meant to solve worse.
     Intel, AMD and NVIDIA GPUs were not available to test.
3. **The extra effort is over the 3-week cap**: about 15–30 person-days for Phase 3 + Phase 6
   (§10). Most of it is rebuilding Tauri's features by hand; deep links, for one, do not exist
   in GPUI on Windows or Linux.

What would change this:
- Zed publishing `gpui` releases that follow its stable tags, or gpui-kit offering gpui-base on
  such releases.
- Phase 3 testing on real Linux hardware showing WebKitGTK problems that a webview cannot work
  around.

Until then, keep the widget content format renderer-independent. It already is: plugins return
JSON and the host draws it. The host renderer can then be replaced later without touching
plugins or the contract.

**If you decide to migrate anyway:**
- Use gpui-kit 0.9.0 on its default `gpui-pre` base, with every crate pinned with `=`.
- Use `gpui-component` only for the host's chrome, and `gpui-base` for the widget kit (§5).
- Do not use `gpui-fast`. It is a fork of the snapshots that adds about 55k lines. It gave
  nothing measurable here, and its binaries are larger and its builds slower on all three
  systems.

## 2. Decision criteria

| Criterion | Result | Evidence |
| --- | --- | --- |
| Items 1 to 5 work on the 3 systems | **Linux: pass. Windows, macOS: pass in CI; by-hand check pending** | In CI on Windows and macOS: builds, clippy, the headless UI tests, a start with 4 widgets that presents frames. A visual check by hand is pending: the `gpui-spike-Windows-gpui-pre` artifact. |
| Dialog and grid usable with the keyboard only, accessibility tree correct | **Keyboard: pass on 3 systems. Tree: pass on Linux after fixes, one gap. macOS and Windows trees: not verified** | `host/tests/ui.rs` (5 tests) passes on all three in CI; `evidence/atspi-linux-*.txt`. Gap: GPUI cannot mark a dialog modal (§7). |
| Linux: opens on every GPU tested; acceptable without Vulkan | **Opens: yes, but on the CPU, at 14–30 fps when dragging. Intel, AMD, NVIDIA: not tested. Without Vulkan: pass (opens on llvmpipe's OpenGL on X11; a clear message on Wayland)** | §8; `scripts/linux-gpu-check.sh` runs the matrix on any machine in one command |
| A way to pin GPUI that does not need weekly updates to keep compiling | **Pass** | `=0.9.0` / `=0.3.11` exact pins plus `Cargo.lock`; crates.io versions are immutable. |
| Active maintainer; history allows updating at most monthly without breaking the build | **Fail** | §4. The maintainer is active (weekly releases), but `gpui-pre` is Zed `main`. 4 of the last 6 snapshots changed GPUI's API, and gpui-kit breaks its own API in minor versions. One snapshot broke every gpui-kit user (#3156), and one release was yanked (0.6.5). |
| Tauri replacements have a clear path, no known blocker | **Pass, with work** | §9 |
| Extra effort for Phase 3 + Phase 6 ≤ 3 weeks | **Fail** | §10: about 15–30 person-days |

## 3. What was built

Everything is in `spikes/gpui/`, in Cargo workspaces of its own. `app/`, `runtime/`, `wit/` and
`specs/` are unchanged, and `runtime` is a path dependency.

| Item | Status | How |
| --- | --- | --- |
| 1. Grid: add, drag to move, resize, save | Done | A 12-column grid. Drag a frame's header to move it or its corner to resize it: the frame follows the pointer, the target cell is outlined, and a taken cell is refused. Arrow keys and Shift-arrows do the same. `layout.json` is saved on every change. The `layout.rs` tests check that widgets never overlap, stay on the grid, and save and read back. |
| 2. A real widget through `runtime` | Done | `hello-widget` (from `plugins/examples/`, contract 1.0.0) and the spike's `ui-sampler` (contract 1.3.0) are embedded, approved, loaded and rendered by `runtime`, off the UI thread. `widget-kit/` draws text, list, table and button on **gpui-base**. It depends on GPUI and gpui-base only, so gpui-component is not even a dependency of the code that draws widgets. Its format is closed: unknown block types, fields and tones are refused (the `model.rs` tests cover a password field, host tones, `color`/`position`/`html` fields and the limits), and bidirectional and zero-width characters are removed. |
| 3. Host frame with the plugin's name and icon | Done | The header is drawn by the host from the catalog and the manifest (name and id). The icon is a monogram, because manifests have no icon yet. The body clips and scrolls the content: a UI test checks that the part of an 80-item list outside the frame is not drawn. |
| 4. Host dialog with the visual secret and host-only colors | Done | The approval dialog shows the person's color (chosen in Settings on first start) as a band at the top, with its name. Its host-only colors (a plum family) are out of the widget kit's reach in two ways. By construction, widget-kit never receives the host palette. By test, every widget color is at least 60 RGB units from every host-reserved color and secret choice, in both themes. Approving calls `CheckedPackage::approve` from the dialog. A widget's own "Approve" button only reaches the host as an action name, which the host ignores. |
| 5. Light and dark themes from tokens | Done | `design/tokens.json` holds typography, spacing, the widget and host palettes, and the secret choices. The theme follows the system appearance or Settings. A test checks WCAG AA contrast for every text pair. |

Screens on Linux: [dashboard](evidence/linux-x11-dashboard.png), [approval dialog](evidence/linux-x11-approval-dialog.png).

**Tests**: 7 in `widget-kit`, 6 host unit tests and 5 in `host/tests/ui.rs`. They pass on
Windows, macOS and Linux. The UI tests open real windows on GPUI's headless platform and drive
them with the keyboard:
- Tab reaches every frame and widget button.
- The arrow keys move and resize a widget, and a blocked move is announced in a status.
- The dialog keeps focus on Tab and Shift-Tab, Escape cancels it and Enter approves.
- Widget blocks have the expected roles and names.
- Clipped content is not drawn.
- The password field and the host color are refused.

Three problems the spike found and fixed, which a migration would meet again:

- **Tab did nothing until something was clicked.** With nothing focused, GPUI dispatches no key
  binding of a child context, so the window must focus its root at start.
- **The dialog lost its role.** gpui-base's `focus_trap` replaces the id and the role of the
  element it wraps, so the dialog lost `Role::Dialog` until the trap was moved inside it.
- **Orca read nothing in body text, table cells or dialog texts.** AT-SPI reads a label's
  *value*, not its label. These texts stayed silent until they used GPUI's `Text` with an id, or
  `aria_value`.

## 4. Ways to depend on GPUI

| | gpui-kit on `gpui-pre` (default) | gpui-kit + `gpui-fast` | git dependency on Zed | gpui-unofficial | `gpui` 0.2.2 (official crate) |
| --- | --- | --- | --- | --- | --- |
| Maintainer | Longbridge (`huacnlee`) | Longbridge | Zed Industries | `gpui-archipelago` (`iamnbutler`) | Zed Industries |
| Affiliated with Zed | No | No | Yes | No (its README says so) | Yes |
| Source | Zed **`main`**, snapshot every Monday | A fork of the snapshots, synced weekly from `main` | Any revision; stable tags weekly (v1.27.0, `c84af5b`) | Every Zed **stable and preview tag**, 0–2 days later | Zed as of Oct 2025 |
| Cadence | gpui-pre 0.3.0–0.3.11 since 09-03; gpui-kit about weekly (0.6.0 → 0.9.0 in 2 months) | 14 releases since 10-06 | weekly | 67 versions since 04-03 | none for 12.5 months |
| Yanked | gpui-kit, gpui-component and gpui-base 0.6.5 (the release notes also say 0.7.2; crates.io disagrees) | none | n/a | none, but repeated "Release failed" issues, up to 2 days late | old test versions |
| How a Zed fix reaches you | up to 7 days to the snapshot, then gpui-kit's next exact pin: about 1–2 weeks | about 1 week | immediately (`main`) or 1–2 weeks (stable) | 0–2 days after the tag; a released tag is never republished | never |
| AccessKit | Yes: macOS, Windows, Linux (AT-SPI, X11 and Wayland) | Yes | Yes (zed#56065, 2026-05-27) | Yes | **No** |
| Windows | Yes (Direct3D 11) | Yes | Yes | Yes | Yes |
| Linux renderer | wgpu 30.0.1, Vulkan then GL | same | wgpu from git (has a Vulkan surface-drop fix the crates.io forms lack) | wgpu 30.0.1 | blade, Vulkan only |
| Divergence from Zed | Mechanical republish: renamed crates, about 5 files rewritten (wgpu 30.0.1 shim) | **A real fork: +55.5k lines**, about 3.1k changed upstream lines | None, but Zed's `[patch.crates-io]` (a khronos-egl fix, among others) does not reach dependents | Generated, with 6 source patches kept in a ledger; some crates published with `--no-verify` | n/a |
| gpui-base / gpui-component | Yes | Yes | No (only Zed's own UI crates, which are GPL) | No | No |
| License | Apache-2.0 (its release script checks the closure for GPL) | Apache-2.0 | gpui crates Apache-2.0; app crates GPL | Apache-2.0 | Apache-2.0 |

**Sources:** the crates.io API and `gh api` on 2026-11-07, and crate sources read from the
registry. Commits read: zed-industries/zed main `b5cd310` and v1.27.0 `c84af5b`;
longbridge/gpui-kit `46bf6d4`; longbridge/gpui-fast `61dccec`;
gpui-archipelago/gpui-unofficial `513f9bc`.

**API changes the snapshots carried**, per gpui-kit's release notes:
- 0.3.6: `register_inspector_element`. This broke gpui-kit 0.6.4 (issue #3156), which is why
  gpui-kit now pins exactly.
- 0.3.8: continuation indent.
- 0.3.10: `Modifiers::secondary(&App)`, static strings in `KeyBinding::new`, `ScrollWheel`
  renamed to `Scroll`.
- 0.3.11: `text_system()` returns `Rc`.

**Pinned in the spike**, every one with `=`:
- `gpui-kit =0.9.0`, which pins `gpui-pre =0.3.11` (`zed@f816e1f`, a `main` commit of
  2026-11-01) and the rest of its family.
- `gpui-fast 0.1.13` comes in through gpui-kit with a **caret** requirement and is held only by
  `Cargo.lock`. That is a reason to pin it directly if it is ever used.

## 5. gpui-kit on `gpui-pre` and on `gpui-fast`

Both compile on all three systems with no code change (one Cargo feature), and both pass the
same tests.

| | `gpui-pre` (default) | `gpui-fast` |
| --- | --- | --- |
| Builds, clippy, tests: Linux / Windows / macOS | pass / pass / pass | pass / pass / pass |
| Release binary: Linux / Windows / macOS | 42.5 / 25.6 / 20.4 MB | 44.0 / 27.0 / 21.5 MB |
| Clean release build in CI: Linux / Windows / macOS | 684 / 729 / 737 s | 779 / 780 / 751 s |
| Clean debug build in CI | 204 / 204 / 284 s | 227 / 215 / 224 s |
| First frame: Linux CI (Xvfb, CPU) / Windows CI / macOS CI | 225 / 172 / 2748 ms | 277 / 178 / 2848 ms |
| Memory, 4 widgets: Linux CI (PSS) / Windows CI (working set) / macOS CI (RSS) | 180 / 66 / 87 MiB | 189 / 70 / 87 MiB |
| This machine, X11 / Wayland: first frame (median of 5) | 276 / 517 ms | 332 / 535 ms |
| This machine, X11 / Wayland: memory (PSS) | 187 / 171 MiB | 198 / 181 MiB |
| This machine: drag (fps, CPU), X11 / Wayland | 16–30 / 14–26 fps, 130–360 % | 15–16 / 13–14 fps, 155–234 % (first session only) |

`gpui-fast` is larger, slower to build and no faster here. Even with it, gpui-kit still depends
on `gpui-pre` and its support crates.

## 6. Numbers, GPUI against Tauri

| | GPUI spike (`gpui-pre`) | Tauri app (`app/`) |
| --- | --- | --- |
| Release binary: Linux / Windows / macOS | 42.5 / 25.6 / 20.4 MB, with Wasmtime and the runtime (the runtime CLI alone is 18.0 / 15.8 / 12.1 MB) | 6.2 / 4.5 / 4.2 MB, without them |
| Clean release build in CI: Linux / Windows / macOS | 684 / 729 / 737 s | 236 / 326 / 221 s, plus a 3–6 s frontend build |
| Incremental release build in CI (touch one host file) | 292 / 297 / 263 s | 80 / 130 / 108 s |
| Clean debug build in CI | 204 / 204 / 284 s | 90 / 133 / 74 s |
| Incremental debug build in CI | 3.4 / 6.0 / 5.6 s | 4.2 / 7.8 / 7.0 s |
| Windows CI: first window / first frame | 281 / 172 ms | 94 ms (window) |
| Windows CI: memory, whole process tree | 66 MiB working set, 44 MiB private, 2 processes | **319 MiB** working set, 113 MiB private, 7 processes (WebView2) |
| Windows CI: idle CPU | 0.6 % | 0.3 % |
| This machine (Linux), first frame X11 / Wayland | 276 / 517 ms | WebKitGTK harness*: 788 / 865 ms |
| This machine, memory (PSS, whole tree) | 171–187 MiB | WebKitGTK harness*: 218–279 MiB (3–6 processes) |
| This machine, idle CPU X11 / Wayland | 0.6 / 0.3 % | WebKitGTK harness*: 1.7 / 0.1 % |
| This machine, **drag**, X11 / Wayland | **16–30 / 14–26 fps**, p95 39–100 ms, 130–360 % CPU | **48 / 55–57 fps**, p95 21–29 ms, 35–47 % CPU |

\* The Tauri app could not be built on this machine (no `libwebkit2gtk-4.1-dev`). The "WebKitGTK
harness" (`scripts/webkitgtk-bench.py` with `webkitgtk-bench.html`) loads the spike's dashboard as
a web page into WebKitGTK 4.1, the engine Tauri 2 uses on Linux, and runs the same drag. Its
numbers include Python, adding about 15 MiB and about 100 ms of startup. It is the engine, not
the Tauri app, and it runs no plugins.

Notes:
- The Tauri app is the current shadcn showcase: eight cards with stats, a form, buttons and
  overlays, and a table. It has no grid, no runtime and no plugins, so it is a lower bound for
  Tauri.
- Linux CI under Xvfb measured the Tauri app at 59 MiB with a single process. WebKit's web
  process was not found in its process tree, so that figure is not comparable and is left out.
- macOS CI measured no Tauri memory. Its web content runs in XPC services outside the app's
  process tree.
- Release builds use fat LTO and one codegen unit (the root profile), which is why an
  incremental release build takes minutes.
- macOS's first frame of about 2.7 s on the CI runner is likely gpui-kit's `runtime_shaders`
  (Metal shaders compiled at start). Not verified.

## 7. Accessibility

**Linux (AT-SPI), measured.** `scripts/atspi-dump.py` reads the tree the way Orca does
(`evidence/atspi-linux-wayland-dashboard.txt` and `…-approval-dialog.txt`). After the fixes in §3:
- The toolbar holds buttons with a `click` action.
- Each frame is a focusable group, named like "Release status, plugin local/ui-sampler. Column
  1, row 1, 5 by 5 cells", with a description of its keys.
- Inside a frame: a heading, text, a list with its items, a table named by its columns with
  headers and cells, and the widget's buttons.
- The dialog is `[dialog] 'Add Hello?'`, with the secret's label, the title, what the plugin
  asks for, and the two buttons.

Gaps:

- **No modal state.** GPUI has no API to mark a node modal (none of the `aria_*` methods sets it).
  Keyboard focus cannot leave the dialog, but a screen reader's virtual cursor may.
- **No live announcements.** A frame's new position is only in its name. A status message has
  `Role::Status`, but whether screen readers announce it is untested.
- **AT-SPI activation.** AccessKit publishes the tree only once the accessibility bus is enabled.
  Orca and Accerciser enable it; that is normal on Linux.

**Keyboard on 3 systems**: `host/tests/ui.rs` passes in CI on Windows, macOS and Linux.

**Not verified: macOS (Accessibility Inspector, VoiceOver) and Windows (Accessibility Insights,
Narrator).** [A11Y-MANUAL-TEST.md](A11Y-MANUAL-TEST.md) is a 16-step script for VoiceOver,
Narrator and Orca, to run by hand.

gpui-kit's claim "AccessKit built in and tested" holds in one sense: AccessKit is wired into GPUI
on all three systems, and gpui-base's controls set roles. But what ends up in the tree still
depends on how each element is written. The three bugs in §3 were in code that looked right.

## 8. Linux in detail

Machine: VirtualBox 7.2.22, VMSVGA with 3D acceleration (the host GPU behind it), 8 vCPUs,
15 GiB, Ubuntu 26.04 with GNOME on Wayland, Mesa 26.0.8. OpenGL through EGL is accelerated
(SVGA3D, OpenGL 4.1 core). OpenGL through GLX is llvmpipe. The only Vulkan is lavapipe.

| Setup | GPUI opens | Backend GPUI chose | GPUI | WebKitGTK 4.1 on the same setup |
| --- | --- | --- | --- | --- |
| Wayland (GNOME) | Yes | Vulkan, **lavapipe (CPU)** | First frame 517 ms, PSS 171 MiB, idle 0.3 %, **drag 14–26 fps** at 180–360 % CPU | GL on the GPU (its web process holds `/dev/dri/renderD128`), first frame 865 ms, **drag 57 fps** at 35 % CPU |
| X11 (Xwayland) | Yes | lavapipe | First frame 276 ms, PSS 187 MiB, idle 0.6 %, **drag 16–30 fps** at 130–225 % CPU | **Drag 48 fps** at 47 % CPU |
| X11, no Vulkan driver (`VK_DRIVER_FILES` pointing nowhere) | Yes | OpenGL, **llvmpipe (CPU)**: GLX/EGL on X11 here is llvmpipe's GL 4.5, which has compute | First frame 247 ms, PSS 191 MiB, drag 26 fps at 238 % CPU | — |
| Wayland, no Vulkan driver | **No, with a clear message** | — | GL is refused (see below), then "Dashboard cannot start: this computer offers no graphics driver it can draw with … installing the Mesa Vulkan drivers (mesa-vulkan-drivers) provides a software renderer". The message goes to the terminal, and to a zenity or kdialog dialog when one is installed; exit code 1. `MESA_GL_VERSION_OVERRIDE=4.5` and `MESA_GLES_VERSION_OVERRIDE=3.2` did not help. | not tested |
| CI runner, Xvfb, no GPU | Yes | lavapipe (LLVM 20) | First frame 225 ms, PSS 180 MiB, idle 0.4 % | see the note in §6 |
| Intel, AMD, NVIDIA | **Not tested** | — | No such hardware here | — |

**Why GPUI refused the accelerated GL adapter.** The log says `Limit
'max_compute_workgroups_per_dimension' value 65535 is better than allowed 0`. GPUI's native wgpu
device always requests `Limits::downlevel_defaults()`, which needs compute shaders and storage
buffers. Only its WebAssembly build uses WebGL2 limits (`gpui-pre-wgpu-0.3.11/src/wgpu_context.rs`,
`create_device`). So the GL backend only helps drivers that have OpenGL 4.3+/GLES 3.1+ and no
Vulkan, which is rare. Old or virtual GPUs fall to lavapipe. On Debian and Ubuntu desktops
`mesa-vulkan-drivers` is installed, so lavapipe is almost always there: GPUI almost always opens
on Linux, but sometimes on the CPU.

**Adapter order.** First a `ZED_DEVICE_ID` override, then the GPU the compositor runs on, then
discrete, integrated, other, virtual and CPU devices in that order, with Vulkan ahead of GL. Each
candidate is tried by configuring the surface. Software adapters are skipped only when recovering
from a lost device.

**On other machines.** `scripts/linux-gpu-check.sh <binary>` prints the GPU, the adapter GPUI
picks on Wayland, X11 and X11 without Vulkan, startup, memory, idle CPU and both drag benchmarks,
in one command (`evidence/linux-vbox-gpu-check.txt` is its output here). Running it on an Intel,
an AMD and an NVIDIA machine closes the "every GPU tested" criterion.

**Drag benchmark.** `SPIKE_DRAG_BENCH` drags the first widget in a circle for 5 s through the
mouse code path, one step per presented frame. The WebKitGTK harness does the same with
`requestAnimationFrame` and a CSS transform. Raw data: `evidence/linux-vbox-drag-bench.txt`,
`…-gpui-measure.txt` and `…-webkitgtk-measure.txt`.

## 9. Replacing what Tauri gives

| Feature | Candidate | Maturity | Blocker | Effort (person-days) | vs Tauri |
| --- | --- | --- | --- | --- | --- |
| Installers: MSI/NSIS, .app + DMG (signed, notarized), AppImage/deb/rpm | cargo-packager 0.11.8 (from CrabNebula, who wrote Tauri's bundler) + cargo-generate-rpm 0.21.1 | High, but no cargo-packager release since 2025-11-27; recent fixes only in git | None (cargo-packager has no rpm) | 6–10 | +2–4 |
| Signed updater | cargo-packager-updater 0.2.3 (minisign, Tauri's scheme) | High | None; no rollback on Windows (left to the installer) | 3–5 | +1–3 |
| Deep links (one-click install) | GPUI's `on_open_urls` works on **macOS only**. `register_url_scheme` returns "unimplemented" on Windows and Linux (`platform.rs` in `gpui-pre-windows` and `gpui-pre-linux` 0.3.11). The scheme must be registered by the installer, and the URL forwarded from argv to the running instance. | macOS: high; Windows and Linux: to be written | None, but it is our own code | 3–5 | +2–4 |
| Tray | tray-icon 0.26.1 with only the `ksni` feature, so no GTK loop runs beside GPUI's | High; the ksni backend is new (2026-09) | GNOME without the AppIndicator extension shows no tray | 2–4 | +1–3 |
| Notifications | GPUI's own `show_system_notification`, implemented for all three systems in gpui-pre 0.3.11 | Medium (the API is from 2026-07) | macOS needs a signed .app with a bundle id | 1–2 | 0–1 |
| Single instance | interprocess 2.4.7 | High | None | 2–3 | +2–3 |

The total is about 17–29 days, 9–18 more than with Tauri.
- GPUI runs its own event loops (AppKit, Win32, and calloop over Wayland/X11), with no winit or
  GTK. The only loop conflict is GTK-based tray code on Linux.
- Tauri's plugins depend on `tauri` and cannot be used; their code can be ported (MIT/Apache-2.0).
- Zed's own updater and single-instance code is GPL-3.0 and cannot be copied.

**Safe Markdown (Phase 4)** with gpui-component's text view (`gpui-base-0.9.0/src/text/`) is
possible, but not by default. Three hooks exist:
- `image_source(resolver)` replaces the source of every image, data URLs included, with no
  fallback to the document's URL.
- `on_link_click(handler)` receives the real URL.
- Markdown plugins are consulted **before** the built-in handling of every inline and block node
  (`format/markdown.rs:763` and `:1016`). The host can therefore take over link, image and HTML
  nodes and render, say, "text (example.com)".

The defaults are unsafe for AI output: without a handler a click opens the URL (`cx.open_url`),
images load remote URLs, and inline and block HTML is parsed and rendered. A host wrapper of
about 2–3 days would be needed. This was verified by reading the source, not by running it.

## 10. Effort if the migration is approved

| Work | Extra over the Tauri plan (person-days) |
| --- | --- |
| Phase 6: installers, updater, deep links, tray, notifications, single instance (§9) | 9–18 |
| Phase 3: host chrome and widget kit in GPUI (the spike is a start, not the product), and learning GPUI's model | 3–5 |
| Accessibility: the modal gap, announcements, macOS and Windows verification and fixes | 3–5 |
| Linux: GPU matrix on real hardware, the no-GPU path, and what to do about CPU rendering | 1–2 |
| Safe Markdown wrapper (Phase 4, counted for completeness) | 1–2 |
| Constitution amendment (the fixed decisions "Rust + Tauri 2" and "TypeScript + React") and specs | 1 |
| Saved: no webview hardening (CSP, IPC capabilities, isolation) | −3 to −5 |
| **Total** | **about 15–30** |

Ongoing costs, not counted in the cap:
- About one day a month to absorb GPUI's API changes when moving gpui-kit.
- Builds 2–3 times slower than Tauri's (§6).
- A dependency tree that changes weekly: 775 crates on Linux, 591 of them for the widget kit
  alone.

## 11. Learning from embedded_gpui

[zed-industries/embedded_gpui](https://github.com/zed-industries/embedded_gpui) (main `28bc6d3`,
branch `surfaces-as-roots` `ceb0df8`) is an official Zed repository with one author and no
release. It depends on a Zed branch that adds `Application::run_embedded`.

- **The opposite split.** The guest runs real GPUI inside `wasm32-wasip2`. It sends each frame
  as a display list of primitives (quads, shadows, glyph references, paths, images), which the
  host replays with `Window::paint_*`. Here, plugins return a declarative tree and the host draws
  everything with its own components.
- **Conflicts with the constitution.**
  - Principle V (plugin UI only inside the host frame, no imitating the host): a plugin that
    paints arbitrary pixels can draw any host chrome inside its slot, a password field included,
    and only clipping stands in the way. On `surfaces-as-roots`, overlays are painted
    **unclipped above the whole host window**, by design.
  - Principle II: embedded_gpui links `wasmtime-wasi` and grants stdout and stderr by default,
    more on request. This project links only the imports granted, on `wasm32-unknown-unknown`.
  - Principles II and VII: its object-capability layer passes authority by reachable references,
    decided by the plugin's code at run time. Here every capability is a WIT import declared in
    the manifest and approved in a host dialog.
  - Its optional QuickJS layer is a second extension system beside Wasm. It is acceptable only as
    the Phase 5 amendment foresees, with the engine itself running as Wasm in Wasmtime.
- **Worth taking.**
  - The UI thread never calls Wasm. Calls run one at a time on a background worker, and results
    apply on the next frame. The spike does the same for `render`; keep it.
  - Every primitive's content mask is intersected with the slot. The host frame's
    `overflow_hidden` plays that role here.
  - Its per-turn time budgets and memory cap match `runtime`'s fuel, epoch and memory limits.
  - Its `Attenuated` and `Revocable` wrappers are a clean model for the Phase 3 item "disable a
    plugin", and for revoking an optional connector at run time.
  - Its JS layer's UI is a small declarative vocabulary (`div`, `text`, flat style flags). That
    supports keeping our content format declarative and small.

## 12. Risks found

- **Pre-release code by default** (§4). `gpui-pre` is Zed `main`, which never went through Zed's
  preview → stable cycle.
- **One third-party maintainer.** gpui-kit, gpui-pre and gpui-fast are Longbridge's, released by
  one account. The project is large (19.7k stars) and Longbridge uses it in its own product, but
  it is not affiliated with Zed.
- **API churn.** Four API changes came with six weeks of snapshots.
- **Linux software rendering.** On machines without Vulkan, or with a GPU older than OpenGL 4.3,
  GPUI renders on the CPU and dragging is not fluid (§8). The drag rate also varied a lot between
  sessions (14 to 30 fps), with the machine's load.
- **Accessibility gaps** (§7) need upstream work in GPUI (modal) and testing on macOS and Windows.
- **Licenses.** Under the repository's `deny.toml`, 5 crates in the spike's tree have licenses
  outside the allow list, all permissive:
  - `arrayref` (BSD-2-Clause)
  - `enum-iterator` and `enum-iterator-derive` (0BSD)
  - `notify` and `tiny-keccak` (CC0-1.0)

  The banned `rustls-native-certs` appears only for the `wasm32` target (gpui-kit's web HTTP
  client), not on desktops.
- **Advisories.** `cargo audit` on the spike's lock finds no vulnerabilities and 8 unmaintained
  crates: `instant`, `paste`, `rustls-pemfile`, `rustybuzz`, `ttf-parser`, `zbus_names`,
  `zvariant` and `zvariant_derive`.
- **Unrelated, found on the way.** The root `Cargo.lock` on `master` has wasmtime 50.0.0, which
  has three advisories published 2026-10-29 (RUSTSEC-2026-0422, -0423 and -0424). 50.0.1 fixes
  them, and the spike resolved to it.

## 13. Dependencies used and their origin

| Crate (pinned) | Origin | How often it changes |
| --- | --- | --- |
| `gpui-kit =0.9.0` (+ `gpui-base`, `gpui-component`, `gpui-kit-assets` 0.9.0) | Longbridge, crates.io | about weekly |
| `gpui-pre* =0.3.11` (through gpui-kit) | Longbridge's snapshots of Zed `main` (`zed@f816e1f`) | weekly |
| `gpui-fast* 0.1.13` (optional, through gpui-kit) | Longbridge's fork | weekly or more |
| `runtime` (path) | this repository, unchanged | — |
| `serde =1.0.228`, `serde_json =1.0.154`, `anyhow =1.0.104`, `dirs =7.0.0`, `log =0.4.35`, `env_logger =0.11.11`, `tempfile =3.27.0` (tests) | crates.io, long established | rarely |
| `wit-bindgen =0.63.0` (the plugin), `wit-component =0.262.0` and `wasm-encoder =0.262.0` (pack) | Bytecode Alliance; the same versions as `plugins/` and `xtask/` | monthly |
| wgpu 30.0.1, AccessKit 0.24 / accesskit_unix 0.22, taffy, cosmic-text, zbus and others | through gpui-pre | — |

## 14. Measurement method

- **Build times**: `scripts/time-build.sh`. A clean build in an empty target directory, then an
  incremental one after touching `host/src/dashboard.rs` (`app/src-tauri/src/lib.rs` for Tauri).
  CI builds run without a cache.
- **Startup, memory, CPU**: `scripts/measure-linux.sh` and `scripts/measure-windows.ps1`.
  - Startup: the time until the window is mapped (X11 and Windows), or until the app logs its
    first presented frame (Wayland).
  - After 5 s, CPU is measured over 10 s, and memory is read. Both are summed over the whole
    process tree, since WebKitGTK and WebView2 run several processes.
  - "First frame" is the spike's own log line: from process start to the `on_next_frame` after
    the first render.
- **Four widgets**: `--dev-approve` lays out sampler, hello, overflow and spoof.
- **Drag**: `SPIKE_DRAG_BENCH=1`, and `scripts/webkitgtk-bench.py --drag` for WebKitGTK.
