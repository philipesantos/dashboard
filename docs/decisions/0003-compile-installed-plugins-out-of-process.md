# 0003. Installed plugins are compiled in a confined process; they run in the app until the next feature

- **Status**: Accepted. The maintainer decided it on 2026-11-12, reviewing decision 0001 as its
  trigger required, before the app installed a plugin it did not build.
- **Date**: 2026-11-12
- **Feature**: [010, plugin installation](../../specs/010-plugin-installation/spec.md) (FR-007,
  FR-008, FR-009, FR-029); research R1, R2, R3, R13
- **Supersedes**: [0001](0001-plugin-process-isolation.md), for plugins installed from a
  repository

## Context

Decision 0001 kept plugins in the app's process during Phase 1, when only the maintainer loaded
them, and named its trigger: "before the app can install a plugin it did not build itself". Feature
010 installs plugins from their authors' repositories, so the decision was reviewed.

Two risks stand apart:

- **Compiling.** `Component::from_binary` compiles a package before any of the runtime's limits
  exists: no CPU budget, time limit or memory cap applies to Cranelift, and it uses every core. A
  package crafted to be slow or memory-hungry to compile could take the app's CPU and memory
  (the threat-model row "Package crafted to be slow or memory-hungry to compile").
- **Running.** A sandbox escape in Wasmtime would run with the app's rights, beside the keychain
  service and the OAuth tokens the app holds. What changed for this risk is not the cost of a call
  but who wrote the code: a third party, no longer the maintainer.

## Decision

1. **Compiling leaves the app's process now.** The app starts its own executable with
   `--compile-package`; the child builds the engine from the same configuration as the app
   (`engine_config()`), confines itself before it reads the package, compiles it, and writes the
   compiled form back. The parent limits its wall-clock time (20 s), CPU time (60 s), memory
   (1.5 GiB), threads (half the cores) and priority, kills it when the app quits, and names the limit
   that stopped it.
2. **The child is confined** on each system: Landlock and seccomp on Linux; the pure-computation
   sandbox on macOS; an AppContainer with no capability and a job object on Windows. Once confined
   it can open no file of the user's, reach no network and no other process.
3. **The app trusts a compiled form only by its digest.** The installed record keeps the SHA-256 of
   the package and of the compiled form; before `deserialize`, both are checked on the bytes read. A
   compiled form that changed, or that this app's engine refuses, is compiled again once; a package
   that changed is refused.
4. **Running stays in the app's process until the next feature**, which moves plugin execution
   (render, update, act, connectors, storage, accounts, logs) behind a process boundary, away from
   the keychain. It is a Phase 3 item of the development guide, and the alpha's gate does not open
   before it lands.

## Consequences

- The compile-cost row of the threat model is covered, with a breaking test per limit
  (`compile-slow`, `compile-cpu`, `compile-memory`, `compile-output`) and one for the confinement
  (`compile-confined`) on the three systems.
- **Accepted, until the next feature**: a sandbox escape at run time reaches what the app holds,
  with 0001's mitigations (pinned versions, `cargo audit` on every change and every week, minimal
  Wasmtime features, updating fast when an advisory appears).
- **Accepted**: the compiler's output is still trusted as code, as it was when the app compiled in
  its own process. A bug in Cranelift that miscompiles a package into something that escapes is not
  stopped by the confinement, which protects the user's data from the compile process, not the app
  from the code it produces.
- Every install costs one more process start, a few milliseconds beside the compile itself.

## Revisit when

- The next feature lands: running then leaves the app's process, and this decision records it.
- An advisory reports a sandbox escape in Wasmtime or Cranelift.
- A legitimate package comes within half of a compile limit on any CI system
  (`compile::measure_compile_heavy_under_the_default_limits`).
- Before AI-generated modules are compiled on the user's machine (Phase 5).

## Options considered

- **Compile in a confined process, run in the app** (chosen for now): closes the compile risk with
  one process start per install and no inter-process protocol for calls.
- **Run every plugin in a process of its own, or all of them in one plugin-host process**: keeps a
  run-time escape away from the keychain, at the cost of a protocol for every host interface, all of
  it security core. Chosen as the next feature rather than this one, so installation lands with its
  own review first.
- **Compile in the app, under no limit**: the package cap alone bounds it; rejected, as 0001
  foresaw.
