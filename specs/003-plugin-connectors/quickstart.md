# Quickstart: Plugin Connectors

Checks that the feature works end to end, on any of the three systems. Every command runs from
the repository root, with a temporary data directory and grant record so nothing touches your
own. [contracts/cli.md](contracts/cli.md) defines every command and its output.

## Prerequisites

As in feature 002: stable Rust with the `wasm32-unknown-unknown` and `wasm32-wasip2` targets,
then:

```sh
cargo xtask plugins
export D=$(mktemp -d)   # PowerShell: $env:D = New-Item -ItemType Directory (Join-Path $env:TEMP dash-003)
alias rt="cargo run -q -p runtime --release -- "   # or call the binary directly
```

Every `rt` command below that reads grants or data takes `--grants $D/grants.json --data $D`:
`permissions`, `approve`, `optional`, `update`, `run` and `bench`; `requests` takes only
`--data $D`, and `pack` and `check-connector` take neither.

## 1. The security suite (no internet needed)

```sh
cargo test --workspace --locked
```

Expected: every hostile case of features 001, 002 and 003 passes
([data-model.md](data-model.md#hostile-cases-fr-020)); the test services report no request from a
refused case.

## 2. A catalog connector, end to end (needs the internet)

```sh
rt permissions target/plugins/github-releases.wasm     # note its "approval digest"
rt approve target/plugins/github-releases.wasm --expect <that digest>
rt update target/plugins/github-releases.wasm --config '{"owner":"rust-lang","repo":"rust"}'
rt run target/plugins/github-releases.wasm --config '{"owner":"rust-lang","repo":"rust"}'
rt requests
```

Expected: `permissions` shows `catalog/github`, `https://api.github.com`, `list-releases`
marked `read (reviewed)`, the parameters and fixed values it sends, and an approval digest;
`approve` with any other digest is refused as `approval-changed`; `update` reports one call;
`run` shows the latest releases; `requests` shows the one `GET` with its arguments.

## 3. Offline, render still shows the last data

Turn the network off (or point the machine at no route), then:

```sh
rt update target/plugins/github-releases.wasm --config '{"owner":"rust-lang","repo":"rust"}'
rt run target/plugins/github-releases.wasm --config '{"owner":"rust-lang","repo":"rust"}'
```

Expected: `update` exits with 1, because the example returns an error when its call fails
(`unavailable`), and keeps the previous data; `run` shows the releases from step 2 with the time
they were fetched and the failed update.

## 4. Your own service in development

Serve a JSON file on your machine, `python3 -m http.server 8765` in a directory holding
`hello.json`, and save as `$D/connectors/hello.json`:

```json
{
  "connector_version": 1,
  "id": "local/hello",
  "version": "0.1.0",
  "author": "You",
  "description": "A file served on this machine",
  "destinations": { "here": "http://127.0.0.1:8765" },
  "operations": {
    "get": {
      "kind": "read", "method": "GET", "destination": "here", "path": "/hello.json",
      "response": { "format": "json" }
    }
  }
}
```

```sh
rt check-connector $D/connectors/hello.json
```

Expected: `ok: local/hello 0.1.0, 1 destination, 1 operation (1 read)`. Change the destination
to `http://192.168.1.10:8765`, `https://api.github.com` or `https://uploads.github.com.` and check
again: refused as `invalid-connector`, `destination-reserved` and `invalid-connector`. Change
`"method"` to `"POST"`, or add `"headers": {"X-HTTP-Method-Override": "DELETE"}`: refused, a read
must be a `GET` or `HEAD` and a local connector fixes no `X-` header.

## 5. Refusals name the rule

With the connector of step 4 as first written, and its service still running, pack `net-probe`
with a manifest that declares `local/hello` and its `get` operation, as the hostile tests do.
Save as `$D/probe.json`:

```json
{
  "manifest_version": 1, "id": "local/net-probe", "version": "0.1.0", "author": "You",
  "permissions": [], "login_providers": [],
  "connectors": [{ "id": "local/hello", "major": 0, "operations": ["get"], "required": true }]
}
```

```sh
rt pack target/plugins/net-probe.wasm --manifest $D/probe.json --output $D/net-probe.wasm
rt permissions $D/net-probe.wasm        # note its "approval digest"
rt approve $D/net-probe.wasm --expect <that digest>
rt update $D/net-probe.wasm --config '{"calls":[{"connector":"local/hello","operation":"other","arguments":[]}]}'
rt run $D/net-probe.wasm --config '{"render_calls":[{"connector":"local/hello","operation":"get","arguments":[]}]}'
HTTPS_PROXY=http://127.0.0.1:9 HTTP_PROXY=http://127.0.0.1:9 \
  rt update $D/net-probe.wasm --config '{"calls":[{"connector":"local/hello","operation":"get","arguments":[]}]}'
rt requests
```

Expected: the first `update` reports one refused call, and `rt requests` shows it as
`operation-not-declared`; `run` shows the answer `refused:called-outside-update`; the update
with the proxy variables set still goes straight to `127.0.0.1:8765` and gets the file. `rt
requests` lists each call with its rule.

## 6. Contract 1.0.0 still runs

```sh
rt approve target/plugins/hello-widget.wasm && rt run target/plugins/hello-widget.wasm
rt permissions target/plugins/hello-widget.wasm
```

Expected: the greeting, as in feature 002, and `contract: v1 (1.0.0)`.

## 7. Measurements

```sh
rt bench target/plugins/github-releases.wasm --iterations 100 \
  --kept-data plugins/examples/github-releases/bench-data.json
```

Expected: `checks_us` under 10 000 (SC-007) and `render_with_data_us` under 1 000 000 (SC-004).
CI records them on the three systems in [performance.md](performance.md).
