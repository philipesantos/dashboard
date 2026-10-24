# Contract: `runtime` command-line tool

Extends [feature 003's contract](../../003-plugin-connectors/contracts/cli.md), whose common rules
still hold: development mode only, local files, every text from a package or a store escaped
before it is printed, a refusal printed as `error: <kind>: <detail>` on stderr with exit 1, usage
errors with exit 2.

## Common rules

- `--data <dir>` now also holds `storage/`, one `<hex>.sqlite` file per store
  ([data-model.md](../data-model.md#store-binding)). The grant record stays where `--grants` says.
- No command offers a way to send SQL, choose a store file or change a limit.

## `runtime permissions <plugin> …`

As in feature 003, plus, for a plugin that declares storage, one line after the permissions and
the `approval digest:` line:

```text
plugin: local/update-counter 0.1.0
author: Dashboard contributors
contract: v1 (1.2.0)
approved: version 0.1.0 (sha256:51c0…) from /home/ada/dashboard/target/plugins/update-counter.wasm
permissions:
  storage  required  granted
storage: quota 65536 bytes (64 KiB), approved up to 65536 bytes, used 412 bytes
login providers: none
approval digest: sha256:7e2a…
```

- The `storage:` line shows the declared quota, `approved up to <n> bytes` (the quota of the last
  approval) or `not approved`, and the usage of this package's store (`used 0 bytes` when it has
  none yet).
- A declared quota above the approved one adds `(above the approved quota: approve again)`.

## `runtime approve <plugin> [--expect <digest>] …`

As in feature 003. `--expect` is required whenever the plugin declares storage, required or
optional; without it the command is a usage error saying to run `permissions` first. The approval
sets the approved quota to the declared one, smaller or larger than before. One more line:

```text
approved local/update-counter 0.1.0
granted: storage
storage: quota 65536 bytes
```

## `runtime optional <plugin> storage on|off …`

As in feature 002. Turning storage off keeps the store and says so: `storage off (data kept)`.
Turning it on is `needs-new-approval` when the declared quota is above the approved one.

## `runtime remove <plugin-id> …`

As in feature 003; the plugin's stores are kept. The output adds
`storage kept: run runtime storage clear to delete it` when the plugin has one.

## `runtime update <plugin> …`

As in feature 003. For a plugin with storage, one more line:

```text
updated local/update-counter 0.1.0
storage: 3 calls, 2 writes kept, used 412 of 65536 bytes
```

When the update fails, nothing it wrote is kept, and the error says so:

```text
error: cpu-budget-exceeded: used the whole CPU budget of 500000000 fuel units (storage writes not kept, previous data kept)
error: storage-failed: the store's file reached its size limit of 393216 bytes, or the disk is full; nothing this update wrote was kept (previous data kept)
```

## `runtime storage list [--grants <path>] [--data <dir>]`

Lists every store in the data directory, one per line, sorted by id then origin; changes nothing.

```text
local/update-counter  /home/ada/dashboard/target/plugins/update-counter.wasm  used 412 of 65536 bytes
local/store-probe  /tmp/old/store-probe.wasm  used 2048 bytes, not approved, no file
```

- `of <n> bytes` is the approved quota when the grant record holds an approval of that id from that
  origin; otherwise `not approved`.
- With no store at all: `no stores`.
- `no file` when the origin's file no longer exists.
- A store that is damaged, or whose file name is not the hash of the id and origin it holds,
  prints `damaged: <detail>` instead of its usage; one whose identity cannot be read shows its
  file name and `-` for the id and origin.

## `runtime storage show <plugin> [--grants <path>] [--data <dir>] [--connectors <dir>]`

Prints the store of this package (its id from the manifest, its origin from the file), changes
nothing, and needs no approval:

```text
store: local/update-counter from /home/ada/dashboard/target/plugins/update-counter.wasm
used 412 of 65536 bytes, 3 entries
"count" = "7"
"notes/0" = "remember the milk\n"
"raw" = bytes(4) 00ff10a0
```

- One entry per line, sorted as the plugin's `list` sorts them.
- Names and values that are UTF-8 are printed in double quotes, escaped as log messages are
  (feature 001) and, inside the quotes, `"` as `\"` and `\` as `\\`, so no entry can add a line,
  control the terminal or pass for two fields; other values print as `bytes(<size>)` and
  hexadecimal.
- Each value is cut at 1 KiB, ending with ` [truncated, <size> bytes]`.
- No store yet: `no store yet`. A damaged store: `error: storage-failed: damaged: <detail>`, exit 1.

## `runtime storage clear <plugin> | --id <plugin-id> --origin <path> [--grants <path>] [--data <dir>]`

Empties one store in one exclusive transaction (every entry deleted and overwritten, usage 0);
other stores are untouched. The file keeps its size; later writes reuse its pages. With `--id` and
`--origin` it reaches a store whose package file is gone. A store whose origin's file no longer
exists, or that is damaged, is removed instead: its file is deleted.

```text
cleared local/update-counter from /home/ada/dashboard/target/plugins/update-counter.wasm (412 bytes)
```

A store removed instead prints `removed <id> from <origin> (<reason>)`, such as `(its package
file is gone)` or `(damaged: …)`; no store at all, `no store of <id> from <origin>`. A store in use
by an update in another process: `error: storage-failed: busy: …`, exit 1, nothing changed.

## `runtime bench <plugin> …`

Unchanged. The storage numbers come from `cargo run -p dashboard-bench --release --bin
storage_cost`, which prints, as medians of 100 runs each ([research.md](../research.md#r16-measuring)):

```text
storage_open_us: <first storage call of a run>
storage_get_us: <one get, grant check included>
storage_set_us: <one set in an update, grant check included>
storage_commit_us: <commit of an update that wrote 100 values>
render_reading_100_us: <a render that reads 100 values>
file_bytes_at_quota: <value size> <usage> <file size>   (one line each for 32 B, 1,000 B, 4 KiB)
```

## New error kind

`storage-failed`, as [data-model.md](../data-model.md#error-kinds) defines it.
