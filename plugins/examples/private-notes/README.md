# private-notes

The example widget for contract 1.3.0, in Rust: it shows the notes of a demonstration service,
read as the person's account. It builds from [`wit/v1`](../../../wit/v1/widget.wit), the world
`account-widget`, and calls its development connector,
[`connectors/notes.json`](connectors/notes.json) (`local/notes`), through `accounts`: the host adds
the account's credential to each request itself, and the widget never sees it.

How a widget acts as an account, connecting and disconnecting one, every answer of a call, and how
to build, pack, approve, update and run this example: [accounts](../../../docs/authors/guides/accounts.md).

`notes.test` is not a real service. The runtime's tests, and the docs' checks, run this example
against the test sign-in and notes service (`runtime/tests/common/signin.rs`), through the test
network only they can reach. Without an account connected the service answers with its public
notes.
