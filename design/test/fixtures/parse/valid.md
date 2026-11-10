---
version: alpha
name: Fixture
description: A small DESIGN.md the parser tests read.
colors:
  background: "oklch(0.99 0.003 250)"
  background-dark: "oklch(0.17 0.012 255)"
  foreground: "oklch(0.19 0.02 255)"
  foreground-dark: "oklch(0.96 0.006 250)"
  card: "oklch(1 0 0)"
  card-dark: "oklch(0.21 0.014 255)"
typography:
  body-md:
    fontFamily: Figtree Variable
    fontSize: 0.875rem
rounded:
  base: 0.625rem
spacing:
  unit: 0.25rem
---

## Overview

A small fixture.

## Colors

- **background**: the page.
- **foreground**: text on the page.
- **card**: panels.

```design-rules
hostOnly:
  - card
pairings:
  - { fg: foreground, on: [background], kind: text }
```

## Typography

- **body-md**: body text.

## Layout

- **unit**: the spacing unit.

## Elevation & Depth

- **flat**: no shadow.

```design-rules
elevation:
  flat: { shadow: none, dark: none }
```

## Shapes

- **base**: the base radius.

## Components

```design-rules
tones:
  default: { text: foreground }
unknownTone: default
```

## Do's and Don'ts

- Do keep it small.
