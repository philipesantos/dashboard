# Implementation Plan: Plugin Installation and the Installation Dialog

**Branch**: `010-plugin-installation` | **Date**: 2026-11-12 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/010-plugin-installation/spec.md`

## Summary

The app installs widgets from their authors' GitHub repositories. The runtime gains an installer:
it learns the repository's permanent identifier, then fetches the latest release and its one
`.wasm` asset by that identifier through a new host GET in its network module, checks the manifest
id against the repository and the version against the tag, and compiles the package in a separate
process, the app's own binary in a compile-only mode, confined by the operating system (no file,
network, keychain or other process) and limited in time, memory, threads and priority. The
compiled form is kept beside the package, content-addressed, and deserialized only from bytes
whose recorded digest matches. The runtime builds the dialog's model, with host-written Portuguese
sentences and a dangerous-combination rule shared with the CLI that also remembers account
sources a store may hold; the page draws it in one `HostDialog` that meets HS-10 to HS-15 and
HS-17 to HS-23, and every approval echoes a digest of exactly what was shown, optional choices and
warning included. Updates are fetched by permanent identifier only. Every installed frame shows its
repository owner. The window gains five reviewed commands, and the app's grant record moves to
disk. Decision 0001 is reviewed in 0003: compile out of process now; run out of process in the
next feature, before the alpha. Constitution v2.7.0 keeps the development tool granting to local
packages and adds the installer and host dialogs to the security core.

## Technical Context

**Language/Version**: Rust stable, edition 2024 (workspace); TypeScript and React 19 in `app/`.

**Primary Dependencies**:
- the `runtime` crate: new modules `install` (GitHub source, records, dialog model), `compile`
  (child, parent, confinement), `flows` (the warning rule); a host GET in `network/`;
- `ureq` 3.5, `rustls` 0.23, `sha2`, `semver`, `tempfile`, `libc` (Unix): already runtime
  dependencies;
- `windows-sys` 0.61, already in the lock through Wasmtime, becomes a direct Windows dependency
  (job objects, AppContainer, priority);
- new crates, Linux only: `seccompiler` (rust-vmm) and `landlock`, under `cargo deny` (research
  R13);
- the existing Radix dialog in `app/src/components/ui/dialog.tsx`.

**Storage**: `<app data dir>/host/`: `grants.json` (now on disk, record version 5),
`installed.json`, `packages/<digest>/{package.wasm, compiled.cwasm}` (research R5).

**Testing**: `cargo test --workspace` (research R11's cases, an in-memory release source, the real
compile child spawned through the runtime's CLI binary, the confinement test on each system), and
the shell probe with the dialog cases and a probe-only release source, on the three systems. What
untrusted events cannot show is in quickstart §6 (research R11's table).

**Target Platform**: Windows (WebView2), macOS (WKWebView), Linux (WebKitGTK under the
constitution's fallback).

**Project Type**: desktop app with a Rust host library and a Wasm plugin contract.

**Performance Goals**: SC-001's ceiling is 1 minute from address to first render; the expected
time for a widget under 1 MiB on a normal connection is under 10 s, and the probe's
`install-on-time` holds the network-free part under 10 s. The window answers input within 100 ms
while the `compile-heavy` fixture compiles (SC-003, probe case
`window-responsive-while-compiling`).

**Constraints**:
- Compile limits: 20 s wall, 60 s CPU, 1.5 GiB memory as each system counts it, half the cores,
  below-normal priority; a CI test compiles `compile-heavy` through the real child on the three
  systems and prints its time and peak (research R2).
- GitHub: 10 s and 256 KiB per API answer, 60 s for the package; 3 requests per installation, 2
  or 3 per update check, against 60 an hour unauthenticated (research R4).
- The dialog's input delay: 1000 ms, restarted on focus and on input during it (research R8).
- The window's content policy and feature 008's probe stay as they are; development records,
  vault and mode are never used by the app.
- Test release sources never build into a release app (research R16).

**Scale/Scope**: 5 window commands; 12 runtime error kinds and 5 app refusals; 1 decision record;
1 constitution amendment; about 45 cases below the page and 17 probe cases; 1 new author guide.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.* Checked against
constitution v2.7.0, after the independent review of this plan (2026-11-12), whose findings are
folded into research R1 to R16; one of them led to the v2.7.0 amendment.

| Principle or rule | Status | How |
| --- | --- | --- |
| I. WebAssembly-only plugins | Pass | Installed packages are Wasm components compiled by Wasmtime; the compile child runs no plugin code (R2). |
| II. Zero capabilities by default | Pass | Nothing is granted before the dialog's approval; an installation grant is built fresh, optionals off unless turned on in the dialog (R5); grants are bound to `github:<permanent id>`. |
| III. External data only through connectors | Pass | Plugins gain no network; the host's requests to GitHub are fixed in shape and hosts (R4). Before every approval, updates included, the dialog shows every destination and the flows from accounts, remembered ones included (R6, R7). Repository and local connectors stay refused (R9). |
| IV. Plugins never see secrets | Pass | No credential on GitHub requests; account connectors install without an account (FR-024); the compile child cannot reach the keychain (R13). |
| V. Plugins stay inside the host frame | Pass | The dialog is host-drawn outside every frame in host-only roles; HS-16 stays; the frame header gains the owner, host-drawn (R14). |
| VI. Human confirmation for AI actions | Pass | No AI in scope. |
| VII. Trust decisions only in host dialogs | Pass | Every grant goes through `request_approve` on a request the host opened from a chrome control, with the choice digest of what was shown (R7, R10). The development tool grants only to local packages in its own record (v2.7.0). |
| VIII. Every security rule has a breaking test | Pass | R11 lists a case per rule and criterion, and says which steps of HS criteria only the review reaches, as FR-026 allows. |
| IX. Human review of the security core | Pass | Security core (v2.7.0): `runtime/src/{install,compile,flows}`, `network/`, `grants.rs`, `permissions.rs`; `app/src-tauri/src/{main,install,widgets,commands}.rs`, `capabilities/`, `window-surface.toml`; `app/src/host/*`, `app/src/components/ui/dialog.tsx`, `app/src/widgets/Frame.tsx`; the records' formats. Reviewed line by line. |
| Fixed decision: Rust + Tauri 2, React UI | Pass | Unchanged. |
| Fixed decision: Wasmtime + Component Model | Pass | Parent and child share one `engine_config()`; Wasmtime refuses any mismatch (R2). |
| Fixed decision: plugin storage, one SQLite database per plugin | Pass | Unchanged; stores keyed by id and the new origin. |
| CI on three systems, audits | Pass | New tests run in the existing jobs; new crates go under `cargo deny` and `cargo audit`. |
| Decision 0001 (plugins in the host process) | Pass, review recorded | 0003: compiling leaves the app's process now; running leaves it in the next feature, before the alpha (R1, maintainer's decision). |

**Re-check after Phase 1 design**: passes. Design inputs that change first: the spec (FR-009,
FR-010, FR-014, FR-016, FR-020, FR-022, FR-026, FR-027, FR-029, FR-030; User Story 3), constitution
v2.7.0 (R15) and criterion HS-02 (R14).

## Project Structure

### Documentation (this feature)

```text
specs/010-plugin-installation/
├── plan.md
├── research.md          # R1–R16 and the review's outcomes
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── window-commands.md
│   └── release.md
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks
```

### Source Code (repository root)

```text
.specify/memory/constitution.md   # v2.7.0 (its own commit)
runtime/
├── src/install/                # mod.rs (installer, requests), github.rs (ReleaseSource), record.rs (installed.json), dialog.rs (DialogModel, sentences, choice digest)
├── src/compile/                # mod.rs (Compiler, limits, attribution), child.rs, confine_{linux,macos,windows}.rs
├── src/flows.rs                # the warning rule, remembered sources
├── src/network/mod.rs          # host GET, the installer's redirect rule
├── src/grants.rs               # record v5 (account_sources), GrantStore::install
├── src/lib.rs                  # engine_config(), load_compiled
├── src/permissions.rs          # permission enum in place of the string registry
├── src/text.rs                 # the name sanitiser, moved from the app, also used for the author
├── src/error.rs                # new kinds
├── src/cli/                    # describe prints DialogModel; optional runs judge; --compile-package
└── tests/                      # install.rs, compile.rs, flows.rs, dialog_model.rs (listed in it.rs)
xtask/                           # compile-heavy fixture (generator moved from bench)
bench/                           # compile_cost uses the shared generator
plugins/fixtures/                # install fixtures: optional connector, account connector, storage
app/
├── src/host/                   # HostDialog.tsx, InstallDialog.tsx, AddWidget.tsx, api.ts
├── src/widgets/                # Widgets.tsx refresh, Frame.tsx owner and update control
├── src/components/ui/dialog.tsx  # dialog roles in place of popover
├── src/probe/dialogs.ts        # HS cases, frame-owner-shown; widgets.ts foreign-actions-refused
└── src-tauri/
    ├── src/main.rs             # --compile-package before Tauri
    ├── src/install.rs          # requests and the five commands' bodies
    ├── src/widgets.rs          # installed widgets, lock-guarded list, records on disk, owner
    ├── src/name.rs             # calls the runtime's sanitiser
    ├── src/commands.rs, capabilities/default.json, window-surface.toml
    └── tests/                  # install.rs (records, crash steps, refusals, embedded sources)
.github/workflows/ci.yml        # compile-heavy measured under default limits; confinement test on three systems
docs/
├── decisions/0003-compile-installed-plugins-out-of-process.md, 0001 note
├── authors/guides/publishing.md, reference/errors.md, reference/manifest.md
├── design/host-surfaces.md      # HS-02: the owner in installed frames
└── threat-model.md              # rows 113, 152, 153, 155, 157, 158, 175; B6; new residual rows; open questions closed
```

**Structure Decision**: the installer lives in the runtime crate, because HTTP may only be made in
`runtime/src/network/` (`clippy.toml:16-60`) and the CLI must print the same dialog model; the app
crate owns requests, records on disk and commands, in a module beside `widgets.rs` so the security
core's review reads them together. The compile child is the app's own binary, so it always has the
app's Wasmtime; confinement code is one file per system so each review reads one mechanism.

## Complexity Tracking

| Violation | Why Needed | Simpler Alternative Rejected Because |
|-----------|------------|-------------------------------------|
| A second process for compiling | Closes the compile-cost threat row and decision 0001's trigger | Compiling in the app: no limit applies to Cranelift's time or memory |
| Per-system confinement and limit code | Each system confines and limits a process differently; macOS enforces no memory rlimit | Limits only: a compiler bug would run with the user's rights (maintainer chose confinement) |
| Two new Linux crates (`seccompiler`, `landlock`) | A seccomp filter and a Landlock ruleset written by hand as raw BPF and syscalls are harder to review | Hand-written filters: more security-core code to review line by line |
