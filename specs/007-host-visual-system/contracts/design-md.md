# Contract: `DESIGN.md`

`DESIGN.md` sits at the repository root. It follows `google-labs-code/design.md` format version
0.4.0 (research R1) and adds the project layer below. The format checker at 0.4.0 MUST report no
error on it.

## Front matter (format layer)

```yaml
---
version: alpha
name: Dashboard
description: <one line>
colors:
  background: "oklch(0.99 0.003 250)"
  background-dark: "oklch(0.17 0.012 255)"
  primary: "oklch(0.5 0.13 245)"
  primary-dark: "oklch(0.74 0.12 235)"
  # … every role below, light under its name and dark under <name>-dark
typography:
  heading:   { fontFamily: Bricolage Grotesque Variable, fontWeight: 600, … }
  body-md:   { fontFamily: Figtree Variable, fontSize: 0.875rem, lineHeight: 1.45 }
  mono:      { fontFamily: IBM Plex Mono, fontSize: 0.8125rem }
  # … the type scale
rounded:
  base: 0.625rem        # the shadcn --radius; sm, md, lg, xl… derive from it
spacing:
  # … the spacing scale
components:
  # The declarative UI's components, one entry per variant, in the format's own properties
  # (backgroundColor, textColor, typography, rounded, padding). They reference tokens:
  button-primary: { backgroundColor: "{colors.primary}", textColor: "{colors.primary-foreground}", rounded: "{rounded.base}" }
---
```

**Color roles.** Every role is listed with a `-dark` twin:

- **shadcn's roles:** `background`, `foreground`, `card`, `card-foreground`, `popover`,
  `popover-foreground`, `primary`, `primary-foreground`, `secondary`, `secondary-foreground`,
  `muted`, `muted-foreground`, `accent`, `accent-foreground`, `destructive`, `border`, `input`,
  `ring`, `chart-1` to `chart-5`, and the eight `sidebar*` roles.
- **Added by this feature** (research R5): `ok`, `warning`, `frame`, `frame-foreground`,
  `frame-border`, `frame-header`, `frame-header-foreground`, `dialog`, `dialog-foreground` and
  `scrim`.

## Body (format layer)

The `##` sections appear in the format's order: Overview, Colors, Typography, Layout, Elevation &
Depth, Shapes, Components, and Do's and Don'ts.

Each token is named in bold in its section's prose, followed by its rationale, for example:

```md
- **input**: the edge of text fields and selects. Darker than `border` because it is the only
  thing that marks the control (WCAG 1.4.11): 3.94:1 on `card` in light.
```

The checker fails when a token in the front matter is not named this way in its section.
Light and dark twins share one entry, under the light name.

## `design-rules` blocks (project layer)

Fenced blocks with the info string `design-rules`, in YAML. Each key below may appear in one
block only, and the block sits in the section given.

### In Colors

````md
```design-rules
hostOnly:
  - sidebar          # every sidebar-* role is covered by its prefix
  - frame            # frame, frame-foreground, frame-border, frame-header…
  - dialog
  - scrim
  - ring
decorative:
  - role: border
    reason: Card edges and dividers. Controls carry their own `input` edge.
pairings:
  - { fg: foreground, on: [background], kind: text }
  - { fg: destructive, on: [card, destructive/10], kind: text }
  - { fg: input, on: [card], kind: boundary }
  - { fg: ring, on: [background], kind: focus }
  - { fg: chart-2, on: [card], kind: chart }
  # …
```
````

- **`hostOnly`** lists role names. An entry also covers every role that starts with it followed
  by `-`.
- **`on`** is a layer stack. The first entry must be an opaque role. The others are `role` or
  `role/opacity`, with opacity in percent.
- **`kind`** is one of `text`, `large-text`, `boundary`, `focus` and `chart`.

### In Elevation & Depth

````md
```design-rules
elevation:
  flat:   { shadow: none, dark: none }
  frame:  { shadow: "0 1px 2px oklch(0 0 0 / 0.05)", dark: "…", hostOnly: true }
  raised: { shadow: "…", dark: "…", hostOnly: true }
  dialog: { shadow: "…", dark: "…", hostOnly: true }
```
````

### In Components

````md
```design-rules
tones:
  default: { text: foreground }
  muted:   { text: muted-foreground }
  accent:  { text: primary }
  ok:      { text: ok, icon: circle-check }
  warning: { text: warning, icon: triangle-alert }
  error:   { text: destructive, icon: circle-x }
unknownTone: default
variants:
  button.primary:   { text: primary-foreground, fill: primary }
  button.secondary: { text: secondary-foreground, fill: secondary }
  button.outline:   { text: foreground, fill: background, border: input }
  table.header:     { text: muted-foreground }
  # … text, list, table, chart, button and form
```
````

- The `icon` names come from the `lucide` set the app already uses. The host draws the icon,
  never the widget.
- A variant's key is `component.variant`, and the component is one of `text`, `list`, `table`,
  `chart`, `button` and `form`.

## Errors the project checker reports

Each error is one line, `DESIGN.md: <what>`, and the check exits non-zero. Examples:

- `DESIGN.md: color "ok" has no "ok-dark"`
- `DESIGN.md: token "chart-5" has no bold rationale in ## Colors`
- `DESIGN.md: pairing chart-2 on card (chart, light): 2.76 < 3.00`
- `DESIGN.md: role "sidebar-border" is in no pairing and not decorative`
- `DESIGN.md: variant button.ghost maps fill to host-only role "frame-header"`
- `DESIGN.md: design-rules key "pairings" appears in two blocks`
