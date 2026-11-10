---
version: alpha
name: Fixture
colors:
  background: "oklch(0.99 0.003 250)"
  background-dark: "oklch(0.17 0.012 255)"
  primary: "oklch(0.5 0.13 245)"
  primary-dark: "oklch(0.74 0.12 235)"
typography:
  heading:
    fontFamily: Bricolage Grotesque Variable, ui-sans-serif, sans-serif
    fontWeight: 600
  body-xs:
    fontFamily: Figtree Variable, ui-sans-serif, sans-serif
    fontSize: 0.75rem
    lineHeight: 1rem
  body-sm:
    fontFamily: Figtree Variable, ui-sans-serif, sans-serif
    fontSize: 0.875rem
    lineHeight: 1.25rem
  mono:
    fontFamily: IBM Plex Mono, monospace
rounded:
  base: 0.625rem
spacing:
  unit: 0.25rem
---

## Overview

A fixture for the generator.

## Colors

- **background**: the page.
- **primary**: the accent.

## Typography

- **heading**: titles.
- **body-xs**: small.
- **body-sm**: default.
- **mono**: ids.

## Layout

- **unit**: the unit.

## Elevation & Depth

- **flat**: nothing.
- **raised**: menus.

```design-rules
elevation:
  flat: { shadow: none, dark: none }
  raised: { shadow: "0 4px 6px -1px rgb(0 0 0 / 0.1)", dark: "0 4px 6px -1px rgb(0 0 0 / 0.4)", hostOnly: true }
```

## Shapes

- **base**: the radius.

## Components

None.

## Do's and Don'ts

- Do test.
