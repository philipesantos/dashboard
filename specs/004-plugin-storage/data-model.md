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
| 1.0.0 | `wit/history/1.0.0/widget.wit` | `log`, `renderer` |
| 1.1.0 | `wit/history/1.1.0/widget.wit` (new, frozen) | `connectors`, `updater`, world `connected-widget` |
| 1.2.0 | `wit/v1/widget.wit` | `storage`, worlds `stored-widget` and `connected-stored-widget` |

## Store binding

What the host links a granted plugin with (R3), computed at load step 9 and never from a call:

| Field | Value |
| --- | --- |
| `id` | The manifest's id |
| `origin` | The package's origin (in the CLI, its file's canonical path) |
| `file` | `<data dir>/storage/<hex SHA-256 of "<id>\n<origin>">.sqlite` |
| `quota_bytes` | The manifest's declared quota |
| `file_cap_bytes` | `2 × quota_bytes + 64 KiB`, rounded up to a whole page (R10) |

## Store file

One SQLite database per store (R5), with exactly this schema, which the host checks at every open:

| Table | Columns | Content |
| --- | --- | --- |
| `store` | `key TEXT PRIMARY KEY`, `value ANY` (STRICT, WITHOUT ROWID) | `format` = 1, `id`, `origin`, `usage` |
| `entries` | `name BLOB PRIMARY KEY`, `value BLOB` (STRICT, WITHOUT ROWID) | One row per entry; `name` is the UTF-8 bytes of the name |

A file whose schema, `format`, `id` or `origin` differs from what the binding expects is
`damaged`; the host neither changes nor deletes it.

## Entry and usage

| Item | Rule |
| --- | --- |
| Name | Text, 1 to `max_storage_name_bytes` (256) bytes in UTF-8; any characters, NUL included; compared and sorted as bytes |
| Value | Bytes, 0 to `max_storage_value_bytes` (256 KiB) |
| Entry usage | `len(name) + len(value) + storage_entry_cost_bytes` |
| Store usage | Sum over entries, kept in `store.usage`, changed in the transaction of each write |
| Write check | Usage after the write (old entry gone, new one counted) ≤ quota, or the write fails as `over-quota`; deletes always pass |

## Run state

Kept by the real `storage` interface for one `render` or `update` (R7, R9):

| Field | Meaning |
| --- | --- |
| `phase` | `render` or `update` |
| `connection` | None until the first storage call; then read-only (render) or read-write in `BEGIN IMMEDIATE` (update) |
| `calls` | Storage calls so far, refused ones included |
| `written_bytes` | Names and values of every `set` so far (update) |
| `taken_away` | Set when a call found storage no longer granted; every later call answers `not-granted`, and the transaction rolls back |

## A storage call

The first rule that applies answers.

| Step | Check | Answer |
| --- | --- | --- |
| 0 | Linked to the stand-in (optional storage off) | `not-granted` |
| 1 | `calls` reached `max_storage_calls_per_run` (counted first) | `refused(over-limit)` |
| 2 | `taken_away`, or the grant record, read again under its lock, no longer grants storage from this origin (then `taken_away` is set) | `not-granted` |
| 3 | `set` or `delete` in `render` | `refused(render-only-reads)` |
| 4 | A name empty or over 256 bytes; a value over 256 KiB; a `set` past `max_storage_written_bytes_per_update` | `refused(invalid-name)`, `refused(over-limit)` |
| 5 | Open the connection, hardened (R4), and check the file (R5) | `failed(damaged)`, `failed(busy)`, `failed(unavailable)` |
| 6 | `set`: usage after it within the quota (R6) | `refused(over-quota)` |
| 7 | Run the statement; SQLite's "full" at the file cap | `refused(over-limit)` |
| 8 | Answer | the value, page or usage |

`list(prefix, after)` returns at most `storage_list_page` (100) names, in byte order, all starting
with `prefix` and greater than `after`, and `next`: the last name returned when more remain.
`usage()` returns the store's usage and quota.

## End of a run

| Run | Outcome | Store |
| --- | --- | --- |
| `render` | Any | The read transaction ends; nothing to keep |
| `update` | Returned `ok`, valid output within its size, storage still granted (checked again) | Committed, then the kept data is recorded |
| `update` | Commit fails (busy, disk full, damaged) | Rolled back; the update fails as `storage-failed`, previous kept data stays |
| `update` | Storage taken away during the run | Rolled back; the update fails as `not-granted`, naming storage |
| `update` | Error, trap, any limit, invalid or oversized output | Rolled back |

## Grant record, version 3

Version 2 (feature 003) plus, in each entry:

| Field | Rule |
| --- | --- |
| `storage_quota_bytes` | The largest quota approved for this entry; present only once an approved manifest declared storage |

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
| 9 | Link: `storage` real with the store binding when granted; the stand-in when optional and off | `contract-mismatch` |

No step deletes a store. Turning storage off, removing the approval and pruning keep it.

## Error kinds

New: `storage-failed` (an update's writes could not be committed: busy, damaged, unavailable,
full), with the cause in its detail. Existing kinds gain cases: `invalid-manifest` (the quota),
`contract-too-old` (storage before 1.2.0), `missing-updater` (storage without `updater`),
`needs-new-approval` (a quota above the approved one), `approval-changed` (no or another digest
for a plugin with storage), `not-granted` (an update whose storage was taken away).

Calls answer with the WIT `storage-error`: `not-granted`, `refused` with `render-only-reads`,
`over-quota`, `over-limit` or `invalid-name`, and `failed` with `damaged`, `busy` or
`unavailable`.

## Combinations (SC-002)

| Declared | Approved | On | Called from | Outcome |
| --- | --- | --- | --- | --- |
| No | any | any | any | Import refused at load as `not-declared`; a plugin that does not import it has no storage |
| Required | No | — | — | Refused at load, `not-granted` |
| Required | Yes, quota ≥ declared | — | render | Reads its own store; writes `render-only-reads` |
| Required | Yes, quota ≥ declared | — | update | Reads and writes its own store, kept on success |
| Required | Yes, quota < declared | — | — | Refused at load, `needs-new-approval` |
| Optional | Yes | Off | any | Stand-in: `not-granted`, store untouched |
| Optional | Yes | On | as required | As required |
| any | Yes, from another origin | — | — | Refused at load, `not-granted`; once approved, its own store |

## Hostile cases (FR-017)

They pack the fixture [`store-probe`](../../plugins/fixtures/store-probe) with each case's manifest,
except `storage-file`, which is its own crate. After each, the example `update-counter` updates and
renders normally, and every other store, the grant record, the kept data and the request log are
unchanged byte for byte (`host-records-untouched`).

| Case | Rule | Expected outcome |
| --- | --- | --- |
| `other-plugin` | FR-002, FR-003 | B reads, lists and writes with A's names: only its own store; A's store unchanged |
| `same-id-other-origin` | FR-002 | `not-granted`; approved, its own empty store |
| `name-injection` | FR-003, FR-004 | Names with `../`, `/`, `\`, NUL, quotes, line breaks, SQL, `ATTACH …`, `%` and `_` round-trip exactly; a prefix `a%` lists no `ab` |
| `storage-file` | FR-002 | `capability-refused`, listing `wasi:filesystem/*` |
| `quota-raised` | FR-008 | `needs-new-approval`, naming both quotas; the approval unchanged |
| `approval-digest` | FR-008 | `approve` with no or a stale digest: `approval-changed`, nothing granted |
| `quota-fill` | FR-006 | `refused(over-quota)`, the write not kept, usage ≤ quota; replacing counts the old entry gone |
| `quota-lowered` | FR-006 | Data kept; deletes succeed; a write that leaves usage over the new quota is refused |
| `many-entries` | FR-005, FR-007 | Thousands of empty values: each counts the entry cost; the file stays under its cap |
| `churn` | FR-007 | Rewrites and deletes over many updates: the file never passes its cap |
| `over-limits` | FR-007 | Name and value sizes, calls per run, bytes per update, page size: each `refused(over-limit)` or `invalid-name` |
| `write-in-render` | FR-009 | `refused(render-only-reads)`; store unchanged |
| `crash-after-writes` | FR-010 | Writes, then a panic, out of fuel, out of memory, the time limit, an error, invalid or oversized output: store as before |
| `optional-off` | FR-001 | `not-granted`, nothing read or written; data there when turned on |
| `taken-away-mid-update` | FR-011 | Storage turned off, or the approval removed, by another grant store during an update: next call `not-granted`; nothing kept; the update fails as `not-granted` |
| `show-spoof` | FR-014 | `runtime storage show` prints every name and value escaped, one entry per line |
| `foreign-store` | FR-004, R5 | A store file replaced by another database (extra table, a trigger, another id): `failed(damaged)`, nothing run, changed or deleted; other stores work |
| `store-hardened` (white box) | FR-004 | On the host's own connection, each layer alone: `ATTACH` refused by the limit and by the authorizer; `load_extension`, `PRAGMA`, `CREATE` refused; `writable_schema` refused; render's connection read-only |
| `store-binding` (white box) | FR-002 | The linked binding is the file of the plugin's id and origin; the stand-in has none |
| `host-records-untouched` | FR-013 | Every case above leaves the grant record, kept data, request log and other stores unchanged |
