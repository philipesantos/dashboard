---
version: alpha
name: Host-only fixture
colors:
  background: "oklch(0.99 0.003 250)"
  background-dark: "oklch(0.17 0.012 255)"
  foreground: "oklch(0.19 0.02 255)"
  foreground-dark: "oklch(0.96 0.006 250)"
  muted-foreground: "oklch(0.52 0.02 255)"
  muted-foreground-dark: "oklch(0.7 0.02 250)"
  primary: "oklch(0.5 0.13 245)"
  primary-dark: "oklch(0.74 0.12 235)"
  primary-foreground: "oklch(0.98 0.01 240)"
  primary-foreground-dark: "oklch(0.18 0.03 245)"
  secondary: "oklch(0.955 0.008 250)"
  secondary-dark: "oklch(0.27 0.016 255)"
  secondary-foreground: "oklch(0.26 0.02 255)"
  secondary-foreground-dark: "oklch(0.96 0.006 250)"
  destructive: "oklch(0.5 0.19 27)"
  destructive-dark: "oklch(0.74 0.16 22)"
  ok: "oklch(0.5 0.12 150)"
  ok-dark: "oklch(0.76 0.14 150)"
  warning: "oklch(0.52 0.12 65)"
  warning-dark: "oklch(0.8 0.14 80)"
  ring: "oklch(0.62 0.12 240)"
  ring-dark: "oklch(0.6 0.1 235)"
  sidebar: "oklch(0.975 0.005 250)"
  sidebar-dark: "oklch(0.19 0.013 255)"
  sidebar-border: "oklch(0.91 0.01 250)"
  sidebar-border-dark: "oklch(1 0 0 / 9%)"
  frame-header: "oklch(0.965 0.006 250)"
  frame-header-dark: "oklch(0.24 0.015 255)"
typography:
  heading: { fontFamily: Bricolage Grotesque Variable }
  body-sm: { fontFamily: Figtree Variable, fontSize: 0.875rem }
  mono: { fontFamily: IBM Plex Mono }
rounded:
  base: 0.625rem
spacing:
  unit: 0.25rem
  control-gap: 0.5rem
components:
  button-primary: { backgroundColor: "{colors.primary}", textColor: "{colors.primary-foreground}" }
  button-primary-dark: { backgroundColor: "{colors.primary-dark}", textColor: "{colors.primary-foreground-dark}" }
  button-secondary: { backgroundColor: "{colors.secondary}", textColor: "{colors.secondary-foreground}" }
  button-secondary-dark: { backgroundColor: "{colors.secondary-dark}", textColor: "{colors.secondary-foreground-dark}" }
---

## Overview

Host-only fixture.

## Colors

- **background**: page.
- **foreground**: text.
- **muted-foreground**: quiet text.
- **primary**: accent.
- **primary-foreground**: on accent.
- **secondary**: quiet fill.
- **secondary-foreground**: on quiet fill.
- **destructive**: errors.
- **ok**: success.
- **warning**: caution.
- **ring**: focus.
- **sidebar**: chrome.
- **sidebar-border**: chrome edge.
- **frame-header**: frame header.

```design-rules
hostOnly:
  - sidebar
  - frame-header
  - ring
```

## Typography

- **heading**: titles.
- **body-sm**: text.
- **mono**: ids.

## Layout

- **unit**: unit.
- **control-gap**: gap.

## Elevation & Depth

- **flat**: none.
- **raised**: menus.

```design-rules
elevation:
  flat: { shadow: none, dark: none }
  raised: { shadow: "0 4px 6px -1px rgb(0 0 0 / 0.1)", dark: "0 4px 6px -1px rgb(0 0 0 / 0.4)", hostOnly: true }
```

## Shapes

- **base**: radius.

## Components

- **button-primary**: main action.
- **button-secondary**: other actions.

```design-rules
tones:
  default: { text: foreground }
  muted: { text: muted-foreground }
  accent: { text: primary }
  ok: { text: ok, icon: circle-check }
  warning: { text: warning, icon: triangle-alert }
  error: { text: destructive, icon: circle-x }
unknownTone: default
variants:
  button.primary: { text: primary-foreground, fill: primary, rounded: md, typography: body-sm }
  button.secondary: { text: secondary-foreground, fill: secondary, rounded: md, spacing: control-gap }
```

## Do's and Don'ts

- Do keep the host's surfaces to the host.
