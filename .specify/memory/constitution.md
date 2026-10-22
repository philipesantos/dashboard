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
with "not granted". An update that requests a new capability MUST require a new approval.

Rationale: least privilege; a capability that was never granted cannot be abused, and an
update cannot escalate silently. Optional capabilities let a plugin run without them; the
stand-in exists only because the Component Model has no optional imports.

### III. Network Only Through the Host Proxy

All plugin network traffic MUST go through the host proxy, which enforces a per-domain
allowlist. Plugins MUST NOT open sockets or send requests by any other path.

Rationale: the proxy is the single place where exfiltration can be blocked and audited.

### IV. Plugins Never See Secrets

Plugins MUST NOT receive passwords, OAuth tokens or AI provider keys. Authentication and
secrets MUST stay in the host and the OS keychain. A plugin MAY refer to a secret by name;
the host injects it into the outgoing request.

Rationale: a malicious or compromised plugin can then misuse only what it was granted, never
the user's credentials themselves.

### V. Plugins Stay Inside the Host Frame

Plugin UI MUST render only inside the host-drawn frame of its widget and MUST NOT contain
password fields. Plugins MUST NOT open windows or dialogs.

Rationale: prevents fake login screens and phishing that imitate the host.

### VI. Human Confirmation for AI Actions

AI output MUST NOT trigger any action without explicit human confirmation in a host dialog.

Rationale: model output can be steered by instructions hidden in the content the AI reads
(prompt injection).

### VII. Trust Decisions Only in Host Dialogs

Granting permissions, installing plugins and connecting accounts MUST happen only in host
dialogs. Neither AI output nor a plugin MAY perform, trigger or pre-approve these actions.

Rationale: these decisions expand what code can do, so they stay out of reach of the
parties the threat model distrusts: plugin authors, external content and manipulated AI.

### VIII. Every Security Rule Has a Breaking Test

Every security rule MUST have an automated test that fails the build when the rule is
violated. The security test suite, with one hostile plugin or hostile content per case,
MUST grow with each phase and MUST NOT shrink.

Rationale: a rule without a test erodes silently.

### IX. Human Review of the Security Core

Every change to the security core (plugin runtime, permission engine, network proxy and
secrets vault) MUST receive line-by-line human review before it is merged.

Rationale: these components enforce every other principle; a bug there breaks them all.

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
| Local data | SQLite, one database per plugin | Simple isolation |
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

**Version**: 1.2.0 | **Ratified**: 2026-10-21 | **Last Amended**: 2026-10-22
