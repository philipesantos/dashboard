# Quickstart: validate plugin permissions

Run from the repository root. Each step lists what proves the feature works. Every command uses
`--grants target/demo-grants.json`, so the demo never touches your real grant record; delete
that file to start over. Commands are in [contracts/cli.md](contracts/cli.md).

```bash
rt() { cargo run -q -p runtime --release -- "$@"; }
G='--grants target/demo-grants.json'
```

## Prerequisites

As in [feature 001](../001-plugin-runtime/quickstart.md#prerequisites).

## 1. Build every plugin

```bash
cargo xtask plugins
```

Expected: `target/plugins/hello-widget.wasm` and every hostile case, each with its manifest;
`probe.wasm` and `quiet.wasm` bare, without one.

## 2. Approve and run the example (User Story 1)

```bash
rt run target/plugins/hello-widget.wasm $G          # refused: not-granted, naming log
rt permissions target/plugins/hello-widget.wasm $G  # log required, not granted
rt approve target/plugins/hello-widget.wasm $G      # granted: log
rt run target/plugins/hello-widget.wasm $G --config '{"name":"Ada"}'
```

Expected: the first `run` exits 1 with `error: not-granted: …log…`; after `approve`, the widget
content and its `[plugin] info:` line, exit 0.

## 3. Pack the probe with your own manifests

```bash
m() { echo "{\"manifest_version\":1,\"id\":\"local/probe\",\"version\":\"$1\",\"author\":\"Quickstart\",\"permissions\":[$2],\"login_providers\":[]}"; }
m 1.0.0 '{"name":"log","required":false}' > target/probe-optional.json
m 1.1.0 '{"name":"log","required":true}'  > target/probe-required.json
m 1.2.0 ''                                > target/probe-none.json
rt pack target/plugins/probe.wasm --manifest target/probe-optional.json --output target/probe-optional.wasm
rt pack target/plugins/probe.wasm --manifest target/probe-required.json --output target/probe-required.wasm
rt pack target/plugins/probe.wasm --manifest target/probe-none.json --output target/probe-none.wasm
```

Expected: three packages, each with its path and digest printed. Packing
`target/probe-optional.wasm` again is refused with `manifest-mismatch`.

## 4. Optional permissions (User Story 4)

```bash
rt approve target/probe-optional.wasm $G               # nothing required to grant
rt run target/probe-optional.wasm $G                   # {"log":"not-granted"}, no [plugin] line
rt optional target/probe-optional.wasm log on $G
rt run target/probe-optional.wasm $G                   # {"log":"ok"} and a [plugin] line
rt optional target/probe-optional.wasm log off $G
rt run target/probe-optional.wasm $G                   # {"log":"not-granted"} again
```

## 5. Updates and removal (User Story 3)

```bash
rt run target/probe-required.wasm $G       # refused: needs-new-approval, naming log
rt approve target/probe-required.wasm $G
rt run target/probe-required.wasm $G       # runs, {"log":"ok"}
rt optional target/probe-required.wasm log off $G   # refused: required-together
rt remove local/probe $G
rt run target/probe-required.wasm $G       # refused: not-granted
```

## 6. Refusals by rule (User Story 2)

```bash
rt approve target/probe-none.wasm $G       # refused: not-declared, naming log
rt run target/plugins/read-file.wasm $G    # still capability-refused (feature 001)
```

## 7. Run the security suite and the contract checks

```bash
cargo test -p runtime --test hostile -- --test-threads=1
cargo test -p runtime --test permissions --test manifest_schema --test contract_v1 --test contract
```

Expected: every case passes on your system; CI repeats them on Windows, macOS and Linux. A
missing fixture fails, never skips. `manifest_schema` also validates every `manifest.json` in
`plugins/` (SC-006).

## 8. Measure the checks (SC-007)

```bash
rt bench target/plugins/hello-widget.wasm $G
```

Expected: `checks_us` and `relink_us` among the lines, far under 10 000. `bench.yml` records
them for all three systems in [performance.md](performance.md).
