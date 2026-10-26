# The development CLI

`runtime` is the host's control while the app has no releases: it packs, approves and runs
widgets on this machine, in development mode, with a development grant record and data directory
of its own. [The walkthrough](../getting-started.md) installs it. Every command prints what it did,
and every text that came from a package, a manifest, a connector or a service is escaped: a
newline shows as `\n`, a control character as `\u{…}`.

The examples below start where the walkthrough ends: in a folder with `my-widget/`, the crate the
walkthrough builds.

```console check output=any
$ cd my-widget
$ cargo build --release --target wasm32-unknown-unknown
```

## Options most commands take

| Option | Meaning |
| --- | --- |
| `--grants <path>` | The grant record to read and change, instead of the development record in the system's data directory |
| `--data <dir>` | Where kept data, stores, accounts and the request log live, instead of the development data directory |
| `--connectors <dir>` | Where local connectors (`local/<name>`) are read from, instead of `<data>/connectors/` |

A command given an unknown option, or without one it needs, prints the usage and exits with 2;
one the host refused exits with 1 ([errors](errors.md)).

<!-- ref: cli pack -->
## `pack`

```text
runtime pack <module-or-component> --manifest <file> --output <file>
```

Turns the core module cargo builds into a component, as needed, checks the manifest and adds it,
and writes the package. It prints the package's digest, which approvals are tied to.

| Option | Meaning |
| --- | --- |
| `--manifest <file>` | The manifest to add ([the manifest](manifest.md)) <!-- ref: cli pack --manifest --> |
| `--output <file>` | Where to write the package <!-- ref: cli pack --output --> |

```console check
$ runtime pack target/wasm32-unknown-unknown/release/my_widget.wasm --manifest manifest.json --output my-widget.wasm
packed my-widget.wasm
digest: sha256:<digest>
```

Text from the manifest is escaped in what the CLI prints:

```json file=sneaky.json check
{
  "manifest_version": 1,
  "id": "local/my-widget",
  "version": "0.1.0",
  "author": "Your name",
  "permissions": [
    { "name": "log\u001b[2J", "required": true }
  ]
}
```

```console check exit=1
$ runtime pack target/wasm32-unknown-unknown/release/my_widget.wasm --manifest sneaky.json --output sneaky.wasm
error: invalid-manifest: permissions[0].name: `log\u{1b}[2J` must be a lowercase letter, then up to 31 lowercase letters, digits or hyphens
```

<!-- ref: cli permissions -->
## `permissions`

```text
runtime permissions <plugin> [--grants <path>] [--data <dir>] [--connectors <dir>]
```

Shows what a package asks for and what is granted. For a widget with connectors or storage, also
each connector with its destinations, the operations the widget calls and where the data goes,
the storage quota, and the approval digest that `approve --expect` needs.

| Option | Meaning |
| --- | --- |
| `--grants <path>` | As above <!-- ref: cli permissions --grants --> |
| `--data <dir>` | As above <!-- ref: cli permissions --data --> |
| `--connectors <dir>` | As above <!-- ref: cli permissions --connectors --> |

```console check
$ runtime permissions my-widget.wasm
plugin: local/my-widget 0.1.0
author: Your name
contract: v1 (1.3.0)
approved: no
permissions:
  log  required  not granted
login providers: none
```

<!-- ref: cli approve -->
## `approve`

```text
runtime approve <plugin> [--expect <digest>] [--grants <path>] [--data <dir>] [--connectors <dir>]
```

Grants every required permission of the package, together, for this package file. A widget that
declares connectors or storage needs `--expect`: the approval digest `permissions` printed, so
what is approved is what was shown, or it is refused as `approval-changed`.

| Option | Meaning |
| --- | --- |
| `--expect <digest>` | The approval digest from `permissions`. For a widget without connectors or storage, which has none, it is not checked <!-- ref: cli approve --expect --> |
| `--grants <path>` | As above <!-- ref: cli approve --grants --> |
| `--data <dir>` | As above <!-- ref: cli approve --data --> |
| `--connectors <dir>` | As above <!-- ref: cli approve --connectors --> |

```console check
$ runtime approve my-widget.wasm
approved local/my-widget 0.1.0
granted: log
```

<!-- ref: cli optional -->
## `optional`

```text
runtime optional <plugin> <permission | connector-id> on|off [--grants <path>] [--data <dir>] [--connectors <dir>]
```

Turns one optional permission or connector on or off. While it is off, every call to it answers
`not-granted`. A required one changes only with a new approval, or by removing the widget:

| Option | Meaning |
| --- | --- |
| `--grants <path>` | As above <!-- ref: cli optional --grants --> |
| `--data <dir>` | As above <!-- ref: cli optional --data --> |
| `--connectors <dir>` | As above <!-- ref: cli optional --connectors --> |

```console check exit=1
$ runtime optional my-widget.wasm log off
error: required-together: log (it is required: required permissions are approved together, and change only with a new approval or by removing it)
```

<!-- ref: cli run -->
## `run`

```text
runtime run <plugin> [--config <json> | --config-file <path>] [--grants <path>] [--data <dir>] [--connectors <dir>]
```

Loads the package, calls `render` once, and prints the widget's log messages, then its content as
indented JSON. For a widget with an updater, `render` gets the data the last `update` kept; it
never reaches the network.

| Option | Meaning |
| --- | --- |
| `--config <json>` | The widget's configuration; `{}` when neither is given <!-- ref: cli run --config --> |
| `--config-file <path>` | The configuration, read from a file <!-- ref: cli run --config-file --> |
| `--grants <path>` | As above <!-- ref: cli run --grants --> |
| `--data <dir>` | As above <!-- ref: cli run --data --> |
| `--connectors <dir>` | As above <!-- ref: cli run --connectors --> |

```json file=config.json check
{"name": "Grace"}
```

```console check
$ runtime run my-widget.wasm --config-file config.json
[plugin] info: rendering my-widget
{
  "text": "Hello, Grace!",
  "title": "Hello"
}
```

<!-- ref: cli update -->
## `update`

```text
runtime update <plugin> [--config <json> | --config-file <path>] [--grants <path>] [--data <dir>] [--connectors <dir>]
```

Calls the widget's `update` once, which may call its connectors and write its store, and keeps the
data it returns for `render`. It prints what the update called and what it did with its store;
[the connectors guide](../guides/connectors.md) shows one. A widget without an updater has nothing
to update:

| Option | Meaning |
| --- | --- |
| `--config <json>` | As for `run` <!-- ref: cli update --config --> |
| `--config-file <path>` | As for `run` <!-- ref: cli update --config-file --> |
| `--grants <path>` | As above <!-- ref: cli update --grants --> |
| `--data <dir>` | As above <!-- ref: cli update --data --> |
| `--connectors <dir>` | As above <!-- ref: cli update --connectors --> |

```console check exit=1
$ runtime update my-widget.wasm
error: missing-updater: local/my-widget exports no updater, so it has nothing to update (previous data kept)
```

<!-- ref: cli requests -->
## `requests`

```text
runtime requests [--plugin <plugin-id>] [--last <n>] [--data <dir>]
```

Prints the request log: every connector call of every update, the refused ones included, with
when, what and how it ended. It prints nothing while the log is empty; [the connectors
guide](../guides/connectors.md) shows one.

| Option | Meaning |
| --- | --- |
| `--plugin <plugin-id>` | Only this widget's calls <!-- ref: cli requests --plugin --> |
| `--last <n>` | Only the last `n` entries; 50 by default <!-- ref: cli requests --last --> |
| `--data <dir>` | As above <!-- ref: cli requests --data --> |

```console check
$ runtime requests --plugin local/my-widget --last 5
```

<!-- ref: cli remove -->
## `remove`

```text
runtime remove <plugin-id> [--grants <path>] [--data <dir>]
```

Removes a widget's approval, every grant with it, and the data the host kept for it.

| Option | Meaning |
| --- | --- |
| `--grants <path>` | As above <!-- ref: cli remove --grants --> |
| `--data <dir>` | As above <!-- ref: cli remove --data --> |

```console check
$ runtime remove local/my-widget
removed local/my-widget
```

<!-- ref: cli storage list -->
## `storage list`

```text
runtime storage list [--grants <path>] [--data <dir>]
```

Lists every widget's store, with its usage and quota; [the storage guide](../guides/storage.md)
shows one.

| Option | Meaning |
| --- | --- |
| `--grants <path>` | As above <!-- ref: cli storage list --grants --> |
| `--data <dir>` | As above <!-- ref: cli storage list --data --> |

```console check
$ runtime storage list
no stores
```

<!-- ref: cli storage show -->
## `storage show`

```text
runtime storage show <plugin> [--grants <path>] [--data <dir>] [--connectors <dir>]
```

Prints a package's store, every name and value escaped.

| Option | Meaning |
| --- | --- |
| `--grants <path>` | As above <!-- ref: cli storage show --grants --> |
| `--data <dir>` | As above <!-- ref: cli storage show --data --> |
| `--connectors <dir>` | As above <!-- ref: cli storage show --connectors --> |

```console check
$ runtime storage show my-widget.wasm
store: local/my-widget from <path>
no store yet
```

<!-- ref: cli storage clear -->
## `storage clear`

```text
runtime storage clear <plugin> | --id <plugin-id> --origin <path> [--grants <path>] [--data <dir>]
```

Empties a widget's store: by its package, or by its id and origin when the package file is gone.

| Option | Meaning |
| --- | --- |
| `--id <plugin-id>` | The widget's id, with `--origin` <!-- ref: cli storage clear --id --> |
| `--origin <path>` | Where its package was, as `storage list` shows it <!-- ref: cli storage clear --origin --> |
| `--grants <path>` | As above <!-- ref: cli storage clear --grants --> |
| `--data <dir>` | As above <!-- ref: cli storage clear --data --> |

```console check
$ runtime storage clear my-widget.wasm
no store of local/my-widget from <path>
```

<!-- ref: cli accounts connect -->
## `accounts connect`

```text
runtime accounts connect <connector-id> [--origin <path>] [--client <id>] [--no-browser] [--grants <path>] [--data <dir>] [--connectors <dir>]
```

Connects an account to a connector: it opens the service's sign-in in the system's browser, or,
for a key, reads it from the terminal without showing it. The account goes to the development
vault in the system's keychain. No command prints a credential, reads one from a file or takes one
as an argument. [The accounts guide](../guides/accounts.md) shows one.

| Option | Meaning |
| --- | --- |
| `--origin <path>` | The connector's file, for a local connector the CLI cannot find from its id <!-- ref: cli accounts connect --origin --> |
| `--client <id>` | The client id of the app you registered with the service, which the CLI remembers for the connector <!-- ref: cli accounts connect --client --> |
| `--no-browser` | Prints the sign-in address instead of opening it <!-- ref: cli accounts connect --no-browser --> |
| `--grants <path>` | As above <!-- ref: cli accounts connect --grants --> |
| `--data <dir>` | As above <!-- ref: cli accounts connect --data --> |
| `--connectors <dir>` | As above <!-- ref: cli accounts connect --connectors --> |

<!-- ref: cli accounts list -->
## `accounts list`

```text
runtime accounts list [--grants <path>] [--data <dir>] [--connectors <dir>]
```

Lists the connected accounts and keys, the widgets that use each, and any that must be connected
again.

| Option | Meaning |
| --- | --- |
| `--grants <path>` | As above <!-- ref: cli accounts list --grants --> |
| `--data <dir>` | As above <!-- ref: cli accounts list --data --> |
| `--connectors <dir>` | As above <!-- ref: cli accounts list --connectors --> |

```console check
$ runtime accounts list
nothing connected
```

<!-- ref: cli accounts disconnect -->
## `accounts disconnect`

```text
runtime accounts disconnect <connector-id> [--origin <path>] [--grants <path>] [--data <dir>] [--connectors <dir>]
```

Deletes an account from the vault and revokes it at the service, when the connector says how.

| Option | Meaning |
| --- | --- |
| `--origin <path>` | The connector's file, when it cannot be found from its id <!-- ref: cli accounts disconnect --origin --> |
| `--grants <path>` | As above <!-- ref: cli accounts disconnect --grants --> |
| `--data <dir>` | As above <!-- ref: cli accounts disconnect --data --> |
| `--connectors <dir>` | As above <!-- ref: cli accounts disconnect --connectors --> |

```console check exit=1
$ runtime accounts disconnect local/notes
error: account-not-connected: local/notes cannot be found now; give the origin of its account with --origin
```

<!-- ref: cli check-connector -->
## `check-connector`

```text
runtime check-connector <file>
```

Checks a connector file against [the connector format](connector-format.md) and the rules only the
host checks.

```json file=weather.json check
{
  "connector_version": 1,
  "id": "local/weather",
  "version": "0.1.0",
  "author": "You",
  "description": "Forecasts from my weather service",
  "destinations": { "api": "https://api.weather.example" },
  "operations": {
    "forecast": {
      "kind": "read",
      "method": "GET",
      "destination": "api",
      "path": "/v1/{city}/forecast",
      "query": { "days": "{days}", "units": "metric" },
      "headers": { "Accept": "application/json" },
      "parameters": {
        "city": { "type": "text", "max_length": 64, "pattern": "[a-z-]+" },
        "days": { "type": "integer", "minimum": 1, "maximum": 7, "required": false }
      },
      "response": { "format": "json", "headers": ["ETag"] }
    }
  }
}
```

```console check
$ runtime check-connector weather.json
ok: local/weather 0.1.0, 1 destination, 1 operation (1 read)
```

`runtime bench` is for the project's own measurements, and is not covered here.
