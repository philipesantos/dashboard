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
- **The runtime's own example, run on the maintainer's desktop** (2026-11-10; Ubuntu, GNOME on
  Wayland; `tauri` branch `v3` at `6620118`, runtime `3.0.0-alpha.7`, CEF 154):
  - the CLI built from source in 4 min 19 s; the first `tauri dev` downloaded 1.8 GB of CEF into
    `~/.cache/tauri-cef`; the debug binary is 295 MB, and the app's processes held 833 MB;
  - **typing reached no text field.** The runtime forces X11 (`--ozone-platform=x11`,
    `GDK_BACKEND=x11`, `runtime.rs:3313-3337`), so on a Wayland session it runs through
    XWayland, which matches #16251 and #16351;
  - **the main process held one core at about 100%** over four 5 s samples, while the page and
    GPU processes stayed near 0%, which matches #16189.

  The maintainer stopped the trial there: either defect alone blocks a dashboard that stays
  open all day.

**Rationale**: a hands-on trial would mean porting the app to Tauri 3 alpha on a throwaway
branch, and the open defects already decide it. The constitution's fallback exists for this
case. WebKitGTK is patched by the distribution, so FR-017's duties (the version in each release,
the weekly Chromium check and the 14-day rule) do not apply until an embedded engine ships. The
constitution says "while the app embeds the engine".

**Alternatives considered**:
- A hands-on trial of the app ported to Tauri 3 alpha. Not needed: the runtime's own example
  failed on keyboard input and idle CPU first.
- Moving the host to Tauri 3 alpha now. Declined: an alpha framework under the security core,
  with the defects above.

## R2. The window is built in Rust, not in the configuration

**Decision**: `tauri.conf.json` declares no window (`app.windows: []`). `setup` builds the single
window with `WebviewWindowBuilder`, attaching every protection in one place,
`app/src-tauri/src/window.rs`:
- `on_navigation`;
- `on_new_window` returning `NewWindowResponse::Deny`;
- `on_permission_request` denying every request, and `on_download` returning `false` (R14);
- `incognito(true)`;
- `general_autofill_enabled(false)`;
- `devtools(tauri::is_dev())`;
- an initialization script that replaces `alert`, `confirm`, `prompt` and `print` (R14);
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
| `script-src` | `'self'` | The bundled module scripts load by `'self'`. Tauri adds a hash only for inline `<script>` text, which the bundle does not have. No `'unsafe-inline'`, no `'unsafe-eval'` |
| `style-src` | `'self'` | Tauri adds the response's nonce, for the empty `<style>` of `index.html`. Run-time `<style>` elements carry that nonce (below). No `'unsafe-inline'` |
| `font-src` | `'self'` | The `@fontsource` fonts are bundled (feature 007) |
| `img-src` | `'self'` | Icons are inline SVG from `lucide-react`, not images |
| `connect-src` | `ipc:`, plus `http://ipc.localhost` on Windows only (`tauri.windows.conf.json`) | The host's channel only. Windows spells it in `http`; elsewhere that address is a real request to port 80 (T028) |
| `object-src`, `frame-src`, `child-src`, `worker-src` | `'none'` | Nothing embedded, no workers |
| `base-uri` | `'none'` | A `<base>` cannot redirect relative URLs |
| `form-action` | `'none'` | No form submits anywhere; the declarative UI's form goes through a host command |
| `frame-ancestors` | `'none'` | No one frames the window |

- **No `data:` anywhere.** Vite inlines assets under 4 KiB as `data:` URIs, so `vite.config.ts`
  sets `build.assetsInlineLimit: 0`. That keeps `font-src` and `img-src` at `'self'`.
- **Inline styles.**
  - React sets `style={{…}}` through the CSSOM, which the policy does not block.
  - Two Radix parts insert `<style>` elements at run time, and the probe's
    `style-runtime-element` case found both (T008):
    - the scroll lock of the Dialog and the Select, `react-remove-scroll-bar` through
      `react-style-singleton`, which takes its nonce from `get-nonce`;
    - the Select's viewport, which takes a `nonce` prop.
  - Each needs `'unsafe-inline'` or a nonce. Tauri generates a nonce **per response**, in
    `get_asset` (`tauri-2.12.5/src/manager/mod.rs:126-137`, called at `:429`), and puts it on
    every `<style>` of the bundled HTML and in `style-src`. So `index.html` holds one empty
    `<style>`, and `main.tsx` reads its `nonce` property (engines hide the attribute) and passes
    it to `get-nonce`'s `setNonce`. The Select's viewport gets `getNonce()` in
    `components/ui/select.tsx`. `get-nonce` 1.0.1 becomes a direct dependency, the copy
    `react-style-singleton` already used.
  - With that, the policy needs no `'unsafe-inline'` for styles. A `<style>` that markup
    injection adds has no nonce and is refused, which `load-style-import` checks.
  - The cost is a rule for later components: one that draws a `<style>` without the nonce is
    refused and draws wrong. The probe opens only the demo's dialog and select, so
    `app/src-tauri/tests/style_nonce.rs` checks the rest statically. It fails when an installed
    Radix package takes a `nonce` and is not on its list (today `react-select` and
    `react-scroll-area`), when a listed part is used in `app/src` without `nonce=` (which also
    catches `shadcn add` overwriting `select.tsx`), or when the empty `<style>` or the
    `setNonce` call goes.
  - Style cannot run script. Because `img-src` and `font-src` stay `'self'`, a style rule
    cannot send data to the internet either.
- **Development.** On desktop, `tauri dev` loads the page straight from Vite's server. Tauri
  proxies it only on mobile (`PROXY_DEV_SERVER = cfg!(all(dev, mobile))`,
  `tauri-2.12.5/src/manager/webview.rs:43`), and it attaches its policy only to what it serves
  through its own protocol. So `devCsp` would do nothing here, and it is not set. Instead, Vite's
  `server.headers` sends the development policy: the release policy with two changes.
  - `script-src` and `style-src` gain a fixed development nonce. Vite writes it on its scripts
    and styles (`html.cspNonce`, set only for `serve`), including the React refresh preamble,
    and `main.tsx` hands it to the run-time styles as it hands them Tauri's nonce in release.
    The plan first had `'unsafe-inline'` in `script-src`, which would also have left Vite's CSS
    `<style>` elements refused (T009).
  - `connect-src` gains `ws://localhost:1420` for hot reload. With `TAURI_DEV_HOST` unset, Vite
    uses its own port, and `'self'` is the server's origin.

  `vite.config.ts` builds that header from `tauri.conf.json`'s `csp`, so the two cannot drift. A
  refusal in development then shows in the console, with the rule that refused it (FR-004).
  The probe tests only the release policy (FR-003).
- **`dangerousDisableAssetCspModification`** stays unset: Tauri's hashes and nonces are the
  protection (`tauri-utils` `SecurityConfig`).

**Rationale**: FR-001 to FR-004. These are the narrowest values that still load the demo.

**Alternatives considered**:
- `'unsafe-inline'` in `style-src-elem`, the plan's first fallback. Not needed: the nonce
  covers both components. It would also let injected markup add styles that could restyle a
  host dialog. The plan first rejected the nonce on the belief that Tauri's nonce was fixed per
  build; the source shows it is drawn per response.
- Tauri's isolation pattern, which passes IPC through a sandboxed iframe. Rejected for now: the
  window has no host command (R4), so there is nothing for it to vet. It needs `frame-src`, and
  it can be revisited when commands arrive.

## R4. Capabilities and host commands: the reviewed list

**Decision**:

- **App commands are checked against the ACL.** Tauri checks app commands from the app's own
  page only when the build produced an app ACL manifest (`tauri-2.12.5/src/webview/mod.rs:2080`).
  That manifest exists only when the app has a default permission, a permission set or a
  permission (`tauri-build-2.7.4/src/acl.rs:421-426`). An `AppManifest` with an empty command
  list produces none. So `app/src-tauri/permissions/default.toml` declares an empty default set,
  which makes the manifest exist with no commands. `build.rs` lists the app's commands in
  `AppManifest::commands`, which generates an `allow-` and a `deny-` permission for each. Every
  app command then needs its `allow-` permission in a capability. The surface test fails when
  the generated `gen/schemas/acl-manifests.json` lacks the app's key, so the check cannot
  silently switch off.
- **No permissions.** `capabilities/default.json` grants none (`"permissions": []`). The host UI calls no
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

- **Navigation.** `on_navigation` allows only the app's own page: `/` and `/index.html` of
  - `tauri://localhost` on macOS and Linux, `http://tauri.localhost` on Windows;
  - in development (`tauri::is_dev()`), `devUrl`.

  A URL the runtime cannot parse is allowed without calling the handler
  (`tauri-runtime-wry-2.12.3/src/lib.rs:4821-4825`), which the review of T028 also raised. The
  engine refuses such URLs itself as far as known, and the probe's navigation cases check that
  the page stayed. It is an accepted residual in Tauri, recorded in the threat model.

  Everything else is refused and counted for the probe: `http(s)`, `file:`, `javascript:`,
  `data:`, `blob:` and any other scheme.
- **New windows.** `on_new_window` returns `Deny` for every request.
- **Dropped files and URLs.** Tauri's own drop handler stays on, and the surface test fails if
  `dragDropEnabled: false` or `disable_drag_drop_handler` appears (`dragDropEnabled`, the
  default). It takes the operating system's drop before the engine can act on it, and the host
  UI does not listen to its event, so a drop does nothing. If a drop still reached the engine,
  `on_navigation` would refuse the load.

**Rationale**: FR-009, FR-010 and FR-012. A synthetic drop event in a page never performs the
browser's default navigation, so the probe tests navigation through the handler instead.

## R6. Developer tools and the context menu

**Decision**:

- **Release builds** enable neither the `tauri` crate's `devtools` feature nor
  `tauri-runtime-wry/devtools` or `wry/devtools`. `devtools` is
  available only in debug builds unless that feature is on (`webview_window.rs:1222`). A test
  fails if `app/src-tauri/Cargo.toml` lists `devtools` among the features of `tauri`,
  `tauri-runtime-wry` or `wry`. Only that file can turn it on for the app.
- **The probe reads each engine's setting** through `with_webview`, in a build made with
  `tauri/custom-protocol`, as `tauri build` makes it (R8):
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

**Limit on macOS (Principle VIII)**: no public WKWebView API turns password AutoFill off for one
app or reports whether it would offer. Apple ships no WebDriver for WKWebView to drive the
system's AutoFill UI. A developer forum reply also says macOS 26 autofills in web views. The
automated tests cover what the project controls:
- the incognito store, read back by the probe;
- the configuration: the surface test fails if `tauri.conf.json` sets macOS entitlements or an
  entitlements file that grants `com.apple.developer.associated-domains` or `webcredentials`.

An unsigned probe binary carries no entitlements, so it proves nothing about the bundle.

Whether AutoFill itself appears has no automated breaking test on macOS. The macOS walk in
`quickstart.md` covers it as a review step, and the plan's Complexity Tracking and the threat
model's row record the gap. The walk's form submits by removing itself, which engines treat as a
submission, because `form-action 'none'` blocks a real submit.

**Rationale**: FR-016 and criterion HS-26 of feature 007.

## R8. Breaking tests against the built app: the probe

**Decision**: a `shell-probe` Cargo feature of the app crate, and a `probe` Vite mode.

- **What differs from a release build.** The probe is built with
  `--release --features shell-probe,tauri/custom-protocol`. The second feature is the one
  `tauri build` turns on. Without it Tauri builds in dev mode: it loads `devUrl`, embeds no
  assets and serves no policy (`tauri-2.12.5/build.rs:252-255`). Its content policy, capability
  list, ACL mode and window settings are those of a release build. It differs in four ways:
  - Vite's `probe` mode adds one module, `src/probe/run.ts`, to `index.html`. It is bundled, so
    the policy allows it.
  - `build.rs` adds two commands to `AppManifest::commands` when the feature is on:
    - `shell_probe_report`, which the probe calls to report;
    - `shell_probe_ungranted`, which no capability grants and which the probe must be refused.
  - At run time, the feature adds through `Manager::add_capability`
    (`tauri-2.12.5/src/lib.rs:841`) one capability granting `allow-shell-probe-report`. It
    resolves against the build-time ACL, which now holds that permission
    (`ipc/authority.rs:174-180`). Neither command is in `capabilities/` or
    `window-surface.toml`, and the release ACL is checked as strictly, because the release
    build already has an app manifest (R4).
  - The binary refuses to start unless `DASHBOARD_PROBE=1` is set, so a build that shipped with
    the feature by mistake would not run.
  - On Windows, WebView2 gets a fake camera (`--use-fake-device-for-media-stream`, with wry's
    default arguments kept), so the camera case meets the permission handler (R16).
- **What the probe module tries** inside the real page, recording each outcome:
  - injected script: an `innerHTML` with an `onerror` handler, a `<script>` element with inline
    text, `eval`, `new Function` and `setTimeout` with a string;
  - loads from `.invalid` hosts by every path: `<script>`, `<link>` stylesheet, `<img>`,
    `FontFace`, `fetch`, `XMLHttpRequest`, `WebSocket`, `EventSource`, `<iframe>`, `<object>`
    and an `@import`;
  - a `<base>` element and a form submitted to another origin;
  - navigation: `location.href`, `location.assign`, a meta refresh and a link click;
  - `window.open`, and a link with `target=_blank`;
  - host commands: `invoke('shell_probe_ungranted')`, a registered command with no grant,
    plus `plugin:opener|open_url` and `plugin:event|listen`, each expected to be refused by the
    ACL rather than missing;
  - a web permission request (`getUserMedia`, `geolocation`, `Notification.requestPermission`),
    a download by an `a[download]` click, `alert`, `confirm`, `prompt` and `print`, each expected
    to be denied or to return without an engine dialog (R14);
  - a worker from a `blob:` URL, a remote `<audio>` and a `data:` image, each expected to be
    refused by the policy;
  - a `contextmenu` event, expected to be cancelled;
  - the demo's dialog and select opened and closed, so the policy's effect on styles shows.

  The `.invalid` hosts never resolve. A refusal counts only when the engine's
  `securitypolicyviolation` event fired, or the side effect is absent, never just because the
  network failed.
- **What the Rust side does**:
  - counts `on_navigation`, `on_new_window`, `on_permission_request` and `on_download` refusals;
  - checks that the app still has exactly one webview window. On WKWebView and WebKitGTK, a
    scripted `window.open` without a user gesture is refused before `on_new_window` is called,
    so a window case holds when no second window exists, whether or not the handler ran;
  - reads the engine settings of R6 and R7 through `with_webview`;
  - waits for the report, at most 60 s;
  - prints one line per case and exits 0 only when every case holds.

  A navigation that succeeded would unload the probe, so a missing report also fails.
- **CI.** The build job runs, on each system:
  - `pnpm --filter ./app build --mode probe`;
  - `cargo build -p dashboard --release --features shell-probe,tauri/custom-protocol --locked`;
  - the binary with `DASHBOARD_PROBE=1`, under `xvfb-run` on Linux with
    `WEBKIT_DISABLE_DMABUF_RENDERER=1`.

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

**Outcome (T010)**: on. The demo renders with no uncaught error, its dialog and select open and
close, and every probe case holds. The `prototype-frozen` case checks that it stays on.

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
  - engine autofill and password saving, with the macOS limit of R7 stated;
  - engine permissions, downloads and dialogs (R14).
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
- **`audit.yml`'s weekly run** gains a job that queries crates.io for `tauri-runtime-cef`.
  Decision record 0002 records the last version tried, `3.0.0-alpha.7`. The job fails, so it shows red, when a stable version newer than that one
  exists. That tells the maintainer the trial is due again (constitution v2.6.1). Recording the
  new trial updates the version and turns the job green.
- **Cost.** The probe adds a release build of the app crate per system. That build reuses the
  workspace's cached dependencies, and the probe is expected to run in under a minute.
  `quickstart.md` records the measured times.

**Measured locally (T026, Linux, 8 cores, 2026-11-10):**
- the probe's release build: 2 min 22 s from a cold release cache, 1 min 35 s once the
  dependencies are built;
- the probe's run under `xvfb-run`: 19 s for 46 cases, most of it the navigation, window and
  download cases, which each wait 1 s to see that the page stayed;
- the rest of the local checks all pass: `cargo test --workspace` (649 tests, 15 min), clippy
  on the workspace and the probe build, `cargo deny`, `cargo audit`, `pnpm audit`, the design
  checks and the npm licenses.

**Measured on CI (T029, PR #20, second run, every job green):**

| System | Probe release build | Probe step | Cases | Build job |
| --- | --- | --- | --- | --- |
| Windows | 4 min 45 s | 22 s | 56 held | 23.5 min |
| macOS | 2 min 43 s | 23 s | 52 held | 13 min |
| Linux | 1 min 20 s | 50 s, `xvfb` start-up included | 52 held | 13 min |

The 40-minute job timeout leaves Windows room.

A third run, on a commit that touched only `specs/`, broke `load-media-remote` on macOS: no
violation within the case's fixed 300 ms, where the run before had one. WKWebView starts a media
load late. The violation cases now poll for up to 2 s and return as soon as the violation
arrives, and the audio element is in the document with `preload="auto"`; the probe got faster on
Linux (about 15 s locally). The `cef-watch` job, run by hand, passed: newest
published `3.0.0-alpha.7`, no stable release.

## R12. Decision 0001, where plugins run

**Decision**: unchanged. This feature does not bring the plugin runtime into the app (FR-008).
Decision 0001 is revisited with the installation item, its stated trigger.

## R13. The security core

**Decision**: CONTRIBUTING's list of the security core gains the window's protections
(constitution v2.6.1, Principle IX):
- `app/src-tauri/src/window.rs`, `lib.rs` (the `commands!` macro and the invoke handler) and
  `probe.rs`;
- `app/src-tauri/build.rs`, `permissions/` and `capabilities/`;
- `app/src-tauri/window-surface.toml`;
- `tauri.conf.json`'s `app.security` and `bundle` blocks, and the `tauri` features in
  `app/src-tauri/Cargo.toml`;
- `app/src/main.tsx`'s context-menu cancel and `app/vite.config.ts`'s development policy;
- `app/index.html`'s empty `<style>`, `main.tsx`'s `setNonce`, and the `nonce` props in
  `app/src/components/ui/` (R3);
- the tests that guard them: `app/src-tauri/tests/surface.rs`, `tests/style_nonce.rs`,
  `app/src/probe/` and the probe step in `ci.yml`.

This feature's pull request is reviewed line by line.

## R14. What the engine itself grants or draws

**Decision**:
- **Permission requests.** `on_permission_request` (`webview_window.rs:475`) denies every one.
  Without a handler, wry grants camera and microphone on macOS
  (`wry-0.57.0/src/wkwebview/class/wry_web_view_ui_delegate.rs:164-166`), and WebView2 shows its
  own prompt.
- **Downloads.** `on_download` (`webview_window.rs:384`) returns `false`. wry allows downloads by
  default, and on macOS they bypass the navigation handler.
- **Engine dialogs.** An initialization script, run before any page script on every engine,
  replaces `alert`, `confirm`, `prompt` and `print` with functions that do nothing:
  - `confirm` returns `false`;
  - `prompt` returns `null`.

  The host UI renders no `<input type=file>`, and the declarative UI has no file component.

**Rationale**: FR-021. These are dialogs and grants drawn by the engine outside the host frame,
which Principles V and VII reserve to the host.

**What the permission cases can show (T015)**: on WebKitGTK the three permission cases hold even
before `on_permission_request` exists. The engine denies location and notifications on its own,
and with no camera the request fails with `OverconstrainedError` before any permission step. So
on Linux they prove that nothing is granted, not that the handler runs; each detail names the
handler's refusals when there are any. The case that breaks without the handler is macOS, where
wry grants camera and microphone by default.

The same holds for `download-link` on WebKitGTK: the click reaches `on_navigation` as a
navigation to a `blob:` URL, which is refused before any download starts. The link is a
same-origin `blob:` with a file name, how a page saves a file: on the first CI run a link to a
bundled file did nothing at all on WKWebView, which ignores `download` on a `tauri:` link, so the
case could not reach `on_download` there. WebView2 hands it to `on_download`.

## R15. The WebKitGTK register

**Decision**: every change made only because Linux uses the system WebKitGTK is listed in
`docs/webkitgtk-register.toml` (FR-022, the maintainer's request of 2026-11-10). Each place the
change is made carries the marker `WEBKITGTK-ONLY(<id>)` in a comment, in whatever syntax the
file uses: `//` in Rust and TypeScript, `#` in YAML and TOML, `<!-- -->` in Markdown. A test,
`app/src-tauri/tests/webkitgtk_register.rs`, compares the markers in the files git tracks with
the register ([contracts/webkitgtk-register.md](contracts/webkitgtk-register.md)). Decision
record 0002 links the register, and CONTRIBUTING states the rule.

The entries this feature starts with:

| Id | Where | Why | On the move |
| --- | --- | --- | --- |
| `system-libs` | `ci.yml`'s apt step, `CONTRIBUTING.md`'s Linux prerequisites | Tauri 2 on Linux builds against `libwebkit2gtk-4.1-dev` | Replace with the embedded engine's build dependencies |
| `ci-dmabuf` | `ci.yml`'s probe step | WebKitGTK's DMA-BUF renderer fails under `xvfb` (R8) | Delete |
| `probe-devtools-read` | `probe.rs`, and the `webkit2gtk` dependency in `app/src-tauri/Cargo.toml` | The `devtools-setting` case reads `enable-developer-extras` (T016) | Replace with the embedded engine's read |
| `probe-ephemeral-read` | `probe.rs` | The `store-incognito` case reads the ephemeral session (T019) | Replace with the embedded engine's read |
| `cef-watch` | `audit.yml` | While Linux uses WebKitGTK, the weekly job reports a stable `tauri-runtime-cef` (T022) | Replace with the weekly Chromium security check of FR-017 |
| `window-close` | `window.rs`, `app/src/probe/window.ts` | WebKitGTK destroys the webview on `window.close()`; the start-up script makes it do nothing (R16) | Delete, unless the new engine also closes the page |
| `probe-download-as-navigation` | `app/src/probe/engine.ts` | WebKitGTK turns a download link into a navigation, which `on_navigation` refuses first, so `download-link` also accepts that refusal (T018) | Require an `on_download` refusal, unless the new engine does the same |

The register lists only what is specific to WebKitGTK. Code shared with WKWebView, such as the
context-menu cancel in `main.tsx` (R6), stays after the move, so it is not listed.

**Rationale**: the maintainer's concern is that WebKitGTK workarounds pile up and stay behind
after the move to the embedded engine. A marker at each place and one list make the move a
walk through the list. The test keeps the two from drifting. A change made for WebKitGTK and
never marked cannot be matched, so the test also trips on the plain signs of one in `app/` and
`.github/`: the engine's name, its `WEBKIT_` variables, a Linux-only condition in the app, and
engine sniffing in the UI, each needing a marker nearby. At T033 it found the apt step, the
probe's reads and the download case, all listed, and two comments about code macOS shares,
which now name the engines by system. A change that names none of these, such as a CSS value chosen
for how WebKitGTK draws, still depends on CONTRIBUTING and review.

**Alternatives considered**:
- Only a section in decision record 0002. Rejected: nothing would check it against the code.
- Keeping every WebKitGTK change under `cfg(target_os = "linux")`. Not enough: CI steps,
  dependencies and UI workarounds have no `cfg`, and `cfg` alone does not say why.

## R16. The independent review (T028)

A fresh agent with none of the feature's context read the security-core files against the spec,
the contracts and the locked Tauri sources. Each finding was checked at the source before any
change.

| # | Finding | Checked | Outcome |
| --- | --- | --- | --- |
| 1 | `on_navigation` and `connect-src` allowed both systems' forms of the app's origin and IPC channel. On macOS and Linux, `http://tauri.localhost` and `http://ipc.localhost` are real requests to the machine's port 80 | `tauri-2.12.5/scripts/core.js:16-18` | Fixed. Each system accepts only its own origin (`APP_ORIGIN` by `cfg`); `http://ipc.localhost` moved to `tauri.windows.conf.json`, which Tauri merges on Windows only (`tauri-codegen-2.7.4/src/lib.rs:84-89`). Cases `navigate-other-app-origin`, `connect-ipc-http-elsewhere` |
| 2 | Tauri's navigation wrapper allows a URL it cannot parse without calling the handler | `tauri-runtime-wry-2.12.3/src/lib.rs:4821-4826` (`unwrap_or(true)`) | Accepted residual, in Tauri, recorded in R5 and the threat model. No URL found that the engine loads and `url` rejects |
| 3 | FR-021 names the file picker, and nothing blocked it | wry's macOS UI delegate opens `NSOpenPanel`; WebKitGTK's default `run-file-chooser` | Fixed in the start-up script: clicks on file inputs cancelled, `click()` and `showPicker()` replaced, the File System Access pickers removed. Case `file-picker`, which breaks without it |
| 4 | `window-open` and `window-target-blank` could not fail: a window the handler let through is wry's, not Tauri's, so the host's count stays 1 | wry creates its own window on `Allow` | Fixed where it can be: on WebView2, which hands scripted popups to the app, the case now needs a handler refusal. The macOS and Linux engines refuse a popup without a user gesture before asking the app, so there the case shows only that no window opened |
| 5 | `permission-camera` could not fail on CI: with no camera, `getUserMedia` fails before any permission step | runners have no camera | Partly fixed: the probe build gives WebView2 a fake camera (`--use-fake-device-for-media-stream` through `additional_browser_args`, wry's defaults kept; the `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS` variable, tried first, had no effect on CI), and on WebView2 the case needs `NotAllowedError`. Elsewhere it still shows only that nothing was granted (R14) |
| 6 | The surface test read only top-level JSON single capabilities and only `tauri.conf.json` | `tauri-utils-2.10.2/src/acl/build.rs`, `acl/capability.rs:250-265` | Fixed: capabilities in any folder under `capabilities/`, JSON or TOML, in all three forms, and inline in any config; entitlements and the drop handler checked in every per-system config |
| 7 | An `about:blank` frame might keep the engine's dialogs | | Checked: the start-up script runs in such a frame on WebKitGTK. Case `dialog-iframe` checks it on every system without calling the dialogs |
| 8 | WebRTC and `dns-prefetch` are outside the content policy | | WebRTC removed by the start-up script, case `connect-webrtc`; this WebKitGTK has no WebRTC, so the case can break only on Windows and macOS. `dns-prefetch` needs injected markup and leaks only a lookup: accepted residual, in the threat model |
| 9 | The probe had no expected list of cases, so a dropped case passed unnoticed | `probe.rs` | Fixed: `PAGE_CASES` lists every page case; one missing or unlisted breaks the run |
| 10 | WebView2's browser shortcut keys stay on; on Linux `window.close()` destroys the webview | wry `lib.rs:1768`; `webkitgtk/mod.rs:466` | Fixed: shortcut keys off in release (`ICoreWebView2Settings3`), case `browser-keys-setting`; `window.close` replaced, case `window-close`, registered as WebKitGTK-only (`window-close`) |

**The first CI run (T029, PR #20).** Linux held every case. macOS held 51 of 52 and Windows 54
of 56; the three that broke were faults of the cases, not of the window, and are fixed:
- `download-link` on macOS: neither handler was reached, because WKWebView ignored `download` on
  a link to a bundled `tauri:` file. The case now uses a same-origin `blob:` link.
- `connect-eventsource` on Windows: no violation, because the case closed the connection at once
  and Chromium checks the policy when the connection starts. It now closes after the check, and
  so does `connect-websocket`.
- `permission-camera` on Windows: `NotFoundError`, because the environment variable that was to
  give WebView2 a fake camera had no effect. The probe build now passes the argument itself.

Everything else held on both, including every engine setting: WebView2's devtools, context
menu, browser keys, general autofill and password saving off, the profile InPrivate; WKWebView
not inspectable and its data store non-persistent; and on both, the new-window handler refused
`window.open`.
