# Contract: The Window's Installation Commands

Each command is listed in `app/src-tauri/window-surface.toml` with its reason, granted in
`capabilities/default.json` and registered in `src/commands.rs` (feature 008's three steps).
Feature 009's three commands stay as they are; `widgets_list` also returns installed widgets.

| Command | Permission | Reason (as listed) |
| --- | --- | --- |
| `install_start` | `allow-install-start` | The person asks, from the chrome, to install the widget of a repository. |
| `update_start` | `allow-update-start` | The person asks whether an installed widget has a newer release. |
| `request_judge` | `allow-request-judge` | The installation dialog shows the warning for the optional grants the person has turned on. |
| `request_approve` | `allow-request-approve` | The person approves, in the host dialog, what it showed. |
| `request_cancel` | `allow-request-cancel` | The person cancels the host dialog, or the window closes with it open. |

## `install_start(address) -> Started`

`address`: the text the person typed, at most 200 characters. Accepted forms:
`github.com/owner/repo` and `https://github.com/owner/repo`, case-insensitive, trailing `/` or `.git`
ignored; anything else is `{ refused: "bad-address" }`. Runs on the blocking pool (fetch, compile,
check); the page shows progress in the chrome, not in a dialog, and cannot cancel it: closing the
window stops it host-side.

`Started`: `{ request, model: DialogModel } | { refused: kind } | { error: { kind, reason } }`.
Refusals before any request: `bad-address`, `request-pending`, `already-installed`. Errors carry
the runtime kinds of `data-model.md` and the existing manifest and connector kinds, and as
`reason` the app's own sentence for the kind, never the error's detail: that can carry what a
package made the compiler say, or paths of the person's files, and goes to the app's error stream
only, escaped.

## `update_start(handle) -> Started`

`handle`: a widget the host shows that is installed (not bundled). `{ none: true }` when no newer
release exists. Otherwise as `install_start`, with `model.kind` `update` or `update-confirm`.

## `request_judge(request, optional_on) -> { warning: [Flow], choice }`

`optional_on`: names of declared optional permissions and connectors, each once, at most 48 (32
permissions and 16 connectors). Unknown names: `{ refused: "bad-choice" }`; a request not
awaiting a decision: `{ refused: "unknown-request" }`. `choice` is the choice digest for this set
and its warning (research R7); the host remembers the last one it returned.

## `request_approve(request, choice, optional_on) -> Approved`

Checked in order, each refusal without granting anything: `unknown-request`; `bad-choice`;
`approval-changed` when `choice` is not the one the host last returned for this request, when the
host's recomputed choice digest for `optional_on` differs, or when the package changed. Then the
host writes package, compiled form, a fresh grant and the installed record (research R5), loads
the plugin, forgets the token and returns `{ approved: { handle } }`. Approving within the
dialog's input delay is prevented by the page; the host does not time it.

## `request_cancel(request) -> {}`

Drops the request and its temporary files. Unknown tokens are ignored (cancel is always safe).

## What the page never gets

A file path, the temporary directory, the release URLs, the compiled bytes, the plugin's raw
manifest, or any way to grant without a request the host opened from `install_start` or
`update_start`.
