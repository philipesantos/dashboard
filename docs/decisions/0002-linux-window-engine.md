# 0002. Linux draws the window with WebKitGTK until the embedded Chromium passes its trial

- **Status**: Accepted, as the temporary fallback of the constitution's "Window engine" decision
  (v2.6.1). The maintainer decided it on 2026-11-10.
- **Date**: 2026-11-10
- **Feature**: [008, the desktop shell](../../specs/008-tauri-shell/spec.md) (FR-013, FR-014,
  FR-022)

## Context

The constitution fixes the window's engine per system: WebView2 on Windows, WKWebView on macOS,
and, on Linux, an embedded Chromium that the app ships, Tauri's CEF runtime, once it passes a
trial. The maintainer chose Chromium for Linux on 2026-11-10 because the system WebKitGTK is not
reliable enough for the host's window. Until the embedded engine passes, the constitution lets
Linux use the system WebKitGTK with every protection that has a breaking test in CI.

The trial asks whether the embedded engine meets the feature's requirements on Linux (FR-001 to
FR-012 and FR-016) and builds and passes the security tests in CI.

## The trial

Evidence gathered on 2026-11-10 ([research R1](../../specs/008-tauri-shell/research.md)):

- **Only for Tauri 3, in alpha.** `tauri-runtime-cef` exists only as `3.0.0-alpha.0` to
  `3.0.0-alpha.7` (the last from 2026-11-06), and it runs only under `tauri` 3.0.0-alpha. The
  host is on Tauri 2 (2.12.5), which has no embedded-engine runtime.
- **Open Linux defects** in `tauri-apps/tauri`, each labelled for the embedded engine:
  - keyboard input lost at start-up or after switching back to the app: #16251 (2026-10-11),
    #16351 (2026-10-21);
  - the idle main thread spinning at 100% CPU: #16189 (2026-10-01);
  - AppImage bundling failing: #16204 (2026-10-04);
  - browser shortcuts that never reach the page and cannot be intercepted: #16249 (2026-10-10);
  - a white frame before the first paint: #16205;
  - X11 sizing after fullscreen: #16480;
  - no `WM_CLASS`: #16474.
- **The runtime's own example, run on the maintainer's desktop** (Ubuntu, GNOME on Wayland;
  the `tauri` branch `v3` at `6620118`, runtime `3.0.0-alpha.7`, CEF 154):
  - the CLI built from source in 4 min 19 s, and the first run downloaded 1.8 GB of CEF; the
    debug binary is 295 MB, and the app's processes held 833 MB;
  - **typing reached no text field.** The runtime forces X11, so on Wayland it runs through
    XWayland, which matches #16251 and #16351;
  - **the main process held one core at about 100%** while the page and GPU processes stayed
    near 0%, which matches #16189.

The maintainer stopped the trial there: either defect alone blocks a dashboard that stays open
all day. Porting the app itself to Tauri 3 alpha was not needed to decide.

| Requirement | Embedded engine on Linux |
| --- | --- |
| FR-001 to FR-012, FR-016 | Not reached: basic use failed first (keyboard input, idle CPU) |
| Builds and passes the security tests in CI | Not reached: needs the move to Tauri 3, still alpha |
| Installer size, start-up time | Not measured: the example's debug binary is 295 MB, before CEF's resources |

## Decision

Linux draws the window with the system WebKitGTK, with every protection of feature 008 and its
breaking tests in CI. The distribution patches WebKitGTK, so the duties that come with shipping
an engine (its version in each release's notes, the weekly Chromium check, the 14-day rule for
critical and high fixes) do not apply until the app embeds one.

## Consequences

- Linux gets the same protections as Windows and macOS. The shell probe runs on all three, and
  the WebKitGTK settings it reads are listed in the register below.
- Linux users get the engine their distribution ships and patches, with its rendering
  differences.
- **The way back out.** Every change made only because Linux uses WebKitGTK is marked
  `WEBKITGTK-ONLY(<id>)` where it is made and listed in
  [`docs/webkitgtk-register.toml`](../webkitgtk-register.toml), with what the move does with it.
  A test keeps the marks and the list in step. Moving to the embedded engine means working
  through that list until it is empty.

## The bar for the next trial

The embedded engine replaces WebKitGTK when, on Linux:

- every window protection of feature 008 holds, with its breaking test passing in CI;
- typing, focus after switching windows, and idle CPU work as on the other systems;
- the app bundles as a `.deb` and an AppImage.

## Revisit when

- A stable (non-pre-release) `tauri-runtime-cef` is published. The weekly `cef-watch` job in
  `audit.yml` reports it. The move also needs the constitution amended to Tauri 3.

Last runtime version tried: 3.0.0-alpha.7

## Options considered

- **The embedded Chromium now**, on Tauri 3 alpha. Rejected: the defects above, and an alpha
  framework under the security core.
- **Porting the app to Tauri 3 alpha on a throwaway branch** for a fuller trial. Not needed: the
  runtime's own example failed on keyboard input and idle CPU first.
- **WebKitGTK through Flatpak's GNOME runtime.** Not a different engine: it only makes
  WebKitGTK current.
