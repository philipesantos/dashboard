# dashboard

Provisional name. An open-source desktop dashboard where third-party and AI-generated plugins run sandboxed as WebAssembly components.

## Layout

| Path | Contents |
| --- | --- |
| `app/` | Desktop app: Tauri 2 host (Rust) and React UI |
| `runtime/` | Rust crate that embeds Wasmtime and loads plugin components |
| `wit/` | WIT contracts between the host and plugins |
| `plugins/examples/` | Example plugins |
| `specs/` | Feature specs (GitHub Spec Kit) |

## Development

```bash
pnpm install
pnpm tauri dev          # run the app
pnpm tauri build        # build installers
cargo check --workspace # check every Rust crate
```

Recommended IDE setup: [VS Code](https://code.visualstudio.com/) with the [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) and [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer) extensions.
