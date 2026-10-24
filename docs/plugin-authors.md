# Writing a plugin

A plugin is a WebAssembly component that renders one widget. It runs in a sandbox and starts
with no power at all: it gets a capability only when its manifest declares it and the user
grants it. A plugin never touches the network: it reaches an external service only by calling
the operations of a connector its manifest declares, and the host makes every request
itself. This guide covers what a plugin needs today; storage and accounts arrive in later
features.

Start from [`plugins/examples/hello-widget`](../plugins/examples/hello-widget), a Rust plugin
that only renders, or from [`plugins/examples/github-releases`](../plugins/examples/github-releases),
one that fetches data through a connector. [`docs/connectors.md`](connectors.md) explains how
to write a connector.

## The contract

The contract is the WIT interface between the host and a plugin. Build against
[`wit/v1`](../wit/v1/widget.wit) (`dashboard:widget@1.1.0`), from one of its two worlds:

- **`widget`**, a widget that only renders. It exports `renderer`, with
  `render(config: string) -> result<string, string>`: it receives its configuration as JSON
  text and returns the widget content as JSON text, at most 64 KiB. An `err` reaches the host as
  a `plugin-error` with your message. It may import `log`.
- **`connected-widget`**, a widget that fetches data. It exports `renderer` and `updater`, and
  may import `log` and `connectors`: `update` fetches the data through connectors, the host
  keeps what it returns, and `render` shows it (see [Connected widgets](#connected-widgets)).

Every capability function returns `result<T, E>`, and the first case of `E` is `not-granted`.
That is the answer when the capability is an optional permission the user has not turned on
(see below). A plugin that ignores it still runs; one that handles it can tell the user what is
missing, inside its own widget.

Each minor version of v1 only adds: 1.1.0 holds everything of 1.0.0 unchanged, plus connectors
and `updater`. The host runs a plugin built for any v1 minor up to its own, so a plugin built
against 1.0.0 ([`wit/history/1.0.0`](../wit/history/1.0.0/widget.wit)) keeps running, as
`hello-widget` does. A plugin built against a newer version than the host runs is refused
(`contract-mismatch`, naming both versions), because it may use what the host does not have.
Every contract name a component uses must carry the one version it was built for.

Contract [`v0`](../wit/v0/widget.wit) (`dashboard:widget@0.1.0`) still runs, but its `log`
cannot answer `not-granted`, so a v0 plugin can only have required permissions. New plugins
should use v1.

## The manifest

Every plugin carries a manifest: who it is and what it needs. It is JSON, described by the
published schema [`wit/manifest.schema.json`](../wit/manifest.schema.json); point `$schema` at
it and your editor checks the file as you type.

```json
{
  "$schema": "../../../wit/manifest.schema.json",
  "manifest_version": 1,
  "id": "local/hello-widget",
  "version": "0.2.0",
  "author": "Dashboard contributors",
  "permissions": [
    { "name": "log", "required": true }
  ],
  "login_providers": [],
  "connectors": [
    { "id": "catalog/github", "major": 1, "operations": ["list-releases"], "required": true }
  ]
}
```

| Field | Rule |
| --- | --- |
| `$schema` | Optional; the host ignores it |
| `manifest_version` | `1` |
| `id` | Where the plugin comes from (see below), lowercase |
| `version` | [SemVer 2.0.0](https://semver.org), at most 64 characters |
| `author` | 1 to 100 characters, no control characters |
| `permissions` | At most 32 entries of `{ "name": …, "required": true \| false }` |
| `login_providers` | Accounts the plugin would ask the user to connect: `github`. Shown now; connecting accounts arrives later |
| `connectors` | Optional, at most 16: each connector the widget calls, its major version, the operations it calls, and whether it is required (see [Connected widgets](#connected-widgets)) |

Any other field, the same field twice, a manifest over 16 KiB, or anything the schema refuses
is `invalid-manifest`, and the error names the field. A permission the host does not know is
`unknown-permission`.

### Plugin ids

A plugin's id says where it comes from:

- `github.com/<owner>/<repository>`: a plugin published in its own repository, one plugin per
  repository. The repository is the id, and installing (a later phase) checks that the package
  came from it.
- `local/<name>`: a plugin that comes from no repository: development builds, examples, test
  fixtures. Every plugin in this project's repository uses one.

### Permissions

Each capability your plugin imports needs a permission in the manifest; an import the manifest
does not declare refuses the plugin (`not-declared`). Declaring a permission you do not use is
allowed, but the user sees it.

| Permission | Interface | What it gives |
| --- | --- | --- |
| `log` | `dashboard:widget/log` | Writing `debug`, `info`, `warn` or `error` messages: at most 100 per call, 1 KiB each, shown escaped |

A permission is required or optional:

- **Required** (`"required": true`): the user approves all of the plugin's required permissions
  together, or the plugin does not run. They cannot be taken back one by one: they change only
  with a new version, approved again, or by removing the plugin's approval.
- **Optional** (`"required": false`): it starts off, and the user turns it on or off at any
  time. While it is off, every call answers `not-granted` and does nothing. Your plugin must
  work without it:

  ```rust
  match write(Level::Info, "rendering") {
      Ok(()) => {}
      Err(LogError::NotGranted) => { /* carry on without logging */ }
  }
  ```

A plugin cannot read its grants, ask for a permission while it runs, or open a dialog: it
learns that an optional permission is off only from the `not-granted` answer.

### Updates

A new version keeps its approval as long as it requires nothing new. One that requires a
permission the user has not granted is refused (`needs-new-approval`) until the user approves
it again; the grants it had stay as they were. A permission a new version adds as optional
starts off and needs no approval. A permission a new version no longer declares stops being
granted.

## Connected widgets

A widget that shows external data declares, in its manifest, each connector it calls:

- `id`: `catalog/<name>` for a connector of the official [catalog](../connectors), built into
  the host; `local/<name>` for one of your own, in development only. Connectors from
  repositories, whose owners verify their destinations, arrive with installation (Phase 3).
- `major`: the connector's major version your widget is built for. Another major is refused
  (`connector-unavailable`).
- `operations`: every operation your widget calls, at least one. Calling any other is
  refused before anything is sent.
- `required`: as for permissions. A required connector is approved with the widget; an optional
  one starts off.

Each connector is a permission of its own. A widget that declares one is built for contract
1.1.0 (`contract-too-old` otherwise) and exports `updater` (`missing-updater` otherwise). It
imports `connectors`, which a widget that declares none cannot (`not-declared`).

### Update and render

Your widget gets data in two steps, never mixing the network with rendering:

1. **`update(config)`** runs when the host updates the widget: in this phase when you run
   `runtime update`, and in the host app on load and on a schedule. Call connectors here, then
   return your data as a JSON document of at most 256 KiB. The host keeps it, with the time,
   across runs. An `err` keeps the previous data. An update has 10 s of wall-clock time and the
   same CPU and memory budget as a render.
2. **`render(config)`** gets a document of the host's, never the network:

   ```json
   {
     "config": { "owner": "rust-lang", "repo": "rust" },
     "data": { "releases": [] },
     "fetched_at_ms": 1792761600000,
     "last_update": { "at_ms": 1792761600000, "outcome": "ok" }
   }
   ```

   `data` and `fetched_at_ms` are `null` before the first successful update, and `last_update`
   before the first update; after a failed one, `last_update` says `"outcome": "failed"` with the
   `error`, and `data` is the last data that worked. Show it, and when it was fetched: your
   widget keeps working offline. A connector call from `render` is refused.

### Calling a connector

`call(connector, operation, arguments)` takes the connector id, an operation the manifest
declares for it, and the arguments, each a name and a value: `text(string)`, `integer(s64)` or
`flag(bool)`, as the operation's parameters say. The host checks every value against its type
and puts each one in its own place of the request, encoded, so a value can never change the
destination. It answers:

- `ok(response)`: the service's `status`, any status, error statuses included; the response
  `headers` the operation declares, each `none` when absent, and never a cookie; and the `body`
  as received, whole.
- `err(not-granted)`: the connector is optional and off, its approval was removed, or your
  manifest does not declare it. Nothing was sent. Carry on without it.
- `err(refused(problem))`: a rule refused the call before anything was sent: an undeclared
  operation, a value outside its type, a call from render, an operation that changes data
  (until user actions arrive in Phase 3, an update only reads), or a limit.
- `err(failed(problem))`: the request was attempted and gave nothing for your widget: the
  destination unreachable or not public, a redirect to anywhere the connector does not declare,
  a certificate that is not valid, a response over a limit, or a success in another format
  than declared. An error status is no failure: it comes as `ok(response)`, whatever its body.

`problem.rule` names the rule and `problem.detail` explains it for you, never for the user:
show your user what is missing in your own words.

### What the host records

Every call your widget makes, allowed or refused, lands in the host's request log: the
connector, the operation, the destination, the values sent and the outcome. Your widget can
neither read nor change it, and the user can read every line. Past 20 calls in one run, calls
are refused and counted in one summary entry.

## Building and packing

The manifest travels inside the plugin, as a custom section named `dashboard-manifest` at the
top level of the component ([package format](../specs/002-plugin-permissions/contracts/package.md)).
One file is the whole package, so the manifest cannot be swapped for another one.

- In this repository, put `manifest.json` beside the crate's `Cargo.toml` and run
  `cargo xtask plugins`: it builds every plugin into `target/plugins/<crate>.wasm` and adds its
  manifest.
- Elsewhere, build your component, then add the manifest with the CLI, which checks it first:

  ```bash
  cargo run -p runtime --release -- pack my-widget.wasm --manifest manifest.json --output my-widget.package.wasm
  ```

A package with no manifest section is `no-manifest`; one with two, or with one inside a nested
module or component, is `manifest-mismatch`.

## Approving and running in development

There is no app yet: the `runtime` command-line tool stands in for the host. See its contracts
for [permissions](../specs/002-plugin-permissions/contracts/cli.md) and
[connectors](../specs/003-plugin-connectors/contracts/cli.md) for every command.

```bash
rt() { cargo run -q -p runtime --release -- "$@"; }
rt permissions target/plugins/hello-widget.wasm   # what it requests and what is granted
rt approve target/plugins/hello-widget.wasm       # grants every required permission
rt run target/plugins/hello-widget.wasm --config '{"name":"Ada"}'
rt optional my-widget.package.wasm log on         # turns an optional permission on or off
rt remove local/hello-widget                      # removes the approval
```

For a connected widget, `permissions` also shows each connector, every destination it reaches
and each operation your widget calls, with the parameters it sends, and ends with an approval
digest. `approve` needs that digest, so what is approved is exactly what was shown:

```bash
rt permissions target/plugins/github-releases.wasm   # note the "approval digest"
rt approve target/plugins/github-releases.wasm --expect <that digest>
rt update target/plugins/github-releases.wasm --config '{"owner":"rust-lang","repo":"rust"}'
rt run target/plugins/github-releases.wasm --config '{"owner":"rust-lang","repo":"rust"}'
rt requests                                          # the request log
rt optional target/plugins/github-releases.wasm catalog/github off   # an optional connector
```

A new version of your widget, or of a connector, that adds a destination or changes an
operation your widget calls needs a new approval (`needs-new-approval`, naming the change); one
that only adds operations you do not call does not. Turning a connector off, or removing the
approval, discards the data the host kept for your widget.

The grants live in a development record in your data directory, or in the file you give with
`--grants <path>`; kept data and the request log in the development data directory, or in
`--data <dir>`; and your local connectors in `<data>/connectors/`, or in `--connectors <dir>`. An approval belongs to the file it was given for: rebuild in place and it
stays; a copy elsewhere, even byte for byte the same, needs its own approval, which replaces
the first one. So another file that claims your plugin's id gets none of its grants.

Every refusal names its rule, in one line:

```text
error: not-granted: log (local/hello-widget has no approval; approve it first)
```

## Limits

The host sets these for every `render` and `update` call; a plugin can neither read nor change
them.

| Limit | Default | When exceeded |
| --- | --- | --- |
| Plugin package | 10 MiB | `invalid-package` at load |
| CPU budget | 500,000,000 fuel units, about 0.35 s of computation | `cpu-budget-exceeded` |
| Time | 1 s | `time-limit-exceeded` |
| Memory | 32 MiB, all memories together | `memory-limit-exceeded` |
| Widget content | 64 KiB, and it must be valid JSON | `output-too-large`, `invalid-output` |
| Update time | 10 s | `time-limit-exceeded`; the previous data stays |
| Update data | 256 KiB, and it must be valid JSON | `output-too-large`, `invalid-output` |
| One connector call | 5 s, within what is left of the update | `failed(over-limit)` |
| Calls per run of `update` or `render` | 20 | `refused(over-limit)` |
| Calls per minute | 30 per widget; 60 per destination host, all widgets together | `refused(over-limit)` |
| Request | Address (path and query) 8 KiB, body 64 KiB, 16 arguments | `refused(over-limit)` |
| Response | Body 1 MiB, headers 16 KiB; 4 MiB of bodies per update; 5 redirects | `failed(over-limit)`, `failed(redirect-not-allowed)` |

Each call runs in a fresh instance: nothing your plugin keeps in memory survives to the next
call. Build Rust plugins for `wasm32-unknown-unknown`: for `wasm32-wasip2`, the standard library
imports WASI, which the host does not offer (`capability-refused`).
