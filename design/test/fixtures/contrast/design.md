---
version: alpha
name: Contrast fixture
colors:
  background: "oklch(0.99 0.003 250)"
  background-dark: "oklch(0.17 0.012 255)"
  foreground: "oklch(0.19 0.02 255)"
  foreground-dark: "oklch(0.96 0.006 250)"
  card: "oklch(1 0 0)"
  card-dark: "oklch(0.21 0.014 255)"
  destructive: "oklch(0.54 0.2 27)"
  destructive-dark: "oklch(0.7 0.18 22)"
  border: "oklch(0.91 0.01 250)"
  border-dark: "oklch(1 0 0 / 9%)"
  input: "oklch(0.60 0.02 250)"
  input-dark: "oklch(1 0 0 / 35%)"
  chart-2: "oklch(0.58 0.10 200)"
  chart-2-dark: "oklch(0.76 0.1 195)"
typography:
  heading: { fontFamily: Bricolage Grotesque Variable }
  body-sm: { fontFamily: Figtree Variable, fontSize: 0.875rem }
  mono: { fontFamily: IBM Plex Mono }
rounded:
  base: 0.625rem
spacing:
  unit: 0.25rem
---

## Overview

Contrast fixture.

## Colors

- **background**: page.
- **foreground**: text.
- **card**: panel.
- **destructive**: errors.
- **border**: dividers.
- **input**: field edges.
- **chart-2**: a series.

```design-rules
decorative:
  - role: border
    reason: Dividers whose meaning the layout carries.
pairings:
  - { fg: foreground, on: [background], kind: text }
  - { fg: foreground, on: [card], kind: text }
  - { fg: destructive, on: [card, destructive/10], kind: text }
  - { fg: input, on: [card], kind: boundary }
  - { fg: chart-2, on: [card], kind: chart }
```

## Typography

- **heading**: titles.
- **body-sm**: text.
- **mono**: ids.

## Layout

- **unit**: unit.

## Elevation & Depth

Flat.

## Shapes

- **base**: radius.

## Components

None.

## Do's and Don'ts

- Do measure.
