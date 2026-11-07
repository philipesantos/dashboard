# Writing widgets

A widget is a WebAssembly component that the dashboard runs with no access to anything until the
person approves what its manifest asks for: no files, no network, no other widget's data. Each
capability is an interface of the contract, `dashboard:widget@1.3.0`, and every call to one can
answer `not-granted`, which a widget handles like any other answer.

These pages are all an author needs; the feature specs they link to are background only.

## Start here

[Get started](getting-started.md): from an empty folder to a widget of your own, built, packed,
approved and running in the development CLI, in Rust. It was walked with Rust 1.99.1, `wit-bindgen`
0.63.0 and `serde_json` 1.0.154, on Windows, macOS and Linux.

## Guides

Each guide starts from an example under `plugins/examples/` of the repository and ends with it
running.

- [Render and update](guides/render-and-update.md): what each call gets and returns, and what the
  host keeps between them (`hello-widget`).
- [Connectors](guides/connectors.md): fetching data through the operations a connector declares
  (`github-releases`).
- [Accounts](guides/accounts.md): calling a connector as the person's account, whose credential
  the widget never sees (`private-notes`).
- [Storage](guides/storage.md): the widget's own store of named values (`update-counter`).
- [Writing connectors](guides/writing-connectors.md): the format of a connector, for a service no
  catalog connector covers.

## Reference

- [The contract](reference/contract.md): every world, interface, function, type and case of 1.3.0,
  when each answer happens and what a widget should do with it.
- [Errors](reference/errors.md): every error the host reports, as the CLI prints it, with its
  cause and its fix.
- [The manifest](reference/manifest.md) and [the connector format](reference/connector-format.md):
  every field.
- [Limits](reference/limits.md): every limit, its default and what happens past it.
- [The development CLI](reference/cli.md): every command and option an author uses.

## What render returns, for now

`render` returns a JSON document of at most 64 KiB. Its shape is the widget's own until Phase 3
of the project, which defines the content the host frame draws (with its design system) in a later
minor of the contract; widgets will adopt it then. Until that minor, the development CLI prints
what `render` returns as indented JSON.

## Other contracts and languages

Contract v0 (`dashboard:widget@0.1.0`), the first one, still runs, with `log` required; new widgets
use v1, and these pages document only v1. A widget built for an older minor of v1 (1.0.0 to 1.2.0)
keeps running unchanged.

The walkthrough and the guides are in Rust. TypeScript plugins arrive in Phase 5. Go and Python
have no docs yet.
