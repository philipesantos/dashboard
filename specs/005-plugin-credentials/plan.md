# Implementation Plan: Plugin Credentials

**Branch**: `005-plugin-credentials`, merged into `master` through one pull request, reviewed
commit by commit | **Date**: 2026-10-25 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/005-plugin-credentials/spec.md`

## Summary

Let widgets act as the user's account at a service without ever holding its credential. A
connector declares one credential (an account or a key): where it goes, in which header, the
access it asks for and how to sign in. The host keeps the credential in the system's keychain
through keyring-rs (`keyring-core` and one store per system) and adds it itself, per request and
per redirect hop, only to the destinations it is for, over HTTPS; it refuses any response that
carries it back. A widget declares `account: true` on a connector and calls it through a new
contract 1.3.0 interface, `accounts`, linked over a table of its own built at link time; the new
answers `account-not-connected` and `subscription-required` live there, since 1.1.0's types
cannot grow. The account a widget acts as is fixed when it is linked; checks during a run only
take it away. Accounts are connected only through the development tool, with the authorization
code flow and PKCE on a loopback callback, never a client secret. No catalog connector gains an
account yet: GitHub signs in a client without a secret only through its device flow, which was
left out, so `catalog/github` stays public-only and the example uses a development connector
against the test sign-in service. The grant record moves to version 4, a new accounts record holds what
is connected (never a token), and kept data follows the account it was fetched with. Decisions
and their sources are in [research.md](research.md).

## Technical Context

**Language/Version**: Rust (edition 2024, stable; MSRV 1.97 because of Wasmtime 50)

**Primary Dependencies**: as in feature 004, plus `keyring-core` `=1.0.1` and, per system,
`windows-native-keyring-store` `=1.1.0` (no default features), `apple-native-keyring-store`
`=1.0.2` (`keychain`) and `dbus-secret-service-keyring-store` `=1.0.1` (`crypto-rust`, never
`vendored`); `zeroize`, `memchr` and `httparse`, already in the tree; `rpassword` 7 and `open` 5 in
the binary only (R5, R17)

**Storage**: credentials in the system keychain (development vault, R5); `<data dir>/accounts.json`
(R6); the grant record moves to version 4 (R8); kept data and the request log gain account fields

**Testing**: `cargo test`, hostile cases on a new `account-probe` fixture and a `token-hunt` crate; the sign-in, the example and the hostile cases as library tests
through the seam, with a test sign-in service on the HTTPS test servers of 003 and the mock store;
CLI tests for the `accounts` commands without the seam (003's rule kept), a key typed in a
pseudo-terminal on Linux and macOS; the real keychain in CLI and `vault` tests on ubuntu-24.04
(gnome-keyring in `dbus-run-session`), macos-latest and windows-latest; no internet (R15)

**Target Platform**: host on Windows, macOS and Linux (Linux builds need `libdbus-1-dev`); plugins
on `wasm32-unknown-unknown` (`token-hunt`'s WASI half on `wasm32-wasip2`)

**Project Type**: Rust library with a CLI binary, inside the existing Cargo workspace

**Performance Goals**: SC-006: a call with a credential within 5 ms of the same call without, once
read from the keychain, measured in-process in release (R18)

**Constraints**: plugins never receive a credential and never start a sign-in (Principles IV,
VII, FR-008, FR-009); accounts are connected only through the development tool, into the
development vault (constitution v2.4.0); everything under `runtime/`, `wit/` and `connectors/`
gets line-by-line review, the vault included (Principle IX)

**Scale/Scope**: one vault module with one vault thread, sign-in (one flow, a loopback listener
in the network module), contract 1.3.0 with 1 interface and 2 worlds, grant record v4, accounts
record v1, 8 new error kinds, 3 new CLI subcommands (`accounts connect|list|disconnect`) and
account and data-flow lines in `permissions`, `approve`, `requests`, `update`; 32 hostile cases
and 14 other named tests; 1 new example with its development connector, 1 new fixture, 1
hostile crate, 1 bench binary

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Checked against constitution v2.4.1 (v2.4.0 lets the development tool connect accounts; v2.4.1
names keyring-rs's crates); the independent review's findings are in
[checklists/requirements.md](checklists/requirements.md).

| Rule | Status | How the plan complies |
| --- | --- | --- |
| I. WebAssembly-only plugins | Pass | Plugins stay components; the vault, sign-in and injection run in the host |
| II. Zero capabilities by default | Pass | A connector's account is a capability the manifest declares (`account: true`) and the approval covers (`uses`, credential digest, access); `accounts` is linked over a table built at link time from what was granted, the stand-in otherwise, and refused when undeclared; the account a widget acts as is a snapshot taken at link time, and call-time checks (grants, accounts record) only take away: an account connected mid-run applies from the next load (R9, R11, `connect-mid-run`). New credentials, wider access and starting to use an account need a new approval (R8) |
| III. External data only through connectors | Pass | Credentials ride only on connector requests through the host proxy; the host's own token, refresh, identity and revocation requests use the same agents (HTTPS, public addresses, Mozilla's roots, no proxy), the destination's rate and the request log; a connector with a credential has no plain-HTTP destination at all, a local loopback one included (`credential-needs-https`). Before approval, `permissions` shows whether data read as an account can reach another destination (the `data flow:` line, as 003 does for this machine). The loopback listener receives the browser's callback; it reaches nothing (R2, R10, R14) |
| IV. Plugins never see secrets | Pass | Credentials live only in the keychain and the host's memory during a run (`Zeroizing`, redacted `Debug`); the header is added per hop only for origins the credential is for, over HTTPS with Mozilla's roots; token, revocation and identity requests go only to the credential's own destinations; every credential is in the bearer-token set, so a response that carries it back, raw or escaped, is refused; keychain and parser errors are mapped to secret-free text; no file, log, kept data, store, error or output holds one, checked after every hostile case by a scan for fresh random tokens (R5, R10, R15) |
| V. Plugins stay inside the host frame | Pass (N/A) | No UI; the sign-in happens in the system browser, which the tool opens, never a plugin |
| VI. Human confirmation for AI actions | Pass (N/A) | No AI |
| VII. Trust decisions only in host dialogs | Pass | Accounts are connected, listed and disconnected only by the development tool for local packages, into the development vault (v2.4.0); no plugin can start a sign-in, open a browser or touch the accounts record (`widget-connect`) |
| VIII. Every security rule has a breaking test | Pass | 32 hostile cases and 14 named tests, on three systems, with the real keychain in CI; the suite of features 001–004 keeps running, with 1.0.0–1.2.0 plugins on the 1.3.0 host |
| IX. Human review of the security core | Pass | The vault in `runtime/src/vault/`, sign-in and injection in `runtime/src/network/`, the contract and schemas in `wit/`, the catalog in `connectors/`; Clippy keeps `keyring-core`, the stores and `TcpListener` in their modules; the vault crates are pinned exactly and listed in the PR template with `libdbus-sys`, whose `vendored` feature must stay off (R19) |
| Fixed decisions: host app, UI | Pass (N/A) | No change to the Tauri shell or the UI |
| Fixed decisions: Wasmtime + Component Model (WIT) | Pass | Contract 1.3.0 is WIT; it only adds (a new interface and worlds), checked by a test, 1.2.0 frozen and pinned by hash (R9) |
| Fixed decisions: WASI 0.3 | Pass (N/A) | No WASI interface offered; `token-hunt`'s WASI build is refused |
| Fixed decisions: JS/TS, Rust, Go and Python plugins | Pass | Example and fixtures in Rust on `wasm32-unknown-unknown`; nothing changes for other languages |
| Fixed decisions: secrets | Pass | keyring-rs, `keyring-core` and its stores, the system keychain (v2.4.1, R5) |
| Fixed decisions: plugin storage | Pass (N/A) | Unchanged; the accounts record is a host file no plugin can reach |
| Fixed decisions: specifications | Pass | Through Spec Kit |
| Fixed decisions: licenses | Pass | New crates are MIT, Apache-2.0, BSD-3-Clause or Unlicense OR MIT, all allowed; libdbus is linked from the system, never vendored (R5, R17) |
| Workflow: gates, nothing visible on the sandbox, CI on 3 systems | Pass | No UI; this feature delivers the gate's token case, the last one; CI gains the keychain setup on Linux |
| Workflow: Spec Kit and the Constitution Check | Pass | specify, clarify, plan, this check with an independent review, then tasks |
| Governance | Pass | v2.4.0 and v2.4.1 were amended before this plan; no exception is taken |

**Post-design re-check**: Pass, after an independent review against v2.4.1. It found 4 conflicts
(no data-flow line for data read as an account, which Principle III requires before approval; a
plain-HTTP destination allowed beside a credential's; token, revocation and identity hosts not
tied to the credential's destinations; an account connected by another process mid-run that a
call would start using, against Principle II), 10 risks and 7 gaps. Each was checked at the
source (the constitution, `keyring-core`, `windows-native-keyring-store` and `rpassword` sources,
the CLI tests) before the plan changed (R3, R5, R9 to R15, R18, R19); the maintainer chose how the
CLI, the example and the bench are tested (library tests, 003's seam rule kept, R15).

## Project Structure

### Documentation (this feature)

```text
specs/005-plugin-credentials/
├── plan.md              # This file
├── research.md          # Phase 0: decisions, GitHub's documentation, the vault spike, the review
├── data-model.md        # Phase 1: credential, account use, records, states, call and sign-in steps, cases
├── quickstart.md        # Phase 1: validation guide
├── contracts/
│   ├── cli.md                             # accounts commands; account lines elsewhere
│   ├── widget-v1.3.wit                    # What wit/v1/widget.wit adds in 1.3.0
│   ├── connector-credential.schema.json   # What wit/connector.schema.json adds
├── checklists/requirements.md
├── performance.md       # Created during implementation (SC-006, R13, R18)
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (repository root)

```text
wit/                            # Security core: contracts
├── README.md                   # + accounts, contract 1.3.0
├── connector.schema.json       # + credential
├── manifest.schema.json        # + connectors[].account; login_providers optional, obsolete
├── v1/widget.wit               # 1.3.0: accounts, worlds account-widget and account-stored-widget
└── history/1.2.0/widget.wit    # New: published 1.2.0, frozen

runtime/                        # Security core
├── Cargo.toml                  # + keyring-core and the three stores (per target), zeroize, memchr, httparse; rpassword, open (binary)
├── src/
│   ├── vault/
│   │   ├── mod.rs              # Secret, entries, the vault thread, bounded waits, the injected store (R5)
│   │   └── store.rs            # The only keyring use: builds each system's store; secret-free errors
│   ├── accounts.rs             # Accounts record, states, client registrations, refresh lock (R6, R11, R12)
│   ├── signin.rs               # The sign-in, identity, comparison, write order (R2; data model, Sign-in)
│   ├── network/
│   │   ├── mod.rs              # + credential per hop, echo check (escaped forms), form requests for the host's own calls (R10)
│   │   └── callback.rs         # New: the loopback listener (R2)
│   ├── connector.rs            # + credential parsing and checks (data model, Connector)
│   ├── calls.rs                # + the account table and steps 7a–12a
│   ├── host.rs                 # + accounts real and stand-in
│   ├── manifest.rs             # + account; login_providers optional
│   ├── permissions.rs          # + accounts in the registry; 1.3.0
│   ├── grants.rs               # Record v4, approval digest with account (R8)
│   ├── kept.rs                 # + accounts in kept data
│   ├── requests.rs             # + account; host entries
│   ├── limits.rs               # + R13
│   ├── error.rs                # + the new kinds
│   └── bin/runtime.rs          # + accounts connect|list|disconnect; account and data-flow lines
└── tests/
    ├── common/signin.rs        # The test sign-in service (R15)
    ├── accounts.rs             # Connect, refresh, disconnect, states, records (US1, US4)
    ├── vault.rs                # Real keychain (three systems)
    ├── network.rs              # + injection, redirects, echo (US3)
    ├── permissions.rs          # + approvals, digest, changes (US5)
    ├── contract.rs, contract_v1.rs   # + 1.3.0 (US6)
    ├── manifest_schema.rs      # + account; connector credential
    ├── cli.rs                  # + accounts commands
    ├── hostile.rs              # + mod credentials
    └── hostile/credentials.rs  # The 32 cases of FR-019

bench/src/bin/credential_cost.rs  # New (R18)

plugins/
├── Cargo.toml                  # + the new crates
├── examples/private-notes/     # New: contract 1.3.0, account-widget, with connectors/notes.json (R16)
├── fixtures/account-probe/     # New: contract 1.3.0, calls through accounts or connectors; dump mode
├── hostile/token-hunt/         # New: looks for credentials everywhere it can (wasm32-wasip2 half)
└── hostile/connectors/         # New: connector files of the load-time cases (plain-http-key.json, …)

docs/plugin-authors.md          # + using an account, the new answers, contract 1.3.0
docs/connectors.md              # + declaring a credential, the narrowest access, a development app
connectors/README.md            # + credentials in catalog review
docs/threat-model.md            # + credential threats and their tests
SECURITY.md                     # Scope names the vault and accounts
plugins/hostile/README.md       # + credential cases
.github/workflows/ci.yml        # Linux: gnome-keyring, libdbus-1-dev, the test step in dbus-run-session
.github/workflows/bench.yml     # Linux: the same packages and session
.github/pull_request_template.md  # vault in the core; Cargo.lock: vault crates, libdbus-sys (never vendored)
clippy.toml                     # keyring only in runtime/src/vault/; TcpListener only in runtime/src/network/
```

**Structure Decision**: the vault gets a module of its own whose `store.rs` is the only code that
touches keyring-rs, so its review covers every keychain call; the loopback listener sits in the
network module, which stays the only code that opens sockets. The 1.2.0 plugins (`update-counter`,
`store-probe`) move to build from `wit/history/1.2.0`, so the suite keeps running them as 1.2.0
plugins on the 1.3.0 host; `github-releases` stays on 1.1.0 and `catalog/github` on 1.0.0.

## Complexity Tracking

No constitution violations to justify.
