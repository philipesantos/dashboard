# Contract: The Window's Widget Commands

Each command is listed in `app/src-tauri/window-surface.toml` with its reason, granted in
`capabilities/default.json` and registered in `src/commands.rs` (feature 008's three steps).

| Command | Permission | Reason (as listed) |
| --- | --- | --- |
| `widgets_list` | `allow-widgets-list` | The dashboard lists the widgets it shows, with each one's drawn name and icon. |
| `widget_render` | `allow-widget-render` | The dashboard draws a widget's content inside its frame. |
| `widget_act` | `allow-widget-act` | A widget's button or form runs one of the actions its manifest declares. |

## `widgets_list() -> Vec<FrameInfo>`

`FrameInfo`: `{ handle: "w0", name: "Vitrine", icon: { size: 32, rgba: [u8; size*size*4] } | null }`.
The name is already sanitised (research R6).

## `widget_render(handle) -> Rendered`

`Rendered`: `{ content: <normalised content> } | { error: { kind, reason, path? } } | { refused: kind }`.
For content the checker refuses (`bad-shape`, `over-bound`), `reason` is the checker's and `path`
the place in the content (`body[0].text`); for a refused load (`not-loaded`) or a failed render
(`render-failed`), `reason` is the runtime's error kind. The frame draws the kind, the path and
the reason as text after the host's own message (FR-005).
Refused with `unknown-widget` for a handle the host is not showing, and `widget-busy` while an
action of that widget runs.

## `widget_act(handle, action, values) -> Rendered`

Checked in this order, each refusal returned as `{ refused: <kind> }` without calling the plugin:

1. `unknown-widget`: the handle is not one the host is showing, or is malformed.
2. `widget-busy`: a render or an action of this widget is running.
3. `undeclared-action`: the manifest does not declare `action`.
4. `bad-values`: `values` is not an object of strings, has more than 20 entries, a key over 32
   characters (the longest field id), or a value over 500 characters.
5. `secret-in-form`: a value holds a secret the host holds (research R11).

Then the action runs; on success the host keeps its data, renders, and returns the content; on
failure, `{ error: { kind: "action-failed", reason } }`, and the widget keeps its last content.

## What the page never gets

The plugin's raw JSON, its error text as anything but text, a file path, the origin, or any
command beyond these three.
