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

`app/src-tauri/permissions/default.toml` declares the app's default permission set, empty:

```toml
[default]
description = "The window starts with no app command. Each one is granted in capabilities/."
permissions = []
```

It exists so that Tauri builds the app's ACL manifest even with no commands, which is what makes
Tauri check app commands against the capabilities (research R4).

## What the test compares

| Listed | Compared with |
| --- | --- |
| `[[permission]].id` | The union of `permissions` in `app/src-tauri/capabilities/*.json` |
| `[[command]].name` | `COMMANDS`, the `const` that the `commands!` macro in `src/lib.rs` expands beside `generate_handler!`, and the `AppManifest` command list in `build.rs` |
| `[[plugin]].crate` | The `tauri-plugin-*` dependencies of `app/src-tauri/Cargo.toml` |

The same test also fails when:
- the generated `app/src-tauri/gen/schemas/acl-manifests.json` has no `__app-acl__` key, because then
  Tauri skips the ACL for app commands;
- `app/src-tauri/Cargo.toml` lists `devtools` among the features of `tauri`, `tauri-runtime-wry`
  or `wry`;
- `tauri.conf.json` gives macOS an entitlements file that grants
  `com.apple.developer.associated-domains` or `webcredentials` (research R7);
- Tauri's drag-and-drop handler is off: `dragDropEnabled: false` in `tauri.conf.json`, or
  `disable_drag_drop_handler` in `src/window.rs` (FR-012).

## Failure lines

One line per difference, then a non-zero exit:

```text
window-surface.toml: permission "core:event:allow-listen" is granted in capabilities/default.json but not listed
window-surface.toml: command "greet" is registered but not listed
window-surface.toml: command "list_widgets" is listed but not registered
window-surface.toml: plugin "tauri-plugin-opener" is a dependency but not listed
window-surface.toml: entry "list_widgets" has no reason
gen/schemas/acl-manifests.json: no app manifest, so app commands skip the ACL (keep permissions/default.toml)
Cargo.toml: the release build enables "devtools" (tauri)
tauri.conf.json: the macOS entitlements grant "webcredentials"
src/window.rs: the drag-and-drop handler is turned off (disable_drag_drop_handler)
```
