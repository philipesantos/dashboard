# Writing a plugin

A plugin is a WebAssembly component that renders one widget. It runs in a sandbox and starts
with no power at all: it gets a capability only when its manifest declares it and the user
grants it. This guide covers what a plugin needs today; network, storage and accounts arrive in
later features.

Start from [`plugins/examples/hello-widget`](../plugins/examples/hello-widget), a Rust plugin
with everything below in place.

## The contract

The contract is the WIT interface between the host and a plugin. Build against
[`wit/v1`](../wit/v1/widget.wit) (`dashboard:widget@1.0.0`):

- Your plugin exports `renderer`, with `render(config: string) -> result<string, string>`. It
  receives its configuration as JSON text and returns the widget content as JSON text, at most
  64 KiB. An `err` reaches the host as a `plugin-error` with your message.
- It may import the capability interfaces of the contract. Today there is one, `log`.

Every capability function returns `result<T, E>`, and the first case of `E` is `not-granted`.
That is the answer when the capability is an optional permission the user has not turned on
(see below). A plugin that ignores it still runs; one that handles it can tell the user what is
missing, inside its own widget.

New capabilities will arrive as new interfaces in later minor versions of v1 (`1.1.0`,
`1.2.0`, …). A plugin built against a newer version than the host runs is refused
(`contract-mismatch`), because it may use what the host does not have.

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
  "login_providers": []
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

There is no app yet: the `runtime` command-line tool stands in for the host. See its
[contract](../specs/002-plugin-permissions/contracts/cli.md) for every command.

```bash
rt() { cargo run -q -p runtime --release -- "$@"; }
rt permissions target/plugins/hello-widget.wasm   # what it requests and what is granted
rt approve target/plugins/hello-widget.wasm       # grants every required permission
rt run target/plugins/hello-widget.wasm --config '{"name":"Ada"}'
rt optional my-widget.package.wasm log on         # turns an optional permission on or off
rt remove local/hello-widget                      # removes the approval
```

The grants live in a development record in your data directory, or in the file you give with
`--grants <path>`. An approval belongs to the file it was given for: rebuild in place and it
stays; a copy elsewhere, even byte for byte the same, needs its own approval, which replaces
the first one. So another file that claims your plugin's id gets none of its grants.

Every refusal names its rule, in one line:

```text
error: not-granted: log (local/hello-widget has no approval; approve it first)
```

## Limits

The host sets these for every `render` call; a plugin can neither read nor change them.

| Limit | Default | When exceeded |
| --- | --- | --- |
| Plugin package | 10 MiB | `invalid-package` at load |
| CPU budget | 500,000,000 fuel units, about 0.35 s of computation | `cpu-budget-exceeded` |
| Time | 1 s | `time-limit-exceeded` |
| Memory | 32 MiB, all memories together | `memory-limit-exceeded` |
| Widget content | 64 KiB, and it must be valid JSON | `output-too-large`, `invalid-output` |

Each call runs in a fresh instance: nothing your plugin keeps in memory survives to the next
call. Build Rust plugins for `wasm32-unknown-unknown`: for `wasm32-wasip2`, the standard library
imports WASI, which the host does not offer (`capability-refused`).
