# Data Model: Declarative Widget UI and the Host Frame

Formats are in [contracts/](contracts/). Bounds are in research R5 and R6.

## Widget content

What a render returns, checked by `widget-content` (research R4).

| Field | Rule |
| --- | --- |
| `format` | `1`. Another value is invalid content. |
| `body` | 1 to 500 components in all, nested at most 8 deep. |

**States**: valid (drawn), drawn with host log entries (unknown fields, tones or components),
invalid (the frame's error state).

## Component

Every component has `type`. The rest by type, all optional unless marked:

| Type | Fields |
| --- | --- |
| `text` | `text` (required), `variant`: `default`, `muted`, `heading`; `tone` |
| `list` | `items` (required, strings or `{text, tone}`) |
| `table` | `columns` (required, strings), `rows` (required, arrays of strings, as many as columns) |
| `chart` | `kind`: `line` or `bar`; `labels` (required); `series` (required, 1 to 5, each `{name, values}`) |
| `button` | `label` (required), `action` (required, a declared action), `variant`: `primary`, `secondary`, `outline`, `ghost`, `destructive` |
| `form` | `fields` (required), `submit` (required, `{label, action}`) |
| `stack` | `children` (required, components) |

**Form field**: `{id, label, kind, …}` with `kind` one of `text`, `number`, `checkbox`, `select`
(`options`: strings). `id` is the key of the value sent to the action. No other kind exists.

**Validation**:
- Strings at most 2,000 characters; lists 200 items; tables 20 columns and 200 rows; charts 5
  series of 200 points; forms 20 fields, 50 options per select.
- `tone` is one of the widget map's six tones; an unknown name draws as `default` and is logged.
- An unknown field is dropped and logged once per document; an unknown `type` draws the host's
  placeholder and is logged.

## Normalised content

What the page receives: the same tree with only known fields, every tone resolved, status tones
carrying their icon name, and each form field's host id. It never holds the plugin's JSON.

## Content error

| Field | Rule |
| --- | --- |
| `kind` | `not-json`, `bad-shape`, `over-bound`, `render-failed` |
| `reason` | Short, host-written; names the field path or the bound, never the plugin's text. |

## Manifest additions

| Field | Rule |
| --- | --- |
| `name` | Optional; 1 to 60 characters; no control characters. |
| `icon` | Optional; base64 PNG, at most 8 KiB decoded, square, 16 to 64 px, not animated. |
| `actions` | Optional; up to 16 `{name}`, unique, `^[a-z][a-z0-9-]{0,31}$`; required when the package exports `actor`, refused when it does not. |

## Frame

| Field | Rule |
| --- | --- |
| `handle` | `w0`, `w1`…, assigned by the host for each widget it shows. |
| `name` | Drawn name: controls and format characters removed, at most 40 characters, or the id's last part. |
| `icon` | RGBA pixels decoded by the host, or none (the host's monogram). |
| `state` | `loading`, `content`, `error`, `busy` (an action running). |

## Action run

| Field | Rule |
| --- | --- |
| `action` | A name the manifest declares; checked before the plugin runs. |
| `values` | The form's values by field id, each at most 500 characters, none holding a secret the host holds. |
| `input` (to the plugin) | `{"config", "data", "values"}` |
| `output` | The widget's data, at most 256 KiB, kept by the host and given to the next render. |

## Bundled widget

| Field | Rule |
| --- | --- |
| `file` | A package built by `cargo xtask plugins`, embedded at build. |
| `digest` | SHA-256 pinned at build; checked before load. |
| `permissions` | None. A bundled package that declares any is refused. |
| `origin` | `bundled:<file>`. |
