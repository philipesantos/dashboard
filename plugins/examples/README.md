# plugins/examples

Example plugins, one folder each.

- [hello-widget](hello-widget/README.md): Rust, contract v0. Greets the name in its
  configuration; the starting point for a new plugin.

Rust plugins build with cargo and wit-bindgen for `wasm32-unknown-unknown` and become
components with wit-component (constitution, Fixed Technical Decisions); `cargo xtask plugins`
does both. TypeScript examples arrive in Phase 2.
