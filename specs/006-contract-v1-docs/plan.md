# Implementation Plan: Contract v1 Closed and Docs for Plugin Authors

**Branch**: `006-contract-v1-docs`, merged into `master` through one pull request, reviewed
commit by commit | **Date**: 2026-10-25 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/006-contract-v1-docs/spec.md`

## Summary

Close contract v1 and give plugin authors docs they can follow without the specs, kept true by
the build. First, a reviewer who did not write the contract checks every item of 1.3.0 against
the host and the docs; each finding is checked at the source and gets an outcome, and a finding
only a breaking change fixes goes to the maintainer, who may choose to ship it here as 2.0.0
with v1 retired (spec, Clarifications). Then the closed version is frozen under `wit/history/`
and pinned like the earlier minors. The docs move to `docs/authors/`: a walkthrough from an
empty folder outside the repository (the contract copied from its frozen version, the
development CLI installed from a clone, `runtime pack` now also turning the core module cargo
builds into a component, so the author needs no repository tooling and no other tool), a guide
per capability starting from its example, and a reference with every contract item, host
error, manifest and connector field, limit and author-facing CLI command. Two checks keep them
true: a drift test compares the reference's markers with what the contract, the schemas,
`ErrorKind` and `Limits` define, and a docs runner executes every marked block of the
walkthrough, the guides and the reference on Windows, macOS and Linux, comparing whole outputs
with named placeholders. Commands that reach a service run through a test-only build of the
same CLI whose network goes through 003's seam, so they run as written against the test
services, never GitHub. The runner gets a CI job of its own per system; the docs it reads count
as code for CI's documentation rule. Decisions and their sources are in
[research.md](research.md).

## Technical Context

**Language/Version**: Rust (edition 2024, stable; MSRV 1.97 because of Wasmtime 50); the docs
job and the walkthrough pin Rust 1.99.1 (R4)

**Primary Dependencies**: as in feature 005, plus `wit-component` `=0.262.0` in the runtime, the
release `xtask` uses and `wit-bindgen` 0.63.0 encodes with, for `pack` (R2); the author's crate uses `wit-bindgen`
`=0.63.0` and `serde_json` `=1.0.154`, the versions in `plugins/Cargo.lock` (R4)

**Storage**: N/A (no new record); the frozen contract under `wit/history/1.3.0/` (R13); the
review's findings in `specs/006-contract-v1-docs/review.md` (R12)

**Testing**: `runtime/tests/docs_reference.rs`, the drift test, in every `cargo test` (R7);
`runtime/tests/docs_walk.rs`, the docs runner, `#[ignore]`d and run with `--ignored` by the docs
job (R8); `contract_v1.rs` gains the 1.3.0 pin and the "v1 equals its frozen copy" check (R13);
CLI tests for `pack` on a core module: the same bytes as `xtask` for every example, and a module
whose names hold newlines and control characters refused on one escaped line (R2); a CLI test
that the real `runtime` ignores the test network's variable (R9); the hostile suite unchanged
(FR-024)

**Target Platform**: host on Windows, macOS and Linux; plugins on `wasm32-unknown-unknown`; the
docs runner's shells are bash on Linux and macOS and PowerShell 7 on Windows (R8)

**Project Type**: documentation, with a Rust library and CLI binary (runtime) and their tests,
inside the existing Cargo workspace

**Performance Goals**: the docs job finishes within 25 minutes on each system and runs in
parallel with the build job, so the run's wall time grows by at most the difference; measured on
the pull request's first run, with the walkthrough's release build of the runtime made without
LTO for the job (279 s from nothing, 149 s with dependencies cached under LTO, 16 s without
LTO, on 8 local cores; R3, R11);
SC-001: a timed walk by hand under 30 minutes

**Constraints**: no change to contract 1.3.0's definitions (doc comments may be corrected before
the freeze), to linking or to any security rule (FR-001, FR-024); the test-only CLI never builds
in release (003's guard), is never what an author installs, and opens connections only from
`runtime/src/network/` (R9); everything under `runtime/`
and `wit/` gets line-by-line review (Principle IX)

**Scale/Scope**: 13 pages under `docs/authors/` (index, walkthrough, 5 guides, 6 reference
pages); about 70 contract items, 36 error kinds, 44 limits, about 40 manifest and connector
fields, about 20 CLI commands; 2 new test files, 1 test-only binary, the CLI moved into a library
module, `pack` extended; 1 CI job on three systems; the walkthrough's crate kept under `docs/authors/getting-started/`
for `audit` and `deny`; 16 manifests lose `login_providers`

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Checked against constitution v2.4.1.

| Rule | Status | How the plan complies |
| --- | --- | --- |
| I. WebAssembly-only plugins | Pass | Plugins stay components; `pack` turns the author's core module into a component with `wit-component`, the fixed decision for Rust plugins, and the host checks it on load as before (R2) |
| II. Zero capabilities by default | Pass | No change to manifests' meaning, grants or linking; `pack` adds no import a module does not declare (the component imports what the module's embedded world names, checked at load) and `login_providers` still grants nothing (R2, R14) |
| III. External data only through connectors | Pass | No network change. The docs runner reaches only the test services, through the test-only CLI whose requests go through 003's seam; that binary needs the `test-network` feature, which refuses to build in release (the existing CI step), and is never installed. Its sign-in follower is in `runtime/src/network/`, so Clippy's rule that only that module opens connections holds with no new `#![allow]`; a CLI test shows the real `runtime` ignores the seam's variable (R9) |
| IV. Plugins never see secrets | Pass | The accounts guide runs the test-only CLI on the development vault in the system's keychain (each command is a process; the mock vault lives in one) with the test sign-in service; no real account or token in CI or in the docs, and the runner disconnects the account after the page (R9, R10) |
| V. Plugins stay inside the host frame | Pass (N/A) | No UI; the docs say the content format comes in Phase 3 (FR-014) |
| VI. Human confirmation for AI actions | Pass (N/A) | No AI in the product; the agent that walks the docs (SC-001) runs commands in a disposable container (R16) |
| VII. Trust decisions only in host dialogs | Pass | The docs show approval and account connection only through the development CLI, as v1.3.0/v2.4.0 allow; the test-only CLI's sign-in follows the test service's page instead of a browser, exists only in test builds and keeps its own data directory, never the record the real CLI reads (R9). The agent's walk runs `runtime approve` on its own package in a thrown-away container's development record: no person's trust decision is made for them (R16) |
| VIII. Every security rule has a breaking test | Pass | No rule added or removed; the suite passes unchanged (a 2.0.0 fix would rebuild cases, never drop one). The drift test and the runner fail the build for docs that lie (R7, R8) |
| IX. Human review of the security core | Pass | The CLI's move into `runtime::cli`, `pack`, `ErrorKind::ALL`, the test-only binary, the frozen contract and any host fix from the review are all security core, reviewed line by line. The CLI work is two commits: the code moved unchanged (`git diff --color-moved` shows the moved lines identical), then the runtime and the opener made parameters, reviewed in full (R9) |
| Fixed decisions | Pass | Wasmtime and WIT; Rust plugins with wit-bindgen on `wasm32-unknown-unknown`, componentized with wit-component (now in `pack`, R2). JS/TS stays jco/componentize-js: the docs say only that TypeScript arrives in Phase 5, naming no toolchain or engine, and Phase 5 amends the decision before building (FR-022, R15) |
| Workflow: CI on three systems, clippy, audit, deny | Pass | `cargo deny` and `cargo audit` cover `wit-component` and, through the crate kept under `docs/authors/getting-started/`, the dependencies the walkthrough builds on CI (R8, R11); the docs job runs on the three systems |
| Workflow: documentation-only rule | Pass | `docs/authors/` and `wit/`, the files the checks read, are used as code, so the `changes` job counts them as code and CI runs in full (FR-019, R11); other Markdown keeps the skip |
| Workflow: phase gates | Pass | 006 is Phase 2's last item; the Phase 2 gate (exfiltration tests blocked) passed with 005, and the hostile suite that makes it up runs unchanged here (FR-024); Phase 2 is marked done in the guide when 006 is merged |
| Workflow: Spec Kit, Constitution Check | Pass | This table, re-checked below, and an independent review before `/speckit-tasks` |

**Post-design re-check**: Pass, after an independent review against v2.4.1. It found no
violation of a principle; one conflict with a fixed decision (the docs would have promised a JS
engine run by the host while the constitution still names jco and componentize-js), 8 gaps (the
mock vault cannot be shared by the separate processes of the accounts guide; the sign-in
follower would open connections outside `runtime/src/network/`; no test that `pack` escapes the
encoder's messages; no test that the real `runtime` ignores the seam's variable; the test-only
CLI and the runner sharing the real development record; the walkthrough's dependencies outside
`audit` and `deny`; the phase gate unmentioned; the agent's `runtime approve` not weighed under
Principle VII) and 4 inaccuracies (the CLI move is not a pure move, since five places build the
runtime; `wit-component` 0.261 against `xtask`'s and `wit-bindgen`'s 0.262; 44 limits, not 49;
the example READMEs counted as code though no check reads them). It also found that cargo's
output cannot be compared whole, that the clone step could not run as written, that pages and
blocks disagreed on where a service is declared, that a `match` does not prove `ErrorKind::ALL`
complete, and that the frozen-copy check must read LF. Each was checked at the source
(`constitution.md:136`, `vault/store.rs:25`, `lib.rs:510`, `.cargo/config.toml`, `runtime.rs`,
`ci.yml`, `xtask/Cargo.toml`, `Cargo.lock`, `limits.rs`, `contract_v1.rs`) before the plan
changed (FR-022, R2, R3, R7 to R11, R13, R15, R16). The maintainer chose `pack` to componentize
(R2) and the test-only CLI (R9). A review finding that changes the host is planned when it is
found (R12), with its own test and review, and the table is checked again then.

## Project Structure

### Documentation (this feature)

```text
specs/006-contract-v1-docs/
├── plan.md              # This file
├── research.md          # Phase 0: decisions R1–R17
├── data-model.md        # Phase 1: reference markers, runnable blocks, findings, walks
├── quickstart.md        # Phase 1: how to check the feature end to end
├── contracts/
│   ├── docs-format.md   # markers, runnable blocks, placeholders, the runner's rules
│   └── cli.md           # `pack` on a core module, the test-only CLI
├── review.md            # the review's findings and outcomes (implementation)
├── walks.md             # the agent's and the maintainer's walks (implementation)
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
docs/
├── authors/
│   ├── README.md              # where to start; what v0 still runs; Phase 3 and Phase 5 notes
│   ├── getting-started.md     # the walkthrough (R1–R4)
│   ├── getting-started/       # the crate it builds, with its Cargo.lock, for audit and deny (R8)
│   ├── guides/
│   │   ├── render-and-update.md
│   │   ├── connectors.md      # from github-releases
│   │   ├── accounts.md        # from private-notes
│   │   ├── storage.md         # from update-counter
│   │   └── writing-connectors.md   # was docs/connectors.md
│   └── reference/
│       ├── contract.md        # every interface, function, type, case, world
│       ├── errors.md          # every ErrorKind as the CLI prints it
│       ├── manifest.md
│       ├── connector-format.md
│       ├── limits.md
│       └── cli.md
├── plugin-authors.md          # a pointer to docs/authors/ (links elsewhere keep working)
└── connectors.md              # a pointer to guides/writing-connectors.md

wit/history/1.3.0/widget.wit   # frozen after the review (R13)
wit/README.md                  # points to the docs; how v1 changes from now on (FR-003)

runtime/
├── Cargo.toml                 # wit-component; the test-only binary (required-features)
├── src/
│   ├── cli/                   # the CLI, moved from src/bin/runtime.rs, behaviour unchanged (R9)
│   ├── bin/runtime.rs         # calls runtime::cli with the real runtime
│   ├── bin/runtime-test-network.rs   # test builds only: the same CLI on the seam (R9)
│   ├── network/               # + the test sign-in follower, behind test-network (R9)
│   ├── package.rs             # pack: componentize a core module (R2)
│   └── error.rs               # ErrorKind::ALL (R7)
└── tests/
    ├── contract_v1.rs         # the 1.3.0 pin; v1 equals its frozen copy (R13)
    ├── docs_reference.rs      # the drift test (R7)
    └── docs_walk.rs           # the docs runner (R8)

plugins/{examples,hostile,fixtures}/*/manifest.json   # without login_providers (R14)
plugins/examples/README.md, */README.md               # point to the guides; TypeScript in Phase 5
Cargo.toml                     # the workspace excludes docs/authors/getting-started
CONTRIBUTING.md                # keeping the reference in step; the docs job
.github/workflows/ci.yml       # the docs job; checked docs count as code; audit/deny the walkthrough's crate (R11)
```

**Structure Decision**: the docs for authors get a directory of their own, split by what an
author is doing (starting, building one capability, looking something up), so each page is short
and the runner and the drift test address pages by role. The old pages stay as pointers because
specs, READMEs and SECURITY.md link to them. Both checks are runtime integration tests, because
they need the runtime's own definitions (`ErrorKind`, `Limits`, the bindings' WIT) and its test
servers; the runner is `#[ignore]`d so the build job's `cargo test` stays as fast as today.

## Complexity Tracking

| Choice | Why needed | Simpler alternative rejected because |
| --- | --- | --- |
| The CLI moves into a library module, with a second, test-only binary | FR-018 needs the connectors and accounts guides to run as written against test services; only the seam does that, and the seam is library-only | Running against the real GitHub is flaky (rate limits per runner address) and its output changes daily; skipping those commands breaks FR-018; a seam inside the real CLI breaks 003's rule that no build anyone runs has one (R9) |
| `pack` componentizes a core module | The walkthrough must not need the repository's xtask (FR-007) | Naming `wasm-tools` adds a tool to install and pin on three systems for one command, whose encoder must be kept in step with the host's by hand; the fixed decision's `wit-component` is what both would run (R2) |
