# Contract: Actions and the Manifest's New Fields

## WIT: `dashboard:widget@1.4.0`

1.4.0 only adds to 1.3.0: one interface and six worlds. Nothing of 1.3.0 changes.

```wit
/// A widget's actions: what its buttons and forms ask the host to run. The manifest declares
/// each action's name, and the host never calls `act` with another.
interface actor {
  /// `action` is a name the manifest declares. `input` is the JSON document
  /// `{"config": <the widget's configuration>, "data": <the data the last update or action
  /// returned, or null>, "values": {<field id>: <string>}}`, with `values` empty for a button.
  /// On success the plugin returns the widget's data, a JSON document of at most 256 KiB, which
  /// the host keeps and gives to the next render; on failure, a message for the maintainer.
  /// Like `update`, an action may write the widget's store, all or nothing, and call its
  /// connectors' operations that only read; an operation that changes data is refused.
  /// A plugin that exports `actor` receives in `render` the same document as a plugin that
  /// exports `updater`.
  act: func(action: string, input: string) -> result<string, string>;
}

world interactive-widget { /* imports of widget */ export renderer; export actor; }
world interactive-connected-widget { /* imports of connected-widget */ export renderer; export updater; export actor; }
world interactive-stored-widget { /* imports of stored-widget */ export renderer; export updater; export actor; }
world interactive-connected-stored-widget { /* … */ export renderer; export updater; export actor; }
world interactive-account-widget { /* … */ export renderer; export updater; export actor; }
world interactive-account-stored-widget { /* … */ export renderer; export updater; export actor; }
```

(Each world repeats the imports of its 1.3.0 counterpart exactly; the elided parts are written
in full in `wit/v1/widget.wit`.)

## The host's rules

| Rule | Error kind |
| --- | --- |
| The action name is not in the manifest's `actions` | `undeclared-action`, before the plugin is instantiated |
| The package exports `actor` and declares no action, or declares actions and does not export `actor` | `invalid-manifest` at load |
| An operation that changes data | `change-needs-user-action`, as today |
| Output over 256 KiB, time over the update limit, fuel or memory over render's | the existing kinds, as for `update` |
| The plugin was built for 1.3.0 or earlier | it has no actions; any action is `undeclared-action` |

## Manifest additions

```json
{
  "manifest_version": 1,
  "id": "local/showcase",
  "version": "0.1.0",
  "author": "Dashboard contributors",
  "name": "Vitrine",
  "icon": "iVBORw0KGgo…",
  "permissions": [],
  "actions": [{ "name": "increment" }, { "name": "add" }]
}
```

| Field | Accepts | Required |
| --- | --- | --- |
| `name` | 1 to 60 characters, no control characters | no |
| `icon` | base64 of a PNG: at most 8 KiB, square, 16 to 64 px, not animated | no |
| `actions` | up to 16 objects `{ "name" }`, names unique, `^[a-z][a-z0-9-]{0,31}$` | when the package exports `actor` |

`wit/manifest.schema.json` gains them, and `runtime/tests/manifest_schema.rs` gains their
agreement cases. The icon's PNG checks the schema cannot express (format, dimensions, animation)
are host-only rules, listed in `the_host_also_refuses_what_a_schema_cannot_see`.

## CLI

`runtime run <package>` also checks the content and prints the host's outcome (drawn, log
entries, or the error). New: `runtime act <package> <action> [--value id=text]…` runs an action
with the development record and prints the data it returned and the content of the next render.
