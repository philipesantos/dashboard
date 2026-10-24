# update-counter

The example plugin for contract 1.2.0, written in Rust. It remembers how many times it has
updated, when it first ran and the last five notes its configuration gave it, such as
`{"note": "remember the milk"}`, in a store of its own that the host keeps across runs of the
host and new versions of the plugin.

## A widget with storage

The plugin builds the world `stored-widget` of [`wit/v1/widget.wit`](../../../wit/v1/widget.wit):

- `update(config)` runs when the host updates the widget. It reads its count, adds one, and
  keeps the new note, writing each value with `storage::set`. The writes are kept together only
  when `update` returns `ok`: an error, a trap or a limit keeps none of them.
- `render(config)` reads the count and the notes with `storage::get` and `storage::list`, and
  shows them. Render only reads: a write from `render` is refused.

The plugin never names a store, a file or a place. The host chooses its store when it links the
plugin, from its id and the file it was approved from, so no plugin reaches another's. Its
manifest declares the `storage` permission with a quota of 65536 bytes, which the host shows
before approval; writes past it are refused.

## Approve, update and run

```bash
rt() { cargo run -q -p runtime --release -- "$@"; }
rt permissions target/plugins/update-counter.wasm     # shows storage, its quota and the approval digest
rt approve target/plugins/update-counter.wasm --expect <the approval digest>
rt update target/plugins/update-counter.wasm --config '{"note":"first"}'
rt update target/plugins/update-counter.wasm --config '{"note":"second"}'
rt run target/plugins/update-counter.wasm
```

- `permissions` shows the storage quota the plugin declares, the quota you last approved and
  what its store uses now. It ends with the approval digest of all of it.
- `approve --expect <digest>` grants exactly what was shown, quota included: if anything changed
  since, it refuses as `approval-changed`, granting nothing. A later version that declares a
  larger quota needs a new approval; one that declares the same or less does not.
- `update` calls the widget's `update` and says how many storage calls it made, how many writes
  were kept and how much the store uses.
- `run` renders what the store holds: `Updated 2 times`, with both notes, in this run of the host
  or any later one.

Each command also takes `--grants <path>` and `--data <dir>`, to keep the approval and the store
out of your own development directory. Stores live in `<data dir>/storage/`, apart from the
host's own records.
