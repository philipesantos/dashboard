# Widget content

What `render` returns is the widget's content: a JSON document of at most 64 KiB, in the format
below. The host reads it, checks it, and draws it with its own components inside the widget's
frame. A widget never draws anything itself: it names components, and picks a tone or a variant
by name; the host chooses every color, font and size. Every text is drawn as text: markup shows
as typed and never runs.

The format's JSON Schema is `wit/content.schema.json` in the repository: check your content
against it in your tests or your editor. Do not put a `$schema` field in content: the format has
no such field, and the host logs it as unknown. `runtime run --check-content` reads content as
the app does and says whether it would be drawn ([the CLI](cli.md#run)).

```json
{
  "format": 1,
  "body": [
    { "type": "text", "text": "Today", "variant": "heading" },
    { "type": "text", "text": "3 tasks overdue", "tone": "warning" },
    { "type": "list", "items": ["Review the PR", { "text": "Deploy", "tone": "ok" }] },
    { "type": "table", "columns": ["Repository", "Stars"], "rows": [["dashboard", "42"]] },
    { "type": "button", "label": "Refresh", "action": "refresh" }
  ]
}
```

## The document

| Field | Accepts | Required |
| --- | --- | --- |
| `format` | `1`, the version of this format | Yes <!-- ref: content format --> |
| `body` | 1 to 500 components, drawn top to bottom | Yes <!-- ref: content body --> |
| `body[]/type` | The component: `text`, `list`, `table`, `chart`, `button`, `form` or `stack`. A type the host does not know is drawn as the host's placeholder, and the rest still draws | Yes <!-- ref: content body[]/type --> |

Each component's other fields follow. A field that is not listed for a component is ignored
([what the host does](#what-the-host-does-with-content)).

## Components

The examples below are single components: each goes in `body`, or in a stack's `children`.

### `text`

A line or paragraph of text.

```json component
{ "type": "text", "text": "Synced 2 minutes ago", "variant": "muted" }
```

| Field | Accepts | Required |
| --- | --- | --- |
| `text` | Any text up to 2,000 characters | Yes <!-- ref: content body[]/text --> |
| `variant` | For a text: `default`, the default; `muted`, quieter; `heading`, a title. For a button, see [`button`](#button) | No <!-- ref: content body[]/variant --> |
| `tone` | A [tone](#tones); `default` when absent | No <!-- ref: content body[]/tone --> |

### `list`

A list of short texts, each with its own tone.

```json component
{ "type": "list", "items": ["Plain", { "text": "Done", "tone": "ok" }, { "text": "Late", "tone": "error" }] }
```

| Field | Accepts | Required |
| --- | --- | --- |
| `items` | Up to 200 items, maybe none. Each is a text, or an object with `text` and `tone` | Yes <!-- ref: content body[]/items --> |
| `items[]/text` | The item's text, up to 2,000 characters | Yes, in an object <!-- ref: content body[]/items[]/text --> |
| `items[]/tone` | A [tone](#tones); `default` when absent | No <!-- ref: content body[]/items[]/tone --> |

### `table`

Rows and columns of text. A table wider than the frame scrolls inside it.

```json component
{
  "type": "table",
  "columns": ["Release", "Published"],
  "rows": [["1.99.1", "2026-10-16"], ["1.99.0", "2026-10-02"]]
}
```

| Field | Accepts | Required |
| --- | --- | --- |
| `columns` | 1 to 20 texts: the header | Yes <!-- ref: content body[]/columns --> |
| `rows` | Up to 200 rows, maybe none. Each row is a list of texts with exactly as many cells as there are columns | Yes <!-- ref: content body[]/rows --> |

### `chart`

A bar or line chart the host draws, with a table of its values that screen readers read. The
series take the host's chart colors in order, so a widget never picks one.

```json component
{
  "type": "chart",
  "kind": "line",
  "labels": ["Mon", "Tue", "Wed"],
  "series": [
    { "name": "Opened", "values": [4, 2, 5] },
    { "name": "Closed", "values": [1, 3, 6.5] }
  ]
}
```

| Field | Accepts | Required |
| --- | --- | --- |
| `kind` | `bar`, the default, or `line`. Any other kind is refused (`bad-shape`) | No <!-- ref: content body[]/kind --> |
| `labels` | Up to 200 texts, one per point | Yes <!-- ref: content body[]/labels --> |
| `series` | 1 to 5 series | Yes <!-- ref: content body[]/series --> |
| `series[]/name` | The series' name, shown in the legend and the table | Yes <!-- ref: content body[]/series[]/name --> |
| `series[]/values` | Numbers, exactly as many as there are labels | Yes <!-- ref: content body[]/series[]/values --> |

### `button`

A button that asks the host to run one of the widget's [actions](#actions).

```json component
{ "type": "button", "label": "Count", "action": "increment", "variant": "secondary" }
```

| Field | Accepts | Required |
| --- | --- | --- |
| `label` | The button's text | Yes <!-- ref: content body[]/label --> |
| `action` | An action the manifest declares. Any other name draws, but the host refuses it when the button is pressed, as `undeclared-action` | Yes <!-- ref: content body[]/action --> |
| `variant` | `primary`, the default; `secondary`; `outline`; `ghost`; `destructive`, for an action that deletes or undoes | No |

### `form`

Fields and one submit button that runs an [action](#actions) with the fields' values.

```json component
{
  "type": "form",
  "fields": [
    { "id": "title", "label": "Title", "kind": "text" },
    { "id": "hours", "label": "Estimate (hours)", "kind": "number" },
    { "id": "priority", "label": "Priority", "kind": "select", "options": ["low", "high"] },
    { "id": "done", "label": "Done", "kind": "checkbox" }
  ],
  "submit": { "label": "Add", "action": "add" }
}
```

| Field | Accepts | Required |
| --- | --- | --- |
| `fields` | 1 to 20 fields | Yes <!-- ref: content body[]/fields --> |
| `fields[]/id` | A lowercase letter, then up to 31 lowercase letters, digits, `-` or `_`; unique in the form. The key of the field's value | Yes <!-- ref: content body[]/fields[]/id --> |
| `fields[]/label` | The field's label | Yes <!-- ref: content body[]/fields[]/label --> |
| `fields[]/kind` | `text`, `number`, `checkbox` or `select` | Yes <!-- ref: content body[]/fields[]/kind --> |
| `fields[]/options` | 1 to 50 texts, the choices of a `select`; required there, and refused on any other kind | For `select` <!-- ref: content body[]/fields[]/options --> |
| `submit` | The submit button | Yes <!-- ref: content body[]/submit --> |
| `submit/label` | The button's text | Yes <!-- ref: content body[]/submit/label --> |
| `submit/action` | An action the manifest declares, as for a button | Yes <!-- ref: content body[]/submit/action --> |

Every value reaches the action as a string:

| Kind | Drawn as | Its value |
| --- | --- | --- |
| `text` | A one-line input | What the person typed; `""` when empty |
| `number` | An input for a decimal number, with a point or a comma | What the person typed, or `""`; the host refuses anything else before the action runs |
| `checkbox` | A checkbox | `"true"` or `"false"` |
| `select` | The host's list of the options, drawn inside the frame | The chosen option, or `""` when none was chosen |

There is no password, masked, date, file or link kind, and no field can ask the browser to fill
it in. Before the values reach the widget, the host refuses them, and runs nothing, if any of them
holds a secret of an account the host keeps. Check every value in the action as you would any
input.

### `stack`

A vertical group of components, nested at most 8 deep.

```json component
{
  "type": "stack",
  "children": [
    { "type": "text", "text": "Build passing", "tone": "ok" },
    { "type": "text", "text": "2 warnings", "tone": "warning" }
  ]
}
```

| Field | Accepts | Required |
| --- | --- | --- |
| `children` | Up to 500 components, maybe none | Yes <!-- ref: content body[]/children --> |

## Tones

A tone says what a text means; the host decides how it looks. `ok`, `warning` and `error` always
draw their icon, so a status never depends on color alone.

| Tone | Drawn as |
| --- | --- |
| `default` | The frame's text color |
| `muted` | A quieter color, for secondary text |
| `accent` | The accent color, for what stands out |
| `ok` | The success color, with a check icon |
| `warning` | The warning color, with a warning icon |
| `error` | The error color, with an error icon |

A tone the host does not know draws as `default`; so does a variant it does not know for its
component (a button's draws as `primary`). Each adds one entry to the plugin's log.

## Actions

A button or a form asks the host to run an action: a name the manifest declares in `actions`
([the manifest](manifest.md)), run by the widget's `act`, which a widget built for contract 1.4.0
exports in one of the `interactive-` worlds ([the contract](contract.md#actor)). The host runs
nothing else: a name the manifest does not declare is refused as `undeclared-action` before the
widget runs.

`act` gets the action's name and a JSON document with the widget's configuration, its current
data and the form's values, `{}` for a button:

```text
{"config": …, "data": …, "values": {"title": "Buy milk", "done": "false"}}
```

It returns the widget's new data, which the next `render` gets in `data`; the host then draws what
`render` returns. That data stays in memory while the app runs: what must outlive a restart goes in
the widget's store. `runtime act` runs an action in the development CLI ([the CLI](cli.md#act)).

## Bounds

The host checks these as it reads content; past one, the content is not drawn (`over-bound`,
except an id that is too long, `bad-shape`). The whole document is at most 64 KiB
(`max_output_bytes`, [limits](limits.md)), checked before it is read.

| Bound | Value | What it bounds |
| --- | --- | --- |
| `MAX_DEPTH` | 8 | Nesting: the body is level 1, and each stack's children one level more |
| `MAX_COMPONENTS` | 500 | Components in all, at every level, placeholders included |
| `MAX_STRING_CHARS` | 2,000 | Characters in any text: a text, an item, a cell, a label, a name, an option, an action |
| `MAX_LIST_ITEMS` | 200 | Items of a list |
| `MAX_TABLE_COLUMNS` | 20 | Columns of a table, and cells of a row |
| `MAX_TABLE_ROWS` | 200 | Rows of a table |
| `MAX_SERIES` | 5 | Series of a chart |
| `MAX_POINTS` | 200 | Labels of a chart, and values of each series |
| `MAX_FIELDS` | 20 | Fields of a form |
| `MAX_OPTIONS` | 50 | Options of a select |
| `MAX_FIELD_ID_CHARS` | 32 | Characters of a field's id |

## What the host does with content

| Content | The frame shows | The plugin's log |
| --- | --- | --- |
| Valid | The content | Nothing |
| A field the host does not know, such as `color`, `style` or `class` | The content, without it | One entry for the document, naming every such field |
| A tone or variant the host does not know | The content, in the default tone or variant | One entry for each |
| A component `type` the host does not know | The host's placeholder in its place, and the rest of the content | One entry for each |
| A wrong type, a missing field, a value outside its set, `format` other than 1, or a bound passed | An error, with its kind (`bad-shape` or `over-bound`) and where | One entry |
| No content: `render` failed or was stopped, or returned something that is not JSON or is too large | An error, with the [error](errors.md) | The host's entry |

The schema refuses unknown fields, tones, variants and types, so your tools catch them; only the
host forgives them, so that one mistake does not blank a widget. A place in content is written
from `$`, the document: `$.body[2].rows[0]` is the first row of the third component.

The development CLI prints the same outcome:

```json logs=1
{
  "format": 1,
  "body": [{ "type": "text", "text": "Hello", "color": "red" }]
}
```

```text
[content] widget content: ignored fields this host does not know: $.body[0]."color"
content: drawn, 1 log entries
```

```json refused=bad-shape
{
  "format": 1,
  "body": [{ "type": "table", "columns": ["Name", "Value"], "rows": [["only one cell"]] }]
}
```

```text
content: not drawn: bad-shape at $.body[0].rows[0]: has 1 cells for 2 columns
```
