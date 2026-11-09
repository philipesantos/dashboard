# Implementation Plan: The Host Window, Host Dialogs and the Rules That Replace the Webview's CSP

**Branch**: `008-host-window-dialogs` | **Date**: 2026-11-09 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/008-host-window-dialogs/spec.md`

## Summary

The dashboard gets its window: a new `host` crate on gpui-unofficial 1.25.3 (all nineteen crates
pinned with `=`, R1) opens a native window drawn only from feature 007's tokens, with the chrome,
one frame per plugin the development tool approved (label and monogram made safe, the render's
outcome as host status, no content), the panel a frame opens, the appearance settings with the
30° rule, and "Remove from dashboard" as the first host dialog. The dialog is a modal layer over
every host window, with the input gate, focus trap and refusal paths of feature 007's criteria,
and it is marked modal for assistive technology by the host itself, since GPUI has no builder for
it (R9, R10). The rules that replace a web view's content security policy are enforced by a
blocked HTTP client, clippy bans on GPUI's image loader, URL opener and native prompts, and an
image path that takes bytes only (R12). Tests run in four layers, two of them in a real window on
Windows, macOS and Linux (R13). Decision 0001 is revisited: the runtime stays in process, and no
plugin byte reaches a decoder (R2).

## Technical Context

**Language/Version**: Rust (edition 2024, stable as CI builds it; the workspace declares no MSRV,
research R15)

**Primary Dependencies**: the nineteen gpui-unofficial crates at `=1.25.3` (Apache-2.0, R1);
`runtime`, `design-tokens`, `host-surfaces` (workspace); `serde_json` for the settings file;
dev-dependencies for the real-window target: `enigo` 0.6.1 (key events), `atspi` (Linux),
`windows` (UI Automation, Windows), `objc2-application-services` (AX, macOS), `accesskit_consumer`
0.38 (the adapters' filter). Code adapted from gpui-base 0.9.0's focus trap, with its notice (R9).

**Storage**: the host's appearance file, `<data dir>/dev.philsantos.dashboard/host/appearance.json`
(R11); the development grant record and data directory as the runtime already keeps them.

**Testing**: `cargo test -p host` (logic layer with GPUI's test platform, the source scans, the pin
and ban checks); the `window` test target with `harness = false` in a real window (keys and the
accessibility tree, R13); `host/tests/hostile/` in the security suite; `cargo deny`, `cargo audit`;
the screen-reader walk before each release.

**Target Platform**: Windows, macOS and Linux desktops; CI on `windows-latest`, `macos-latest`
and `ubuntu-24.04` (Xvfb, Mesa's software Vulkan, an AT-SPI bus).

**Project Type**: desktop application (a new binary crate in the existing Cargo workspace) plus
one read-only addition to the runtime's grant store.

**Performance Goals**: the window with 5 approved plugins drawn within 2 s of start on the
reference machine (SC-001); a host dialog on screen within 100 ms of being asked (SC-006);
measured in the real-window target and recorded in `performance.md` (R14).

**Constraints**: no web view, HTML, script or style sheet; no native prompt; no plugin string to
GPUI's image loader or URL opener; a blocked HTTP client; no plugin image, icon or font; no
change of behavior in the runtime, permission engine, connectors, vault or WIT (FR-033); every
host string from the catalog (FR-039); every color, size, radius and line from 007's tokens.

**Scale/Scope**: one main window (a second only in tests); a handful of approved development
plugins; one dialog type in use ("Remove from dashboard") through a component every later dialog
will use.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle or decision | Status | How this plan meets it |
| --- | --- | --- |
| I. WebAssembly-only plugins | Pass | Plugins load through the existing runtime only; the window adds no other way to run plugin code. |
| II. Zero capabilities by default | Pass | The window grants nothing; it reads approvals (R5) and its one dialog only removes an approval, through the same calls as `runtime remove`. |
| III. External data only through host-run connectors | Pass | GPUI's HTTP client is blocked (R12); the window opens no connection; `open_url` is banned. The runtime runs without development mode, so local connectors stay the development tool's only (R5; 003 FR-018). |
| IV. Plugins never see secrets | Pass | The window touches no credential; GPUI's keychain calls are disallowed (R9). |
| V. Plugins stay inside the host frame | Pass | The chrome is above a clipping content area, so no frame or panel can cover it (R6, HS-01); a plugin's strings appear only as the safe label (R8), under the host's words; no content drawn yet. |
| VI. Human confirmation for AI actions | N/A | No AI in this feature. |
| VII. Trust decisions only in host dialogs | Pass | The dialog's form (cover, block, focus, AT) is built and tested (R9, R10, HS-02, HS-10 to HS-12, HS-16 to HS-19); native prompts are banned by clippy and replaced by a refusing builder; the development tool keeps its role until installation brings the grant dialogs: the window reads its record and can only withdraw an approval, never grant one (R5). |
| VIII. Every security rule has a breaking test | Pass | Each CSP rule and each form rule has a hostile case (FR-031, R12); bans are checked by tests that read `clippy.toml`, `Cargo.toml` and `Cargo.lock`. |
| IX. Human review of the security core | Pass, with review | `GrantStore::approvals()` is in the permission engine, and `deny.toml` (licenses for GPUI's graph) and `clippy.toml` (which keeps the network, vault and storage code in their modules, and now GPUI's bans) are reviewed line by line. The dialog and CSP modules of `host` are not listed as core by the constitution; the PR asks for the same review of them. |
| Fixed: host app | Pass | gpui-unofficial, every crate direct and `=`, from stable tag v1.25.3, Apache-2.0 only, patch list recorded and checked (R1). |
| Fixed: UI | Pass | Drawn only from 007's generated tokens; the host's fonts embedded (R4). |
| Fixed: licenses | Pass, with review | Client Apache-2.0; copied gpui-base code keeps its Apache-2.0 notice (R9); new licenses in GPUI's graph are added to `deny.toml` one by one (R15). |
| Decision 0001 | Revisited | R2: in process for this feature; no plugin byte reaches a decoder; next trigger unchanged. |

No violation needs justifying. Re-checked after the design below: unchanged.

## Project Structure

### Documentation (this feature)

```text
specs/008-host-window-dialogs/
├── plan.md              # This file
├── research.md          # Phase 0 (R1–R15)
├── data-model.md        # Phase 1
├── quickstart.md        # Phase 1
├── contracts/
│   ├── host-binary.md          # the dashboard binary, its files and exit codes
│   ├── dialog.md               # the dialog component: request, answer, form, gate, focus, a11y
│   └── bans-and-checks.md      # clippy bans, pin check, catalog scan, hostile cases
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
host/ban-fixture/                  # outside the workspace: calls every banned GPUI method, must fail clippy
host/                              # new workspace member: library `host`, binary `dashboard`
├── Cargo.toml                     # nineteen gpui-unofficial crates at =1.25.3 (R1)
├── README.md                      # the GPUI pin, its Zed tag and the patch list commit reviewed
├── src/
│   ├── main.rs                    # the binary: start, refuse clearly without a GPU (FR-013)
│   ├── lib.rs
│   ├── app.rs                     # fonts (R4), blocked HTTP client and prompt builder (R9, R12)
│   ├── theme.rs                   # design-tokens and host-surfaces to GPUI values
│   ├── text.rs                    # Msg catalog, HostText (R7)
│   ├── label.rs                   # PluginText, label and monogram rules (R8)
│   ├── chrome.rs
│   ├── frame.rs                   # header, monogram, label, status, menu
│   ├── panel.rs                   # attached panel inside the content area (R6)
│   ├── dialog/                    # DialogHost, queue, modal layer, gate, focus (R9)
│   ├── a11y.rs                    # set_modal / set_hidden through write_a11y_info (R10)
│   ├── appearance.rs              # settings screen and appearance.json (R11)
│   ├── plugins.rs                 # approvals, load, render off the UI thread (R5)
│   ├── media.rs                   # the only path to gpui::img, bytes only (R12)
│   └── vendor/gpui_base/          # focus trap adapted from gpui-base 0.9.0, NOTICE, PROVENANCE
└── tests/
    ├── logic.rs                   # GPUI test platform: gate, queue, focus, HS-01 bounds, files
    ├── checks.rs                  # pins, clippy bans, catalog scan, deny wrappers
    ├── hostile/                   # one case per CSP and form rule (FR-031)
    └── window.rs                  # harness = false, test = false: real input and the tree (R13)

runtime/src/grants.rs              # + GrantStore::approvals(), read-only (R5)
plugins/hostile/                   # + fixtures whose output and names carry addresses, markup, bidi
docs/decisions/0001-plugin-process-isolation.md   # + the revisit of R2
docs/threat-model.md               # decoder row's mitigation and the process question (R16)
docs/design/host-surfaces.md       # "Decided elsewhere": the decoder entry's next owners (R16)
docs/accessibility/screen-reader-walk.md          # the release walk script (FR-026)
clippy.toml                        # + GPUI bans (R9, R12)
deny.toml                          # + licenses of GPUI's graph, each reviewed (R15)
.github/workflows/ci.yml           # + a "Host window" job on three systems: Linux packages, AT-SPI bus, Xvfb (R13)
Cargo.toml                         # + host in members
```

**Structure Decision**: one new crate in the existing workspace, as features 001–007 kept one
workspace; the runtime changes by one read-only function; the CI keeps its jobs and adds steps.

## Complexity Tracking

None: the Constitution Check has no violation.
