# Feature Specification: Plugin Runtime

**Feature Branch**: `001-plugin-runtime`

**Created**: 2026-10-21

**Status**: Draft

**Input**: User description: "Plugin runtime (Phase 1 of the development plan: sandbox proof of
concept, no graphical interface yet). Goal: prove, before any UI exists, that a hostile plugin
cannot do anything beyond what the host allows."

## User Scenarios & Testing *(mandatory)*

The people using this feature are the project maintainer, who runs plugins and the security
tests, and plugin authors, who write plugins against the first contract. End users are not
involved yet; they benefit later from the guarantees proven here.

### User Story 1 - Run a plugin and see its widget content (Priority: P1)

The maintainer points the command-line test tool at a plugin file, optionally with a
configuration, and sees the widget content the plugin produced, plus any log messages the
plugin wrote.

**Why this priority**: every other story needs a host that can load a plugin, call it and get
its result. It is also the first proof that the plugin contract works end to end.

**Independent Test**: run the tool on a well-behaved plugin and check that it prints the
expected widget content and log messages.

**Acceptance Scenarios**:

1. **Given** a valid plugin file, **When** the maintainer runs the test tool on it with a
   configuration, **Then** the tool prints the widget content returned for that configuration
   and exits successfully.
2. **Given** a plugin that writes log messages while rendering, **When** it runs, **Then** the
   messages are shown separately from the widget content and marked as coming from the plugin.
3. **Given** a file that is not a valid plugin package, **When** the tool tries to load it,
   **Then** it refuses with an error that says why and exits with a failure status.

---

### User Story 2 - Hostile plugins are contained (Priority: P1)

The maintainer runs a suite of deliberately hostile plugins: one that never stops computing,
one that tries to allocate a gigantic amount of memory, one that tries to open a file and one
that tries to open a network connection. Each is stopped or refused, the host reports which
rule stopped it, and the host keeps working. The suite runs automatically on Windows, macOS and
Linux for every change that is not documentation-only and breaks the build if any hostile
plugin is not blocked.

**Why this priority**: this is the phase gate. Nothing user-visible may be built on the sandbox
until it passes (constitution, Development Workflow).

**Independent Test**: run the hostile-plugin suite; every case must be reported as blocked, and
a well-behaved plugin must still run correctly right afterwards.

**Acceptance Scenarios**:

1. **Given** a plugin that loops forever, **When** it is called, **Then** the call is stopped
   once its time budget runs out and the error names the time or CPU limit.
2. **Given** a plugin that tries to allocate memory beyond its cap, **When** it is called,
   **Then** the allocation fails, the call ends with an error naming the memory limit, and the
   plugin's memory never grows beyond the cap.
3. **Given** a plugin that tries to open a file or a network socket, **When** it is loaded or
   called, **Then** it is refused, because the host never offers those capabilities, and the
   error names the capability it asked for.
4. **Given** any hostile plugin was just stopped, **When** a well-behaved plugin runs next,
   **Then** it produces its normal result.
5. **Given** a plugin that returns malformed widget content, or content larger than the size
   cap, **When** it is called, **Then** the content is rejected with an error naming the rule.
6. **Given** a plugin that writes more or longer log messages than allowed, **When** it runs,
   **Then** the excess is dropped and the tool reports that messages were dropped.

---

### User Story 3 - Write a plugin against the v0 contract (Priority: P2)

A plugin author reads the v0 contract and the example plugin, written in Rust, builds it from
source and runs it with the test tool.

**Why this priority**: the contract only proves its value when a plugin built from it, outside
the runtime's own code, runs as expected. The example is also the starting point for every
future author.

**Independent Test**: build the example from the repository and run it with the test tool; it
must return the expected widget content.

**Acceptance Scenarios**:

1. **Given** the repository sources, **When** the author builds the example and runs it,
   **Then** it returns the expected widget content for a sample configuration.
2. **Given** the example, **When** the author changes the configuration, **Then** the widget
   content changes accordingly.

---

### User Story 4 - Know the runtime's cost and isolation model (Priority: P3)

The maintainer measures how long it takes to load a plugin and to render it, on each operating
system, sets performance targets from those numbers, and records whether plugins will run in a
separate operating-system process now or later.

**Why this priority**: these results shape the next phases (permissions, the dashboard UI) but
do not change whether the sandbox is secure today.

**Independent Test**: check that measurements exist for the example plugin on all three
systems, that targets are written down, and that the process-isolation decision is recorded
with its rationale.

**Acceptance Scenarios**:

1. **Given** the example plugin, **When** the measurements run on Windows, macOS and Linux,
   **Then** load time and render time are recorded for each system.
2. **Given** the measurements, **When** the phase is reviewed, **Then** documented performance
   targets and a process-isolation decision with rationale exist.

### Edge Cases

- A plugin file is corrupted, empty, far larger than any reasonable plugin, or not a plugin
  package at all.
- A plugin lacks the render entry point or was built for a different contract version.
- A plugin asks for a capability the host does not offer (files, network, clocks, random
  numbers, environment variables, starting processes): it must be refused before it can run
  any code that uses it.
- A plugin crashes or aborts while rendering.
- A plugin recurses until its stack is exhausted.
- A plugin returns malformed widget content, or content far larger than any widget needs.
- A plugin floods the host with log messages, or writes extremely long ones.
- A plugin writes log messages or error text with line breaks or terminal control sequences,
  to pass for the host's own output.
- A plugin hits a limit in the middle of writing logs or producing output: nothing partial is
  presented as a valid result.
- Two plugins run one after another: neither can see or change the other's memory or state.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The host MUST load plugins only from plugin packages that are sandboxed
  WebAssembly components (constitution, Principle I) and MUST refuse anything else with an
  error that states the reason.
- **FR-002**: The plugin contract v0 MUST define exactly one entry point, render, which receives
  a configuration and returns the widget content as structured data (JSON).
- **FR-003**: The only capability the host offers a plugin in v0 MUST be writing log messages.
  The host MUST NOT offer any other interface, such as files, network, clocks, random numbers,
  environment variables, process creation or the user's data (Principle II).
- **FR-004**: A plugin that asks for any capability the host does not offer MUST be refused
  before it runs, with an error naming that capability.
- **FR-005**: The host MUST enforce on every plugin call a CPU budget, a memory cap and a
  time limit. Exceeding any of them MUST stop the call and return an error that names the
  limit.
- **FR-006**: Limit values MUST be set by the host. A plugin MUST NOT be able to raise or
  remove its own limits.
- **FR-007**: After a plugin call fails or is stopped, the host MUST stay operational and able
  to run the same or another plugin immediately.
- **FR-008**: A plugin MUST NOT be able to read or change the memory or state of the host or of
  another plugin.
- **FR-009**: The host MUST check the widget content a plugin returns: content that is
  malformed or larger than a host-defined size cap MUST be rejected with an error.
- **FR-010**: The host MUST cap the size and number of log messages a plugin can write per
  call. The excess MUST be dropped, and the drop MUST be reported.
- **FR-011**: The command-line test tool MUST take a plugin file and an optional
  configuration. On success it MUST print the widget content and exit successfully; on
  failure it MUST print the error, including which rule or limit stopped the plugin, and exit
  with a failure status. Plugin log messages MUST be printed separately and marked as coming
  from the plugin. Text that comes from a plugin (log messages, error details, import names)
  MUST be shown so that it cannot pass for the host's own output: line breaks, terminal
  control sequences and bidirectional-text controls in it are escaped, never interpreted.
- **FR-012**: The repository MUST include an example plugin implementing contract v0, written
  in Rust and buildable from source.
- **FR-013**: The repository MUST include a hostile-plugin test suite with at least four
  cases: an infinite loop, a gigantic memory allocation, an attempt to open a file and an
  attempt to open a network socket. Each case MUST pass only when the hostile behavior is
  blocked.
- **FR-014**: The hostile-plugin suite MUST run automatically on Windows, macOS and Linux on
  every change except documentation-only ones (constitution, Development Workflow) and MUST
  fail the build if any case is not blocked (Principle VIII).
- **FR-015**: Plugin load time and render time MUST be measured for the example plugin on all
  three systems, and performance targets MUST be recorded from those measurements.
- **FR-016**: A decision on running plugins in a separate operating-system process, now or
  later, MUST be recorded in the repository with its rationale.

### Key Entities

- **Plugin package**: the file an author ships; a sandboxed WebAssembly component that
  implements a contract version.
- **Plugin contract (v0)**: the agreement between host and plugin: the render entry point
  and the log capability, nothing else.
- **Configuration**: structured data the host passes to render, describing what the widget
  should show.
- **Widget content**: the structured data render returns; later phases turn it into UI.
- **Log message**: text a plugin writes through the log capability, attributed to that plugin.
- **Resource limits**: CPU budget, memory cap, time limit, output size cap and log caps, all
  set by the host.
- **Hostile-plugin test case**: a plugin built to break one rule, plus the expected blocking
  outcome; the seed of the security test suite in `docs/threat-model.md`.
- **Performance record**: load and render times per system, and the targets derived from them.
- **Decision record**: the process-isolation decision and its rationale.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the hostile-plugin cases (at least four) are blocked on all three
  operating systems, on every change that is not documentation-only. This is the phase gate.
- **SC-002**: A runaway plugin is stopped no later than one second after its time limit, and a
  well-behaved plugin run right afterwards returns its normal result.
- **SC-003**: The example plugin returns the expected widget content on all three operating
  systems.
- **SC-004**: Every refusal or stop tells the maintainer, in one message, which rule or limit
  was involved, without reading source code or logs from the host.
- **SC-005**: Load and render times are recorded for the example plugin on all three systems,
  and written performance targets exist before the phase gate is declared.
- **SC-006**: The process-isolation decision is recorded with its rationale before the phase
  gate is declared.

## Assumptions

- No graphical interface is part of this feature; the command-line test tool is the only way
  to run plugins.
- The configuration is structured data (JSON), like the widget content.
- Default values for every limit are chosen in the plan, informed by the measurements of User
  Story 4; plugins cannot change them.
- Whether a plugin keeps state between render calls is decided in the plan; either way,
  FR-008 holds.
- The "attempt to open a network socket" case means the plugin asks for a networking
  capability, which the host refuses; there is no network capability in v0 to misuse.
- Running plugins inside the host process is acceptable for this phase if the recorded
  decision (FR-016) says so.
- Out of scope: any graphical interface, the manifest and permissions (Phase 2), network access
  through the proxy, storage, secrets and AI.
- Out of scope: plugins written in TypeScript. They move to Phase 2, where the minimal system
  interfaces their toolchain needs will be listed and justified.
