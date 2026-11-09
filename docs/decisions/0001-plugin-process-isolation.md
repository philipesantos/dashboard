# 0001. Plugins run in the host process during Phase 1

- **Status**: Accepted. The maintainer confirmed it on 2026-10-22 (SC-006).
- **Date**: 2026-10-22
- **Feature**: [001, plugin runtime](../../specs/001-plugin-runtime/spec.md) (FR-016)

## Context

The runtime (`runtime/`) loads each plugin as a WebAssembly component and runs it with
Wasmtime. The question is whether that happens inside the host process, the app itself, or in a
separate operating-system process that only talks to the host.

What Phase 1 has in place:

- The Wasmtime sandbox is the trust boundary (Principle I). A plugin can import only the `log`
  capability; anything else is refused at load (Principle II).
- Every render call runs in a fresh instance, with a CPU budget, a time limit and a memory cap.
  The hostile-plugin suite proves each one on Windows, macOS and Linux.
- Wasmtime is built with only the features the runtime needs (no GC, threads, async or stack
  switching), and the WebAssembly proposals it does not need are turned off.

What a call costs, measured on the reference machine (8 cores; see
[performance.md](../../specs/001-plugin-runtime/performance.md) for the three CI systems):

- Rendering hello-widget, instantiation included: about 50 µs at the median.
- Loading it, which compiles it: about 22 ms.

A separate process would add a round trip between processes to every call. That is tens of
microseconds or more, as much as the call itself, before any work to keep the process alive,
restart it and pass logs back.

Compiling is the exception to the limits above. It happens when a plugin loads, before any
call, so no CPU budget, time limit or memory cap applies; only the 10 MiB package cap does, and
compilation uses every core. A package near that cap took 0.33–0.49 s and up to about 590 MB on
the reference machine (`bench/src/bin/compile_cost.rs`).

## Decision

During Phase 1, plugins run in the host process. No separate process is built yet.

## Consequences

- Calls stay at the measured cost, and every limit keeps working as tested, with no
  inter-process protocol to design, secure or test.
- Accepted risk: a sandbox escape in Wasmtime or Cranelift would run with the host's privileges,
  which include the user's data and, from Phase 2 on, access to the keychain. It is mitigated by
  pinned versions, `cargo audit` on every change and every week, minimal Wasmtime features, and
  updating fast when an advisory appears.
- Accepted risk: a package crafted to be slow or memory-hungry to compile can take the host's
  CPU and memory while it loads (the threat-model row "Package crafted to be slow or
  memory-hungry to compile"). In Phase 1 only the maintainer loads plugins, from the CLI.

## Revisit when

- Before the app can install a plugin it did not build itself. Installation comes only from
  repositories (the threat model; this line once said "from a local file", which the project no
  longer plans), and Phase 3's plan reviewed this decision first (below). The likely direction is
  compiling at install time in a separate process with operating-system limits, then loading the
  result; whether rendering also moves out of process is decided then.
- Immediately when an advisory reports a sandbox escape in Wasmtime or Cranelift.
- When loading a package near the size cap takes more than 2 s, or more than 1 GiB of memory,
  on any CI system.
- Before AI-generated modules are compiled on the user's machine (Phase 5).

## Revisited for the host window (feature 008, 2026-11-09)

[Feature 008](../../specs/008-host-window-dialogs/research.md) (research R2) is the first plan of
Phase 3, where this decision asked to be reviewed at the latest. It stands, with these changes:

- **Plugins keep running, and compiling, in the host process.** The window now compiles packages
  at start and on every reload, where before only the command-line tool did; it loads only
  packages the maintainer approved with that tool, the trust this decision already accepts. It
  measures the compile time of each package it loads, so the third trigger below can be seen from
  the window as well as from the bench.
- **No image, SVG or font a plugin supplies reaches a decoder.** GPUI decodes in the host process,
  beside the vault, so the window keeps plugin bytes out: frames show no widget content yet, no
  image block or plugin icon exists, GPUI's image and SVG loaders are banned outside one module
  that takes only the host's bytes, and the only fonts are the host's own (with the system's for
  glyphs they lack). A plugin's name reaches text shaping only as text in those fonts.
- **All four triggers stay**, unchanged except for the correction above.
- **Compiling in a separate process** (the option below) was weighed and deferred: until the app
  installs a package the maintainer did not build, the window compiles only what the maintainer
  already compiles with the tool. Moving the whole runtime out now would mean designing and
  securing an inter-process protocol before any plugin can be installed; the cost is better paid
  where the risk arrives.

Next owners: **installation**, for where the runtime and compilation run once the app installs
packages it did not build; **the widget kit**, before any image block or plugin-supplied icon,
for where decoding runs.

## Options considered

- **In process** (chosen): fastest and simplest; the sandbox is the only boundary.
- **One separate process for every plugin**: the strongest isolation, the most memory, and a
  process round trip on every call.
- **One plugin-host process for all plugins**: keeps a sandbox escape away from the host's
  secrets, but plugins share that process with each other.
- **Compile in a separate process, run in the host**: takes the compile cost out of the host,
  but keeps the risk of an escape at run time. A candidate for Phase 3.
