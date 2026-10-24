# github-releases

The example plugin for contract 1.1.0, written in Rust. It shows the latest releases of the
GitHub repository in its configuration, such as `{"owner": "rust-lang", "repo": "rust"}`,
fetched through the official catalog's GitHub connector, `catalog/github`.

## A connected widget

The plugin builds the world `connected-widget` of [`wit/v1/widget.wit`](../../../wit/v1/widget.wit):

- `update(config)` runs when the host updates the widget. It calls the connector's
  `list-releases` operation, the only one its manifest declares, and returns the releases as
  JSON. The host makes the request itself and keeps what `update` returns. An `err` keeps the
  previous data.
- `render(config)` receives `{"config", "data", "fetched_at_ms", "last_update"}` and shows the
  kept data. It never waits on the network: a call made from `render` is refused.

The plugin never touches the network. It can call only the operations its manifest declares,
with values the host checks against each parameter's type, and only after you approve the
connector.

## Approve, update and run

```bash
rt() { cargo run -q -p runtime --release -- "$@"; }
rt permissions target/plugins/github-releases.wasm     # shows catalog/github and its approval digest
rt approve target/plugins/github-releases.wasm --expect <the approval digest>
rt update target/plugins/github-releases.wasm --config '{"owner":"rust-lang","repo":"rust"}'
rt run target/plugins/github-releases.wasm --config '{"owner":"rust-lang","repo":"rust"}'
rt requests
```

- `permissions` shows every destination the connector reaches, `https://api.github.com`, and
  the operation the widget calls, with the parameters it sends and the values the connector
  fixes. It ends with the approval digest of all of it.
- `approve --expect <digest>` grants exactly what was shown: if anything changed since, it
  refuses as `approval-changed`, granting nothing.
- `update` calls the widget's `update`: the host makes the one request, the widget returns the
  releases, and the host keeps them, with the time.
- `run` renders from what the host kept, even with the network down, and shows when it was
  fetched and how the last update went.
- `requests` prints the request log: every call, allowed or refused, with its destination and
  outcome.

Each command also takes `--grants <path>` and `--data <dir>`, to keep the approval, the data and
the log out of your own development directory. Updates run only when you ask, in this phase; the
host app updates on load and on a schedule (Phase 3).
