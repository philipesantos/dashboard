# Implementation Plan: Plugin Storage

**Branch**: `004-plugin-storage`, merged into `master` through one pull request, reviewed commit by
commit | **Date**: 2026-10-24 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/004-plugin-storage/spec.md`

## Summary

Give each plugin a store of named values of its own, and no way to reach anyone else's. Storage
is a permission, `storage`, with a quota declared on the same manifest entry and approved with
the plugin's approval digest. When the plugin is linked, the host binds the store from its id and
origin; no call names a store. Plugins see a key-value interface (`get`, `set`, `delete`, `list`,
`usage`) that the host runs with its own prepared statements on one SQLite database per plugin
(`rusqlite` with its bundled SQLite). Every connection is hardened layer by layer: no attaching,
no extensions, defensive mode, no triggers or views, and an authorizer that allows only the host's
statements. Render reads one consistent view on a read-only connection; update writes in one
transaction, committed only after it returned `ok`, under the grant record's lock while storage is
still granted; a transaction SQLite ends on its own (a full file) ends the run's writes. Usage
counts names, values and a fixed cost per entry, the same on every system, and a file cap of five
times the approved quota bounds the disk. Contract v1 moves to 1.2.0 (1.1.0 frozen first), the grant record to version 3, and the CLI
gains `storage list`, `show` and `clear`. Spikes and decisions are in [research.md](research.md).

## Technical Context

**Language/Version**: Rust (edition 2024, stable; MSRV 1.97 because of Wasmtime 50)

**Primary Dependencies**: in the runtime, as in feature 003, plus `rusqlite` `=0.41.0`
(no default features; `bundled`, `hooks`, `limits`) and `libsqlite3-sys` `=0.39.0`, which builds
SQLite 3.54.0 from source with `cc`, checked at startup (R1)

**Storage**: plugin stores, one SQLite file each, in `<data dir>/storage/` (R3, R5); the
development grant record moves to version 3 (R12); kept data and the request log unchanged

**Testing**: `cargo test` (runtime integration tests in `runtime/tests/`), hostile cases packing the
`store-probe` fixture; CI on ubuntu-24.04, macos-latest and windows-latest, no internet needed

**Target Platform**: host on Windows, macOS and Linux; plugins on `wasm32-unknown-unknown`
(`storage-file` on `wasm32-wasip2`, as the other WASI cases)

**Project Type**: Rust library with a CLI binary, inside the existing Cargo workspace

**Performance Goals**: SC-006: each storage call under 1 ms, grant check included, and a render
reading 100 values within render's limits (R9, R16)

**Constraints**: plugins never send SQL and never name a store (Principle II, FR-002 to FR-004);
grants change only through the development CLI (Principle VII); everything under `runtime/` and
`wit/` gets line-by-line review (Principle IX)

**Scale/Scope**: one storage module, contract 1.2.0 with 2 new worlds, grant record v3, 1 new
error kind and 8 call rules, 3 new CLI subcommands (`storage list|show|clear`) and storage in
`permissions`, `approve`, `optional`, `update` and `remove`, a `storage_cost` bench binary, 23 hostile cases (2 white
box) and 8 other named tests, 1 example, 1 fixture and 1 hostile crate

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Checked against constitution v2.3.1, after an independent review (findings in
[checklists/requirements.md](checklists/requirements.md)).

| Rule | Status | How the plan complies |
| --- | --- | --- |
| I. WebAssembly-only plugins | Pass | Plugins stay components; storage is a host interface, and SQLite runs in the host, never in or for plugin code |
| II. Zero capabilities by default | Pass | `storage` is a permission in the registry, declared and approved; the import is linked real only when granted, to the stand-in when optional and off, and refused when undeclared. The store binding is built at link time, like the connector table; call-time checks (the grant record re-read on every call) only take storage away (R9). A larger quota is a new approval (R12) |
| III. External data only through connectors | Pass (N/A) | Storage reaches no network; nothing in it opens a connection |
| IV. Plugins never see secrets | Pass | Storage holds only what a plugin wrote; plugins still receive no credential. The author guide says storage is not for secrets |
| V. Plugins stay inside the host frame | Pass (N/A) | No UI |
| VI. Human confirmation for AI actions | Pass (N/A) | No AI |
| VII. Trust decisions only in host dialogs | Pass | Approvals, quotas and turning storage on or off go through the development CLI, on local files and the development record; no plugin can read or change its grants, its quota or another store (R3, R12) |
| VIII. Every security rule has a breaking test | Pass | 23 hostile cases, two of them white box (hardening layers read back and tried on the production setup, binding), and 8 other named tests, on three systems; mid-run grant changes are tested without a race, the commit-time check included (R17); the cases of features 001 to 003 keep running, 1.0.0 and 1.1.0 plugins on the 1.2.0 host |
| IX. Human review of the security core | Pass | The storage module, the engine and record changes live in `runtime/`, the contract and schema in `wit/`; Clippy keeps `rusqlite` inside `runtime/src/storage/`; `rusqlite` and `libsqlite3-sys` are pinned exactly in `runtime/Cargo.toml`, so the bundled SQLite moves only there, and the PR template lists both under `Cargo.lock` and names the new fixture (R1, R15) |
| Fixed decisions: host app, UI | Pass (N/A) | No change to the Tauri shell or the UI |
| Fixed decisions: Wasmtime + Component Model (WIT) | Pass | Contract 1.2.0 is WIT; minors only add, checked by a test, and the frozen versions are pinned by hash (R13) |
| Fixed decisions: WASI 0.3 | Pass (N/A) | Still no WASI interface offered; `wasi:filesystem` refused (`storage-file`) |
| Fixed decisions: JS/TS, Rust, Go and Python plugins | Pass | The example and fixtures are Rust on `wasm32-unknown-unknown` with wit-bindgen, `storage-file` on `wasm32-wasip2` as the other WASI cases; nothing changes for other languages |
| Fixed decisions: secrets | Pass (N/A) | No secret is stored; the `keyring` decision is untouched |
| Fixed decisions: plugin storage | Pass | SQLite, one database per plugin (R1, R3); the host's own records stay separate files no plugin can reach (FR-013) |
| Fixed decisions: specifications | Pass | This feature goes through Spec Kit |
| Fixed decisions: licenses | Pass | The new tree is MIT, Apache-2.0 or Zlib, all already allowed (R1) |
| Workflow: gates, nothing visible on the sandbox, CI on 3 systems | Pass | No UI; this feature delivers the gate's storage case; `wit/*.json` and the plugins are code for the docs-only rule, so their changes run the whole CI |
| Workflow: Spec Kit and the Constitution Check | Pass | specify, clarify, plan, this check with an independent review, then tasks |
| Governance | Pass | No conflict needs an amendment; no exception is taken |

**Post-design re-check**: Pass, after an independent review against v2.3.1. It found 3 conflicts
(a write that fills the file let SQLite end the update's transaction and commit later writes one
by one; a `Cargo.lock` update alone could change the bundled SQLite; no load step for an
unavailable data directory), 13 risks and 11 consistency gaps. Each was reproduced in a spike or
read in the sources before the plan changed (R1, R4, R6 to R12, R14, R15, R17), and the maintainer
chose the file cap (five times the quota, R10) and which quota an approval keeps (the last shown,
R12).

## Project Structure

### Documentation (this feature)

```text
specs/004-plugin-storage/
├── plan.md              # This file
├── research.md          # Phase 0: decisions and spike results
├── data-model.md        # Phase 1: manifest field, binding, store file, usage, calls, record v3, cases
├── quickstart.md        # Phase 1: validation guide
├── contracts/
│   ├── cli.md                   # CLI commands, output and exit codes
│   ├── widget-v1.2.wit          # Draft of wit/v1/widget.wit (1.2.0)
│   └── manifest.schema.json     # Draft of wit/manifest.schema.json, with quota_bytes
├── checklists/requirements.md
├── performance.md       # Created during implementation (SC-006, R10, R16)
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
wit/                            # Security core: contracts
├── README.md                   # + storage, contract 1.2.0
├── manifest.schema.json        # + quota_bytes on storage
├── v1/widget.wit               # 1.2.0: storage, worlds stored-widget and connected-stored-widget
└── history/1.1.0/widget.wit    # New: published 1.1.0, frozen

runtime/                        # Security core
├── Cargo.toml                  # + rusqlite =0.41.0 (bundled, hooks, limits), libsqlite3-sys =0.39.0
├── src/
│   ├── storage/
│   │   ├── mod.rs              # Store binding, run state, the five calls, commit (R3, R6, R7, R9)
│   │   ├── connection.rs       # The only rusqlite use: opening, hardening, schema check, statements (R4, R5, R8)
│   │   └── admin.rs            # list, show, clear for the CLI (R14)
│   ├── lib.rs                  # Binding at link time; update commits storage before kept data
│   ├── host.rs                 # + storage real and stand-in
│   ├── manifest.rs             # + quota_bytes, storage at most once
│   ├── permissions.rs          # + storage in the registry; 1.2.0; storage needs 1.2 and updater
│   ├── grants.rs               # Record v3, approval digest with storage, quota at load and on
│   ├── text.rs                 # + quoting for `storage show`: `"` and `\` escaped (R14)
│   ├── limits.rs               # + storage limits (R10)
│   ├── error.rs                # + storage-failed
│   └── bin/runtime.rs          # + storage list|show|clear; storage in permissions, approve, optional, update, remove
└── tests/
    ├── storage.rs              # Calls, usage, quota, render read-only, commit, concurrency (US1, US3, US4)
    ├── permissions.rs          # + storage approvals, quotas, optional, taken away (US3, US5)
    ├── contract.rs             # + 1.0.0 and 1.1.0 plugins on the 1.2.0 host; newer minor refused (US6)
    ├── contract_v1.rs          # + 1.2.0 is a superset of 1.1.0; storage answers not-granted
    ├── manifest_schema.rs      # + quota_bytes agreement
    ├── cli.rs                  # + storage commands and output
    ├── hostile.rs              # + mod storage
    └── hostile/storage.rs      # The 23 cases of FR-017

bench/src/bin/storage_cost.rs   # New: storage call costs and file sizes (R16)

plugins/
├── Cargo.toml                  # + the new crates
├── examples/update-counter/    # New: contract 1.2.0, stored-widget (FR-018)
├── fixtures/store-probe/       # New: makes the storage calls its configuration lists (R17)
├── fixtures/net-probe/, examples/github-releases/  # Build from wit/history/1.1.0
└── hostile/storage-file/       # New: wasm32-wasip2, opens a file with std::fs

docs/plugin-authors.md          # + storage, quota, read-only render, not for secrets, contract 1.2.0
docs/threat-model.md            # + storage threats and their tests
SECURITY.md                     # Scope names plugin storage
plugins/hostile/README.md       # + storage cases
.github/pull_request_template.md  # Cargo.lock: + rusqlite, libsqlite3-sys; tests: + plugins/fixtures/store-probe
clippy.toml                     # rusqlite only inside runtime/src/storage/ (R15)
```

**Structure Decision**: everything stays in the existing crates. Storage gets a module of its own,
whose `connection.rs` is the only code that touches SQLite, so the review of that file covers every
statement the host runs and every hardening layer. The 1.1.0 plugins (`github-releases`,
`net-probe`) move to build from `wit/history/1.1.0`, as 1.0.0 ones build from `wit/history/1.0.0`,
so the suite keeps running them as 1.1.0 plugins on the 1.2.0 host.

## Complexity Tracking

No constitution violations to justify.
