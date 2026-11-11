# Contract: The Widget Content Format

A widget's `render` returns a JSON document of at most 64 KiB (contract v1). This is its format,
published for authors as `wit/content.schema.json` (JSON Schema draft 2020-12) and described in
`docs/authors/reference/content.md`. The host reads it with the `widget-content` crate, tested
to agree with the schema.

## Shape

```json
{
  "format": 1,
  "body": [
    { "type": "text", "text": "Hoje", "variant": "heading" },
    { "type": "text", "text": "3 tarefas em atraso", "tone": "warning" },
    { "type": "list", "items": ["Revisar PR", { "text": "Deploy", "tone": "ok" }] },
    { "type": "table", "columns": ["Repo", "Estrelas"], "rows": [["dashboard", "42"]] },
    { "type": "chart", "kind": "bar", "labels": ["seg", "ter"],
      "series": [{ "name": "Commits", "values": [3, 5] }] },
    { "type": "button", "label": "Contar", "action": "increment", "variant": "primary" },
    { "type": "form",
      "fields": [
        { "id": "title", "label": "Título", "kind": "text" },
        { "id": "priority", "label": "Prioridade", "kind": "select", "options": ["baixa", "alta"] },
        { "id": "done", "label": "Feita", "kind": "checkbox" }
      ],
      "submit": { "label": "Adicionar", "action": "add" } },
    { "type": "stack", "children": [ { "type": "text", "text": "Rodapé", "variant": "muted" } ] }
  ]
}
```

## Components

| `type` | Required | Optional | Drawn as |
| --- | --- | --- | --- |
| `text` | `text` | `variant` (`default`, `muted`, `heading`), `tone` | text; a status tone adds its icon |
| `list` | `items` | | a list; each item a string or `{text, tone}` |
| `table` | `columns`, `rows` | | a table; every row has as many cells as columns |
| `chart` | `labels`, `series` | `kind` (`line`, `bar`; default `bar`) | host-drawn SVG, series in `chart-1`…`chart-5`, with a text table of the values |
| `button` | `label`, `action` | `variant` (`primary`, `secondary`, `outline`, `ghost`, `destructive`) | a button that asks the host to run `action` |
| `form` | `fields`, `submit` | | inputs and one submit button that run `submit.action` with the values |
| `stack` | `children` | | a vertical group |

Form field kinds: `text`, `number` (a decimal, sent as a string), `checkbox` (sent as `"true"`
or `"false"`), `select` (`options`, sent as the chosen option). There is no password, masked,
date, file, link or free-HTML kind.

## Tones and variants

`tone`: `default`, `muted`, `accent`, `ok`, `warning`, `error` (the widget map). `ok`, `warning`
and `error` always draw their icon. A variant names the look of its component, from the widget
map; there is no field for a color, font, size, class or style.

## Bounds

| Bound | Value |
| --- | --- |
| Nesting depth | 8 |
| Components in all | 500 |
| Any string | 2,000 characters |
| List items | 200 |
| Table | 20 columns, 200 rows |
| Chart | 5 series, 200 values each, as many labels as values |
| Form | 20 fields; 50 options per select |

## What the host does with content

| Content | Outcome | Plugin's log |
| --- | --- | --- |
| Valid | drawn | nothing |
| An unknown field (e.g. `color`, `style`, `class`) | the field is ignored; the rest draws | one entry per document naming the fields |
| An unknown tone | drawn in `default` | one entry naming the tone |
| An unknown `type` | the host's placeholder in its place; the rest draws | one entry naming the type |
| A wrong type, a missing field, a bound passed, `format` not 1 | the frame's error state: `bad-shape` or `over-bound` with the path | one entry |
| Render failed (error, trap, time, memory, output too large, output not JSON) | the frame's error state: `render-failed` with the runtime's error kind | the runtime's entry |

The schema refuses unknown fields and tones, so an author's tools catch them; the host is lenient
there only, as criterion HS-09 requires.

## Agreement test

`widget-content/tests/schema.rs` holds a corpus: every schema-valid document is accepted with no
log entry; every document the host refuses is schema-invalid, except the host-only rules a schema
cannot express, listed apart (the 500-component total, the 64 KiB size, rows as long as the
columns, as many chart labels as values); the lenient cases (unknown fields, tones and types) are
listed apart too, schema-invalid and drawn with log entries.
