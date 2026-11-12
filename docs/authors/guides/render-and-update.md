# Render and update

A widget has two entry points: `render`, which every widget implements, and `update`, which a
widget implements when it fetches data or writes its store. A widget with buttons or forms has a
third, `act`, [at the end](#actions). This guide starts from the example
`hello-widget`, which only renders, and says what each call gets, what it may do and what the host
keeps between calls. It starts in the clone of [the walkthrough](../getting-started.md), with the
development CLI installed.

## render

```console check output=any
$ cd dashboard/plugins/examples/hello-widget
$ cargo build --release --target wasm32-unknown-unknown
```

The examples build in one workspace, `plugins/`, so cargo writes the module under
`plugins/target/`:

```console check
$ runtime pack ../../target/wasm32-unknown-unknown/release/hello_widget.wasm --manifest manifest.json --output hello-widget.wasm
packed hello-widget.wasm
digest: sha256:<digest>
$ runtime approve hello-widget.wasm
approved local/hello-widget 0.2.0
granted: log
$ runtime run hello-widget.wasm --config '{"name":"Ada"}' --check-content
[plugin] info: rendering hello-widget
{
  "body": [
    {
      "text": "Hello",
      "type": "text",
      "variant": "heading"
    },
    {
      "text": "Hello, Ada!",
      "type": "text"
    }
  ],
  "format": 1
}
content: drawn
```

What `render` gets and gives:

- **It gets** the widget's configuration as JSON text: here `{"name":"Ada"}`, or `{}` when none is
  given. For a widget with an updater it gets a document of the host's instead, with the data
  the last update kept ([the contract](../reference/contract.md#renderer)).
- **It returns** its content as a JSON document of at most 64 KiB, or an error message for the
  maintainer. The content is in the host's format, components the app draws in the widget's
  frame ([widget content](../reference/content.md)); `--check-content` says whether it would.
- **It reaches nothing.** `render` never waits on the network: a connector call from it answers
  `refused(called-outside-update)`, and a storage write `refused(render-only-reads)`. It may read
  its store.
- **It starts from nothing.** Each call runs in a fresh instance of the widget: a global variable
  set in one call is gone in the next. What must last goes in the store, or in the data `update`
  returns.
- **It is bounded.** One second, 500,000,000 units of fuel and 32 MiB of memory per call
  ([limits](../reference/limits.md)); past one, the call stops and the CLI says which.

A `render` that returns an error shows it as the host shows every text from a widget, escaped:

```console check exit=1
$ runtime run hello-widget.wasm --config '{"name":42}'
[plugin] info: rendering hello-widget
error: plugin-error: "name" in the configuration must be a string
```

## update

A widget whose manifest declares a connector or storage exports `updater` too, in a world that
has it, such as `connected-widget` or `stored-widget`. `hello-widget` has none:

```console check exit=1
$ runtime update hello-widget.wasm
error: missing-updater: local/hello-widget exports no updater, so it has nothing to update (previous data kept)
```

What `update` gets and gives:

- **It gets** the widget's configuration, as `render` would without an updater.
- **It may call** its connectors and write its store; [connectors](connectors.md),
  [accounts](accounts.md) and [storage](storage.md) walk through each.
- **It returns** the widget's data as a JSON document of at most 256 KiB, which the host keeps
  and gives to the next `render` with when it was fetched and how the update went. An error, or
  data over the limit, keeps the previous data, and `render` gets the failure in `last_update`.
- **It is bounded** like `render`, with 10 seconds instead of one: an update waits on the network.

In the development CLI, an update runs when you run `runtime update`; the app will update on load
and on a schedule. `render` never waits for one: it shows what the last update kept, so a widget
renders even with the network down.

## Actions

A button or a form in a widget's content asks the host to run an action, which the widget's
`act` carries out. The widget declares each action's name in its manifest and builds an
`interactive-` world, which exports `actor` too ([the contract](../reference/contract.md#actor)).
`hello-widget` has none:

```console check exit=1
$ runtime act hello-widget.wasm greet
error: undeclared-action: `greet` is not an action local/hello-widget declares
```

What `act` gets and gives:

- **It gets** the action's name, one the manifest declares, and a document with the widget's
  configuration, its current data, and the form's values by field id, each a string.
- **It may call** its connectors' operations that only read, and write its store, all or nothing,
  as `update` does.
- **It returns** the widget's new data, which the next `render` gets; the app then draws the new
  content. The host keeps that data in memory only, while the app runs: what must outlive a
  restart goes in the store.
- **It is bounded** like `update`.

[Widget content](../reference/content.md#actions) shows the buttons and forms that run actions,
and `runtime act` runs one in the development CLI ([the CLI](../reference/cli.md#act)).
