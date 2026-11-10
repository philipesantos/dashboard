# Research: The Desktop Shell, with Minimal Capabilities and a Restrictive CSP

Checked on 2026-11-10 against the locked versions: `tauri` 2.12.5, `tauri-build` 2.7.4,
`tauri-utils` 2.x and `wry` 0.57.0. Source paths refer to `~/.cargo/registry/src/` for those
versions.

## R1. The Linux engine: a desk trial, and WebKitGTK for now

**Decision**: the trial of FR-014 is decided from evidence, without a build, and Linux keeps the
system WebKitGTK with every protection of this feature (the maintainer's choice, 2026-11-10;
constitution v2.6.1). The trial, its evidence and the conditions for the next one are recorded
as decision record `docs/decisions/0002-linux-window-engine.md`. A weekly job reports when a
stable release of the embedded-engine runtime appears, which is when the constitution requires
a new trial.

**Evidence**:

- **Only for Tauri 3, in alpha.** `tauri-runtime-cef` is published only as `3.0.0-alpha.0` to
  `3.0.0-alpha.7`, the latest from 2026-11-06 (crates.io API). It runs only under `tauri`
  3.0.0-alpha (latest `3.0.0-alpha.6`, 2026-11-06). The host is fixed on Tauri 2 (latest
  2.12.5), which has no embedded-engine runtime. Adopting it would mean moving the host to an
  alpha framework, which needs its own amendment.
- **Open Linux defects** in `tauri-apps/tauri`, each labelled for the embedded engine. Several
  block requirements of this feature or basic use:
  - keyboard input lost at start-up or after switching back to the app: #16251 (2026-10-11),
    #16351 (2026-10-21);
  - the idle main thread spinning at 100% CPU: #16189 (2026-10-01);
  - AppImage bundling failing: #16204 (2026-10-04);
  - browser shortcuts that never reach the page and cannot be intercepted, which matters for
    FR-011's shortcuts: #16249 (2026-10-10);
  - a white frame before the first paint: #16205;
  - X11 sizing after fullscreen: #16480;
  - no `WM_CLASS`: #16474.
- **Start-up was blocked** by Chromium 151's first-run screen (#15979, closed). This shows that
  each Chromium bump can break start-up.
- **Other projects** that tried it keep Linux on WebKitGTK while using the embedded engine
  elsewhere, citing the same packaging gap.

**Rationale**: a hands-on trial would mean porting the app to Tauri 3 alpha on a throwaway
branch, and the open defects already decide it. The constitution's fallback exists for this
case. WebKitGTK is patched by the distribution, so FR-017's duties (the version in each release,
the weekly Chromium check and the 14-day rule) do not apply until an embedded engine ships. The
constitution says "while the app embeds the engine".

**Alternatives considered**:
- A hands-on trial on Tauri 3 alpha. Offered, and declined by the maintainer.
- Moving the host to Tauri 3 alpha now. Declined: an alpha framework under the security core,
  with the defects above.

## R2. The window is built in Rust, not in the configuration

**Decision**: `tauri.conf.json` declares no window (`app.windows: []`). `setup` builds the single
window with `WebviewWindowBuilder`, attaching every protection in one place,
`app/src-tauri/src/window.rs`:
- `on_navigation`;
- `on_new_window` returning `NewWindowResponse::Deny`;
- `incognito(true)`;
- `general_autofill_enabled(false)`;
- `devtools(cfg!(debug_assertions))`;
- the per-engine settings applied through `with_webview` (R6, R7).

**Rationale**: the navigation and new-window handlers exist only on the builder
(`tauri-2.12.5/src/webview/webview_window.rs:266, 315`). One file holding all of the window's
protections is what Principle IX's line-by-line review covers.

**Alternatives considered**: a configured window with handlers added afterwards. Rejected:
`on_navigation` cannot be attached to an existing window.

## R3. The content policy

**Decision**: the release policy in `tauri.conf.json` `app.security.csp`, as directives:

| Directive | Value | Why |
| --- | --- | --- |
| `default-src` | `'self'` | Everything else falls back to the bundle |
| `script-src` | `'self'` | Tauri adds hashes for the bundled scripts at build time. No `'unsafe-inline'`, no `'unsafe-eval'` |
| `style-src` | `'self'`, plus `'unsafe-inline'` only if the probe shows a component needs it | See below |
| `font-src` | `'self'` | The `@fontsource` fonts are bundled (feature 007) |
| `img-src` | `'self'` | Icons are inline SVG from `lucide-react`, not images |
| `connect-src` | `ipc: http://ipc.localhost` | The host's channel only; the second form is how Windows spells it |
| `object-src`, `frame-src`, `child-src`, `worker-src` | `'none'` | Nothing embedded, no workers |
| `base-uri` | `'none'` | A `<base>` cannot redirect relative URLs |
| `form-action` | `'none'` | No form submits anywhere; the declarative UI's form goes through a host command |
| `frame-ancestors` | `'none'` | No one frames the window |

- **No `data:` anywhere.** Vite inlines assets under 4 KiB as `data:` URIs, so `vite.config.ts`
  sets `build.assetsInlineLimit: 0`. That keeps `font-src` and `img-src` at `'self'`.
- **Inline styles.**
  - React sets `style={{…}}` through the CSSOM, which the policy does not block.
  - Some Radix dependencies insert `<style>` elements at run time. One example is
    `react-remove-scroll-bar`, through `react-style-singleton`, under the Select and Dialog. Such
    an element needs `'unsafe-inline'` or a nonce, and Tauri's nonce covers only the bundled
    files.
  - The probe (R8) runs the demo's dialog and select under `style-src 'self'`. If they break,
    `'unsafe-inline'` is added to `style-src` alone, with the component named in
    `contracts/content-policy.md`.
  - Style cannot run script. Because `img-src` and `font-src` stay `'self'`, a style rule
    cannot send data to the internet either.
- **Development.** `devCsp` relaxes the policy only where Vite's server needs it:
  - `script-src` gains `'unsafe-inline'` for the React refresh preamble that
    `@vitejs/plugin-react` writes into the page;
  - `connect-src` gains `ws://localhost:1421` for hot reload;
  - `'self'` is then the development server's origin.

  The probe runs only on release builds, so the tests check the release policy (FR-003).
- **`dangerousDisableAssetCspModification`** stays unset: Tauri's hashes and nonces are the
  protection (`tauri-utils` `SecurityConfig`).

**Rationale**: FR-001 to FR-004. These are the narrowest values that still load the demo.

**Alternatives considered**:
- Nonces for runtime styles through `get-nonce`. Rejected: Tauri's nonce is per build, not per
  page load, so reusing it for runtime styles gains nothing over `'unsafe-inline'` for styles.
- Tauri's isolation pattern, which passes IPC through a sandboxed iframe. Rejected for now: the
  window has no host command (R4), so there is nothing for it to vet. It needs `frame-src`, and
  it can be revisited when commands arrive.

## R4. Capabilities and host commands: the reviewed list

**Decision**:

- `build.rs` declares an `AppManifest` with an empty command list
  (`tauri-build-2.7.4/src/acl.rs:91,104`). Every app command then needs an explicit `allow-`
  permission in a capability. Without the manifest, Tauri allows every app command to every
  window.
- `capabilities/default.json` grants no permission (`"permissions": []`). The host UI calls no
  host command once `greet` goes, and it uses no Tauri API beyond `invoke`.
- The opener plugin goes from `Cargo.toml`, `lib.rs`, `app/package.json` and the capability.
- `greet` goes from `lib.rs`, and the demo's "Tauri" tab, which only called it, goes too.

The reviewed list is `app/src-tauri/window-surface.toml`, one entry per permission, app command
and Tauri plugin, each with `reason`. It starts empty, with a comment on how to add an entry.

A test in the app crate, `app/src-tauri/tests/surface.rs`, fails when the list and the app
disagree. It compares the list with:
- the union of the permissions in `capabilities/*.json`;
- the app's command list, a `const` that the same `commands!` macro expands beside
  `generate_handler!`, so the two cannot drift;
- the `tauri-plugin-*` dependencies in `Cargo.toml`.

**Rationale**: FR-005 to FR-008. Tauri's ACL already refuses what no capability grants. The test
makes every widening of the surface a reviewed, named change.

**Alternatives considered**: reading the resolved ACL from the built app. Rejected: Tauri exposes
no public API for the resolved authority, so the test would depend on generated internals.

## R5. Navigation, new windows and dropped files

**Decision**:

- **Navigation.** `on_navigation` allows only the app's own origin:
  - `tauri://localhost` on macOS and Linux, `http://tauri.localhost` on Windows;
  - in debug builds, also `devUrl`.

  Everything else is refused and counted for the probe: `http(s)`, `file:`, `javascript:`,
  `data:`, `blob:` and any other scheme.
- **New windows.** `on_new_window` returns `Deny` for every request.
- **Dropped files and URLs.** Tauri's own drop handler stays on (`dragDropEnabled`, the
  default). It takes the operating system's drop before the engine can act on it, and the host
  UI does not listen to its event, so a drop does nothing. If a drop still reached the engine,
  `on_navigation` would refuse the load.

**Rationale**: FR-009, FR-010 and FR-012. A synthetic drop event in a page never performs the
browser's default navigation, so the probe tests navigation through the handler instead.

## R6. Developer tools and the context menu

**Decision**:

- **Release builds** do not enable the `tauri` crate's `devtools` feature. `devtools` is
  available only in debug builds unless that feature is on (`webview_window.rs:1222`). A test
  reads `cargo metadata` and fails if a release build of the app enables it.
- **The probe reads each engine's setting** through `with_webview`:
  - WebView2: `AreDevToolsEnabled` false;
  - WKWebView: `isInspectable` false;
  - WebKitGTK: `enable-developer-extras` false.
- **The engine's default context menu** is turned off in release builds:
  - WebView2: `AreDefaultContextMenusEnabled` false;
  - WKWebView and WebKitGTK: the host UI's entry module cancels `contextmenu` on `document`.

  The probe checks the WebView2 setting, and that a `contextmenu` event on WebKit engines is
  cancelled.

**Rationale**: FR-011. Pressing a shortcut at the operating-system level is not possible on CI
runners, so the tests check the settings that make the shortcut do nothing.

## R7. Autofill and password saving (HS-26)

**Decision**: every engine runs `incognito(true)`, so it keeps no form data, password or cookie
between runs. The host keeps its own state in host files, never in the engine. On top of that,
per engine:

| Engine | Setting | Probe check |
| --- | --- | --- |
| WebView2 | `general_autofill_enabled(false)`, and `ICoreWebView2Settings4::put_IsPasswordAutosaveEnabled(FALSE)` through `with_webview` (the Tauri option covers general autofill, not passwords) | Both read back as false; the profile is InPrivate |
| WKWebView | The data store is non-persistent (incognito). The app has no `com.apple.developer.associated-domains` / `webcredentials` entitlement, without which WKWebView offers no saved password | `websiteDataStore.isPersistent` is false; the built app's entitlements carry no associated domains |
| WebKitGTK | Ephemeral network session (incognito). WebKitGTK has no password manager or form autofill of its own; browsers built on it add their own | The session reads as ephemeral |

**Residual**: on macOS, no public WKWebView setting turns password AutoFill off for one app. A
developer forum reply says macOS 26 autofills in web views. The probe checks the settings
above. The macOS walk in `quickstart.md` also opens a field labelled "password" and confirms
that no AutoFill appears. That walk is a review step, recorded in the threat model's row.

**Rationale**: FR-016 and criterion HS-26 of feature 007.

## R8. Breaking tests against the built app: the probe

**Decision**: a `shell-probe` Cargo feature of the app crate, and a `probe` Vite mode.

- **What differs from a release build.** The probe build is a release-profile build. Its
  content policy, capability list and window settings are those of a release build. It differs
  in only two ways:
  - Vite's `probe` mode adds one module, `src/probe/run.ts`, to `index.html`. It is bundled, so
    the policy allows it.
  - The feature adds, at run time through `Manager::add_capability`
    (`tauri-2.12.5/src/lib.rs:841`), one capability and one command, `shell_probe_report`.
    Neither is in `capabilities/` or `window-surface.toml`, so the release list stays as
    reviewed.
- **What the probe module tries** inside the real page, recording each outcome:
  - injected script: an `innerHTML` with an `onerror` handler, a `<script>` element with inline
    text, `eval`, `new Function` and `setTimeout` with a string;
  - loads from `.invalid` hosts by every path: `<script>`, `<link>` stylesheet, `<img>`,
    `FontFace`, `fetch`, `XMLHttpRequest`, `WebSocket`, `EventSource`, `<iframe>`, `<object>`
    and an `@import`;
  - a `<base>` element and a form submitted to another origin;
  - navigation: `location.href`, `location.assign`, a meta refresh and a link click;
  - `window.open`, and a link with `target=_blank`;
  - host commands: `invoke('greet')`, `plugin:opener|open_url` and `plugin:event|listen`, each
    expected to fail;
  - a `contextmenu` event, expected to be cancelled;
  - the demo's dialog and select opened and closed, so the policy's effect on styles shows.

  The `.invalid` hosts never resolve. A refusal counts only when the engine's
  `securitypolicyviolation` event fired, or the side effect is absent, never just because the
  network failed.
- **What the Rust side does**:
  - counts `on_navigation` and `on_new_window` refusals;
  - reads the engine settings of R6 and R7 through `with_webview`;
  - waits for the report, at most 60 s;
  - prints one line per case and exits 0 only when every case holds.

  A navigation that succeeded would unload the probe, so a missing report also fails.
- **CI.** The build job runs, on each system:
  - `pnpm --filter ./app build --mode probe`;
  - `cargo build -p dashboard --release --features shell-probe --locked`;
  - the binary, under `xvfb-run` on Linux.

  After that, `pnpm --filter ./app build` runs again, so the normal `dist` is what the rest of
  the job embeds.

**Rationale**: FR-018 and Principle VIII. The official WebDriver server, `tauri-driver`, has no
macOS support (Apple ships no WebDriver for WKWebView), and the macOS options are a paid fork or
an unproven community server. A probe inside the app runs identically on the three systems and
tests the real policy, settings and capability list.

**Alternatives considered**:
- WebDriver on Windows and Linux only. Rejected: it leaves macOS untested.
- A separate probe page. Rejected: the real `index.html` is what ships, so the probe runs inside
  it.

## R9. Freezing the object prototype

**Decision**: set `app.security.freezePrototype: true` if the demo and the probe pass with it.
It runs `Object.freeze(Object.prototype)` before any page script (`tauri-utils` `SecurityConfig`).
If a dependency breaks, the setting stays off, and the plan records which dependency.

**Rationale**: defence in depth against prototype pollution reaching the Tauri API, at no cost if
the bundle tolerates it.

## R10. The threat model

**Decision**: `docs/threat-model.md` gains the following.

- **A new boundary, B7, "Host window ↔ host core".** What crosses it: host commands and engine
  events. Its controls: the reviewed list, the content policy and the window settings.
- **Phase 3 rows on B7**, each citing its probe case:
  - injected script;
  - loads from the internet;
  - navigation away;
  - new windows;
  - a host command that is not listed;
  - developer tools in a release build;
  - engine autofill and password saving.
- **The open question on the content policy** is closed with this feature's answer.
- **The assumption about the webview** says how each engine is patched:
  - WebView2 and WKWebView by their vendors;
  - WebKitGTK by the distribution;
  - an embedded engine by the project's releases, under the constitution's 14-day duty.

  It no longer cites `cargo audit` for the webview.
- **The diagram** reads "Host UI (React in the system webview: WebView2, WKWebView, WebKitGTK)".

The comment at `.github/workflows/ci.yml:27` cites constitution v2.6.1.

**Rationale**: FR-019.

## R11. CI

**Decision**:

- **The build job**, on each system, after the app build:
  - the surface test, which runs within `cargo test --workspace`;
  - the probe of R8;
  - the release feature check of R6.

  On Linux, the apt step adds `xvfb`. `libwebkit2gtk-4.1-dev` stays, since WebKitGTK is the
  engine.
- **`audit.yml`'s weekly run** gains a job that queries crates.io for `tauri-runtime-cef`. The job
  fails, so it shows red, when a version without a pre-release tag exists. That tells the
  maintainer the trial is due again (constitution v2.6.1).
- **Cost.** The probe adds a release build of the app crate per system. That build reuses the
  workspace's cached dependencies, and the probe is expected to run in under a minute.
  `quickstart.md` records the measured times.

## R12. Decision 0001, where plugins run

**Decision**: unchanged. This feature does not bring the plugin runtime into the app (FR-008).
Decision 0001 is revisited with the installation item, its stated trigger.

## R13. The security core

**Decision**: CONTRIBUTING's list of the security core gains the window's protections
(constitution v2.6.1, Principle IX):
- `app/src-tauri/src/window.rs`;
- `app/src-tauri/capabilities/`;
- `app/src-tauri/window-surface.toml`;
- the `app.security` block of `tauri.conf.json`;
- `app/src-tauri/build.rs`'s app manifest.

This feature's pull request is reviewed line by line.
