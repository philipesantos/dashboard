# Implementation Plan: Plugin Connectors

**Branch**: `003-plugin-connectors`, merged into `master` through one pull request, reviewed
commit by commit | **Date**: 2026-10-23 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/003-plugin-connectors/spec.md`

## Summary

Give widgets external data without giving them the network. A connector is a JSON declaration
of a service's operations, each with a fixed destination, typed parameters and a kind (read or
change), which the host runs; the catalog's connectors are built into the host, and local ones
serve development. A widget declares the connectors and operations it calls, each connector a
permission under feature 002's engine, and calls them only from a new `update` entry point,
whose JSON result the host keeps and hands to `render`, so render never waits on the network.
Every request goes through one module: `ureq` with the system's certificate verifier, no proxy,
no cookies, no compression, redirects checked by the host, and a resolver that connects only to
public addresses. Every call, allowed or refused, lands in a request log. Contract v1 moves to
1.1.0 with two worlds (`widget`, `connected-widget`), and widgets built for 1.0.0 keep running.
Spikes and decisions are in [research.md](research.md).

## Technical Context

**Language/Version**: Rust (edition 2024, stable; MSRV 1.97 because of Wasmtime 50)

**Primary Dependencies**: in the runtime, as in feature 002 (`wasmtime` 50, `serde`,
`serde_json`, `semver`, `sha2`, `wasmparser`, `dirs`, `tempfile`), plus `ureq` `=3.5.0`
(`rustls-no-provider`, `platform-verifier`), `rustls` 0.23 (`ring`), `url` and `regex`;
`rcgen` as a dev-dependency (R5, R18)

**Storage**: the development grant record, now version 2 (R13); kept data and the request log
in the development data directory (R14)

**Testing**: `cargo test` (runtime integration tests in `runtime/tests/`), with HTTPS and HTTP
test servers on loopback and a test-only network seam (`test-network` feature, R15); CI on
ubuntu-24.04, macos-latest and windows-latest, no internet needed

**Target Platform**: host on Windows, macOS and Linux; plugins on `wasm32-unknown-unknown`
(hostile WASI cases on `wasm32-wasip2`, as before)

**Project Type**: Rust library with a CLI binary, inside the existing Cargo workspace

**Performance Goals**: SC-004: a widget whose service takes 5 s renders in under 1 s from kept
data; SC-007: the connector checks add under 10 ms to loading a widget (R19)

**Constraints**: no generic network for plugins (Principle III, v2.2.0); credentials refused
until feature 005 (Principle IV); grants change only through the development CLI (Principle VII);
everything under `runtime/`, `wit/` and `connectors/` gets line-by-line review (Principle IX)

**Scale/Scope**: one connector format and schema, one catalog connector (3 reads), one network
module, contract 1.1.0 with 2 worlds, grant record v2, kept data and a request log, 7 new error
kinds and 10 call rules, 3 new CLI commands (`update`, `requests`, `check-connector`) and
connectors in `permissions`, `approve` and `optional`, 15 hostile cases, 1 example and 1 fixture
plugin

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Checked against constitution v2.2.0.

| Rule | Status | How the plan complies |
| --- | --- | --- |
| I. WebAssembly-only plugins | Pass | Widgets stay components; connectors are JSON the host reads, never code (R1) |
| II. Zero capabilities by default | Pass | Each connector is a permission declared in the manifest and approved; `connectors` is linked only when a declared connector is granted, and a stand-in answers `not-granted` when all are optional and off; a connector that is off answers `not-granted` per call from the link snapshot (R12); a change to an approved connector's destinations or operations needs a new approval (R13) |
| III. External data only through host-run connectors | Pass | No socket or HTTP import is offered; the host runs declarations; destinations come from the catalog, whose hosts are reserved, or are local in development; repository connectors are refused until verification exists (Phase 3); HTTPS, exact hosts, public addresses only (checked at every connection, redirects included), no system proxy, limits, request log; `permissions` shows every destination before approval (R5–R10, R14) |
| III. Local network | Pass (N/A) | No catalog connector reaches the user's machine or local network yet; the proxy refuses every non-public address; the destination kinds leave room (spec, assumptions) |
| IV. Plugins never see secrets | Pass | A connector that declares a credential is `credentials-not-supported`; the host sets `User-Agent` and forbids `Authorization` and `Cookie` headers; the log never records credentials (R1, R14) |
| V. Plugins stay inside the host frame | Pass (N/A) | No UI; render output rules unchanged |
| VI. Human confirmation for AI actions | Pass (N/A) | No AI |
| VII. Trust decisions only in host dialogs | Pass | Approvals of widgets and connectors go through the development CLI, on local files and the development record, as v2.2.0 allows; local connectors live only in development; no plugin can read or write the grants, the kept data of other widgets or the log |
| VIII. Every security rule has a breaking test | Pass | 15 hostile cases (data-model, FR-020) on three systems, without the internet; the cases of features 001 and 002 keep running, the v1 ones as 1.0.0 plugins on the 1.1.0 host |
| IX. Human review of the security core | Pass | The connector parser, network module, engine changes and record live in `runtime/`; the contract and schemas in `wit/`; the catalog in `connectors/`; the pull request template's list grows to name `connectors/` |
| Fixed decisions: Wasmtime + Component Model (WIT) | Pass | Contract 1.1.0 is WIT; minor versions only add, checked by a test (R11) |
| Fixed decisions: WASI 0.3 | Pass (N/A) | Still no WASI interface offered; `wasi:http` is refused like sockets |
| Workflow: gates, nothing visible on the sandbox, CI on 3 systems | Pass | No UI; this feature delivers the gate's network case; `deny.toml` gains ISC (R18) |

**Post-design re-check**: Pass. One deviation from the letter of Principle II's wording is
recorded, not a violation: the `connectors` interface serves every connector, so a connector that
is off is refused per call from an immutable snapshot rather than by linking a separate stand-in;
the outcome the principle requires (no capability, answer `not-granted`) is the same and has its
own hostile case (R12). New dependencies pass `cargo deny` once ISC is allowed, and the test seam
cannot reach a release build (R15).

## Project Structure

### Documentation (this feature)

```text
specs/003-plugin-connectors/
├── plan.md              # This file
├── research.md          # Phase 0: decisions and spike results
├── data-model.md        # Phase 1: connector, manifest field, record v2, kept data, log, load, calls, cases
├── quickstart.md        # Phase 1: validation guide
├── contracts/
│   ├── cli.md                   # CLI commands, output and exit codes
│   ├── widget-v1.1.wit          # Draft of wit/v1/widget.wit (1.1.0)
│   ├── connector.schema.json    # Draft of wit/connector.schema.json
│   ├── manifest.schema.json     # Draft of wit/manifest.schema.json, with connectors
│   └── catalog-github.json      # Draft of connectors/catalog/github.json
├── checklists/requirements.md
├── performance.md       # Created during implementation (SC-004, SC-007, R10)
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
wit/                            # Security core: contracts
├── README.md                   # + v1 minors, connectors, the connector schema
├── manifest.schema.json        # + connectors
├── connector.schema.json       # New: the connector format (FR-002)
├── v1/widget.wit               # 1.1.0: connectors, updater, worlds widget and connected-widget
└── history/1.0.0/widget.wit    # New: published 1.0.0, frozen

connectors/                     # Security core: the official catalog (Principle IX)
├── README.md                   # What the catalog is; criteria arrive in Phase 3
└── catalog/github.json         # catalog/github (FR-021)

runtime/                        # Security core
├── Cargo.toml                  # + ureq, rustls, url, regex; rcgen (dev); feature test-network
├── src/
│   ├── lib.rs                  # Runtime::new / with_test_network; Plugin::update, render with kept data
│   ├── connector.rs            # Connector parsing and checks (R1, R3, R4)
│   ├── catalog.rs              # The built-in catalog and its reserved hosts (R2)
│   ├── network/
│   │   ├── mod.rs              # The only use of ureq: agents, requests, redirects, limits (R5, R7, R10)
│   │   ├── address.rs          # Public-address classifier (R6)
│   │   └── resolver.rs         # PublicOnly resolver; the test resolver behind test-network (R6, R15)
│   ├── calls.rs                # One connector call, steps 1–12 of the data model; rates (R10, R12)
│   ├── kept.rs                 # Kept data store (R14)
│   ├── requests.rs             # Request log (R14)
│   ├── manifest.rs             # + connectors
│   ├── permissions.rs          # Minor versions, connectors in the registry and the load decision (R11, R12)
│   ├── grants.rs               # Record v2 (R13)
│   ├── host.rs                 # + connectors real and stand-in; render/update phase
│   ├── limits.rs               # + update and network limits (R10)
│   ├── error.rs                # + the new kinds
│   └── bin/runtime.rs          # + update, requests, check-connector; connectors in permissions, approve, optional
├── tests/
│   ├── common/servers.rs       # HTTPS and HTTP test servers that record requests (R15)
│   ├── connectors.rs           # Format, schema agreement, parameters, catalog rules (US1, US5)
│   ├── network.rs              # Addresses, redirects, TLS, proxy, limits (US2)
│   ├── updates.rs              # update, kept data, render with data, offline (US3)
│   ├── permissions.rs          # + connector approvals, changes, optional, revocation (US4)
│   ├── contract.rs             # + 1.0.0 plugins on the 1.1.0 host; newer minor refused (US5)
│   ├── contract_v1.rs          # + 1.1.0 is a superset of 1.0.0; connectors answers not-granted
│   ├── manifest_schema.rs      # + connector schema agreement; every repo connector validates
│   ├── cli.rs                  # + new commands and output
│   └── hostile.rs              # + the 15 cases of FR-020

plugins/
├── Cargo.toml                  # + the new crates
├── examples/hello-widget/      # Builds from wit/history/1.0.0 (SC-006)
├── examples/github-releases/   # New: contract 1.1.0, connected-widget, catalog/github
├── fixtures/probe, quiet/      # Build from wit/history/1.0.0
└── fixtures/net-probe/         # New: makes the calls its configuration lists (R16)

docs/plugin-authors.md          # + connectors, update, contract 1.1.0
docs/connectors.md              # New: writing a connector, testing it in development
docs/threat-model.md            # Connectors replace the allowlist (B2, threats, tests)
SECURITY.md, CONTRIBUTING.md    # Scope and security core name connectors
.github/pull_request_template.md  # Security core: runtime/, wit/, connectors/
deny.toml                       # + ISC
```

**Structure Decision**: everything stays in the existing crates. The network module is the only
code that opens connections and the only user of `ureq`, so its review covers every byte that
leaves the machine. The catalog gets a top-level `connectors/` directory, outside `runtime/`,
because Phase 3 adds connectors there without touching host code, and it is listed in the
security core like `wit/`. Old contract versions live under `wit/history/`, frozen, because
`bindgen!` and `wit-bindgen` read one package per directory.

## Complexity Tracking

No constitution violations to justify. The per-call refusal of a connector that is off, in
place of a separate stand-in, is explained in the post-design re-check above.
