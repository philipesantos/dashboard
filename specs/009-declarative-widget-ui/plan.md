# Implementation Plan: Declarative Widget UI and the Host Frame

**Branch**: `009-declarative-widget-ui` | **Date**: 2026-11-10 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/009-declarative-widget-ui/spec.md`

## Summary

The app shows its first real widget. A bundled showcase plugin, built with the app and pinned by
digest, runs in the existing runtime inside the app, approved in memory because it declares no
permission. Its render returns content in a new published format, which a new crate,
`widget-content`, checks and normalises; the page draws it with host components inside a frame
whose header only the host draws, with the manifest's new `name` and PNG `icon`. Buttons and forms
run actions through contract minor 1.4.0, which adds an `actor` export in six new worlds; actions
write the widget's store and call read operations only, and return data the host keeps. The window
gains three reviewed commands. Every criterion and refusal has a breaking test: unit and agreement
tests below the page, and new cases in feature 008's probe, run inside the real page on the three
systems against five hostile widgets.

## Technical Context

**Language/Version**: Rust stable, edition 2024 (workspace); TypeScript and React 19 in `app/`.

**Primary Dependencies**:
- the workspace's `runtime` crate, now a dependency of the app crate;
- new crate `widget-content` (`serde`, `serde_json` only);
- `png` 0.18.1, already in the lock through Tauri, to decode icons;
- `boon` 0.6 (dev), already used by the manifest test, for the content schema test;
- `lucide-react` for the status icons, already installed.

No chart library: charts are host-drawn SVG elements.

**Storage**: the app's own data directory `<app data dir>/host/` for kept data and stores; an
in-memory grant record (research R2, R3).

**Testing**: `cargo test` (runtime, widget-content, app crate), the agreement tests with the two
schemas, the docs reference test, and the window probe of feature 008 with new widget cases.

**Target Platform**: Windows (WebView2), macOS (WKWebView), Linux (WebKitGTK under the
constitution's fallback).

**Project Type**: desktop app with a Rust host library and a Wasm plugin contract.

**Performance Goals**: the bundled widget appears within 1 second of the window (load 22–68 ms,
render under 1 ms); an action's new state within 1 second.

**Constraints**:
- The window's content policy and its probe stay as feature 008 left them; the icon reaches the
  page as pixels.
- Contract 1.3.0 and every frozen version stay byte for byte; 1.4.0 only adds.
- Development records, vault and mode are never used by the app.
- `cargo xtask plugins` runs before any build of the app, in CI and locally (R1).
- The probe's release build now links Wasmtime under LTO: its time and peak memory are measured
  under the memory cap before the first push (R1).

**Scale/Scope**: 1 bundled widget in release; 5 hostile widgets in the probe build; 7 components;
3 window commands; 1 new WIT interface, 6 new worlds; 11 new probe cases.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.* Checked against
constitution v2.6.3, after the independent review of this plan (2026-11-11), whose findings are
folded into research R1 to R16; one of them led to the v2.6.3 amendment.

| Principle or rule | Status | How |
| --- | --- | --- |
| I. WebAssembly-only plugins | Pass | The bundled plugin is a Wasm component run by the existing runtime. |
| II. Zero capabilities by default | Pass | `actor` is an export, not an import. The bundled plugin declares no permission, and one that declares any is refused (R2). Optional capabilities keep their stand-ins; an action reaches only what was granted (R7). |
| III. External data only through connectors | Pass | Actions call connectors only as granted, read operations only; `change-needs-user-action` stays (R7). Development mode is off, so no unverified connector loads (R3). |
| IV. Plugins never see secrets | Pass | Form values holding a secret the host holds are refused before the plugin runs (R11); typed text is the residual v2.6.2 names, recorded in the threat model. The app's vault is its own (R3). |
| V. Plugins stay inside the host frame | Pass | The frame's body contains and clips content; the header is the host's; no portal, tooltip, native select or validation bubble; no password field (R10). Probe cases break each (R14). |
| VI. Human confirmation for AI actions | Pass | No AI in scope. |
| VII. Trust decisions only in host dialogs | Pass | No control grants, installs, connects or answers a dialog (FR-014). The bundled plugins are approved in memory only when they declare no permission, no connector and no storage, as v2.6.3 allows: "A plugin built into the app and declaring no permission is part of the app … the approval grants it nothing" (R2). Installed plugins will be approved in the installation dialog. |
| VIII. Every security rule has a breaking test | Pass | R13 and R14 list a test for each criterion and refusal; quickstart §3, §4 and §5 break three of them by hand. |
| IX. Human review of the security core | Pass | `wit/`, the runtime's grants and phases, the app's window module, commands and `window-surface.toml`, and the new frame and content components join the security core's review; this pull request is reviewed line by line. |
| Fixed decision: Rust + Tauri 2, React UI | Pass | Unchanged. |
| Fixed decision: the window engine | Pass | Unchanged; the probe runs on the three engines. |
| Fixed decision: plugin storage, one SQLite database per plugin | Pass | Kept data stays a host cache, discarded when the package or its approval changes; durable state is the `storage` permission's (R7). |
| Fixed decision: plugin contract by WIT, minors only add | Pass | 1.4.0 follows `wit/README.md`'s procedure: frozen as `history/1.4.0`, pinned, only-adds tested (R7). |
| CI on three systems, audits | Pass | The new tests and probe cases run in the existing jobs; no new dependency outside the lock. |
| Decision 0001 (plugins in the host process) | Pass, review recorded | Its trigger is installing a plugin the app did not build; the app now loads its own bundled plugins, which the decision gains as a dated note (R15). |

**Re-check after Phase 1 design**: unchanged. One design input changes first: criterion HS-02's
check, from "an SVG icon … drawn as an image" to "an SVG or any image that is not a PNG within
bounds is refused at load" (R6, maintainer's decision), as feature 007's rule allows.

## Project Structure

### Documentation (this feature)

```text
specs/009-declarative-widget-ui/
├── plan.md
├── research.md          # R1–R16
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── content-format.md
│   ├── actions.md
│   └── window-commands.md
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
wit/
├── v1/widget.wit                # 1.4.0: + interface actor, + six interactive-* worlds
├── history/1.4.0/widget.wit     # frozen copy
├── manifest.schema.json         # + name, icon, actions
└── content.schema.json          # new: the widget content format
widget-content/                  # new crate: parse, bound, normalise; schema agreement tests
runtime/
├── src/permissions.rs           # V1_VERSION 1.4.0, ACTIONS_SINCE, actor export, actions rules
├── src/manifest.rs              # name, icon (PNG checks), actions
├── src/calls.rs, lib.rs, storage/  # Phase::Action
├── src/vault/                   # vault_service option
├── src/network/mod.rs           # shared secret-echo check
├── src/cli/                     # run checks content; new `act`
└── tests/                       # actions.rs, contract_v1, manifest_schema, docs_reference
plugins/
├── examples/showcase/           # the bundled widget (interactive-widget, no permission)
├── fixtures/action-probe/       # actions with store and connector reads
└── hostile/ui-header, ui-overflow, ui-strings, ui-dialog, ui-invalid, action-escape
app/
├── src/widgets/                 # Widgets.tsx, Frame.tsx, content/*.tsx, api.ts
├── src/probe/widgets.ts         # the R14 cases
├── src/App.tsx                  # the widget stack above the demo's tabs
└── src-tauri/
    ├── build.rs                 # embeds and pins the bundled packages
    ├── src/widgets.rs           # runtime, bundled approval, handles, commands (security core)
    ├── src/icon.rs, name.rs     # PNG decode, name sanitising
    ├── src/commands.rs          # widgets_list, widget_render, widget_act
    ├── capabilities/default.json, window-surface.toml
    └── tests/                   # widgets.rs (records, approval, handles, busy, secret)
.github/workflows/ci.yml           # cargo xtask plugins before the app is built
CONTRIBUTING.md                    # the order of the build commands; the widget rules
docs/
├── authors/reference/content.md   # new
├── authors/reference/{manifest,contract,errors,limits,cli}.md, getting-started.md
├── design/host-surfaces.md      # HS-02's check (R6)
├── decisions/0001-…md           # dated note (R15)
└── threat-model.md              # rows on B1, B4, B7; B1/B2 text; typed-text residual
```

**Structure Decision**: the content format gets its own crate so the app, the CLI and the tests
share one reader. The app's widget host sits in one module, `widgets.rs`, beside `window.rs`, so
the security core's review reads them together. Hostile widgets are ordinary packages under
`plugins/hostile/`, built by `cargo xtask plugins`, embedded only in the probe build.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| Six new WIT worlds for one export | The only-adds test compares each world's imports and exports, so no 1.3.0 world can gain `actor` | A v2 contract: forbidden for an addition (`wit/README.md`) |
| A new workspace crate | The app, the CLI and the tests read the same format | Code in the app crate only: authors could not check content before running the app (US2) |
