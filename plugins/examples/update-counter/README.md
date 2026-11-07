# update-counter

The example widget for contract 1.2.0, in Rust: it remembers how many times it has updated, when
it first ran and the last five notes its configuration gave it, such as
`{"note": "remember the milk"}`, in a store of its own. It builds from the frozen contract 1.2.0
([`wit/history/1.2.0`](../../../wit/history/1.2.0/widget.wit)), the world `stored-widget`.

What the store does, every answer of a storage call, and how to build, pack, approve, update and
run this example: [storage](../../../docs/authors/guides/storage.md).

Inside the repository, `cargo xtask plugins` builds it into `target/plugins/`, with its manifest,
for the runtime's tests.
