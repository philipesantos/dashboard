# Storage

A widget can keep data of its own between runs: a store of named values that only it reaches,
carried by the `storage` permission and its quota. This guide starts from the example
`update-counter`, which counts its updates, in the clone of [the walkthrough](../getting-started.md).

## The widget

`update-counter` builds the world `stored-widget`: it imports `storage` and exports `updater` as
well as `renderer`. Its manifest asks for storage with a quota, the most its store may hold:

```json
"permissions": [{ "name": "storage", "required": true, "quota_bytes": 65536 }]
```

Its `update` reads `count`, adds one and writes it back; its `render` reads it and shows it.

```console check output=any
$ cd dashboard/plugins/examples/update-counter
$ cargo build --release --target wasm32-unknown-unknown
```

```console check
$ runtime pack ../../target/wasm32-unknown-unknown/release/update_counter.wasm --manifest manifest.json --output update-counter.wasm
packed update-counter.wasm
digest: sha256:<digest>
$ runtime permissions update-counter.wasm
plugin: local/update-counter 0.1.0
author: Dashboard contributors
contract: v1 (1.2.0)
approved: no
permissions:
  storage  required  not granted
storage: quota 65536 bytes (64 KiB), not approved, used 0 bytes
login providers: none
approval digest: sha256:<digest>
$ runtime approve update-counter.wasm --expect sha256:<digest>
approved local/update-counter 0.1.0
granted: storage
storage: quota 65536 bytes
```

## Writes in update, reads anywhere

```console check
$ runtime update update-counter.wasm
updated local/update-counter 0.1.0
storage: 4 calls, 2 writes kept, used 147 of 65536 bytes
$ runtime update update-counter.wasm
updated local/update-counter 0.1.0
storage: 3 calls, 1 writes kept, used 147 of 65536 bytes
$ runtime run update-counter.wasm
{
  "first_run_ms": "0",
  "items": [],
  "text": "Updated 2 times",
  "title": "Update counter"
}
```

What the store does:

- **Only the widget's.** The host picks the store from the widget's id and the package's origin.
  No function names a store, a file or a place, so no widget reaches another's.
- **All or nothing.** The writes of one `update` are kept together, only when it returns `ok`
  with data the host keeps; an error keeps none of them. `render` reads one consistent view and
  never writes: `set` there answers `refused(render-only-reads)`.
- **Bounded.** Names up to 256 bytes, values up to 256 KiB, the quota the manifest declares
  (usage is every name and value plus 64 bytes an entry), and limits per run
  ([limits](../reference/limits.md)).

```console check
$ runtime storage show update-counter.wasm
store: local/update-counter from <path>
used 147 of 65536 bytes, 2 entries
"count" = "2"
"first_run_ms" = "0"
```

## Every answer of a call

Each storage function answers `ok` or one of three errors
([the contract](../reference/contract.md#storage)):

- `not-granted`: storage is optional and off, or was turned off during the run. Nothing was read
  or written; when it was turned off during an `update`, the update fails and none of its writes
  are kept. `update-counter`'s `render` shows "Storage is off" instead of failing.
- `refused(problem)`: the host refused before changing anything: a write in `render`, a write
  past the quota, a name too long, a limit.
- `failed(problem)`: the store could not do it: another process is writing it (`busy`), its
  file is not one the host wrote (`damaged`), the data directory is unavailable, the store's file
  or the disk is full (`over-limit`). A write that fails ends the update's writes: every later
  storage call of the run answers `run-aborted`, and the update fails whatever it returns, keeping
  none of them. When the store cannot be opened in `update`, later calls answer `run-aborted` too,
  but nothing was written, so what `update` returns still stands.

The person can see a store with `runtime storage show` and empty it with `runtime storage clear`.
Removing the widget (`runtime remove`) removes its approval and the data the host kept, but keeps
its store, until `runtime storage clear` deletes it.
