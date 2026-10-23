# dashboard Constitution

## Core Principles

### I. WebAssembly-Only Plugins

Plugins MUST execute only as WebAssembly components inside the host's Wasmtime runtime.
The host MUST NOT load or run plugin code that is native, Node.js, Python or any other
runtime with system access.

Rationale: one mature sandbox is the trust boundary; any other runtime hands the plugin
author direct access to the user's machine.

### II. Zero Capabilities by Default

A plugin MUST start with no capabilities. Every capability MUST be a host function that the
plugin declares in its manifest and the user approves, and the host MUST link only the
imports that were granted. A capability the manifest marks as optional and the user has not
granted MAY be linked only to a host stand-in that gives no capability and answers every call
with "not granted". When several capabilities share one import, such as the connectors a plugin
declares, the host MUST build, when it links the plugin, a table that holds only the granted
ones; a capability missing from it MUST answer every call with "not granted" and reach nothing,
and checks made at call time MAY only take capabilities away, never add one to the table. An
update that requests a new capability MUST require a new approval.

Rationale: least privilege; a capability that was never granted cannot be abused, and an
update cannot escalate silently. Optional capabilities let a plugin run without them; the
stand-in exists only because the Component Model has no optional imports. A shared import
cannot be linked once per capability, so the table built at link time does the linker's job:
what was not granted is never built, rather than built and guarded on every call.

### III. External Data Only Through Host-Run Connectors

Plugins MUST NOT have generic network access: the host MUST NOT offer them sockets or
requests to a destination of their choosing. A plugin MAY reach an external service only by
calling the operations of a connector its manifest declares. A connector is a declaration,
never code, and the host runs it: each operation has a fixed destination and typed
parameters, the host MUST validate every call against them, and operations that change data
MUST be declared apart from those that only read.

A connector's destination MUST be a domain of the project's official connector catalog, or a
domain whose owner authorizes the connector's repository in a file published on that domain,
which the host MUST verify at installation, at every update and periodically. Domains of the
official catalog are reserved to it. A catalog connector MAY instead take its destination
from the user in a host dialog, never from the plugin: a protocol connector, such as RSS, an
address on the internet; a connector for a service people run themselves, such as a home
server, an address on the user's own machine or local network.

All connector traffic MUST go through the host proxy, to the exact hosts declared or chosen,
with size and rate limits and a request log. It MUST use HTTPS and never reach a loopback or
private address, except an address on the user's machine or local network that the user
chose for a catalog connector. That address MUST stay local. There the host MAY trust a
certificate the user accepted at first use in a host dialog, and MUST refuse any other
afterwards; it MAY reach the address without HTTPS, showing that the connection is not
encrypted, only for a connector that sends no credential. Before approval, the host MUST show
every destination a plugin's connectors reach, and whether data read from one connected
account or from the local network can reach another destination.

The host's development command-line tool MAY run a local connector whose destination is not
verified, or is a loopback address without HTTPS, recording it only in the development record
that installed plugins never use.

Rationale: a domain allowlist cannot prevent exfiltration, because any approved domain may
forward what it receives. Running declarations instead of plugin code limits a plugin to the
operations it declares, and verified domains let only the owner of a service publish a
connector for it, so no impostor or duplicate connector exists. A plugin reaches the local
network only at an address the user chose, through operations the project reviewed, so it
cannot scan the network or reach other devices. Local addresses repeat from one network to the
next, so only a certificate, valid for the address or accepted by the user, proves that the
server is theirs. What remains, a destination receiving what the plugin sends it, the user
judges with every destination in view.

### IV. Plugins Never See Secrets

Plugins MUST NOT receive passwords, OAuth tokens or AI provider keys. Authentication and
secrets MUST stay in the host and the OS keychain. A connector declares the credential its
operations need, and the host MUST inject it only into that connector's requests to its own
destination, and only over HTTPS with a certificate that is valid for that destination or that
the user accepted for it.

Rationale: a malicious or compromised plugin can then misuse only what it was granted, never
the user's credentials themselves, and a credential bound to its destination cannot reach
another service, not even a device that answers at the same local address on another network.

### V. Plugins Stay Inside the Host Frame

Plugin UI MUST render only inside the host-drawn frame of its widget and MUST NOT contain
password fields. Plugins MUST NOT open windows or dialogs.

Rationale: prevents fake login screens and phishing that imitate the host.

### VI. Human Confirmation for AI Actions

AI output MUST NOT trigger any action without explicit human confirmation in a host dialog.

Rationale: model output can be steered by instructions hidden in the content the AI reads
(prompt injection).

### VII. Trust Decisions Only in Host Dialogs

Granting permissions, installing plugins and connectors, and connecting accounts MUST happen
only in host dialogs. Until the host app has these dialogs (Phase 3), the host's development command-line
tool MAY grant permissions to local packages, recording them only in a development grant
record that installed plugins never use. Neither AI output nor a plugin MAY perform, trigger
or pre-approve these actions.

Rationale: these decisions expand what code can do, so they stay out of reach of the
parties the threat model distrusts: plugin authors, external content and manipulated AI.

### VIII. Every Security Rule Has a Breaking Test

Every security rule MUST have an automated test that fails the build when the rule is
violated. The security test suite, with one hostile plugin or hostile content per case,
MUST grow with each phase and MUST NOT shrink.

Rationale: a rule without a test erodes silently.

### IX. Human Review of the Security Core

Every change to the security core (plugin runtime, permission engine, connector engine and
network proxy, secrets vault, and the official connector catalog) MUST receive line-by-line
human review before it is merged.

Rationale: these components enforce every other principle, and the catalog decides which
destinations every user trusts; a bug there breaks them all.

## Fixed Technical Decisions

| Layer | Decision | Why |
| --- | --- | --- |
| Host app | Rust + Tauri 2 | Light installers on Windows, macOS and Linux; permissions denied by default |
| UI | TypeScript + React | Familiar to many contributors; runs in the Tauri webview |
| Plugin runtime | Wasmtime + Component Model (WIT), WASI 0.3 | Mature sandbox, typed multi-language contract |
| JS/TS plugins | jco / componentize-js | Most accessible language for authors and for AI generation |
| Rust plugins | cargo + wit-bindgen on `wasm32-unknown-unknown`, componentized with wit-component | Actively maintained; components import only what their WIT world declares |
| Go, Python plugins | TinyGo, componentize-py | Advanced authors |
| Secrets | `keyring` crate (OS keychain) | Tokens stay out of the app's files |
| Plugin storage | SQLite, one database per plugin | Simple isolation; the host's own records (grants, the data an update returns, the request log) are host files that no plugin can reach |
| Specifications | GitHub Spec Kit | Independent of the AI agent |
| Licenses | Client Apache-2.0, servers AGPL-3.0 | Client adoption, protection of paid services |

Changing any of these decisions requires an amendment (see Governance).

## Development Workflow and Quality Gates

- Work is organized in phases, and each phase has a gate. A phase ends only when its gate
  passes. When a phase runs late, scope inside the phase MUST be cut; the gate MUST NOT be
  skipped.
- Nothing user-visible MAY be built on top of the plugin sandbox until the Phase 1 gate
  (hostile-plugin tests blocked) and the Phase 2 gate (exfiltration tests blocked) pass.
- Every feature goes through Spec Kit (specify → plan → tasks → implement), and every plan
  MUST pass the Constitution Check.
- CI MUST build and test on Windows, macOS and Linux, run `cargo clippy`, `cargo audit` and
  `cargo deny`, and run the security test suite on every change except one that touches only
  documentation: Markdown files, `specs/`, `docs/`, `.specify/` and `LICENSE`. A Markdown file
  that the build uses as code, such as a README included as crate documentation (its examples
  run as tests), is not documentation for this rule.

## Governance

This constitution supersedes every other practice or guideline in this project.

- Amendments: a rule changes only through a recorded decision: an edit to this file that
  bumps the version, updates the Last Amended date and states the rationale, committed to
  the repository.
- Versioning: MAJOR for removing or redefining a principle or rule; MINOR for adding a
  principle or section, or materially expanding guidance; PATCH for clarifications and
  wording.
- Compliance: every plan's Constitution Check and every code review MUST verify compliance.
  A conflict is resolved by changing the plan or by amending this constitution first, never
  by a case-by-case exception.

**Version**: 2.3.1 | **Ratified**: 2026-10-21 | **Last Amended**: 2026-10-23
