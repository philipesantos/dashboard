# Contract: The Window's Reviewed Surface

`app/src-tauri/window-surface.toml` lists everything the window may call. It belongs to the
security core (constitution v2.6.1, Principle IX), so every change to it gets line-by-line
review.

## Format

```toml
# What the host window may call. Every entry needs a reason. The test in tests/surface.rs fails
# when this list and the app disagree. To add a host command:
# 1. add it to the commands! macro in src/lib.rs;
# 2. add its permission, "allow-<command>", to capabilities/default.json;
# 3. add both entries here, with the reason the host UI needs it.

[[permission]]
id = "allow-list-widgets"
reason = "The dashboard grid lists the installed widgets."

[[command]]
name = "list_widgets"
reason = "Same as above; the command behind the permission."

[[plugin]]
crate = "tauri-plugin-dialog"
reason = "…"
```

At the end of this feature, the file holds no entries, only the comment.

## What the test compares

| Listed | Compared with |
| --- | --- |
| `[[permission]].id` | The union of `permissions` in `app/src-tauri/capabilities/*.json` |
| `[[command]].name` | `COMMANDS`, the `const` that the `commands!` macro in `src/lib.rs` expands beside `generate_handler!`, and the `AppManifest` command list in `build.rs` |
| `[[plugin]].crate` | The `tauri-plugin-*` dependencies of `app/src-tauri/Cargo.toml` |

The same test also fails when a release build of the app crate enables the `tauri` crate's
`devtools` feature.

## Failure lines

One line per difference, then a non-zero exit:

```text
window-surface.toml: permission "core:event:allow-listen" is granted in capabilities/default.json but not listed
window-surface.toml: command "greet" is registered but not listed
window-surface.toml: command "list_widgets" is listed but not registered
window-surface.toml: plugin "tauri-plugin-opener" is a dependency but not listed
window-surface.toml: entry "list_widgets" has no reason
Cargo.toml: the release build enables tauri's "devtools" feature
```
