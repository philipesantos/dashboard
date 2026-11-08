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
  solid-text:                 # text on steps 9-10, declared per scale (FR-009)
    blue: dark
    red: none                 # neither white nor dark reaches 4.5:1: large text only
    # ... one entry per chromatic scale
picks:
  gray: [gray, mauve, slate, sage, olive, sand]
  hue-distance: 30            # degrees, OKLCH, light step 9
  neutral-below-chroma: 0.08
  default: { gray: gray, host: monochrome, widget: blue, radius: medium, theme: system }
roles:
  text:
    source: gray
    light: 12
    dark: 12
    why: "Body text; step 12 is Radix's high-contrast text."
  border-control:
    source: gray
    light: 9
    dark: 9
    off-role: wcag-1.4.11     # pending the maintainer's choice (research R5)
    why: "A control's boundary must reach 3:1 (WCAG 1.4.11); step 8 does not on step 1."
  focus:
    source: host              # monochrome host: gray
    light: 12
    dark: 12
    off-role: wcag-1.4.11     # steps 7-8, Radix's focus-ring steps, fail 3:1 (R5)
    host-surface: true
    why: "The host's own focus style; widget colors such as lime fail 3:1 as a ring."
pairings:
  - { fg: text, bg: surface, kind: text }
  - { fg: border-control, bg: surface, kind: non-text }
  - { fg: divider, bg: surface, kind: decorative, why: "Spacing separates rows; the line only helps." }
tones:
  default: { text: text, surface: surface, border: border-control }
  accent:  { text: accent-text, surface: accent-surface, border: accent-border }
  ok:      { text: text, icon: ok-icon, surface: ok-surface, status-icon: check }
```

```design-roles
# in ## Elevation & Depth
elevation:
  flat:   { shadow: none, why: "..." }
  frame:  { shadow: "0 1px 2px blackA-3", why: "..." }      # shadow colors from blackA (R2)
  panel:  { shadow: "...", host-surface: true, why: "..." }
  dialog: { shadow: "...", scrim: { light: blackA-7, dark: blackA-9 }, host-surface: true, why: "..." }
```

```design-roles
# in ## Shapes
radius-picks:
  none: 0px
  small: 3px
  medium: 6px
  large: 10px
  full: 9999px
```

```design-roles
# in ## Components
icons:
  check: "M…"                 # SVG path data on a 16×16 grid, drawn by the project (R9)
  alert: "M…"
  cross: "M…"
host-surfaces: [chrome, dialog, scrim, panel-attachment]   # component names from the front matter
rationale:
  typography.body-md: "..."   # every format-layer token without a `why` of its own
  spacing.sm: "..."
```

The values above are illustrations of the shape; the real ones are written in `DESIGN.md`
during implementation (R13).

## Checker rules

The checker fails, naming the line or token, when:

1. the front matter is missing, is not first, or a format key has the wrong type;
2. a `{reference}` does not resolve to a primitive token; only inside `components` may it resolve
   to a typography level;
3. a section is out of order or appears twice;
4. a `design-roles` key appears twice, is unknown, or a block's YAML does not parse;
5. a role names a step outside 1–12, a `source` that does not exist, or has no `why`; or uses a
   step outside its Radix role without an `off-role` mark naming the WCAG criterion that needs it,
   or with one the lint shows is not needed;
6. a format-layer token has no rationale, in `rationale` or its own `why`;
7. a pairing names a role that does not exist, or a `decorative` pairing has no `why`;
8. a tone lacks `text`, or `ok`, `warning` or `error` lacks a `status-icon` (FR-025);
9. the front matter's `colors` differ from the roles resolved at the default appearance;
10. any allowed appearance, in either theme, fails a pairing (the contrast lint, FR-008), or the
    default appearance fails the hue rule;
11. a role or tone part used as a foreground has no pairing against every role it can sit on, or
    `focus` is not paired against every background a control can have, accent step 9 included;
12. a token that is not a host surface references one that is;
13. `solid-text` is missing for a chromatic scale, declares a color that does not reach 4.5:1 on
    steps 9 and 10, or declares `none` where one does;
14. a component has a property that takes an image, an icon source or a URL; icons come only
    from `icons` (FR-023).
