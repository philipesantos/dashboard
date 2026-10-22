# Data Model: Plugin Runtime

Host-side types of the `runtime` crate, derived from the spec's Key Entities. Names are
indicative; fields and rules are the contract.

## Limits

Host-defined resource limits (FR-005, FR-006). A plugin can neither read nor change them.

| Field | Type | Default | Rule |
| --- | --- | --- | --- |
| `max_package_bytes` | integer | 10 MiB | Larger packages are refused as `invalid-package` |
| `fuel` | integer | 500,000,000 | CPU budget per render call |
| `time_limit` | duration | 1 s | Wall-clock limit per render call, enforced in 10 ms ticks counted from real elapsed time |
| `max_memory_bytes` | integer | 32 MiB | Sum of all linear memories per render call |
| `max_output_bytes` | integer | 64 KiB | Checked before parsing the widget content |
| `max_log_messages` | integer | 100 | Per render call; the rest are dropped and counted |
| `max_log_message_bytes` | integer | 1 KiB | Longer messages are truncated |

Defaults are provisional until User Story 4 confirms them (see `research.md`, R9).

## Plugin

A loaded plugin package (FR-001, FR-004).

| Field | Meaning |
| --- | --- |
| compiled component | Compiled once at load |
| pre-instance | Linker resolved once, so each call only instantiates |

Lifecycle: `bytes` → size check → component check → compile → import allowlist check →
export check → **loaded**. Any failure stops at that step with its error kind; a loaded plugin
never changes afterwards.

## Configuration

A JSON document passed as text to `render` (spec Assumptions). The CLI validates it before
calling the plugin; the plugin receives it unchanged.

## Render report

The outcome of one render call. A fresh store and instance exist only during the call (R6).

| Field | Meaning |
| --- | --- |
| `result` | Either the widget content or a runtime error |
| `logs` | Log messages written before the call ended, in order |
| `dropped_logs` | Number of messages over `max_log_messages` |

Rule: on any error, no widget content is returned, even if the plugin produced some before it
stopped. Logs are still returned, because they help explain the failure.

## Widget content

A JSON value of at most `max_output_bytes`, parsed by the host before it is returned (FR-009).

## Log message

| Field | Rule |
| --- | --- |
| `level` | `debug`, `info`, `warn` or `error` |
| `message` | Escaped first: `char::escape_default` applied to every character for which `char::is_control` is true and to the bidirectional controls U+061C, U+200E, U+200F, U+202A–U+202E and U+2066–U+2069. Then at most `max_log_message_bytes`; longer text is cut and marked as truncated |

## Runtime error

One kind per rule (SC-004). The kinds and their detail are listed in
[contracts/cli.md](contracts/cli.md#error-kinds). Every detail goes through the same escaping
as a log message and is cut to 1 KiB; in `capability-refused`, each import name is escaped.

| Kind | Source |
| --- | --- |
| `invalid-package` | Load: size, magic number or compile failure |
| `contract-mismatch` | Load: missing or wrongly typed `renderer` export, or an import from another version of `dashboard:widget` |
| `capability-refused` | Load: imports outside the allowlist, all listed |
| `cpu-budget-exceeded` | Call: `Trap::OutOfFuel` |
| `time-limit-exceeded` | Call: `Trap::Interrupt` from the epoch deadline |
| `memory-limit-exceeded` | Call: the host limiter refused a memory growth |
| `plugin-crashed` | Call: any other trap |
| `plugin-error` | Call: `render` returned `err` |
| `invalid-output` | Call: the content is not valid JSON |
| `output-too-large` | Call: the content exceeds `max_output_bytes` |

## Hostile-plugin case

| Field | Meaning |
| --- | --- |
| plugin crate | Source under `plugins/hostile/<name>/` |
| target | `wasm32-unknown-unknown`, or `wasm32-wasip2` for file and socket access |
| expected outcome | The error kind that proves it was blocked, plus a well-behaved render right after |

| Case | Expected kind |
| --- | --- |
| `infinite-loop` | `cpu-budget-exceeded` with the default limits; `time-limit-exceeded` when fuel is raised |
| `huge-alloc` | `memory-limit-exceeded` |
| `deep-recursion` | `plugin-crashed` |
| `read-file` | `capability-refused`, listing `wasi:filesystem/*` |
| `open-socket` | `capability-refused`, listing `wasi:sockets/*` |
| `malformed-output` | `invalid-output` |
| `oversized-output` | `output-too-large` |
| `log-flood` | Success, with `dropped_logs` > 0 and truncated messages |
| `state-leak` | Success with `{"calls":1}` on every call, even alternating two loads (FR-008) |
| `out-of-bounds` | `plugin-crashed` |
| `log-spoof` | `plugin-error`, with every log message and the detail escaped (FR-011) |

## Performance record

`specs/001-plugin-runtime/performance.md`: for each operating system, the load time and
render p50/p95/max of the example plugin, the commit measured, and the targets derived from
them (FR-015).

## Decision record

`docs/decisions/0001-plugin-process-isolation.md`: status, date, context, decision,
consequences and revisit triggers (FR-016).
