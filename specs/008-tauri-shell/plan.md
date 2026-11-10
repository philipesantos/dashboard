# Implementation Plan: The Desktop Shell, with Minimal Capabilities and a Restrictive CSP

**Branch**: `008-tauri-shell` | **Date**: 2026-11-10 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/008-tauri-shell/spec.md`

## Summary

The app's single window gets every protection in one reviewed place:
- a release content policy that allows only the bundle and the host's channel;
- an empty, reviewed list of capabilities and host commands. Tauri's ACL enforces it, through an
  app manifest kept present by an empty default permission set, and a test checks it;
- navigation and new windows refused;
- developer tools and the engine's context menu off in release builds;
- each engine's autofill and password saving off, with an incognito data store (HS-26);
- every web permission request denied, downloads refused, and engine dialogs (`alert`,
  `confirm`, `prompt`, `print`) replaced.

The template's opener plugin and `greet` command go. A `shell-probe` build, the release build
plus one bundled script and one report command, tries every forbidden action inside the real
page on Windows, macOS and Linux and fails CI if any succeeds.

On Linux the engine stays the system WebKitGTK, after a trial of Tauri's embedded-Chromium
runtime. That runtime exists only for Tauri 3 alpha and has open Linux defects; its own example,
run on the maintainer's desktop, took no keyboard input and held a core at 100% (R1). Every change
made only for WebKitGTK is marked where it is made and listed in a register that a test checks,
so the move to the embedded engine can remove each one (R15). The trial is
recorded as decision record 0002, and a weekly job reports when a stable runtime release makes
the next trial due. The threat model gains the window as boundary B7, and its webview assumption
and diagram are fixed.

## Technical Context

**Language/Version**:
- Rust, stable, edition 2024, as the workspace;
- TypeScript and React 19 in `app/`.

**Primary Dependencies**:
- `tauri` 2.12.5 and `tauri-build` 2.7.4, already locked;
- `webview2-com`, already in the tree through `wry`, for the WebView2 settings `wry` does not
  expose;
- `objc2` and `webkit2gtk`, already in the tree, for the WKWebView and WebKitGTK reads.

No new crate from outside the current lock is expected. `tauri-plugin-opener` and
`@tauri-apps/plugin-opener` are removed.

**Storage**: none. The engine runs incognito.

**Testing**:
- `cargo test` in the app crate, for the surface list and the release feature check;
- the probe, a release-profile run of the app with the `shell-probe` feature, on the three
  systems.

**Target Platform**: Windows (WebView2), macOS (WKWebView), Linux (WebKitGTK 4.1, under the
constitution's temporary fallback).

**Project Type**: desktop app (the Tauri host in `app/src-tauri`, the React UI in `app/src`).

**Performance Goals**: the probe completes in under 60 s per system. Start-up of the app is
unchanged from today, measured in the quickstart.

**Constraints**:
- **No change to** `runtime/`, `wit/`, `plugins/` or `connectors/`.
- **The look is unchanged** (feature 007), apart from the removed "Tauri" demo tab.
- **The release build is what the probe tests.** The probe adds only its script and its report
  command.

**Scale/Scope**:
- 1 window;
- 0 host commands and 0 permissions on the reviewed list;
- about 48 probe cases;
- 8 threat-model rows;
- 6 entries in the WebKitGTK register.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.* Checked against
constitution v2.6.1.

| Principle or rule | Status | How |
| --- | --- | --- |
| I. WebAssembly-only plugins | Pass | No plugin code involved. |
| II. Zero capabilities by default | Pass | The same rule applied to the window. An empty default permission set keeps the app ACL manifest present, so every app command needs an explicit grant, and the surface test fails if the manifest disappears. The window starts with no grant (R4). |
| III. External data only through connectors | Pass | The window can load nothing from the internet (R3). |
| IV. Plugins never see secrets | Pass | Not touched. The engine keeps no credential (R7). |
| V. Plugins stay inside the host frame | Pass, strengthened | No window, popup or navigation can come from content (R5), and no engine offers a password (R7, HS-26). |
| VI. Human confirmation for AI actions | Pass | Not touched. |
| VII. Trust decisions only in host dialogs | Pass, strengthened | Injected script, which could operate a host dialog, is refused by the policy (R3). The window can reach no host command (R4). |
| VIII. Every security rule has a breaking test | Pass, with one limit recorded | Each rule has a probe case or a test (R4, R6, R8, R14). The probe is built as `tauri build` builds, with `tauri/custom-protocol`. One limit: on macOS, whether the system offers password AutoFill has no automated test, because no API or WebDriver reaches it. Tests cover the incognito store and the absence of the entitlement that enables it, and a review step covers the offer itself (R7, Complexity Tracking). |
| IX. Human review of the security core | Pass | The window's protections are core since v2.6.0. They live in the files R13 lists, the tests that guard them included, which CONTRIBUTING gains, and this pull request is reviewed line by line. |
| Fixed decisions: Tauri 2 and React | Pass | Unchanged. |
| Fixed decision: the window engine | Pass, through the fallback | WebView2 and WKWebView as decided. On Linux, WebKitGTK under the v2.6.1 fallback, after the trial (R1, decision record 0002), with each WebKitGTK-only change in the register (R15). The weekly watch (R11) triggers the next trial at each stable release of the runtime. |
| Engine security duties (14 days, weekly check, version in releases) | Not yet applicable | They apply "while the app embeds the engine", as the spec's FR-017 now says too. WebKitGTK is patched by the distribution (R1). |
| CI on three systems, audit duties | Pass | The probe runs on all three. `cargo audit`, `cargo deny`, `pnpm audit` and the license check continue. |
| Phase gates | Pass | Phases 1 and 2 have passed. |

**Re-check after Phase 1 design**: unchanged, all pass. No complexity to justify.

## Project Structure

### Documentation (this feature)

```text
specs/008-tauri-shell/
├── plan.md              # This file
├── research.md          # Phase 0: R1–R15
├── data-model.md        # Phase 1
├── quickstart.md        # Phase 1
├── contracts/
│   ├── content-policy.md   # release and development policies, directive by directive
│   ├── window-surface.md   # window-surface.toml and the check's error lines
│   ├── probe.md            # probe cases, report lines and exit codes
│   └── webkitgtk-register.md  # the register of WebKitGTK-only changes and its markers
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
app/
├── package.json                 # − @tauri-apps/plugin-opener
├── vite.config.ts               # assetsInlineLimit 0; development policy in server.headers; `probe` mode adds src/probe/run.ts
├── index.html
├── src/
│   ├── main.tsx                 # cancels the engine's context menu in release
│   ├── App.tsx                  # − the "Tauri" tab (it only called greet)
│   └── probe/run.ts             # probe cases (bundled only in `probe` mode)
└── src-tauri/
    ├── Cargo.toml               # − tauri-plugin-opener; + feature shell-probe
    ├── build.rs                 # AppManifest: the app's commands (+ the probe's, with the feature)
    ├── permissions/default.toml # an empty default set, so the app ACL manifest exists
    ├── tauri.conf.json          # app.windows []; app.security: csp, freezePrototype
    ├── capabilities/default.json   # "permissions": []
    ├── window-surface.toml      # the reviewed list (empty, with how to add)
    ├── src/
    │   ├── lib.rs               # builds the window in setup; commands! macro
    │   ├── window.rs            # every window protection (security core)
    │   └── probe.rs             # cfg(feature = "shell-probe"): report and ungranted commands, reads, exit
    └── tests/
        ├── surface.rs           # list vs capabilities, commands, plugins; app ACL present; no devtools; no macOS web-credential entitlements
        ├── style_nonce.rs       # every run-time <style> carries the nonce (Radix parts, index.html, main.tsx)
        └── webkitgtk_register.rs   # WEBKITGTK-ONLY markers vs docs/webkitgtk-register.toml
docs/
├── decisions/0002-linux-window-engine.md   # the trial and when to repeat it
├── webkitgtk-register.toml                 # every WebKitGTK-only change, removed on the move
└── threat-model.md              # B7, rows, webview assumption, diagram, CSP question closed
.github/workflows/ci.yml         # probe step on three systems; xvfb on Linux; comment cites v2.6.1
.github/workflows/audit.yml      # weekly tauri-runtime-cef stable-release watch
CONTRIBUTING.md                  # security core gains the window's protections
```

**Structure Decision**: everything stays in the existing app crate and UI. The window's
protections sit in one module, `window.rs`, so the security core's line-by-line review has one
place to read. The probe is a feature of the same crate, so it builds the same code as release.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| macOS password AutoFill has no end-to-end breaking test (Principle VIII) | WKWebView has no public API to turn it off or read it, and Apple ships no WebDriver for WKWebView to observe the system's AutoFill UI | A paid or unproven third-party macOS driver would put the security test on code outside the project's review. The project controls the incognito store and the entitlement, and tests both. The offer itself is a review step in the quickstart, and the threat model records the gap. |
