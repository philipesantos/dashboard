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

## How they are built

Like any plugin, from source, by `cargo xtask plugins`, into `target/plugins/<crate>.wasm`;
no built `.wasm` is ever committed. `read-file` and `open-socket` declare
`[package.metadata.dashboard] target = "wasm32-wasip2"`, so they import WASI the way the
standard library does; the others build for `wasm32-unknown-unknown`. A test whose fixture was
not built fails instead of skipping.

## Adding a case

Every security rule has a test that breaks the build when the rule is broken (Principle VIII),
and the suite only grows: never delete or weaken a case.

1. Add a crate here that breaks the rule, and add it to `members` in `plugins/Cargo.toml`.
2. Add its test to `runtime/tests/hostile.rs`, asserting the error kind and calling
   `assert_host_still_works`.
3. Add the case to the data model's table and, if it is a new threat, to the threat model.
