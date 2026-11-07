# Spike: GPUI as the host UI

A throwaway prototype that answers one question: can [GPUI](https://github.com/zed-industries/zed)
(Zed's UI framework), through [gpui-kit](https://github.com/longbridge/gpui-kit), replace Tauri 2 +
React as the host UI of Phase 3? The answer, with the evidence, is in [REPORT.md](REPORT.md).

Nothing here is part of the product. It lives in its own Cargo workspaces, changes nothing in
`app/`, `runtime/`, `wit/` or `specs/`, and uses the `runtime` crate unchanged.

## What it does

- A dashboard window with a 12-column grid of widgets. Add them from the toolbar, drag a frame by
  its header to move it, drag its bottom-right corner to resize it. The layout is saved on every
  change.
- Each widget is a plugin run by the `runtime` crate: the repository's `hello-widget` and the
  spike's `ui-sampler` (`plugin/`). The host calls `render` off the UI thread and draws the JSON it
  returns with its own components (`widget-kit/`): text, list, table and button. Nothing else: a
  password field, a host color, an unknown field or block is refused, and the frame shows why.
- Every widget sits in a host frame with the plugin's name, a monogram icon and its id from the
  manifest. The content is clipped to the frame's body and scrolls inside it.
- Adding a plugin that has no approval opens the host's approval dialog, which shows the
  permissions and, at the top, the person's visual secret (a color chosen in Settings). The
  dialog uses host-only colors that the widget kit cannot reach.
- Light and dark themes from design tokens (`design/tokens.json`), following the system or set in
  Settings.
- Keyboard: Tab and Shift-Tab go through the toolbar, every frame and every widget button. On a
  focused frame, the arrow keys move it, Shift and the arrow keys resize it, and Delete removes
  it. In a dialog, focus stays inside and Escape cancels.

## Layout

| Path | What |
| --- | --- |
| `widget-kit/` | The host's widget components on gpui-base, and the content format they accept. Depends on GPUI and gpui-base only, not on gpui-component. |
| `host/` | The app: grid, frames, dialogs, tokens, the bridge to `runtime`. `host/tests/ui.rs` drives it headless. |
| `plugin/ui-sampler/` | The spike's plugin (contract v1, `widget` world), in a workspace of its own for `wasm32-unknown-unknown`. |
| `pack/` | Builds and packs `ui-sampler` as `cargo xtask plugins` packs `plugins/`. |
| `design/tokens.json` | Design tokens: typography, spacing, widget and host palettes for light and dark, visual secret choices. |
| `scripts/` | Measurement: build times, startup, memory, idle CPU (Linux and Windows). |
| `.github/workflows/spike-gpui.yml` (repository root) | CI on Windows, macOS and Linux, on both GPUI bases, with the Tauri app measured beside it. |

## Run it

Linux needs these packages (Debian and Ubuntu names):

```sh
sudo apt-get install -y libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libvulkan-dev \
  libx11-xcb-dev libfontconfig-dev libfreetype-dev libdbus-1-dev mesa-vulkan-drivers
```

From the repository root, build the plugins the spike embeds, then run it:

```sh
cargo xtask plugins                                                   # target/plugins/hello-widget.wasm
cargo run --manifest-path spikes/gpui/Cargo.toml -p pack              # spikes/gpui/target/plugins/ui-sampler.wasm
cargo run --manifest-path spikes/gpui/Cargo.toml -p dashboard-gpui-spike --release
```

On first start, Settings opens to choose your color. Then add widgets from the toolbar; the first
widget of each plugin opens the approval dialog.

To build on GPUI Fast instead of the default `gpui-pre` snapshot, add `--features gpui-fast`.

The spike keeps its approvals, layout and settings in its own directory, never the app's or the
development CLI's: `<data dir>/dev.philsantos.dashboard-gpui-spike/` (`~/.local/share/…` on Linux,
`%APPDATA%\…` on Windows, `~/Library/Application Support/…` on macOS), or `SPIKE_DATA_DIR`.

### Prebuilt binaries

Each run of the **Spike GPUI** workflow uploads the binaries for Windows, macOS and Linux, on
both bases (`gpui-spike-<system>-<base>`). They embed both plugins, so they run on their own. The
macOS binary is not signed: after unzipping, run `xattr -d com.apple.quarantine dashboard-gpui-spike`
and start it from a terminal.

### For measurement

| | |
| --- | --- |
| `--dev-approve` | Approves the two embedded packages and lays out four widgets without the dialog, as the development CLI grants to local packages (Principle VII), so measurements can run unattended. |
| `SPIKE_DATA_DIR` | Where the spike keeps its records. |
| `SPIKE_EXIT_AFTER_MS` | Quits after this many milliseconds. |
| `SPIKE_DRAG_BENCH=1` | Drags the first widget along a path for 5 s through the mouse code path and prints frame times and CPU. |
| `SPIKE_REQUEST_ADD=<entry>` | Asks to add a catalog entry at start (`hello`, `sampler`, `overflow`, `spoof`, `password`, `host-color`), so the approval dialog opens. |
| `RUST_LOG=gpui_wgpu=info` | Shows which graphics adapter and backend GPUI picked. |

`scripts/measure-linux.sh` and `scripts/measure-windows.ps1` measure any app the same way (time
until the window appears, memory over the whole process tree, idle CPU), so the Tauri app is
measured with the same scripts.

## Tests

```sh
cargo test --manifest-path spikes/gpui/Cargo.toml --workspace
```

`widget-kit` tests the content format (refusals, limits, hidden characters). `host` tests the
grid rules and the tokens (WCAG AA contrast; no widget color near a host-reserved color or a
secret choice). `host/tests/ui.rs` opens the real window on GPUI's headless test platform and
drives it with the keyboard: Tab reaches every frame and widget button, the arrow keys move and
resize, the dialog keeps focus and Escape cancels, content outside a frame is not drawn, and
the accessibility roles and names are the expected ones.
