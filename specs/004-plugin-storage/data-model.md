# Data Model: Plugin Storage

What the host keeps and checks for plugin storage. Decisions and their reasons are in
[research.md](research.md); the contract is [contracts/widget-v1.2.wit](contracts/widget-v1.2.wit),
the manifest schema [contracts/manifest.schema.json](contracts/manifest.schema.json) and the CLI
[contracts/cli.md](contracts/cli.md).

## Manifest: the `storage` permission

An entry of `permissions` (R2):

| Field | Rule |
| --- | --- |
| `name` | `storage` |
| `required` | `true` or `false`, as for every permission (feature 002) |
| `quota_bytes` | Required on `storage`, refused on any other permission; an integer from `storage_entry_cost_bytes` (64) to the host's cap (`max_storage_quota_bytes`, 8 MiB) |

`storage` appears at most once. Refusals name the field path, such as `permissions[1].quota_bytes`
(`invalid-manifest`). A quota above the cap of the host that loads it is `invalid-manifest`,
naming both bounds, even when the schema's default cap allows it.

## Permission registry

| Name | v0 interface | v1 interface | Since |
| --- | --- | --- | --- |
| `log` | `dashboard:widget/log@0.1.0` | `dashboard:widget/log` | 1.0.0 |
| `storage` | none | `dashboard:widget/storage` | 1.2.0 |

Connectors keep their own path (feature 003, R12).

## Contract versions

| Version | Where | Adds |
| --- | --- | --- |
| 1.0.0 | `wit/history/1.0.0/widget.wit` (frozen, pinned by its SHA-256 in a test) | `log`, `renderer` |
| 1.1.0 | `wit/history/1.1.0/widget.wit` (new, frozen, pinned the same way) | `connectors`, `updater`, world `connected-widget` |
| 1.2.0 | `wit/v1/widget.wit` | `storage`, worlds `stored-widget` and `connected-stored-widget` |

## Store binding

What the host links a granted plugin with (R3), computed at load step 9 and never from a call:

| Field | Value |
| --- | --- |
| `id` | The manifest's id |
| `origin` | The package's origin (in the CLI, its file's canonical path) |
| `file` | `<data dir>/storage/<hex SHA-256 of "<id>\n<origin>">.sqlite`, the data directory absolute and canonical (R4) |
| `quota_bytes` | The manifest's declared quota |
| `file_cap_bytes` | `5 × storage_quota_bytes + 64 KiB` (the approved quota), rounded up to a whole page, never below the file's current size (R10) |

## Store file

One SQLite database per store (R5), with exactly this schema, which the host checks at every open:

| Table | Columns | Content |
| --- | --- | --- |
| `store` | `key TEXT PRIMARY KEY`, `value ANY` (STRICT, WITHOUT ROWID) | `format` = 1, `id`, `origin`, `usage` |
| `entries` | `name BLOB PRIMARY KEY`, `value BLOB` (STRICT, WITHOUT ROWID) | One row per entry; `name` is the UTF-8 bytes of the name |

A file whose schema, `format`, `id` or `origin` differs from what the binding expects is
`damaged`; the host neither changes nor deletes it on its own. `storage list`, which has no
binding, shows as `damaged` a file whose name is not the hash of the `id` and `origin` in it. A
hot journal beside a store (a process killed during an update) is rolled back before reading (R8).

## Entry and usage

| Item | Rule |
| --- | --- |
| Name | Text, 1 to `max_storage_name_bytes` (256) bytes in UTF-8; any characters, NUL included; compared and sorted as bytes |
| Value | Bytes, 0 to `max_storage_value_bytes` (256 KiB) |
| `list` prefix and `after` | At most 256 bytes each; an empty prefix lists every name |
| Entry usage | `len(name) + len(value) + storage_entry_cost_bytes` |
| Store usage | Sum over entries, kept in `store.usage`, changed in the transaction of each write |
| Write check | Usage after the write (old entry gone, new one counted) ≤ quota, or the write fails as `over-quota`; deletes always pass |

Usage is logical, not disk space: a file may reach about 4.4 times its usage (R6), which the file
cap allows for.

## Run state

Kept by the real `storage` interface for one `render` or `update` (R7, R9):

| Field | Meaning |
| --- | --- |
| `phase` | `render` or `update` |
| `connection` | None until the first storage call; then read-only (render) or read-write in `BEGIN IMMEDIATE` (update) |
| `calls` | Storage calls so far, refused ones included |
| `written_bytes` | Names and values of every `set` so far (update) |
| `taken_away` | Set when a call found storage no longer granted; every later call answers `not-granted`, and the transaction rolls back |
| `aborted` | Set when SQLite ended the run's transaction (the file cap, a full disk, an I/O error); every later call answers `failed(run-aborted)`, and the update fails |

The stand-in keeps no run state: it answers every call `not-granted` and counts nothing.

## A storage call

The first rule that applies answers.

| Step | Check | Answer |
| --- | --- | --- |
| 0 | Linked to the stand-in (optional storage off) | `not-granted` |
| 1 | `calls` reached `max_storage_calls_per_run` (counted first) | `refused(over-limit)` |
| 2 | `taken_away`, or the grant record, read again under its lock, no longer grants storage from this origin (then `taken_away` is set) | `not-granted` |
| 3 | `aborted` | `failed(run-aborted)` |
| 4 | `set` or `delete` in `render` | `refused(render-only-reads)` |
| 5 | A name, prefix or `after` empty (name only) or over 256 bytes; a value over 256 KiB; a `set` past `max_storage_written_bytes_per_update` | `refused(invalid-name)`, `refused(over-limit)` |
| 6 | Open the connection, hardened (R4); roll back a hot journal (R8); check the file (R5) | `failed(damaged)`, `failed(busy)`, `failed(unavailable)` |
| 7 | `set`: usage after it within the quota (R6) | `refused(over-quota)` |
| 8 | Run the statement. If it fails and SQLite ended the transaction (`is_autocommit()`), set `aborted` | `failed(over-limit)` (the file cap), `failed(unavailable)`, `failed(busy)` |
| 9 | Answer | the value, page or usage |

`list(prefix, after)` returns at most `storage_list_page` (100) names, in byte order, all starting
with `prefix` and greater than `after`, and `next`: the last name returned when more remain.
`usage()` returns the store's usage and quota.

## End of a run

| Run | Outcome | Store |
| --- | --- | --- |
| `render` | Any | The read transaction ends; nothing to keep |
| `update` | Returned `ok`, valid output within its size, not aborted, storage still granted (checked under the grant record's lock, held until the commit ends) | Committed, then the kept data is recorded |
| `update` | Commit fails (busy, disk full) | Rolled back; the update fails as `storage-failed`, previous kept data stays |
| `update` | Aborted (R7) | Nothing of it kept; the update fails as `storage-failed`, naming the first failure |
| `update` | Storage taken away during the run, or between its last call and the commit | Rolled back; the update fails as `not-granted`, naming storage |
| `update` | Error, trap, any limit, invalid or oversized output | Rolled back |

## Grant record, version 3

Version 2 (feature 003) plus, in each entry:

| Field | Rule |
| --- | --- |
| `storage_quota_bytes` | The quota of the last approval that covered storage; present only once an approved manifest declared storage. `approve` sets it to the quota it showed; a load that needs no approval never changes it |

Versions 1 and 2 are read as version 3 without it and written back as 3. A newer version is
refused, as before.

## Approval digest

The document of feature 003 gains, only when the manifest declares storage:

```json
"storage": { "required": true, "quota_bytes": 262144 }
```

So widgets without storage keep the digest, and the kept data, they had. `approve` needs
`--expect <digest>` whenever storage is declared.

## Load decision

`Runtime::load` as in feature 003, with these changes:

| Step | Check | Refusal |
| --- | --- | --- |
| 3 | The manifest, with `quota_bytes` on `storage` | `invalid-manifest` |
| 3a | The declared quota within this host's cap | `invalid-manifest`, naming both bounds |
| 5 | v1 accepts any minor up to 1.2.0; storage needs 1.2 or later; a plugin that declares storage exports `updater` | `contract-mismatch`, `contract-too-old`, `missing-updater` |
| 6 | Imports, as before; `storage` through the registry | `capability-refused`, `not-declared` |
| 7 | Grants, as before, plus: storage required, or optional and on, has `storage_quota_bytes` ≥ the declared quota | `needs-new-approval`, naming both quotas |
| 8a | Storage granted: `<data dir>/storage/` exists or can be created and written, and the linked SQLite is 3.54.0 with defensive mode (R1, R11) | Required: `storage-failed`, naming the cause; optional and on: linked, its calls `failed(unavailable)` |
| 9 | Link: `storage` real with the store binding when granted; the stand-in when optional and off | `contract-mismatch` |

No step deletes a store. Turning storage off, removing the approval and pruning keep it.

## Error kinds

New: `storage-failed` (an update's storage could not be committed: busy, damaged, unavailable,
the file cap, an aborted run; or, at load, required storage that is unavailable), with the cause
in its detail. Existing kinds gain cases: `invalid-manifest` (the quota), `contract-too-old`
(storage before 1.2.0), `missing-updater` (storage without `updater`), `needs-new-approval` (a
quota above the approved one, at load or when turning storage on), `approval-changed` (no or
another digest for a plugin with storage), `not-granted` (an update whose storage was taken
away).

Calls answer with the WIT `storage-error`: `not-granted`, `refused` with `render-only-reads`,
`over-quota`, `over-limit` or `invalid-name`, and `failed` with `damaged`, `busy`, `unavailable`,
`over-limit` (the file cap, which ends the run) or `run-aborted`.

## Combinations (SC-002)

| Declared | Approved | On | Called from | Outcome |
| --- | --- | --- | --- | --- |
| No | any | any | any | Import refused at load as `not-declared`; a plugin that does not import it has no storage |
| Required | No | — | — | Refused at load, `not-granted` |
| Required | Yes, quota ≥ declared | — | render | Reads its own store; writes `render-only-reads` |
| Required | Yes, quota ≥ declared | — | update | Reads and writes its own store, kept on success |
| Required | Yes, quota < declared | — | — | Refused at load, `needs-new-approval` |
| Optional | Yes | Off | any | Stand-in: `not-granted`, store untouched |
| Optional | Yes, quota < declared | Turning on | — | `optional … storage on` refused, `needs-new-approval` |
| Optional | Yes | On | as required | As required |
| any | Yes, from another origin | — | — | Refused at load, `not-granted`; once approved, its own store |

`tests/permissions.rs` names a test for each row; the rows the hostile cases below already cover
are cross-referenced there.

## Hostile cases (FR-017)

They pack the fixture [`store-probe`](../../plugins/fixtures/store-probe) with each case's manifest,
except `storage-file`, which is its own crate. After each, the example `update-counter` updates and
renders normally. `host-records-untouched` follows feature 002's pattern: it takes a snapshot of
the grant record, the kept data, the request log and every store after each case's setup, and
compares after the case runs, except what the case changes on purpose (its own store on a
successful update, its grants when the case changes them).

| Case | Rule | Expected outcome |
| --- | --- | --- |
| `other-plugin` | FR-002, FR-003 | B reads, lists and writes with A's names: only its own store; A's store unchanged |
| `same-id-other-origin` | FR-002 | `not-granted`; approved, its own empty store |
| `name-injection` | FR-003, FR-004 | Names with `../`, `/`, `\`, NUL, quotes, line breaks, SQL, `ATTACH …`, `%` and `_` round-trip exactly; a prefix `a%` lists no `ab` |
| `storage-file` | FR-002 | `capability-refused`, listing `wasi:filesystem/*` |
| `quota-raised` | FR-008 | `needs-new-approval`, naming both quotas; the approval unchanged; and after a re-approval at a smaller quota, a version back at the earlier larger one is refused again |
| `approval-digest` | FR-008 | `approve` with no or a stale digest: `approval-changed`, nothing granted |
| `quota-fill` | FR-006 | `refused(over-quota)`, the write not kept, usage ≤ quota; replacing counts the old entry gone |
| `quota-lowered` | FR-006 | Data kept; deletes succeed; a write that leaves usage over the new quota is refused; the file cap stays that of the approved quota |
| `many-entries` | FR-005, FR-007 | Thousands of empty values, then values of 32 B, 500 B, 1,000 B, 1,500 B and 4 KiB up to the quota: each counts the entry cost; `over-quota` comes before the file cap; the file stays under its cap |
| `churn` | FR-007 | Rewrites and deletes over many updates, with the same value sizes: the file never passes its cap |
| `file-cap-hit` | FR-007, FR-010 | With test limits, a write reaches the file cap: `failed(over-limit)`; later calls `failed(run-aborted)`; then the update (a) returns `ok` and (b) traps: in both, the store exactly as before and the update failed as `storage-failed` |
| `over-limits` | FR-007 | A name of exactly 256 B with a value of exactly 256 KiB is kept; one byte more of either, a prefix or `after` over 256 B, calls per run, bytes per update: each `invalid-name` or `refused(over-limit)` |
| `write-in-render` | FR-009 | `refused(render-only-reads)`; store unchanged |
| `crash-after-writes` | FR-010 | Writes, then a panic, out of fuel, out of memory, the time limit, an error, invalid or oversized output: store as before |
| `killed-mid-update` | FR-010, R8 | The `runtime update` process killed after writing past the page cache: `run` and `storage show` read the state from before the update; nothing reported as `damaged` |
| `optional-off` | FR-001 | `not-granted`, nothing read or written; data there when turned on |
| `taken-away-mid-update` | FR-011 | Storage turned off, or the approval removed, by another grant store while the probe spins after its writes, the change made once the store's journal exists: (a) with no further call, the update still returns `ok` and nothing is kept, the update failing as `not-granted`; (b) with one more call, it answers `not-granted`; (c) in a render with a 10 s limit, the second read answers `not-granted` |
| `show-spoof` | FR-014 | `runtime storage show` prints names and values with line breaks, terminal escapes, `"` and `\` escaped, one entry per line; no entry can pass for two fields |
| `foreign-store` | FR-004, R5 | A store file replaced by another database (an extra table, a trigger, a view, another id), or a file renamed in the directory: `failed(damaged)` or `damaged` in `storage list`; nothing run, changed or deleted; other stores work |
| `store-hardened` (white box) | FR-004 | The production setup function, read back and tried layer by layer: attach limit 0, length and SQL-length limits, defensive on, triggers off, views off, trusted schema off, `max_page_count` and `secure_delete` as set; `ATTACH`, any function, `PRAGMA`, `CREATE`, an over-long statement and value refused; render's connection read-only |
| `store-binding` (white box) | FR-002 | The linked binding is the file of the plugin's id and origin; the stand-in has none |
| `host-records-untouched` | FR-013 | Every case above leaves the grant record, kept data, request log and other stores as its snapshot, except what the case changes on purpose |

## Other tests named

Rules that are not hostile cases, each with its own test:

| Test | Where | Checks |
| --- | --- | --- |
| `storage_needs_its_declaration` | `tests/permissions.rs` | A component importing storage without the manifest declaring it: `not-declared` |
| `required_storage_needs_approval` | `tests/permissions.rs` | Required storage not approved: `not-granted` |
| `turning_storage_on_checks_the_quota` | `tests/permissions.rs` | `optional … storage on` above the approved quota: `needs-new-approval` |
| `approval_records_the_quota_shown` | `tests/permissions.rs` | Re-approving at a smaller quota lowers `storage_quota_bytes` |
| `storage_unavailable_at_load` | `tests/storage.rs` | A data directory that cannot hold `storage/`: required refused as `storage-failed`; optional calls `failed(unavailable)` |
| `data_dir_is_absolute` | `tests/storage.rs`, `tests/cli.rs` | A relative `--data`, and one starting with `file:`, give store paths that are absolute files, never URIs |
| `bundled_sqlite` | `tests/storage.rs` | The linked SQLite is 3.54.0 and defensive mode takes effect |
| `frozen_contracts` | `tests/contract_v1.rs` | `wit/history/1.0.0` and `1.1.0` match their pinned SHA-256 |
