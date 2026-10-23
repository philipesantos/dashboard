# dashboard

Provisional name. An open-source desktop dashboard where third-party and AI-generated plugins run sandboxed as WebAssembly components.

## Layout

| Path | Contents |
| --- | --- |
| `app/` | Desktop app: Tauri 2 host (Rust) and React UI |
| `runtime/` | Rust crate that embeds Wasmtime and runs plugin components, with a command-line test tool |
| `wit/` | WIT contracts between the host and plugins |
| `plugins/examples/` | Example plugins, starting with [hello-widget](plugins/examples/hello-widget/README.md) |
| `plugins/hostile/` | Hostile plugins: the security test suite's fixtures, never shipped |
| `xtask/` | Repository tasks, such as `cargo xtask plugins` |
| `specs/` | Feature specs (GitHub Spec Kit) |
| `docs/` | Project documents, such as the [threat model](docs/threat-model.md) and [decisions](docs/decisions/) |

## Development

```bash
rustup target add wasm32-unknown-unknown wasm32-wasip2  # plugins build for WebAssembly
pnpm install
pnpm tauri dev          # run the app
pnpm tauri build        # build installers
cargo xtask plugins     # build every plugin into target/plugins/
cargo run -p runtime -- approve target/plugins/hello-widget.wasm  # approve its permissions
cargo run -p runtime -- run target/plugins/hello-widget.wasm      # run a plugin
cargo check --workspace # check every Rust crate
```

A plugin runs only with the permissions its manifest declares and you approve.
[docs/plugin-authors.md](docs/plugin-authors.md) explains how to write, pack, approve and run
one.

Recommended IDE setup: [VS Code](https://code.visualstudio.com/) with the [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) and [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) extensions.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) and the [Code of Conduct](CODE_OF_CONDUCT.md).

## Security

See [SECURITY.md](SECURITY.md) to report a vulnerability, and the
[threat model](docs/threat-model.md) for what the project defends against.

## License

[Apache-2.0](LICENSE)
