# plugins/examples

Example plugins, one folder each.

- [hello-widget](hello-widget/README.md): Rust, contract v1. Greets the name in its
  configuration; the starting point for a new plugin. It builds from the frozen contract 1.0.0
  (`wit/history/1.0.0`), to show that plugins built for an older v1 minor keep running.
- [github-releases](github-releases/README.md): Rust, contract 1.1.0. Shows the latest releases
  of a GitHub repository, fetched through the catalog's GitHub connector in `update`.

Each example has a `manifest.json` beside its `Cargo.toml`: who the plugin is and the
permissions it needs. [`docs/plugin-authors.md`](../../docs/plugin-authors.md) explains the
contract, the manifest, and how to pack, approve and run a plugin.

Rust plugins build with cargo and wit-bindgen for `wasm32-unknown-unknown` and become
components with wit-component (constitution, Fixed Technical Decisions); `cargo xtask plugins`
does both and adds the manifest. TypeScript examples arrive in feature 006.
