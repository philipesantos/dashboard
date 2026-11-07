# github-releases

The example widget for contract 1.1.0, in Rust: it shows the latest releases of the GitHub
repository in its configuration, such as `{"owner": "rust-lang", "repo": "rust"}`, fetched in
`update` through the catalog's GitHub connector, `catalog/github`. It builds from the frozen
contract 1.1.0 ([`wit/history/1.1.0`](../../../wit/history/1.1.0/widget.wit)), the world
`connected-widget`.

How a widget calls a connector, what the person approves, every answer of a call, and how to
build, pack, approve, update and run this example: [connectors](../../../docs/authors/guides/connectors.md).

Inside the repository, `cargo xtask plugins` builds it into `target/plugins/`, with its manifest,
for the runtime's tests, which reach a test service answering as GitHub does.
