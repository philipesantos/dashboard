# Data Model: The Host Window, Host Dialogs and the CSP Rules

## Approved plugin

What the window loads (R5).

| Field | Type | Rule |
| --- | --- | --- |
| `id` | `PluginId` | from the grant record's approval |
| `origin` | path | the package the approval came from; the window loads it from there |
| `label` | `PluginText` | the manifest's name made safe (R8); the id when nothing is left |
| `monogram` | one grapheme | the label's first grapheme cluster, uppercased |
| `status` | `FrameStatus` | below |

Source: `GrantStore::approvals()` (new, read-only) on the development grant record. Reread when the
window regains focus and on "Reload" (FR-040); a change while a dialog is open waits for its answer.

## FrameStatus

| Variant | Shown as (host words, tone) |
| --- | --- |
| `Loading` | "Loading", default tone |
| `Rendered` | "Rendered", ok tone with its icon |
| `Failed { reason: HostText }` | "Did not render" and the runtime's reason in host words, error tone with its icon |

What `render` returned is never stored for display (FR-008).

## Appearance choice

Saved in `appearance.json` (R11).

| Field | Values |
| --- | --- |
| `version` | `1` |
| `gray` | a gray family of 007 (`gray`, `mauve`, `slate`, `sage`, `olive`, `sand`) |
| `host` | `monochrome` or a chromatic scale |
| `widget` | a chromatic scale |
| `radius` | `none`, `small`, `medium`, `large`, `full` |
| `theme` | `light`, `dark`, `system` |

Valid only when `design_tokens::Appearance::validate` passes. Unreadable or invalid: the default
appearance is used and a notice in host words says so. Written only by the host.

## Dialog request and answer

| Field | Type | Rule |
| --- | --- | --- |
| `name` | `HostText` | the host's name for the decision, for example "Remove a widget" |
| `question` | `HostText` | in the host's words |
| `subject` | `Option<PluginText>` | a plugin's name, shown and exposed after the host's words |
| `refuse` | `HostText` | first control; initial focus; never default |
| `confirm` | `HostText` | last control; never default |
| `window` | window handle | where the dialog is drawn; every other host window shows the scrim |

Answer: `Confirmed` or `Refused`. Closing a window, quitting and Escape answer `Refused`.

States: `Queued` → `Shown { gate: Closed }` → `Shown { gate: Open }` → `Answered`. The gate goes
back to `Closed` when the window regains focus or the content changes; a new dialog starts
`Closed`.

## Input gate

| Field | Type | Rule |
| --- | --- | --- |
| `opened_at` | instant from `Clock` | the first frame that shows the dialog, or the last restart |
| `held` | set of keys and pointer buttons | down when the gate started; ignored until released |
| `delay` | 1000 ms | feature 007's decision |

Input answers the dialog only when `now - opened_at >= delay` and the key or button is not in
`held`.

## Host text

`Msg`: one variant per string the host writes (chrome, frame status, menus, appearance labels and
refusals, dialog names, questions and controls, accessible names). `HostText` is built only from a
`Msg`; `PluginText` only by `label` from a plugin's string. English only in this feature.
