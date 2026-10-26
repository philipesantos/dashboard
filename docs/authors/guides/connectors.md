<!-- check: service=github -->
# Connectors

A widget never opens a connection. It calls the operations of a **connector**: a declaration, as
data, of one service's operations, where each request goes, which values the widget fills in and
what comes back. The host makes every request itself, checks it against the connector and the
manifest, and refuses it before sending anything when a rule says so. Before any of that, the
person approves the connector, seeing every destination it reaches and what the widget sends.

This guide starts from the example `github-releases`, which shows the latest releases of a GitHub
repository through the catalog's connector `catalog/github`. It starts in the clone of
[the walkthrough](../getting-started.md). In the project's CI, its commands reach a test service
that answers as GitHub does, so the releases below are the test service's; yours are GitHub's.

## The widget

`github-releases` builds the world `connected-widget`: it imports `connectors` and exports
`updater` as well as `renderer`. Its manifest declares the one connector and the one operation it
calls:

```json
"connectors": [
  { "id": "catalog/github", "major": 1, "operations": ["list-releases"], "required": true }
]
```

Its `update` calls `list-releases` with the repository of its configuration and returns the
releases; its `render` shows what the host kept. Build and pack it:

```console check output=any
$ cd dashboard/plugins/examples/github-releases
$ cargo build --release --target wasm32-unknown-unknown
```

```console check
$ runtime pack ../../target/wasm32-unknown-unknown/release/github_releases.wasm --manifest manifest.json --output github-releases.wasm
packed github-releases.wasm
digest: sha256:<digest>
```

## What the person approves

`runtime permissions` shows the connector, every destination it reaches, the operation the widget
calls with what it sends, and the approval digest of all of it:

```console check
$ runtime permissions github-releases.wasm
plugin: local/github-releases 0.1.0
author: Dashboard contributors
contract: v1 (1.1.0)
approved: no
permissions: none
connectors:
  catalog/github 1.0.0  required  not granted
    from: official catalog
    destinations: https://api.github.com
    list-releases  read (reviewed)  GET https://api.github.com/repos/{owner}/{repo}/releases
      sends: owner (text), repo (text), page (integer, optional), per_page (integer, optional)
      fixed: Accept: application/vnd.github+json, X-GitHub-Api-Version: 2022-11-28
login providers: none
approval digest: sha256:<digest>
```

`runtime approve` needs that digest, so what is approved is exactly what was shown: if anything
changed since, it is refused as `approval-changed`, granting nothing.

```console check
$ runtime approve github-releases.wasm --expect sha256:<digest>
approved local/github-releases 0.1.0
connectors: catalog/github (https://api.github.com)
```

## Update, then render

`runtime update` runs the widget's `update`. The host makes the one request and keeps what the
widget returns:

```console check
$ runtime update github-releases.wasm --config '{"owner":"rust-lang","repo":"rust"}'
updated local/github-releases 0.1.0
calls: 1 (0 not granted, 0 refused, 0 failed), 332 bytes received
```

`runtime run` renders from what the host kept, without any request:

```console check
$ runtime run github-releases.wasm --config '{"owner":"rust-lang","repo":"rust"}'
{
  "fetched_at_ms": <n>,
  "items": [
    {
      "name": "Rust 1.99.1",
      "published_at": "2026-10-16T00:00:00Z",
      "tag": "1.99.1",
      "url": "https://github.com/rust-lang/rust/releases/tag/1.99.1"
    },
    {
      "name": "Rust 1.99.0",
      "published_at": "2026-10-02T00:00:00Z",
      "tag": "1.99.0",
      "url": "https://github.com/rust-lang/rust/releases/tag/1.99.0"
    }
  ],
  "last_update": {
    "at_ms": <n>,
    "outcome": "ok"
  },
  "title": "rust-lang/rust releases"
}
```

`runtime requests` shows every call, allowed or refused, with its destination and how it ended:

```console check
$ runtime requests
<time> local/github-releases catalog/github list-releases GET api.github.com /repos/rust-lang/rust/releases?per_page=5 200 332B <time>
```

Every call a widget makes, allowed or refused, lands in this log: the connector, the operation,
the destination, the values sent and the outcome. A widget can neither read nor change it, and
the person can read every line. Past 20 calls in one run, calls are refused and counted in one
summary entry.

## Every answer of a call

`call` answers `ok` with the service's response, whatever its status: a widget decides what a 404
means for it. Otherwise it answers one of three errors ([the contract](../reference/contract.md#connectors)):

- `not-granted`: the connector is optional and off, or its approval was removed. Nothing was
  sent. Show the widget without its data; never fail because of it.
- `refused(problem)`: the host refused the call before sending anything, naming the rule: an
  operation the manifest does not declare, a value outside its parameter's type, a call from
  `render`, a limit. Most are mistakes to fix while writing the widget.
- `failed(problem)`: the call could not complete, and nothing of it reached the widget: the
  service is unreachable, its certificate is wrong, it answered too much or too late, or in the
  wrong format. Keep showing what the last update fetched; the next update tries again.

`github-releases` returns an error from `update` for all three, so the host keeps the previous
data and `render` shows that the last update failed. A widget can also return what it has with a
note, as [accounts](accounts.md) shows.

## Writing your own connector

A widget can use a connector of the official catalog, such as `catalog/github`, or, in
development, a local one of your own, `local/<name>`, for your own service:
[writing connectors](writing-connectors.md).
