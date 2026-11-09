# Contract: `DESIGN.md`

How `DESIGN.md` is written, so the checker in `design-gen` can read it (R1). The format layer is
Google's DESIGN.md format at tag `0.4.0`; the project layer is this project's.

## Format layer (DESIGN.md 0.4.0)

- YAML front matter between two `---` lines, first thing in the file, with `version: alpha`,
  `name`, `description`, `colors`, `typography`, `rounded`, `spacing` and `components`, typed as
  the format says (colors as `#RRGGBB`, dimensions in `px`, references as `{group.token}`).
- `##` sections in the format's order: Overview, Colors, Typography, Layout, Elevation & Depth,
  Shapes, Components, Do's and Don'ts. No section appears twice. Other `##` sections may follow
  the ones they belong to (the format keeps unknown sections).
- `colors` holds every color role resolved at the **default appearance**: `<role>` for light and
  `<role>-dark` for dark. The checker fails when one differs from what the project layer
  resolves to.

## Project layer

Fenced blocks with the info string `design-roles`, holding YAML. Neither the format's 0.4.0 nor
its later interleaved ` ```yaml ` blocks read them. Each top-level key appears in exactly one
block, in the section named below.

```design-roles
# in ## Colors
scales:
  radix: "3.0.0"              # must match design/radix/<version>/
  status: { ok: green, warning: amber, error: red }
  solid-text:                 # text on steps 9 and 10, declared for every non-gray scale (FR-009)
    cyan: dark
    red: none                 # neither white nor dark reaches 4.5:1 on steps 9 and 10
    # ... one entry per scale
picks:
  gray: [gray, mauve, slate, sage, olive, sand]
  hue-distance: 30            # degrees, OKLCH, light step 9
  neutral-below-chroma: 0.08
  default: { gray: gray, host: monochrome, widget: blue, radius: medium, theme: system }
roles:
  # A background: source and steps only
  surface: { source: gray, light: 1, dark: 2, why: "A widget's frame." }
  # A foreground: what it is checked as, and every background it is drawn on
  text: { source: gray, light: 12, dark: 12, kind: text, on: [canvas, surface], why: "Body text." }
  border-control: { source: gray, light: 9, dark: 9, kind: non-text, on: [surface], why: "3:1 (WCAG 1.4.11); step 8 does not reach it." }
  # Exempt kinds name the WCAG exemption they rest on
  divider: { source: gray, light: 6, dark: 6, kind: decorative, on: [surface], exempt: "WCAG 1.4.11 exempts decoration.", why: "Lines between rows." }
  # A host surface goes to the host-surfaces crate
  focus: { source: host, light: 11, dark: 11, kind: non-text, on: [surface, accent-soft], host-surface: true, why: "The host's focus ring." }
  # The overlay source reads Radix's blackA
  scrim: { source: overlay, light: 7, dark: 9, host-surface: true, why: "Dims the window behind a host dialog." }
tones:
  default: { text: text, surface: surface, border: border-control }
  accent: { text: accent-strong, surface: accent-soft }
  ok: { text: text, surface: ok-soft, icon: ok-icon, status-icon: check }
```

A role's `source` is `gray`, `widget`, `host` (the gray family while the host is monochrome), `ok`,
`warning`, `error` or `overlay`; its steps are 1–12, `white`, `black` or `solid-text` (the scale's
declared text color). `kind` is `text`, `large-text`, `non-text`, `decorative` or `inactive`.

```design-roles
# in ## Elevation & Depth
elevation:
  flat: { shadow: none, why: "..." }
  frame: { shadow: "0px 1px 2px blackA-2", why: "..." }      # `<x>px <y>px <blur>px blackA-<1-12>`
  panel: { shadow: "0px 4px 12px blackA-4", host-surface: true, why: "..." }
  dialog: { shadow: "0px 12px 32px blackA-6", host-surface: true, why: "..." }
```

```design-roles
# in ## Shapes
radius-picks: [none, small, medium, large, full]           # names of the front matter's `rounded`
```

```design-roles
# in ## Components
host-surfaces: [chrome, panel, dialog]                      # components of the front matter
icons:
  check: "M…"                 # SVG path data only, on a 16-unit grid, drawn by the project (R9)
  alert: "M…"
  cross: "M…"
rationale:
  typography.body-md: "..."   # every format-layer token and component
  components.frame: "..."
```

A component's `textColor` and `backgroundColor` name roles (`{colors.<role>}`); the front matter's
`colors` are those roles resolved at the default appearance.

The values above show the shape; the real ones are in `DESIGN.md`.

## Checker rules

The checker fails, naming the line or token, when:

1. the front matter is missing, is not first, or a format key has the wrong type;
2. a `{reference}` does not resolve to a primitive token; only inside `components` may it resolve
   to a typography level;
3. a section is out of order or appears twice;
4. a `design-roles` key appears twice, is unknown, or a block's YAML does not parse;
5. a role names a step outside 1–12 (or `white`, `black`), a `source` that does not exist, or has
   no `why`;
6. a format-layer token has no rationale, in `rationale` or its own `why`;
7. a role's `on` names a role that does not exist, a role with `on` has no `kind` (or the
   reverse), or a `decorative` or `inactive` role has no `exempt`;
8. a tone lacks `text`, or `ok`, `warning` or `error` lacks a `status-icon` (FR-025);
9. the front matter's `colors` differ from the roles resolved at the default appearance;
10. any allowed appearance, in either theme, fails a pairing (the contrast lint, FR-008), or the
    default appearance fails the hue rule;
11. a foreground role, or a tone's text, border or icon, has no pairing against every background
    it can be drawn on (constitution v5.0.0), or `focus` is not paired against every background a
    control can have, accent step 9 included; translucent roles are measured after compositing
    over that background;
12. a token that is not a host surface references one that is;
13. `solid-text` is missing for a chromatic scale, declares a color that does not reach 4.5:1 on
    steps 9 and 10, or declares `none` where one does;
14. a component has a property that takes an image, an icon source or a URL; icons come only
    from `icons` (FR-023).
