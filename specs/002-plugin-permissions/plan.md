# Implementation Plan: Plugin Permissions

**Branch**: `002-plugin-permissions`, merged into `master` through one pull request, reviewed
commit by commit | **Date**: 2026-10-22 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/002-plugin-permissions/spec.md`

## Summary

Give plugins capabilities only through a manifest they declare and grants the user approves.
A plugin package stays one WebAssembly component file; its manifest is a JSON document in one
top-level custom section, so neither can be swapped without changing the package. The runtime
reads and validates the manifest before compiling, checks the component's imports against what
the manifest declares and what the user granted, and builds a linker for each plugin that holds
only the granted capabilities, plus a stand-in that answers "not granted" for each optional
capability that is off. Grants live in a host file, outside every plugin's reach; any change
to them takes effect from the plugin's next call. Contract v1 (`dashboard:widget@1.0.0`) gives
every capability function a `not-granted` answer, and contract v0 plugins keep running. The
CLI gains commands to list, approve, turn optional permissions on and off, remove an approval
and pack a plugin. Spikes behind every decision are in [research.md](research.md).

## Technical Context

**Language/Version**: Rust (edition 2024, stable; MSRV 1.97 because of Wasmtime 50)

**Primary Dependencies**: in the runtime, `wasmtime` 50 and `serde_json` (as in feature 001),
plus `serde` with `serde_path_to_error` (manifest and grant record), `semver` (versions),
`sha2` (package digest), `wasmparser` (finding the manifest section), `dirs` (default grant
record path) and `tempfile` (atomic writes); `boon` and `wit-parser` as dev-dependencies; in
the xtask, `wasm-encoder` to add the manifest section. Every crate except `boon` and
`serde_path_to_error` is already in `Cargo.lock` (R12).

**Storage**: the development grant record, a JSON file in the user's data directory (R4)

**Testing**: `cargo test` (runtime integration tests in `runtime/tests/`), plus CI on
ubuntu-24.04, macos-latest and windows-latest

**Target Platform**: host on Windows, macOS and Linux; plugins on `wasm32-unknown-unknown`
(hostile file and socket cases on `wasm32-wasip2`, as in feature 001)

**Project Type**: Rust library with a CLI binary, inside the existing Cargo workspace

**Performance Goals**: SC-007: reading the manifest and checking the grants adds under 10 ms to
loading a plugin, on every CI system; re-linking after a grant change stays within the render
targets of feature 001 (R11)

**Constraints**: a plugin links only granted capabilities, or a stand-in for an optional one
that is off (Principle II); grants change only through the development CLI, on local packages
and the development record (Principle VII, constitution v1.3.0); everything under `runtime/`
and `wit/` gets line-by-line human review (Principle IX)

**Scale/Scope**: one capability (`log`) in two contract versions, one manifest format with its
JSON Schema, one grant record, nine refusal rules, five new CLI commands, one example plugin
moved to v1, three new test fixtures

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Checked against constitution v1.3.0.

| Rule | Status | How the plan complies |
| --- | --- | --- |
| I. WebAssembly-only plugins | Pass | Packages stay WebAssembly components; the manifest is a custom section, never code |
| II. Zero capabilities by default | Pass | A plugin without an approval does not run; each plugin's linker holds only its granted capabilities; an optional capability that is off gets only a stand-in that answers `not-granted` (R5); a new required permission needs a new approval, and an approval applies only to the file it was approved from (R8) |
| III. Network only through the proxy | Pass (N/A) | No network capability in this feature; socket imports stay refused |
| IV. Plugins never see secrets | Pass (N/A) | No secret interface; login providers are declared and shown only |
| V. Plugins stay inside the host frame | Pass (N/A) | No UI; a plugin's text about its permissions reaches only its own output |
| VI. Human confirmation for AI actions | Pass (N/A) | No AI in this feature |
| VII. Trust decisions only in host dialogs | Pass | Until Phase 3, grants change only through the development CLI, on local packages and the development record, as Principle VII allows (v1.3.0); no plugin can read, write or request them, and a hostile case checks that running a plugin leaves the record unchanged; the dialogs of Phase 3 replace the CLI for end users |
| VIII. Every security rule has a breaking test | Pass | One hostile case per rule of FR-016, in `runtime/tests/hostile.rs`, on three systems in CI; the feature 001 cases keep passing |
| IX. Human review of the security core | Pass | The permission engine, manifest parser and grant record live in `runtime/`, and contract v1 and the manifest schema in `wit/`: all security core |
| Fixed decisions: Wasmtime + Component Model (WIT) | Pass | Contract v1 is WIT; the manifest rides in the component itself |
| Fixed decisions: WASI 0.3 | Pass (N/A) | Still no WASI interface offered |
| Fixed decisions: Rust plugins | Pass | Same toolchain; the xtask adds the manifest section |
| Workflow: gates, nothing visible on the sandbox, CI on 3 systems | Pass | No UI; the hostile suite grows; CI is unchanged apart from new tests |

**Post-design re-check**: Pass. The design adds no crate to the workspace. New dependencies are
small and must pass `cargo deny` and `cargo audit` (R12). The stand-in is the only import
linked without a grant, exactly as Principle II now allows, and its tests check that it gives
nothing.

## Project Structure

### Documentation (this feature)

```text
specs/002-plugin-permissions/
├── plan.md              # This file
├── research.md          # Phase 0: decisions and spike results
├── data-model.md        # Phase 1: package, manifest, grants, load decision, error kinds, cases
├── quickstart.md        # Phase 1: validation guide
├── contracts/
│   ├── cli.md                   # CLI commands, output and exit codes
│   ├── package.md               # Package format: the manifest section
│   ├── manifest.schema.json     # Draft of wit/manifest.schema.json
│   └── widget-v1.wit            # Draft of wit/v1/widget.wit
├── performance.md       # Created during implementation (SC-007)
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
wit/                            # Security core: contracts
├── README.md                   # Which contract is where; how the manifest is published
├── manifest.schema.json        # Published manifest schema (FR-003)
├── v0/widget.wit               # Contract v0, moved from wit/widget.wit, unchanged
└── v1/widget.wit               # Contract v1 (FR-013)

runtime/                        # Security core (Principle IX)
├── Cargo.toml                  # + serde, serde_path_to_error, semver, sha2, wasmparser, dirs, tempfile
├── src/
│   ├── lib.rs                  # Runtime, Plugin, render; re-links on grant changes (R6)
│   ├── package.rs              # Finds and adds the manifest section; package digest (R1)
│   ├── manifest.rs             # Manifest parsing and validation (R2, R3)
│   ├── permissions.rs          # Permission registry, import check, load decision (R5)
│   ├── grants.rs               # Grant record: file, lock, atomic writes, generation (R4)
│   ├── host.rs                 # log for v0 and v1, the v1 stand-in, per-call state
│   ├── imports.rs              # Removed: replaced by permissions.rs
│   ├── limits.rs, text.rs      # Unchanged
│   ├── error.rs                # + the new error kinds
│   └── bin/runtime.rs          # CLI: run, bench, permissions, approve, optional, remove, pack
├── tests/
│   ├── contract.rs             # Example plugin on v1; a v0 plugin still runs (US1, US5)
│   ├── permissions.rs          # Approval, optional permissions, updates, next call (US1, US3, US4)
│   ├── manifest_schema.rs      # The schema and the parser agree; every repo manifest validates (SC-006)
│   ├── contract_v1.rs          # Every v1 capability function can answer not-granted (FR-013)
│   ├── cli.rs                  # CLI output and exit codes
│   └── hostile.rs              # Security suite: feature 001 cases + one per FR-016 rule

plugins/
├── Cargo.toml                  # + the new fixture crates
├── examples/hello-widget/      # Moved to contract v1; + manifest.json
├── fixtures/                   # Well-behaved test plugins, not examples for authors
│   ├── hello-v0/               # Contract v0 plugin that logs (FR-013)
│   ├── probe/                  # v1, logs and reports what `log` answered; built without a manifest
│   └── quiet/                  # v1, imports nothing; built without a manifest
└── hostile/*/                  # + manifest.json in each feature 001 case

xtask/src/main.rs               # Adds each crate's manifest.json as the manifest section

docs/plugin-authors.md          # First author documentation: manifest, packing, contract v1 (FR-017)
docs/threat-model.md            # + rows for two manifests and for a dev package claiming an id
```

**Structure Decision**: everything stays in the existing crates, as feature 001 laid them out.
The permission engine, manifest parser and grant record go into `runtime/`, because they decide
what a plugin can do and belong to the security core. Contracts move into versioned directories
under `wit/`, because `bindgen!` reads one package per directory and the host must bind v0 and
v1 at once. The manifest schema lives in `wit/` with the contracts, inside the security core.
Test plugins that are neither examples nor hostile go into a new `plugins/fixtures/`.

## Complexity Tracking

No constitution violations to justify.
