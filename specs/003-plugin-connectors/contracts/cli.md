# Contract: `runtime` command-line tool

Extends [feature 002's contract](../../002-plugin-permissions/contracts/cli.md), whose common
rules still hold: the tool is the host's control in this phase, in development mode only, on
local files; text that comes from a package or a connector is escaped before it is printed; a
refusal prints `error: <kind>: <detail>` on stderr and exits with 1; usage errors exit with 2.

## Common rules

- `--data <dir>`: the development data directory, holding `connectors/` (local connectors),
  `widget-data/` (kept data) and `requests.jsonl` (the request log). Default:
  `<data dir>/dev.philsantos.dashboard/development/`; if the system has no data directory, it is
  a usage error.
- `--connectors <dir>`: where local connectors are read, `<dir>/<name>.json` for
  `local/<name>`. Default: `<data>/connectors/`. A local connector's origin is its file's
  canonical path.
- Catalog connectors are built into the tool; nothing on disk adds to the catalog.
- No command offers a way to change how the network is reached: no proxy, resolver or
  certificate option exists.

## `runtime update <plugin> [--config <json> | --config-file <path>] [--grants <path>] [--data <dir>] [--connectors <dir>]`

Loads the plugin (load decision of [data-model.md](../data-model.md#load-decision)), calls its
`update` with the configuration (default `{}`), and keeps the data it returns. Updates run only
when this command runs (FR-012).

```text
updated local/github-releases 0.1.0
calls: 1 (0 refused, 0 failed), 3412 bytes received
```

A refused or failed call does not fail the update: the widget sees it and decides. The update
fails, keeping the previous data and exiting with 1, when the widget returns an error, hits a
limit, or returns output that is not valid JSON or is too large:

```text
error: time-limit-exceeded: still running at the update limit of 10s (previous data kept)
```

A widget that exports no `updater` is `missing-updater`.

## `runtime run <plugin> [--config …] [--grants <path>] [--data <dir>] [--connectors <dir>]`

As in feature 002. For a widget that exports `updater`, `render` receives the kept data, when it
was fetched and the outcome of the last update ([contracts/widget-v1.1.wit](widget-v1.1.wit));
nothing goes to the network.

## `runtime permissions <plugin> [--grants <path>] [--data <dir>] [--connectors <dir>]`

As in feature 002, plus the contract's minor version and a `connectors` section. Changes
nothing.

```text
plugin: local/github-releases 0.1.0
author: Dashboard contributors
contract: v1 (1.1.0)
approved: version 0.1.0 (sha256:9a4e…) from /home/ada/dashboard/target/plugins/github-releases.wasm
permissions: none
connectors:
  catalog/github 1.0.0  required  granted
    from: official catalog
    destinations: https://api.github.com
    list-releases  read (reviewed)  GET https://api.github.com/repos/{owner}/{repo}/releases
      sends: owner (text), repo (text), per_page (integer, optional), page (integer, optional)
login providers: none
```

- `from:` is `official catalog` or `local file <path> (development)`.
- The kind reads `read (reviewed)` or `change (reviewed)` for the catalog, and
  `read (declared by the developer)` or `change (declared by the developer)` for local
  connectors.
- `destinations:` lists every origin of the connector, the ones redirects may reach included.
- A connector that cannot be used shows its refusal instead of its operations, for example
  `github.com/ada/weather  required  error: destination-not-verified: connectors from repositories arrive with installation (Phase 3)`.

## `runtime approve <plugin> [--grants <path>] [--data <dir>] [--connectors <dir>]`

As in feature 002: grants every required permission and every required connector, all together,
and records each declared connector's destinations and operations. Refused, changing nothing,
when any declared connector is refused at step 6a.

```text
approved local/github-releases 0.1.0
granted: none
connectors: catalog/github (https://api.github.com)
```

## `runtime optional <plugin> <permission | connector-id> on|off [--grants <path>] [--data <dir>] [--connectors <dir>]`

As in feature 002; a connector id (it contains `/`) turns that connector on or off. Turning a
connector off discards the widget's kept data and says so:

```text
catalog/github: off (kept data discarded)
```

## `runtime remove <plugin-id> [--grants <path>] [--data <dir>]`

As in feature 002, and discards the kept data of that plugin id.

## `runtime requests [--plugin <plugin-id>] [--last <n>] [--data <dir>]`

Prints the request log, oldest first, the last `n` entries (default 50), one line each; time in
UTC; changes nothing.

```text
2026-10-23T14:02:11.204Z local/github-releases catalog/github list-releases GET api.github.com /repos/rust-lang/rust/releases?per_page=5 200 3412B 184ms
2026-10-23T14:02:11.391Z local/net-probe local/echo fetch refused value-outside-type: id (longer than 16 characters)
```

## `runtime check-connector <file> [--data <dir>]`

Validates a connector file as the host would load it as a local connector, the catalog's
reserved hosts included, and prints a summary; for authors.

```text
ok: local/weather 0.1.0, 1 destination, 2 operations (2 reads)
```

## `runtime bench <plugin> …`

As in feature 002; `checks_us` now includes the connector checks, and for a widget that exports
`updater` it prints one more line, the median of 100 renders from kept data:

```text
render_with_data_us: <render from kept data, no network>
```

## New error kinds

`invalid-connector`, `credentials-not-supported`, `destination-not-verified`,
`destination-reserved`, `connector-unavailable`, `contract-too-old` and `missing-updater`, as
[data-model.md](../data-model.md#error-kinds) defines them.
