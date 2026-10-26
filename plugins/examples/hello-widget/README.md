# hello-widget

The example widget for contract v1, in Rust: it greets the name in its configuration and writes
one log message. It builds from the frozen contract 1.0.0
([`wit/history/1.0.0`](../../../wit/history/1.0.0/widget.wit)), the world `widget`, to show that a
widget built for an older v1 minor keeps running; a new widget binds its copy of
`wit/history/1.3.0`.

- What `render` gets and returns, what the host keeps between calls, and how to build, pack,
  approve and run this example: [render and update](../../../docs/authors/guides/render-and-update.md).
- A widget of your own, from an empty folder outside the repository:
  [get started](../../../docs/authors/getting-started.md).
- Every item of the contract, every error and every limit:
  [the reference](../../../docs/authors/README.md#reference).

Inside the repository, `cargo xtask plugins` builds every crate of `plugins/`, this one included,
into `target/plugins/`, with its manifest, for the runtime's tests.
