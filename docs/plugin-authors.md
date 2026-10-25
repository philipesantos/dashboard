# Writing a plugin

A plugin is a WebAssembly component that renders one widget. It runs in a sandbox and starts
with no power at all: it gets a capability only when its manifest declares it and the user
grants it. A plugin never touches the network: it reaches an external service only by calling
the operations of a connector its manifest declares, and the host makes every request
itself. It keeps data only in a store of its own, which the host binds to it. When a connector
needs an account or a key, the host adds it to the requests itself: a plugin never sees a
credential.

Start from [`plugins/examples/hello-widget`](../plugins/examples/hello-widget), a Rust plugin
that only renders, from [`plugins/examples/github-releases`](../plugins/examples/github-releases),
one that fetches data through a connector, or from
[`plugins/examples/update-counter`](../plugins/examples/update-counter), one that keeps its own
data, or from [`plugins/examples/private-notes`](../plugins/examples/private-notes), one that
reads as the user's account. [`docs/connectors.md`](connectors.md) explains how to write a
connector.

## The contract

The contract is the WIT interface between the host and a plugin. Build against
[`wit/v1`](../wit/v1/widget.wit) (`dashboard:widget@1.3.0`), from one of its six worlds:

- **`widget`**, a widget that only renders. It exports `renderer`, with
  `render(config: string) -> result<string, string>`: it receives its configuration as JSON
  text and returns the widget content as JSON text, at most 64 KiB. An `err` reaches the host as
  a `plugin-error` with your message. It may import `log`.
- **`connected-widget`**, a widget that fetches data. It exports `renderer` and `updater`, and
  may import `log` and `connectors`: `update` fetches the data through connectors, the host
  keeps what it returns, and `render` shows it (see [Connected widgets](#connected-widgets)).
- **`stored-widget`**, a widget that keeps data of its own. It exports `renderer` and
  `updater`, and may import `log` and `storage`: `update` writes its store, and `render` reads
  it (see [Storage](#storage)).
- **`connected-stored-widget`**, both: it may import `log`, `connectors` and `storage`.
- **`account-widget`**, a widget that reads as the user's account: it may import `log`,
  `connectors` and `accounts` (see [Using a connector's account](#using-a-connectors-account)).
- **`account-stored-widget`**, the same with `storage`.

Every capability function returns `result<T, E>`, and the first case of `E` is `not-granted`.
That is the answer when the capability is an optional permission the user has not turned on
(see below). A plugin that ignores it still runs; one that handles it can tell the user what is
missing, inside its own widget.

Each minor version of v1 only adds: 1.1.0 holds everything of 1.0.0 unchanged, plus connectors
and `updater`; 1.2.0 everything of 1.1.0, plus `storage`; and 1.3.0 everything of 1.2.0, plus
`accounts`. The host runs a plugin built for any v1 minor up to its own, so a plugin built
against 1.0.0 ([`wit/history/1.0.0`](../wit/history/1.0.0/widget.wit)), 1.1.0
([`wit/history/1.1.0`](../wit/history/1.1.0/widget.wit)) or 1.2.0
([`wit/history/1.2.0`](../wit/history/1.2.0/widget.wit)) keeps running, as `hello-widget`,
`github-releases` and `update-counter` do. Such a plugin never gets an answer its contract does
not have: it calls a connector whose credential is optional without the credential, and one
whose credential is required cannot be declared before 1.3.0. A plugin built against a newer version than the host runs is refused
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
| `permissions` | At most 32 entries of `{ "name": …, "required": true \| false }`; `storage` also takes `quota_bytes` (see [Storage](#storage)) |
| `login_providers` | Obsolete: the accounts a widget uses come from its connectors. A manifest that still carries it stays valid, and it grants nothing |
| `connectors` | Optional, at most 16: each connector the widget calls, its major version, the operations it calls, whether it is required, and whether it uses the connector's account (see [Connected widgets](#connected-widgets)) |

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
| `storage` | `dashboard:widget/storage` (1.2.0) | A store of named values of the plugin's own, up to the quota it declares (see [Storage](#storage)) |

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
- `account`: optional, `true` when your widget calls the connector as the user's account (see
  [Using a connector's account](#using-a-connectors-account)).

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

## Using a connector's account

Some connectors declare a credential: an account the user signs in to, or a key the user types
(see [`docs/connectors.md`](connectors.md#credentials)). The account belongs to the connector,
not to your widget: the user connects it once, in the host's controls, and every widget approved
to use it acts as that one account. Your widget can neither connect, reconnect nor disconnect
an account, nor open a browser, nor learn the credential: the host adds it to each request
itself, only to the destinations the connector names, over HTTPS.

To read as the account, set `"account": true` on the connector in your manifest and call it
through `accounts` instead of `connectors`, from the `account-widget` or
`account-stored-widget` world. A widget that does so is built for contract 1.3.0
(`contract-too-old` otherwise); `account: true` on a connector without a credential is
`invalid-manifest`, and a connector whose credential is required must be declared with it
(`account-use-not-declared` otherwise). The user sees, before approving, what the account lets
the connector do and that your widget acts as it.

```json
"connectors": [
  { "id": "local/notes", "major": 1, "operations": ["list-notes"], "required": true, "account": true }
]
```

`accounts.call` takes the same arguments as `connectors.call` and answers the same way, with two
more answers:

- `err(account-not-connected)`: no account is connected and the credential is required, or the
  account must be connected again (the service refused it, it could not be refreshed, or the
  connector now asks for more access than it holds). Nothing reached your widget. Show your
  user that the account needs connecting, in your own words; only they can do it.
- `err(subscription-required)`: the service answered that the account lacks a paid plan this
  operation needs. The response was dropped.

When the credential is optional and no account is connected, the call goes without one, and the
service answers as it does for anyone. A response that carries the credential back is
`failed(invalid-response)`, and your widget gets nothing of it.

Which account your widget acts as is fixed when the host loads it: an account connected while
it runs applies from the next load, and one disconnected while it runs stops at the next call.
The data the host kept from an update made as an account is discarded when that account is
disconnected, or replaced by another one.

A connector your manifest declares with `account: true` answers only through `accounts`, and one
without only through `connectors`; the other way, the call is `not-granted`. A widget whose
connectors all use their account still imports `connectors` for the types `accounts` shares;
it never calls it.

A new version of your widget that starts using an account, or of a connector that adds a
credential, changes it, or asks for more access, needs a new approval (`needs-new-approval`,
naming the change). One that asks for less, or that stops using the account, does not.

## Storage

A plugin keeps data across runs of the host only in its store: a set of named values that the
host keeps for that plugin alone. Each call of `render` and `update` runs in a fresh instance,
so nothing your plugin holds in memory survives the call.

### Declaring storage

Storage is a permission, declared with its quota on the same entry, and the plugin is built for
contract 1.2.0 or later (an older one that declares it is `contract-too-old`):

```json
"permissions": [
  { "name": "storage", "required": true, "quota_bytes": 65536 }
]
```

- `quota_bytes` is the most your store may hold, from 64 to 8,388,608 (8 MiB), the host's cap.
  `storage` appears at most once, and no other permission takes a quota.
- The user sees the quota before approving it, and approves it with the plugin's approval
  digest. A later version that declares a larger quota does not run until approved again; one
  that declares the same or less runs, keeps what is stored, and accepts only writes that leave
  usage within the new quota (deletes always pass).
- A widget that declares storage exports `updater` (`missing-updater` otherwise): storage is
  written only in an update. It needs no connector to do so.
- The `storage` import follows your calls, not the world: a guest built on `stored-widget` that
  never calls a storage function does not import it. Declare storage when your plugin calls it.

### Names, values and usage

The interface has five functions: `get(name)`, `set(name, value)`, `delete(name)`,
`list(prefix, after)` and `usage()`.

- A name is any non-empty text up to 256 bytes in UTF-8, kept and compared exactly as given, as
  bytes: `../x`, `a/b`, a NUL or a quote are ordinary names. A value is bytes, up to 256 KiB.
- `list` returns at most 100 names that start with `prefix` (empty: every name), in byte order,
  after `after`, and `next`, the name to pass as `after` for the next page when more remain. A
  prefix is never a pattern: `a%` lists only names that start with `a%`.
- Usage is the bytes of every name and value plus 64 per entry, the same on every system;
  `usage()` returns it with your quota. A `set` that would take usage past the quota is
  `refused(over-quota)`, and nothing of it is kept.
- Your plugin never names a store, a file or a place: the host binds the store to your plugin's
  id and the file it was approved from. Another plugin, or a copy of yours at another path,
  reaches its own store, never yours.

### Render reads, update writes, all or nothing

- `render` may `get`, `list` and `usage`; a `set` or `delete` there is
  `refused(render-only-reads)`. Render reads one consistent view for its whole run.
- An update's writes are kept together, and only when `update` returns `ok` with valid output.
  When it returns an error, traps, runs out of a limit, or its storage is turned off or its
  approval removed while it runs, none of them is kept. A write that fails, such as on a store
  file that cannot be written, ends the update's writes: later calls answer
  `failed(run-aborted)`, and the update fails as `storage-failed`.
- Storage is kept across new versions of your plugin from the same file; the data the host kept
  from your last update (feature 003) is not. Turning optional storage off, or removing the
  approval, keeps the store; only the user clears it.

Every call answers, besides its value, `not-granted` (optional storage that is off, or taken
away during the run), `refused(<rule>)` (the host refused before changing anything:
`render-only-reads`, `over-quota`, `over-limit`, `invalid-name`) or `failed(<rule>)` (the store
could not do it: `damaged`, `busy`, `unavailable`, `over-limit` when the file reached its size
limit or the disk is full, `run-aborted`), each with a detail for you.

The store is not encrypted, and it is not for secrets: your plugin never receives a password or
a token to keep (constitution Principle IV): accounts and keys live in the host's own vault, in
the system's keychain (see [Using a connector's account](#using-a-connectors-account)).

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
for [permissions](../specs/002-plugin-permissions/contracts/cli.md),
[connectors](../specs/003-plugin-connectors/contracts/cli.md),
[storage](../specs/004-plugin-storage/contracts/cli.md) and
[accounts](../specs/005-plugin-credentials/contracts/cli.md) for every command.

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

For a widget that uses an account, `permissions` also shows each connector's credential: what
it is, the header and destinations it goes to, the sign-in and token addresses, what the
account lets the connector do, and which account the widget acts as. `accounts` connects one to
a development vault in your system's keychain, which installed plugins never use; the account
and the request log name the account, never the credential:

```bash
rt accounts connect local/notes --client <id>   # signs in in your browser; a key is typed, not shown
rt accounts list                                # what is connected, and what needs connecting again
rt accounts disconnect local/notes              # deletes it from the keychain and revokes it
```

For a widget with storage, `permissions` shows the quota it declares, the quota last approved
and what its store uses, with the approval digest `approve --expect` needs; `update` says how
many storage calls it made and how many writes were kept; and `storage` shows and clears stores:

```bash
rt storage show target/plugins/update-counter.wasm   # every name and value, escaped
rt storage list                                      # every store, also those whose file is gone
rt storage clear target/plugins/update-counter.wasm  # empties it
```

A new version of your widget, or of a connector, that adds a destination or changes an
operation your widget calls needs a new approval (`needs-new-approval`, naming the change); one
that only adds operations you do not call does not. Turning a connector off, or removing the
approval, discards the data the host kept for your widget.

The grants live in a development record in your data directory, or in the file you give with
`--grants <path>`; kept data, the request log and stores in the development data directory, or
in `--data <dir>`; and your local connectors in `<data>/connectors/`, or in `--connectors <dir>`. An approval belongs to the file it was given for: rebuild in place and it
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
| Storage quota | What the manifest declares, at most 8 MiB | `refused(over-quota)` |
| Storage name and value | 256 bytes, 256 KiB | `refused(invalid-name)`, `refused(over-limit)` |
| Storage calls per run | 1,000, refused ones included | `refused(over-limit)` |
| Storage written per update | 1 MiB of names and values | `refused(over-limit)` |
| Store file | 5 times the approved quota plus 64 KiB | `failed(over-limit)`, and the update keeps nothing |

Each call runs in a fresh instance: nothing your plugin keeps in memory survives to the next
call; keep what must last in [storage](#storage). Build Rust plugins for `wasm32-unknown-unknown`: for `wasm32-wasip2`, the standard library
imports WASI, which the host does not offer (`capability-refused`).
