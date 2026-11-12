# The manifest

The manifest says who a widget is and what it asks for. It is a JSON file, at most 16 KiB, that
`runtime pack` checks and puts inside the package; the host reads it again before it loads the
widget. Its JSON Schema is `wit/manifest.schema.json` in the repository: point `$schema` at it and
an editor checks the file as you write it.

```json
{
  "manifest_version": 1,
  "id": "local/my-widget",
  "version": "0.1.0",
  "author": "Your name",
  "permissions": [
    { "name": "log", "required": true },
    { "name": "storage", "required": true, "quota_bytes": 65536 }
  ],
  "connectors": [
    { "id": "catalog/github", "major": 1, "operations": ["latest-release"], "required": true }
  ]
}
```

Unknown fields are refused, so a misspelt one never passes silently: the error is
`invalid-manifest`, naming the field ([errors](errors.md)).

## Who the widget is

| Field | Accepts | Required | At approval |
| --- | --- | --- | --- |
| `$schema` | Any text up to 2048 characters | No | Ignored; it is for editors <!-- ref: manifest $schema --> |
| `manifest_version` | `1` | Yes | The format's version <!-- ref: manifest manifest_version --> |
| `id` | `local/<name>` for a widget from no repository, or `github.com/<owner>/<repo>`; lowercase, at most 160 characters. A repository id is accepted, but nothing checks yet that the package came from that repository: installing from repositories (Phase 3) will | Yes | An approval is for this id from this package's file: another file with the same id has none <!-- ref: manifest id --> |
| `version` | A Semantic Versioning 2.0.0 version, such as `0.1.0` | Yes | Shown; a later version that asks for more needs a new approval <!-- ref: manifest version --> |
| `author` | 1 to 100 characters, no control characters | Yes | Shown to the person, escaped <!-- ref: manifest author --> |
| `login_providers` | `["github"]` or `[]` | No | Obsolete since contract 1.3.0: accepted, ignored, grants nothing. Accounts come from the connectors a widget declares <!-- ref: manifest login_providers --> |

## How the frame shows it

The app draws each widget in a frame: a header with the widget's name and icon, and the content
`render` returns below it ([widget content](content.md)). The header comes from the manifest
only, never from content, so a widget cannot change it while it runs.

| Field | Accepts | Required | In the frame |
| --- | --- | --- | --- |
| `name` | 1 to 60 characters, no control characters | No | The name in the header, drawn as text: markup shows as typed, bidirectional controls are removed, and a name too long for the header is cut. Without it, the frame shows the last part of the id <!-- ref: manifest name --> |
| `icon` | Standard base64 of a PNG of at most 8 KiB, square, 16 to 64 pixels on a side, not animated. Anything else, an SVG included, is refused as `invalid-manifest` | No | The host decodes it and draws its pixels in a fixed box beside the name. Without it, the frame draws a monogram <!-- ref: manifest icon --> |

## Actions

`actions` names what the widget's buttons and forms may ask the host to run
([widget content](content.md#actions)). A widget that declares any exports `actor`, from contract
1.4.0, in one of the `interactive-` worlds ([the contract](contract.md#actor)); one that exports
`actor` declares at least one. Either without the other is refused as `invalid-manifest` when
the host loads it. The host runs only these names, and only from the widget's own controls: any
other is `undeclared-action`, refused before the widget runs ([errors](errors.md)).

```json
"actions": [{ "name": "increment" }, { "name": "add" }]
```

| Field | Accepts | Required | At approval |
| --- | --- | --- | --- |
| `actions` | Up to 16 entries, each name once; absent means none | With `actor` | Not shown and not approved: an action runs under the permissions and connectors the widget was approved for, like `update`. A new version may add actions without a new approval <!-- ref: manifest actions --> |
| `actions[]/name` | A lowercase letter, then up to 31 lowercase letters, digits or hyphens | Yes | The name `act` gets <!-- ref: manifest actions[]/name --> |

## Permissions

`permissions` lists every capability interface the widget imports, by name: `log` and `storage`.
A component that imports one its manifest does not list is refused as `not-declared`; a name the
host does not know, as `unknown-permission`.

| Field | Accepts | Required | At approval |
| --- | --- | --- | --- |
| `permissions` | Up to 32 entries; `storage` at most once | Yes | Each is shown, granted or not <!-- ref: manifest permissions --> |
| `permissions[]/name` | `log`, `storage` | Yes | The capability <!-- ref: manifest permissions[]/name --> |
| `permissions[]/required` | `true`: approved together with every other required permission, or the widget does not run. `false`: optional; it starts off, and while it is off every call answers `not-granted` | Yes | Required ones are granted by `runtime approve`; optional ones are turned on and off with `runtime optional` <!-- ref: manifest permissions[]/required --> |
| `permissions[]/quota_bytes` | On `storage` only, and required there: 64 to 8388608, the most the store may hold in usage bytes (every name and value, plus 64 bytes per entry) | On `storage` | Shown; a later version that raises it needs a new approval. A quota above the host's cap is `invalid-manifest` <!-- ref: manifest permissions[]/quota_bytes --> |

## Connectors

`connectors` lists the connectors the widget calls, each a permission of its own, and the
operations it calls of each. A widget that declares one is built for contract 1.1.0 or later and
exports `updater` ([the connectors guide](../guides/connectors.md)).

| Field | Accepts | Required | At approval |
| --- | --- | --- | --- |
| `connectors` | Up to 16 entries; absent means none | No | Each connector is shown with its destinations, the operations the widget calls and where the data goes; the approval digest covers all of it <!-- ref: manifest connectors --> |
| `connectors[]/id` | `catalog/<name>`, `local/<name>` (the development CLI only, from `--connectors <dir>`), or a repository id, refused until Phase 3 | Yes | The connector <!-- ref: manifest connectors[]/id --> |
| `connectors[]/major` | 0 to 65535: the connector's major version the widget is built for | Yes | Another major is `connector-unavailable` <!-- ref: manifest connectors[]/major --> |
| `connectors[]/operations` | 1 to 32 operation names, each once | Yes | Only these are shown and callable; any other is `refused(operation-not-declared)` <!-- ref: manifest connectors[]/operations --> |
| `connectors[]/required` | `true` or `false`, as for a permission | Yes | As for a permission <!-- ref: manifest connectors[]/required --> |
| `connectors[]/account` | `true`: the widget calls this connector as the account connected to it, through `accounts` (contract 1.3.0 or later); the connector must declare a credential. `false`, the default: calls go through `connectors`, without a credential | No | Shown with the credential and the access it carries. A connector whose credential is required needs `true`, or the widget is refused as `account-use-not-declared` <!-- ref: manifest connectors[]/account --> |

## Approvals and new versions

- **Required and optional.** The person approves every required permission and connector
  together, or the widget does not run; they change only with a new version, approved again, or
  by removing the widget's approval (`runtime remove`). An optional one starts off and is turned
  on and off at any time, except optional `storage`, which turns on only with a quota already
  approved; while it is off, every call to it answers `not-granted` and does nothing. A widget cannot read its grants, ask for a permission while it runs, or open a
  dialog: it learns that something is off only from `not-granted`.
- **An approval belongs to a package file.** Rebuilding the widget in place keeps it; a copy
  elsewhere, even byte for byte the same, needs its own approval, which replaces the first one.
  So another file that claims a widget's id gets none of its grants.
- **A new version keeps its approval** as long as it asks for nothing new. One that requires a
  permission, a connector, an operation, a larger quota or more of an account's access than was
  approved is refused as `needs-new-approval`, naming the change, until the person approves it
  again; the grants it had stay as they were. An optional permission a new version adds starts off
  and needs no approval; a connector it adds, optional or not, needs one, and so does a connector that
  was optional and off and is now required. A permission it no longer declares stops being
  granted.
- **Data the host kept** for a widget is discarded when a connector it used is turned off, when
  the approval is removed or changes, when the package changes (a rebuild included), and when the
  account its data was fetched as is disconnected. The widget's own store is not: it stays until
  the person clears it.
