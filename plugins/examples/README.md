# plugins/examples

Example plugins, one folder each.

- [hello-widget](hello-widget/README.md): Rust, contract v1. Greets the name in its
  configuration; the starting point for a new plugin. It builds from the frozen contract 1.0.0
  (`wit/history/1.0.0`), to show that plugins built for an older v1 minor keep running.
- [github-releases](github-releases/README.md): Rust, contract 1.1.0. Shows the latest releases
  of a GitHub repository, fetched through the catalog's GitHub connector in `update`. It builds
  from the frozen contract 1.1.0 (`wit/history/1.1.0`).
- [update-counter](update-counter/README.md): Rust, contract 1.2.0. Counts its updates and keeps
  the last notes its configuration gives it, in its own storage: `update` writes, `render`
  reads. It builds from the frozen contract 1.2.0 (`wit/history/1.2.0`).
- [private-notes](private-notes/README.md): Rust, contract 1.3.0. Shows notes read as the user's
  account, through its development connector `local/notes`: the host adds the account's
  credential to each request, and the widget never sees it.

Each example has a `manifest.json` beside its `Cargo.toml`: who the plugin is and the
permissions it needs. Each has a guide in [the docs for authors](../../docs/authors/README.md),
which start with [a walkthrough](../../docs/authors/getting-started.md) that builds a widget of
your own outside the repository.

Rust plugins build with cargo and wit-bindgen for `wasm32-unknown-unknown` and become
components with wit-component (constitution, Fixed Technical Decisions): `cargo xtask plugins`
does both inside the repository and adds the manifest, and `runtime pack` does the same for a
widget outside it. TypeScript plugins arrive in Phase 5.
