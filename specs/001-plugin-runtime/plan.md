# Implementation Plan: Plugin Runtime

**Branch**: `001-plugin-runtime`, merged into `master` through one pull request, reviewed
commit by commit | **Date**: 2026-10-21 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/001-plugin-runtime/spec.md`

## Summary

Build the `runtime` crate: it loads plugins packaged as WebAssembly components, refuses any
import other than the v0 `log` capability, and runs each `render` call in a fresh Wasmtime
instance under fuel, epoch-deadline and memory limits. A CLI (`runtime run`, `runtime bench`)
exercises it. Plugins, including a Rust example and eleven hostile cases, are built from source
by `cargo xtask plugins`. A hostile-plugin test suite runs in CI on Windows, macOS and Linux and
is the phase gate. Spikes behind every decision are in [research.md](research.md).

## Technical Context

**Language/Version**: Rust (edition 2024, stable; MSRV 1.97 because of Wasmtime 50)

**Primary Dependencies**: `wasmtime` 50 (Component Model, Cranelift) and `serde_json` in the
runtime; `wit-bindgen` 0.63 in plugins; `wit-component` 0.262 in the xtask

**Storage**: N/A

**Testing**: `cargo test` (runtime integration tests in `runtime/tests/`), plus CI on
ubuntu-24.04, macos-latest and windows-latest

**Target Platform**: host on Windows, macOS and Linux; plugins on `wasm32-unknown-unknown`
(hostile file and socket cases on `wasm32-wasip2`)

**Project Type**: Rust library with a CLI binary, inside the existing Cargo workspace

**Performance Goals**: set by User Story 4 in [performance.md](performance.md), on every CI
system: load a plugin up to 100 KB under 100 ms; render, instantiation included, under 200 µs
at p50 and 1 ms at p95; load a package near the 10 MiB cap under 2 s and 1 GiB.

**Constraints**: only `dashboard:widget/log@0.1.0` may be imported; per-call limits as in
[research.md](research.md#r9-default-limits); no WASI linked;
runtime code gets line-by-line human review (Principle IX)

**Scale/Scope**: one contract (one export, one import), one example plugin, eleven hostile
plugins, one CLI with two commands

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Checked against constitution v1.1.0.

| Rule | Status | How the plan complies |
| --- | --- | --- |
| I. WebAssembly-only plugins | Pass | `Runtime::load` accepts only WebAssembly components; anything else is `invalid-package` |
| II. Zero capabilities by default | Pass | The linker holds only `log`; an explicit import allowlist refuses everything else before linking (R4) |
| III. Network only through the proxy | Pass | No network in v0; socket imports are refused and tested (`open-socket`) |
| IV. Plugins never see secrets | Pass (N/A) | No secret or credential interface exists in v0 |
| V. Plugins stay inside the host frame | Pass (N/A) | No UI in this feature |
| VI. Human confirmation for AI actions | Pass (N/A) | No AI in this feature |
| VII. Trust decisions only in host dialogs | Pass (N/A) | The CLI is a developer tool; there is no install or permission flow yet |
| VIII. Every security rule has a breaking test | Pass | Eleven hostile cases in `runtime/tests/hostile.rs`, run on three systems in CI; a missing fixture fails, never skips |
| IX. Human review of the security core | Pass | Everything under `runtime/` and `wit/` is security core; its pull requests need line-by-line review |
| Fixed decisions: Rust host, Wasmtime + Component Model | Pass | Wasmtime 50 with WIT contracts |
| Fixed decisions: WASI 0.3 | Pass | v0 links no WASI interface; the decision applies when the first one is offered (R5) |
| Fixed decisions: Rust plugins | Pass | cargo + wit-bindgen + wit-component, per the v1.1.0 amendment (R1) |
| Workflow: gates, nothing visible on the sandbox, CI on 3 systems | Pass | No UI; the hostile suite is the gate; CI gains the plugin build and suite |

**Post-design re-check**: Pass. The design adds a separate `plugins/` workspace and an `xtask`
crate; neither touches a rule. Every new dependency must pass `cargo deny` (licenses, sources)
and `cargo audit` in both workspaces.

## Project Structure

### Documentation (this feature)

```text
specs/001-plugin-runtime/
├── plan.md              # This file
├── research.md          # Phase 0: decisions and spike results
├── data-model.md        # Phase 1: host types, error kinds, hostile cases
├── quickstart.md        # Phase 1: validation guide
├── contracts/
│   └── cli.md           # CLI commands, output and exit codes (the plugin contract is wit/widget.wit)
├── performance.md       # Created during implementation (User Story 4)
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
wit/
└── widget.wit                  # Plugin contract v0; the only copy, used by host and plugins

runtime/                        # Security core (Principle IX)
├── Cargo.toml                  # wasmtime 50, serde_json
├── src/
│   ├── lib.rs                  # Runtime, Plugin, render, public API
│   ├── limits.rs               # Limits defaults and the memory limiter
│   ├── imports.rs              # Import allowlist check (R4)
│   ├── host.rs                 # log capability with per-call caps
│   ├── text.rs                 # Escaping and truncation of plugin-supplied text
│   ├── error.rs                # Runtime error kinds
│   └── bin/runtime.rs          # CLI: run, bench
├── tests/
│   ├── contract.rs             # Example plugin: User Stories 1 and 3
│   ├── cli.rs                  # CLI output and exit codes (contracts/cli.md)
│   └── hostile.rs              # Security suite: User Story 2, the phase gate
└── examples/hello_world.rs     # Existing Wasmtime spike

plugins/                        # Separate Cargo workspace for guest crates
├── Cargo.toml
├── Cargo.lock
├── examples/hello-widget/      # Rust example (FR-012)
└── hostile/
    ├── infinite-loop/
    ├── huge-alloc/
    ├── deep-recursion/
    ├── malformed-output/
    ├── oversized-output/
    ├── log-flood/
    ├── state-leak/
    ├── out-of-bounds/
    ├── log-spoof/
    ├── read-file/              # wasm32-wasip2, std::fs
    └── open-socket/            # wasm32-wasip2, std::net

xtask/                          # `cargo xtask plugins`: builds plugins/, writes target/plugins/
└── src/main.rs

docs/decisions/
└── 0001-plugin-process-isolation.md   # FR-016

.cargo/config.toml              # `xtask` alias
.github/workflows/ci.yml        # + wasm targets, plugin build, hostile suite, checks for plugins/
.github/workflows/bench.yml     # Run by hand: bench and compile cost on the 3 systems (US4)
```

**Structure Decision**: the runtime stays in the existing `runtime/` crate and the contract in
`wit/`, as the constitution fixes. Guest crates live in their own workspace under `plugins/`,
because they build for WebAssembly targets and must not be compiled for the host by
`cargo clippy --workspace`; the root workspace excludes `plugins/`. The `xtask` crate joins the
root workspace so the plugin build runs the same way on all three systems, without shell
scripts.

## Complexity Tracking

No constitution violations to justify.
