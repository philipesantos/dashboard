# Quickstart: Plugin Storage

Checks that the feature works end to end, on any of the three systems, with no internet. Every
command runs from the repository root, with a temporary data directory and grant record so nothing
touches your own. [contracts/cli.md](contracts/cli.md) defines every command and its output.

## Prerequisites

As in feature 003:

```sh
cargo xtask plugins
export D=$(mktemp -d)   # PowerShell: $env:D = New-Item -ItemType Directory (Join-Path $env:TEMP dash-004)
alias rt="cargo run -q -p runtime --release -- "   # or call the binary directly
```

Every `rt` command below takes `--grants $D/grants.json --data $D`.

## 1. The security suite

```sh
cargo test --workspace --locked
```

Expected: every hostile case of features 001 to 004 passes
([data-model.md](data-model.md#hostile-cases-fr-017)); after each storage case, every other store
and every host record is unchanged byte for byte.

## 2. A widget remembers across runs

```sh
rt permissions target/plugins/update-counter.wasm     # note the storage line and the approval digest
rt approve target/plugins/update-counter.wasm --expect <that digest>
rt update target/plugins/update-counter.wasm --config '{"note":"first"}'
rt update target/plugins/update-counter.wasm --config '{"note":"second"}'
rt run target/plugins/update-counter.wasm
rt storage show target/plugins/update-counter.wasm
```

Expected: `permissions` shows `storage: quota 65536 bytes (64 KiB), not approved, used 0 bytes`;
`approve` without `--expect` is a usage error, and with another digest `approval-changed`; each
`update` prints the storage line; `run` shows a count of 2 and both notes; `storage show` lists
the entries, quoted and escaped. Run the last two commands again in a new shell: the same values (SC-008).

## 3. A new version keeps the data; a larger quota needs approval

Rebuild the example with its `version` bumped in `plugins/examples/update-counter/manifest.json`,
then with its `quota_bytes` doubled, running `cargo xtask plugins` after each change:

```sh
rt run target/plugins/update-counter.wasm        # after the version bump: count 2, same notes
rt run target/plugins/update-counter.wasm        # after the larger quota: needs-new-approval, naming both quotas
```

Expected: the version bump runs with its earlier data, without approval; the larger quota is
refused until `permissions` and `approve --expect` again, and the data is still there afterwards.
Put the manifest back before going on.

## 4. Optional storage off keeps the data; clear removes it

With a copy of the example whose manifest makes storage optional (`"required": false`), approved
and turned on:

```sh
rt optional <copy>.wasm storage off      # storage off (data kept)
rt run <copy>.wasm                        # renders "Storage is off": storage answers not-granted
rt optional <copy>.wasm storage on
rt run <copy>.wasm                        # the earlier values again
rt storage list
rt storage clear <copy>.wasm
rt storage show <copy>.wasm               # no entries, used 0 bytes
```

Expected as written; `storage list` shows both stores (the example's and the copy's), each with
its origin.

## 5. Orphaned stores

Move or delete a package file that has a store, then:

```sh
rt storage list                                             # the store, marked "no file"
rt storage clear --id local/update-counter --origin <the old path>
rt storage list                                             # gone
```

## 6. Numbers

```sh
cargo run -p dashboard-bench --release --bin storage_cost
```

Expected: the lines of [contracts/cli.md](contracts/cli.md#runtime-bench-plugin-), each
storage call well under 1 ms (SC-006). CI's `bench.yml` records them on the three systems in
`performance.md`.
