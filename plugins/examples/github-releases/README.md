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
```

`permissions` shows every destination the connector reaches, `https://api.github.com`, and the
operation the widget calls, with the parameters it sends. `approve` grants exactly what was
shown: if anything changed since, it refuses.
