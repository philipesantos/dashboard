# Data Model: Host Visual System

Entities of [spec.md](spec.md) as the generator and the checks see them. File formats are in
[contracts/](contracts/); decisions in [research.md](research.md).

## Scale

A Radix scale read from `design/radix/3.0.0/` (R2). `blackA`, Radix's overlay scale, is read
beside them for the scrim and shadows; it has no hue and is never a pick.

| Field | Type | Rule |
| --- | --- | --- |
| `name` | one of the 31 solid scale names | `gray`, `mauve`, `slate`, `sage`, `olive`, `sand` are gray families |
| `light`, `dark` | 12 sRGB colors each | steps 1–12, from the solid exports only (no `A`, no `P`) |
| `hue` | degrees, OKLCH, of light step 9 | R3 |
| `chroma` | OKLCH chroma of light step 9 | under 0.08 means neutral |
| `kind` | `gray-family` \| `neutral` \| `chromatic` | derived from `name` and `chroma` |
| `solid_text` | `white` \| `dark` \| `none` | declared in `DESIGN.md` (FR-009); the checker verifies it reaches 4.5:1 on steps 9 and 10, or that neither does for `none` (R5) |

## Role

A color role of the project layer (`text`, `text-secondary`, `surface`, `border-control`, …).

| Field | Type | Rule |
| --- | --- | --- |
| `name` | kebab-case | unique |
| `source` | `gray` \| `widget` \| `host` \| `ok` \| `warning` \| `error` | which pick or fixed scale it reads |
| `light`, `dark` | step 1–12, or `solid-text` | per theme |
| `why` | text | required (FR-001) |
| `host_surface` | bool | true routes it to the `host-surfaces` crate (R6) |
| `off_role` | WCAG criterion, optional | required when the step is outside its Radix role (R5) |

## Pairing

A foreground role over a background role that the contrast lint checks.

| Field | Type | Rule |
| --- | --- | --- |
| `fg`, `bg` | role names | must exist |
| `kind` | `text` \| `large-text` \| `non-text` \| `decorative` | thresholds 4.5, 3, 3, none (R5) |
| `why` | text | required for `decorative` |

## Tone

`default`, `accent`, `ok`, `warning`, `error`: the only color a widget names. Each resolves to a
set of roles (text, surface, border, icon) from `gray`, `widget`, or the fixed status scales
green, amber and red (FR-024). `ok`, `warning` and `error` carry a status icon (FR-025). An
unknown name is returned as `UnknownTone`; the kit draws `default` and logs the name escaped
(FR-017).

## Appearance

The person's picks.

| Field | Type | Default | Rule |
| --- | --- | --- | --- |
| `gray` | gray family | `gray` | any of the six |
| `host` | `monochrome` \| chromatic scale | `monochrome` | not neutral |
| `widget` | chromatic scale | `blue` | hue ≥ 30° from `host` (if chromatic) and from green, amber and red (FR-011) |
| `radius` | `none` \| `small` \| `medium` \| `large` \| `full` | `medium` | |
| `theme` | `light` \| `dark` \| `system` | `system` | |

**Validation** returns either the appearance or a refusal naming the rule and the colors involved
(`TooClose { widget, other, degrees }`, `NotChromatic { scale }`). The host turns the refusal into
its own words when the picking screen exists.

**Allowed set**: every appearance that validates; at most 6 × 23 × 10 × 5 × 3, since a chromatic
host removes the widget colors near it. Contrast depends only on gray, host, widget and the
resolved theme: at most about 2,760 cases (R5).

## Token

Anything generated: a resolved color per role and theme for the default appearance, a type
level, a spacing step, a radius, an elevation level, a component property.

| Field | Type | Rule |
| --- | --- | --- |
| `path` | dotted path (`typography.body-md`) | unique across both layers |
| `value` | typed (color, dimension, number, typography, shadow) | |
| `why` | text | required (FR-001) |
| `host_surface` | bool | from the project layer's marks |

## Criterion

One `HS-NN` entry of `docs/design/host-surfaces.md` (R10).

| Field | Type | Rule |
| --- | --- | --- |
| `id` | `HS-` + two digits | unique, never reused |
| `observe` | text | what is measured: a bound, a focus position, an accessibility-tree property, an input outcome |
| `expect` | text | the pass condition |
| `answers` | constitution rule(s) + threat-row title(s) | rows named by their exact first cell |
| `tested_by` | feature name | a later Phase 3 feature |

**Coverage rule**: every threat-model row on boundary B4 tested from Phase 3 is answered by at
least one criterion, or listed under "Decided elsewhere" with its owner (SC-005).
