# plugins/hostile

Hostile plugins: the fixtures of the security test suite. Each one breaks one rule on purpose,
and [`runtime/tests/hostile.rs`](../../runtime/tests/hostile.rs) passes only when the runtime
blocks it with the error kind that names that rule and then renders a well-behaved plugin
normally. They are security test fixtures and are never shipped.

| Crate | What it does | Expected outcome |
| --- | --- | --- |
| `infinite-loop` | Writes one log, then loops forever | `cpu-budget-exceeded`, or `time-limit-exceeded` when fuel is raised; the log is kept |
| `huge-alloc` | Allocates 1 GiB | `memory-limit-exceeded` |
| `deep-recursion` | Recurses until the stack runs out | `plugin-crashed` |
| `read-file` | Reads `/etc/passwd` with `std::fs` | `capability-refused`, listing `wasi:filesystem/*` |
| `open-socket` | Opens a TCP connection with `std::net` | `capability-refused`, listing `wasi:sockets/*` |
| `malformed-output` | Returns content that is not JSON | `invalid-output` |
| `oversized-output` | Returns 128 KiB of JSON | `output-too-large` |
| `log-flood` | Writes 1,000 messages, the first 4 KiB long | Renders; 100 messages kept, 900 dropped, the long one cut |
| `state-leak` | Counts its calls in a static | Renders `{"calls":1}` every time, also across two loads |
| `out-of-bounds` | Reads far beyond its own memory | `plugin-crashed` |
| `log-spoof` | Logs and errors with line breaks and terminal escapes | `plugin-error`, with every text escaped |

The [data model](../../specs/001-plugin-runtime/data-model.md#hostile-plugin-case) is the
source of these outcomes, and [`docs/threat-model.md`](../../docs/threat-model.md) maps them to
threats.

Since feature 002 every crate here carries a manifest (`manifest.json`, id `local/<crate>`),
declaring `log` as required when it logs, and each test approves it first, so the case still
meets the rule it breaks.

## Permission cases

The cases of [feature 002](../../specs/002-plugin-permissions/data-model.md#hostile-plugin-cases)
break the permission rules, which are about the package and the grants, not the plugin's code.
So they need no crate of their own: `runtime/tests/hostile.rs` packs the bare fixtures
[`probe`](../fixtures/probe) (calls `log` and returns what it answered) and
[`quiet`](../fixtures/quiet) (imports nothing) with each case's manifest.

| Case | Rule | Expected outcome |
| --- | --- | --- |
| `no-manifest` | FR-001 | `no-manifest` |
| `invalid-manifest` | FR-003 | `invalid-manifest`, naming the field |
| `two-manifests` | FR-004 | `manifest-mismatch` |
| `nested-manifest` | FR-004 | `manifest-mismatch` |
| `undeclared-capability` | FR-006 | `not-declared`, naming `log`, even when approved |
| `ungranted-capability` | FR-007 | `not-granted`, naming `log` |
| `unknown-permission` | FR-008 | `unknown-permission`, naming it |
| `escalating-update` | FR-011 | `needs-new-approval`, naming `log`; the approval unchanged |
| `optional-off` | FR-019 | Runs; `log` answers `not-granted` and nothing is logged |
| `same-id-other-origin` | FR-020 | `not-granted`, naming the file approved; the approval unchanged |
| `grant-record-untouched` | FR-009, FR-010 | Every case above, and a probe using `log`, leaves the record in its file unchanged |

## How they are built

Like any plugin, from source, by `cargo xtask plugins`, into `target/plugins/<crate>.wasm`;
no built `.wasm` is ever committed. `read-file` and `open-socket` declare
`[package.metadata.dashboard] target = "wasm32-wasip2"`, so they import WASI the way the
standard library does; the others build for `wasm32-unknown-unknown`. A test whose fixture was
not built fails instead of skipping.

## Adding a case

Every security rule has a test that breaks the build when the rule is broken (Principle VIII),
and the suite only grows: never delete or weaken a case.

1. Add a crate here that breaks the rule, with its `manifest.json`, and add it to `members` in
   `plugins/Cargo.toml`. A case about the package or the grants can pack `probe` or `quiet`
   instead, as the permission cases do.
2. Add its test to `runtime/tests/hostile.rs`, asserting the error kind and calling
   `assert_host_still_works`; a permission case also joins `PERMISSION_CASES`, so
   `grant_record_untouched` runs it.
3. Add the case to the data model's table and, if it is a new threat, to the threat model.
