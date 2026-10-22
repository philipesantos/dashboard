# Research: Plugin Runtime

All findings come from spikes run on 2026-10-21 on the maintainer's Linux machine (x86_64,
rustc 1.99.1, Wasmtime 50.0.0, wit-bindgen 0.63.0, wit-component 0.262.0, release builds).
Numbers are indicative; User Story 4 re-measures them on all three CI systems.

## R1. Building Rust plugins that import only `log`

**Decision**: build plugin crates with plain `cargo build --target wasm32-unknown-unknown`,
generate guest bindings with `wit-bindgen` 0.63, and turn the core module into a component with
the `wit-component` library (`ComponentEncoder`). Amended in constitution v1.1.0.

**Rationale**:

- The spike component imported exactly one interface, `dashboard:widget/log@0.1.0`, and no WASI.
- `wit-bindgen` and `wit-component` are released together with Wasmtime's toolchain and are
  current; no extra CLI is installed in CI.
- `cargo-component` (the original constitution choice) has had no release since v0.21.1
  (2025-04-07). It works with `--target wasm32-unknown-unknown`, but its default target adapts
  WASI preview 1 and imported ten WASI interfaces for a trivial plugin, and CI would have to
  build or download it.

**Alternatives considered**:

- `cargo-component --target wasm32-unknown-unknown`: same result, stale tool, extra CI install.
- `wasm32-wasip2` target: produces components directly, but Rust's standard library makes even
  a trivial plugin import WASI (24 interfaces in the spike). Kept only for hostile fixtures
  (R3).
- `wasm-tools component new` CLI: equivalent to the library, but one more tool to install.

## R2. CPU, time and memory limits

**Decision**:

- **CPU budget**: Wasmtime fuel (`Config::consume_fuel`), set per call with
  `Store::set_fuel`. Exhaustion traps with `Trap::OutOfFuel`.
- **Time limit**: epoch interruption (`Config::epoch_interruption`), one ticker thread per
  engine incrementing the epoch every 10 ms, and `Store::set_epoch_deadline` per call.
  Expiry traps with `Trap::Interrupt`.
- **Memory cap**: a host `ResourceLimiter` that refuses growth beyond the cap and returns the
  host's own error type, so the failure is reported as "memory limit" instead of a generic
  trap. Wasmtime's `StoreLimits` also caps instances, tables and table elements.

**Rationale**:

- Fuel is deterministic: the same plugin fails at the same point on Windows, macOS and Linux,
  which keeps the hostile tests reproducible. Epoch is wall-clock time, the guarantee SC-002
  asks for.
- Spike: a runaway loop with a 500 ms epoch deadline stopped after 649 ms, within SC-002's
  "no later than one second after the limit".
- Spike: `StoreLimits` with `trap_on_grow_failure` blocked a 1 GiB allocation against a
  32 MiB cap, but surfaced it as an untyped error. A custom limiter gives the typed error that
  SC-004 needs.
- Spike: infinite recursion in Rust surfaced as `Trap::MemoryOutOfBounds`, because Rust keeps
  its stack in linear memory and overflows it before Wasm's native stack. Native stack
  exhaustion would surface as `Trap::StackOverflow`. Both map to "plugin crashed".
- Fuel cost: a hello render used 1,896 fuel units; the CPU burned about 1.4 million units
  per millisecond.

**Alternatives considered**: fuel only (no wall-clock guarantee); epoch only (results depend on
machine speed, so tests become flaky across systems); async yielding (adds an async runtime for
no v0 benefit).

## R3. Producing hostile test plugins

**Decision**: every hostile plugin is a small Rust crate built from source in CI, never a
committed binary.

- CPU, memory, recursion, malformed output, oversized output and log flooding:
  `wasm32-unknown-unknown`, same build path as the example (R1).
- File and socket access: `wasm32-wasip2` with Rust's standard library (`std::fs`,
  `std::net`), which is what an author who ignores the contract would ship. These components
  import `wasi:filesystem/*` and `wasi:sockets/*` and must be refused at load.
- An `xtask` binary (`cargo xtask plugins`) builds all plugin crates and writes components to
  `target/plugins/`. Tests fail, never skip, when a fixture is missing.

**Rationale**: source is reviewable and reproducible on all three systems; committed binaries
would be an opaque supply-chain risk in a security project. Hand-written WAT was rejected
because a hostile plugin must implement the real contract (string parameters and results),
otherwise it fails for the wrong reason (a missing export) and the test proves nothing.

**Alternatives considered**: committed `.wasm` fixtures; WAT fixtures; a shell script (does not
run the same way on Windows).

## R4. Refusing capabilities with a precise error

**Decision**: before linking, compare every import of the component
(`Component::component_type().imports`) with the allowlist of contract v0
(`dashboard:widget/log@0.1.0`) and refuse the plugin with the full list of disallowed imports.

**Rationale**: the spike's file/socket plugin imported 24 WASI interfaces. Wasmtime's own link
error named only the first one (`wasi:io/poll`), not the filesystem or sockets, which fails
FR-004 ("an error naming that capability"). The explicit check also enforces Principle II
independently of what happens to be registered in the linker.

## R5. Where WASI 0.3 fits

**Decision**: contract v0 links no WASI interface at all, so Phase 1 neither uses nor depends
on WASI. The constitution's "WASI 0.3" decision applies to the first WASI interface the host
offers, expected in Phase 2 (network proxy, TypeScript plugins).

**Rationale**: `wasmtime-wasi` 50 ships `p1`, `p2` and `p3` support; nothing has to be decided
now. Note for Phase 2: Rust's `wasm32-wasip2` target still imports WASI 0.2 (`@0.2.12`), so
WASI 0.3 for guests depends on toolchain support that must be checked then.

## R6. Fresh instance per call or persistent instance

**Decision**: compile each plugin once at load (`Component::new`, then `InstancePre`) and run
every `render` in a fresh `Store` and instance, dropped when the call ends.

**Rationale**: FR-008 holds by construction, because no memory survives a call. The spike
measured 25–90 µs to instantiate and 23–35 µs to render, so the isolation is nearly free. State
between calls arrives with storage in Phase 2.

**Alternatives considered**: a persistent instance per plugin (faster for heavy plugins, but a
crashed or limited plugin could leave corrupt state, and memory stays allocated).

## R7. Measuring load and render time

**Decision**: the CLI gets a `bench` subcommand that reports load time (compile) and the
p50/p95/max of instantiate + render over N iterations, in a release build. CI runs it on the
example plugin on all three systems and prints the results; the maintainer records them and
the targets in `specs/001-plugin-runtime/performance.md`.

**Spike baseline (Linux)**, kept for history; [performance.md](performance.md) has the
measurements on all three systems: load 16–18 ms for a 37 KB plugin (78 ms for a 189 KB one);
instantiate 25–90 µs; render 23–35 µs.

**Alternatives considered**: `criterion` benchmarks (more dependencies, statistical detail not
needed yet).

## R8. Process-isolation decision record

**Decision**: record it as `docs/decisions/0001-plugin-process-isolation.md` (status, date,
context, decision, consequences, revisit triggers).

**Recommendation for that record**: run plugins in-process during Phase 1. Wasmtime's sandbox
is the trust boundary under Principle I, the measured call cost is microseconds that a process
hop would dominate, and every limit is enforced in-process. The accepted risk is a sandbox
escape in Wasmtime or Cranelift; it is mitigated by pinned versions, `cargo audit` and fast
updates. Revisit before third-party plugins can be installed (Phase 3 alpha), or immediately if
a Wasmtime sandbox-escape advisory appears.

## R9. Default limits

| Limit | Default | Basis |
| --- | --- | --- |
| Plugin package size | 10 MiB | Spike plugins were 37–189 KB |
| CPU budget (fuel) | 500,000,000 units | About 0.35 s of tight compute on the reference machine; 260,000× a hello render |
| Time limit | 1 s (100 epoch ticks of 10 ms) | Generous for a widget; SC-002 adds at most 1 s |
| Memory | 32 MiB per call | A hello plugin needs about 1 MiB |
| Widget content | 64 KiB | Far above any widget's JSON |
| Log messages | 100 per call, 1 KiB each (truncated) | FR-010 |

User Story 4 measured the runtime on all three systems and confirmed every default
([performance.md](performance.md)).

Limits are fields of a host-side `Limits` value; nothing in the plugin can read or change them
(FR-006).
