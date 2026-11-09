---
version: alpha
name: Dashboard host
description: The host's visual system. Colors come from Radix scales the person picks; widgets ask only for tones, and the host's own surfaces stay out of their reach.
colors:
  text: "#202020"
  text-dark: "#eeeeee"
  text-secondary: "#646464"
  text-secondary-dark: "#b4b4b4"
  accent-text: "#0d74ce"
  accent-text-dark: "#70b8ff"
  accent-strong: "#113264"
  accent-strong-dark: "#c2e6ff"
  border-control: "#8d8d8d"
  border-control-dark: "#6e6e6e"
  divider: "#d9d9d9"
  divider-dark: "#3a3a3a"
  ok-icon: "#218358"
  ok-icon-dark: "#3dd68c"
  warning-icon: "#ab6400"
  warning-icon-dark: "#ffca16"
  error-icon: "#ce2c31"
  error-icon-dark: "#ff9592"
  focus: "#646464"
  focus-dark: "#b4b4b4"
  canvas: "#f9f9f9"
  canvas-dark: "#111111"
  surface: "#fcfcfc"
  surface-dark: "#191919"
  neutral-soft: "#f0f0f0"
  neutral-soft-dark: "#222222"
  accent-soft: "#e6f4fe"
  accent-soft-dark: "#0d2847"
  ok-soft: "#e6f6eb"
  ok-soft-dark: "#132d21"
  warning-soft: "#fff7c2"
  warning-soft-dark: "#302008"
  error-soft: "#feebec"
  error-soft-dark: "#3b1219"
  chrome-surface: "#f0f0f0"
  chrome-surface-dark: "#222222"
  dialog-surface: "#fcfcfc"
  dialog-surface-dark: "#191919"
  panel-surface: "#fcfcfc"
  panel-surface-dark: "#191919"
  scrim: "#00000080"
  scrim-dark: "#000000b3"
typography:
  caption:
    fontFamily: Inter
    fontSize: 12px
    fontWeight: 400
    lineHeight: 16px
  label-sm:
    fontFamily: Inter
    fontSize: 12px
    fontWeight: 500
    lineHeight: 16px
  body-sm:
    fontFamily: Inter
    fontSize: 13px
    fontWeight: 400
    lineHeight: 18px
  body-md:
    fontFamily: Inter
    fontSize: 14px
    fontWeight: 400
    lineHeight: 20px
  label-md:
    fontFamily: Inter
    fontSize: 14px
    fontWeight: 500
    lineHeight: 20px
  title-sm:
    fontFamily: Inter
    fontSize: 16px
    fontWeight: 600
    lineHeight: 24px
  title-md:
    fontFamily: Inter
    fontSize: 20px
    fontWeight: 600
    lineHeight: 28px
    letterSpacing: -0.01em
  title-lg:
    fontFamily: Inter
    fontSize: 24px
    fontWeight: 600
    lineHeight: 32px
    letterSpacing: -0.015em
  display:
    fontFamily: Inter
    fontSize: 32px
    fontWeight: 600
    lineHeight: 40px
    letterSpacing: -0.02em
rounded:
  none: 0px
  small: 3px
  medium: 6px
  large: 10px
  full: 9999px
spacing:
  none: 0px
  xxs: 2px
  xs: 4px
  sm: 8px
  md: 12px
  lg: 16px
  xl: 24px
  2xl: 32px
  3xl: 48px
components:
  chrome:
    backgroundColor: "{colors.chrome-surface}"
    textColor: "{colors.text}"
    typography: "{typography.label-md}"
    height: 40px
    padding: 8px
  frame:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.medium}"
    padding: 12px
  frame-label:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text-secondary}"
    typography: "{typography.label-sm}"
  monogram:
    backgroundColor: "{colors.neutral-soft}"
    textColor: "{colors.text}"
    typography: "{typography.label-sm}"
    rounded: "{rounded.small}"
    size: 20px
  panel:
    backgroundColor: "{colors.panel-surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.medium}"
    padding: 16px
  dialog:
    backgroundColor: "{colors.dialog-surface}"
    textColor: "{colors.text}"
    rounded: "{rounded.large}"
    padding: 24px
    width: 480px
  button:
    backgroundColor: "{colors.neutral-soft}"
    textColor: "{colors.text}"
    typography: "{typography.label-md}"
    rounded: "{rounded.medium}"
    height: 28px
    padding: 12px
  button-accent:
    backgroundColor: "{colors.accent-soft}"
    textColor: "{colors.accent-strong}"
    typography: "{typography.label-md}"
    rounded: "{rounded.medium}"
    height: 28px
    padding: 12px
  field:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text}"
    typography: "{typography.body-md}"
    rounded: "{rounded.medium}"
    height: 28px
    padding: 8px
  field-note:
    backgroundColor: "{colors.surface}"
    textColor: "{colors.text-secondary}"
    typography: "{typography.caption}"
  badge:
    backgroundColor: "{colors.neutral-soft}"
    textColor: "{colors.text}"
    typography: "{typography.label-sm}"
    rounded: "{rounded.full}"
    height: 20px
    padding: 8px
---

# Dashboard host

The visual system of the dashboard's host: the window's chrome, the frame around every widget,
the panels a frame opens, the host's dialogs, and the kit of components widget content is drawn
with. It is the only file edited by hand; `cargo xtask design` generates the Rust tokens
(`design/tokens/`, `design/host-surfaces/`) and a W3C Design Tokens export
(`design/tokens.dtcg.json`) from it, and `cargo xtask design --check` checks it. Decisions and
their measurements are in `specs/007-host-visual-design/research.md`.

The file has two layers. The YAML front matter is Google's DESIGN.md format at version 0.4.0
(`design/format/0.4.0/spec.md`). The `design-roles` blocks are this project's, for what the
format has no place for: themes, the Radix step each role uses, elevation, the person's picks and
a rationale for every token.

## Overview

Calm, dense and legible. The dashboard shows many small widgets at once, so the host stays quiet:
a neutral gray frame, one accent color for widgets, and the status colors only where something
is ok, needs attention or failed. Nothing about the host's look is a secret; a host dialog is told
apart by its form (it covers the chrome and blocks every host window), which nothing a widget
supplies can produce (constitution, Principles V and VII).

The person picks the appearance: a gray family, the host color (monochrome by default), the
widget color, the corner radius and the theme. Widgets never name a color; they ask for a tone.

## Colors

Every color is a step of a Radix Colors 3.0.0 scale (`design/radix/3.0.0/`), white or black. Each
role below says which source it reads and which step it takes in each theme: `gray` is the
person's gray family, `widget` the widget color, `host` the host color (the gray family while the
host is monochrome), `ok`, `warning` and `error` the fixed status scales, and `overlay` Radix's
`blackA`, for the scrim. A role drawn over others lists them in `on` with what it is checked as,
and CI checks the contrast of every such pair in every appearance the host accepts, in both
themes (constitution v5.0.0, Development Workflow).

The front matter's `colors` are these roles resolved at the default appearance (gray, monochrome
host, blue widgets); the dark ones end in `-dark`.

Some roles take a step other than the one Radix names for their role, because Radix's own step
falls short of WCAG's minimums. The worst case of each, over the 1,176 appearances the host
accepts in both themes, is what the lint measured on 2026-11-08:

- **Control boundaries** take step 9: Radix's border steps (6–8) reach 1.86:1 on step 1 in the
  light theme, under the 3:1 a control's boundary needs (WCAG 1.4.11). Step 9: 3.21:1 at worst
  (slate, on a frame, light).
- **The focus ring** takes step 11 of the host color: Radix's focus-ring steps fail 3:1 too, and a
  ring in the widget color would be lost on lime or sky. Step 11: 3.90:1 at worst (an orange host,
  on a pink widget's soft accent).
- **Status text** is gray step 12 beside an icon in the status color's step 11: amber 11 on sage 2
  is 4.36:1, under 4.5:1 for text. The icons reach 4.21:1 at worst (ok, on its soft badge).
- **Text on a soft accent** is the widget color's step 12: step 11 on step 3 of the same scale
  reaches only 4.21:1 for green. Step 12: 9.82:1 at worst (lime).
- **No normal-size text sits on a solid step** (9–10): on 14 of the 25 scales, blue, the purples
  and the reds among them, neither white nor dark text reaches 4.5:1 on both step 9 and its hover
  step 10. `solid-text` declares, per scale, the text color that does, or `none`.

```design-roles
scales:
  radix: "3.0.0"
  status: { ok: green, warning: amber, error: red }
  solid-text:
    tomato: none
    red: none
    ruby: none
    crimson: none
    pink: none
    plum: none
    purple: none
    violet: none
    iris: none
    indigo: none
    blue: none
    cyan: dark
    teal: dark
    jade: dark
    green: dark
    grass: dark
    brown: none
    bronze: none
    gold: none
    sky: dark
    mint: dark
    lime: dark
    yellow: dark
    amber: dark
    orange: dark
picks:
  gray: [gray, mauve, slate, sage, olive, sand]
  hue-distance: 30
  neutral-below-chroma: 0.08
  default: { gray: gray, host: monochrome, widget: blue, radius: medium, theme: system }
roles:
  text: { source: gray, light: 12, dark: 12, kind: text, on: [canvas, surface, neutral-soft, chrome-surface, dialog-surface, panel-surface, ok-soft, warning-soft, error-soft], why: "Body text; step 12 is Radix's high-contrast text." }
  text-secondary: { source: gray, light: 11, dark: 11, kind: text, on: [canvas, surface, neutral-soft, chrome-surface, dialog-surface, panel-surface], why: "Labels, captions and the field note; step 11 is Radix's low-contrast text, still AA on steps 1-3." }
  accent-text: { source: widget, light: 11, dark: 11, kind: text, on: [surface, panel-surface], why: "Accented text on a plain surface, such as a link in widget content." }
  accent-strong: { source: widget, light: 12, dark: 12, kind: text, on: [accent-soft], why: "Text on a soft accent; step 11 on step 3 of the same scale is under 4.5:1 for some scales." }
  border-control: { source: gray, light: 9, dark: 9, kind: non-text, on: [surface, panel-surface, dialog-surface], why: "A control's boundary identifies it and needs 3:1 (WCAG 1.4.11); Radix's border steps 6-8 fall short." }
  divider: { source: gray, light: 6, dark: 6, kind: decorative, on: [canvas, surface, chrome-surface, dialog-surface, panel-surface], exempt: "WCAG 1.4.11 exempts decoration: spacing and alignment separate what the line divides; the line only reinforces it.", why: "Radix's subtle border step, for lines between rows and around the chrome." }
  ok-icon: { source: ok, light: 11, dark: 11, kind: non-text, on: [surface, panel-surface, dialog-surface, ok-soft], why: "The status icon carries the status beside gray text, so it needs 3:1 (WCAG 1.4.11)." }
  warning-icon: { source: warning, light: 11, dark: 11, kind: non-text, on: [surface, panel-surface, dialog-surface, warning-soft], why: "As ok-icon; amber 9 is too light to carry meaning on a light surface." }
  error-icon: { source: error, light: 11, dark: 11, kind: non-text, on: [surface, panel-surface, dialog-surface, error-soft], why: "As ok-icon." }
  focus: { source: host, light: 11, dark: 11, kind: non-text, on: [canvas, surface, neutral-soft, accent-soft, chrome-surface, dialog-surface, panel-surface], host-surface: true, why: "The host's own focus ring; Radix's focus-ring steps 7-8 fail 3:1, and a widget-colored ring would vanish on lime or sky." }
  canvas: { source: gray, light: 2, dark: 1, why: "The window behind the frames; Radix's app background steps." }
  surface: { source: gray, light: 1, dark: 2, why: "A widget's frame and the fields drawn in it, one step off the canvas so frames stand out." }
  neutral-soft: { source: gray, light: 3, dark: 3, why: "Buttons, the monogram and badges: Radix's component background step." }
  accent-soft: { source: widget, light: 3, dark: 3, why: "An accented button or badge: the widget color's component background step." }
  ok-soft: { source: ok, light: 3, dark: 3, why: "The background of an ok badge." }
  warning-soft: { source: warning, light: 3, dark: 3, why: "The background of a warning badge." }
  error-soft: { source: error, light: 3, dark: 3, why: "The background of an error badge." }
  chrome-surface: { source: host, light: 3, dark: 3, host-surface: true, why: "The host's own bar, in the host color when there is one; no frame or panel covers it." }
  dialog-surface: { source: gray, light: 1, dark: 2, host-surface: true, why: "A host dialog's surface; what tells a dialog apart is its form, not this color." }
  panel-surface: { source: gray, light: 1, dark: 2, host-surface: true, why: "The panel a frame opens for a plugin's controls, attached to that frame." }
  scrim: { source: overlay, light: 7, dark: 9, host-surface: true, why: "Dims the window behind a host dialog; blackA because gray 12 is near white in the dark theme." }
tones:
  default: { text: text, surface: surface, border: border-control }
  accent: { text: accent-strong, surface: accent-soft }
  ok: { text: text, surface: ok-soft, icon: ok-icon, status-icon: check }
  warning: { text: text, surface: warning-soft, icon: warning-icon, status-icon: alert }
  error: { text: text, surface: error-soft, icon: error-icon, status-icon: cross }
```

## Typography

Inter 4.1 (`design/fonts/inter/`), built into the host, for everything: no font comes from a
plugin or the network. Nine levels from 12 px to 32 px; body text is 14 px, dense enough for many
widgets and still comfortable to read. Larger levels tighten their letter spacing, as Inter's
optical sizes expect. Tables turn on tabular figures (`tnum`) so numbers line up.

## Layout

A 4 px base. Frames pad their content by `md` (12 px), panels by `lg` (16 px) and dialogs by
`xl` (24 px); the grid's gutter is `md`. The dashboard grid itself is a later feature.

## Elevation & Depth

Depth is mostly tonal: the canvas, frames and soft surfaces are one Radix step apart. Shadows,
all from `blackA`, are kept for what floats: a frame barely, a panel more, a dialog most. The
panel's and the dialog's levels are the host's own surfaces; the kit cannot reach them.

```design-roles
elevation:
  flat: { shadow: none, why: "Most content sits flat on its frame." }
  frame: { shadow: "0px 1px 2px blackA-2", why: "A hint of lift so a frame reads as one object on the canvas." }
  panel: { shadow: "0px 4px 12px blackA-4", host-surface: true, why: "A panel floats beside its frame, never over the chrome." }
  dialog: { shadow: "0px 12px 32px blackA-6", host-surface: true, why: "A host dialog floats above everything, over the scrim." }
```

## Shapes

Corners are a pick: none, small, medium (the default), large or full. The radius applies to the
host and the kit alike; it never tells a dialog apart.

```design-roles
radius-picks: [none, small, medium, large, full]
```

## Components

The front matter's components are the host's surfaces and the kit's first parts. `chrome`,
`panel` and `dialog` are host surfaces: they and the `focus` and `scrim` roles are generated into
`host-surfaces`, which only the host crate may depend on. No component takes an image, an icon
source or a URL; the three status icons are drawn by the project, on a 16-unit grid.

```design-roles
host-surfaces: [chrome, panel, dialog]
icons:
  check: "M8 1a7 7 0 1 0 0 14A7 7 0 0 0 8 1Zm3.35 4.65-4.1 4.6a.75.75 0 0 1-1.1.03L4.4 8.53a.75.75 0 1 1 1.06-1.06l1.2 1.2 3.57-4.02a.75.75 0 1 1 1.12 1Z"
  alert: "M8.87 1.5a1 1 0 0 0-1.74 0L.63 13a1 1 0 0 0 .87 1.5h13a1 1 0 0 0 .87-1.5L8.87 1.5ZM8 5a.75.75 0 0 1 .75.75v3.5a.75.75 0 0 1-1.5 0v-3.5A.75.75 0 0 1 8 5Zm0 7.5a1 1 0 1 1 0-2 1 1 0 0 1 0 2Z"
  cross: "M8 1a7 7 0 1 0 0 14A7 7 0 0 0 8 1Zm2.53 4.47a.75.75 0 0 1 0 1.06L9.06 8l1.47 1.47a.75.75 0 1 1-1.06 1.06L8 9.06l-1.47 1.47a.75.75 0 0 1-1.06-1.06L6.94 8 5.47 6.53a.75.75 0 0 1 1.06-1.06L8 6.94l1.47-1.47a.75.75 0 0 1 1.06 0Z"
rationale:
  typography.caption: "Timestamps and the field note: the smallest text, regular weight."
  typography.label-sm: "Frame labels, badges and the monogram: small but medium weight so it holds."
  typography.body-sm: "Dense secondary content inside widgets."
  typography.body-md: "Body text and fields: 14 px reads well at desktop distance and fits dense widgets."
  typography.label-md: "Buttons and the chrome: body size, medium weight."
  typography.title-sm: "A widget's own heading."
  typography.title-md: "Panel and dialog titles."
  typography.title-lg: "Large numbers and page titles."
  typography.display: "The largest figure a widget shows, such as a timer."
  rounded.none: "A pick for square corners."
  rounded.small: "The monogram, and corners when the person picks small."
  rounded.medium: "The default: frames, buttons and fields."
  rounded.large: "Dialogs, a step rounder than what they float over."
  rounded.full: "Pills: badges, and every corner when the person picks full."
  spacing.none: "Flush."
  spacing.xxs: "Between an icon and its label."
  spacing.xs: "Inside compact controls."
  spacing.sm: "Between related items, and a field's padding."
  spacing.md: "A frame's padding and the grid's gutter."
  spacing.lg: "A panel's padding."
  spacing.xl: "A dialog's padding."
  spacing.2xl: "Between groups in a dialog."
  spacing.3xl: "Around empty states."
  components.chrome: "The host's bar at the top of the window: only a host dialog covers it."
  components.frame: "The host-drawn frame around every widget's content."
  components.frame-label: "The plugin's name in its frame, drawn by the host from the manifest."
  components.monogram: "The plugin's mark until decision 0001 allows plugin icons: its first letter."
  components.panel: "The panel a frame opens for a plugin's declared settings and actions."
  components.dialog: "A host dialog: the only place a trust decision is made."
  components.button: "A declared action's control, as the host draws it."
  components.button-accent: "The primary action among several."
  components.field: "A declared setting's field, drawn by the host, never masked."
  components.field-note: "The host's note beside every typed field: which widget gets the text, and that it is not for passwords, tokens or codes."
  components.badge: "A short status or count."
```

## Do's and Don'ts

- Do ask for a tone in widget content; never a color.
- Do show a status with its icon and words; never by color alone (WCAG 1.4.1).
- Don't put normal-size text on a solid step (9–10).
- Don't use the host's surfaces, the scrim or the focus style outside the host.
- Do change a role's step, never a threshold, when the contrast check fails.
