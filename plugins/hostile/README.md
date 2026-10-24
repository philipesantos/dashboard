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
| `storage-file` | Declares storage, then opens a store file with `std::fs` | `capability-refused`, listing `wasi:filesystem/*` |

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

## Connector cases

The cases of [feature 003](../../specs/003-plugin-connectors/data-model.md#hostile-cases-fr-020)
are the gate case of Phase 2: no request leaves the machine outside what a widget declared and
the user approved. They live in
[`runtime/tests/hostile/connectors.rs`](../../runtime/tests/hostile/connectors.rs), part of the
same suite, and need no crate of their own either: each packs the fixture
[`net-probe`](../fixtures/net-probe), which makes the connector calls its configuration lists,
with the case's manifest, and writes the connectors the case needs. Requests go through the
runtime's test seam to HTTPS and HTTP test services on this machine, which record everything
they receive; a refused case leaves them empty. After each case, the example
[`github-releases`](../examples/github-releases) updates and renders normally on the same
runtime. Nothing reaches the internet.

| Case | Rule | Expected outcome |
| --- | --- | --- |
| `generic-http` | FR-001 | `capability-refused` for `wasi:http` 0.2 and 0.3 |
| `read-with-body` | FR-002 | `invalid-connector`: outside the catalog a read is a body-less `GET` or `HEAD` |
| `fixed-headers` | FR-002 | `invalid-connector` for `X-` headers outside the catalog and the headers the host owns |
| `credentials` | FR-017 | `credentials-not-supported` |
| `value-outside-type` | FR-003 | `refused(value-outside-type)`, nothing sent |
| `value-encoding` | FR-003 | Values stay in their own path segment, query value or body field |
| `repository-connector` | FR-004 | `destination-not-verified` |
| `local-outside-development` | FR-018 | `destination-not-verified` |
| `reserved-host` | FR-005 | `destination-reserved` (a trailing dot: `invalid-connector`) |
| `other-file` | FR-018 | `needs-new-approval`: another file inherits nothing |
| `connector-changed` | FR-014 | `needs-new-approval`, naming the change |
| `pruned-connector` | FR-014 | `needs-new-approval` |
| `approval-changed` | FR-014 | `approval-changed`, nothing granted |
| `undeclared-operation` | FR-006 | `refused(operation-not-declared)`; an undeclared connector: `not-granted` |
| `private-address` | FR-007 | `failed(non-public-address)`; IP destinations and plain HTTP never load |
| `rebinding` | FR-007 | The second lookup, now private, is refused |
| `exact-port` | FR-007 | `failed(redirect-not-allowed)` |
| `redirect-escape` | FR-007 | `failed(redirect-not-allowed)`, the target reached by nothing |
| `tls` | FR-007 | `failed(unavailable)`, nothing past the handshake, nothing fetched |
| `system-proxy` | FR-007 | Requests go straight to their destination; the proxy receives nothing |
| `cookie` | FR-007 | No `Cookie` is ever sent |
| `over-limits` | FR-008 | `over-limit` for every size, time, call and rate limit |
| `log-flood` | FR-009 | One summary entry per run past its limit; earlier entries kept |
| `log-spoof` | FR-009 | `runtime requests` prints every text escaped, one line per entry |
| `log-complete` | FR-009 | Every call and redirect has its entry or count; rotation keeps the newest |
| `call-in-render` | FR-010 | `refused(called-outside-update)`, nothing sent |
| `change-in-update` | FR-013 | `refused(change-needs-user-action)` |
| `optional-off` | FR-015 | `not-granted`; turning it off or removing the approval drops a response in flight |
| `connector-table` (white box) | FR-015 | The table holds exactly the granted connectors; the stand-in when none |
| `catalog-reserves` (white box) | FR-005 | A catalog connector outside its reserves is refused |

## Storage cases

The cases of [feature 004](../../specs/004-plugin-storage/data-model.md#hostile-cases-fr-017)
are the storage case of the Phase 2 gate: no plugin reads or changes another plugin's storage,
or anything outside its own store. They live in
[`runtime/tests/hostile/storage.rs`](../../runtime/tests/hostile/storage.rs) and pack the
fixture [`store-probe`](../fixtures/store-probe), which makes the storage calls its
configuration lists, with each case's manifest; only `storage-file` is a crate of its own. Every
case runs between two snapshots of the data directory (`host-records-untouched`): the grant
record, kept data, the request log and every other store must come out byte for byte as they
went in, except what the case changes on purpose. After each case, the example
[`update-counter`](../examples/update-counter) updates and renders normally on the same
runtime.

| Case | Rule | Expected outcome |
| --- | --- | --- |
| `other-plugin` | FR-002, FR-003 | B reads, lists and writes with A's names: only its own store; A's unchanged |
| `same-id-other-origin` | FR-002 | `not-granted`; approved, its own empty store; another id at the same path, too |
| `name-injection` | FR-003, FR-004 | Paths, NUL, quotes, line breaks, SQL and `ATTACH` text round-trip as names; `a%` is no pattern |
| `storage-file` | FR-002 | `capability-refused`, listing `wasi:filesystem/*` |
| `quota-raised` | FR-008 | `needs-new-approval`, naming both quotas |
| `approval-digest` | FR-008 | `approval-changed` with no or a stale digest, nothing granted |
| `quota-fill` | FR-006 | `refused(over-quota)`, the write not kept |
| `quota-lowered` | FR-006 | Data kept; deletes pass; a write must fit the new quota |
| `many-entries` | FR-005, FR-007 | Each entry counts its cost; the quota comes before the file cap |
| `churn` | FR-007 | Rewrites and deletes never take the file past its cap |
| `file-cap-hit` | FR-007, FR-010 | `failed(over-limit)`, then `failed(run-aborted)`; nothing kept, `storage-failed` |
| `unwritable-store` | FR-007, FR-010 | `failed(unavailable)`; nothing kept, `storage-failed` |
| `over-limits` | FR-007 | `invalid-name` or `refused(over-limit)` for names, values, calls and bytes per update |
| `write-in-render` | FR-009 | `refused(render-only-reads)`; nothing changed |
| `crash-after-writes` | FR-010 | A trap, a limit, an error or invalid output after writes: nothing kept |
| `killed-mid-update` | FR-010 | The process killed mid-update: the state from before, never `damaged` |
| `optional-off` | FR-001 | `not-granted`, nothing read or written; the data there when turned on |
| `taken-away-mid-update` | FR-011 | `not-granted` from the next call, and nothing of the update kept |
| `show-spoof` | FR-014 | `runtime storage show` prints every name and value escaped, one entry per line |
| `foreign-store` | FR-004 | A planted table, trigger, view or identity, or another plugin's file: `failed(damaged)`, left alone |
| `store-hardened` (white box) | FR-004 | Every hardening layer as set, every attempt past one refused |
| `store-binding` (white box) | FR-002 | The bound store is the file of the plugin's id and origin; the stand-in has none |
| `host-records-untouched` | FR-013 | Every case above leaves the host's records and other stores as they were |

## How they are built

Like any plugin, from source, by `cargo xtask plugins`, into `target/plugins/<crate>.wasm`;
no built `.wasm` is ever committed. `read-file`, `open-socket` and `storage-file` declare
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
