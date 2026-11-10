# Data Model: Host Visual System

Everything here lives in `DESIGN.md` and the files generated from it. There is no runtime
storage. File formats are in [contracts/design-md.md](contracts/design-md.md) and
[contracts/outputs.md](contracts/outputs.md).

## Token

A named design value.

| Field | Rule |
| --- | --- |
| `group` | One of `color`, `typography`, `rounded`, `spacing`, `elevation`. |
| `name` | Kebab-case. Colors are named by role (`primary`, `muted-foreground`). The format's own groups use the format's names. |
| `value` | Colors: an `oklch()` string, alpha allowed. Dimensions: `px` or `rem`. Typography: the format's typography object. |
| `dark value` | Colors only: the value of `<name>-dark`. Every color role MUST have one (FR-002). |
| `scope` | `host-only` or `shared`. Shared unless listed in `design-rules.hostOnly`. |
| `rationale` | The prose after the token's bold name in its section. It MUST exist (FR-002). |

**Validation**:
- No two tokens share a name within a group.
- A `-dark` color without its light role is an error.
- A token reference, such as `{colors.primary}`, must resolve.

## Layer stack

What a foreground is drawn on: a base color role, then zero or more `role/opacity` fills
composited in order. For example: `card`, then `destructive/10`.

## Pairing

| Field | Rule |
| --- | --- |
| `fg` | A color role, optionally `role/opacity`. |
| `on` | A layer stack whose base is opaque. |
| `kind` | `text` 4.5, `large-text` 3, `boundary` 3, `focus` 3 or `chart` 3. The number is the minimum ratio. |
| `themes` | Both unless listed. The check measures light with light values and dark with `-dark` values. A pairing lists one theme only for a layer a component draws in that theme alone, such as `dark:hover:bg-destructive/25`. |

**Validation**:
- Each pairing's ratio, after compositing, is at least its kind's minimum in each theme (FR-010).
- Every color role appears in at least one pairing, as `fg` or in `on`, or is listed in
  `decorative` with a reason (FR-011).

## Decorative role

A color role, with a reason, exempt from contrast under WCAG 1.4.11. For example, `border` for
card edges and dividers. It may never be a pairing's `fg` of kind `boundary` or `focus`.

An entry may also name one opacity of a role, as `destructive/40`, for a tint that nothing is
drawn on and that carries no meaning alone: the `aria-invalid` halo beside the focus ring, whose
error the field's text states. It covers that opacity only, and the role keeps its pairings.

## Usage

A color utility found in `app/src/**/*.tsx` or in an `@apply` of `app/src/*.css`, such as
`ring-ring/50`. It is identified by role and opacity (100 when none is written).

**Validation**:
- Every usage must match a pairing's `fg`, a layer in some pairing's `on`, or a decorative role,
  at the same opacity.
- A color utility that names no role (Tailwind's palette, `black`, `white`) fails.
- The scan reports the file and utility of each failure.
- `opacity-*` on disabled controls is not a color usage. WCAG exempts inactive components.
- `transparent`, `current` and `inherit` are not color usages. They add no color of their own.
- A utility is a color usage only when what follows its prefix, without `/opacity`, is a role, a
  Tailwind palette color, `black`, `white` or an arbitrary `[…]` value. `text-sm`, `ring-3` and
  `shadow-md` are sizes.

## Tone

A name widget content can select: `default`, `muted`, `accent`, `ok`, `warning` or `error`. A
tone maps to roles for `text` and, optionally, `fill` and `border`.

**Validation**:
- No role in a tone is host-only (FR-015).
- `ok`, `warning` and `error` each name the status icon the host draws beside them (FR-012).
- An unknown tone name resolves to `default`. That rule is recorded in the widget map for the
  kit to apply.

## Component variant

A named look of a declarative UI component (`text`, `list`, `table`, `chart`, `button`, `form`),
for example `button.primary`. It maps to roles for `text`, `fill` and `border`, and to tokens for
`rounded`, `spacing` and `typography`.

**Validation**: no role it maps to is host-only. A variant never names the focus style (`ring`),
which the host applies to every control.

## Elevation level

| Level | Scope | Use |
| --- | --- | --- |
| `flat` | shared | widget content |
| `frame` | host-only | the widget frame |
| `raised` | host-only | the host's menus and popovers above frames |
| `dialog` | host-only | host dialogs, over `scrim` |

Each level has a shadow value per theme.

## Criterion

A rule of the frame or dialog design, in `docs/design/host-surfaces.md`.

| Field | Rule |
| --- | --- |
| `id` | `HS-NN`, unique. |
| `statement` | One checkable sentence. |
| `check` | `test`, `hostile case` or `review`, with what it checks. |
| `feature` | The development guide's Phase 3 item that runs it. |

**Validation** (SC-005):
- Every criterion has `check` and `feature`.
- Every threat-model row on boundary B4 in Phase 3 cites at least one existing `HS-` id.
- A cited criterion's `check` is `test` or `hostile case`, never `review` (Principle VIII).

## Relationships

```text
DESIGN.md ─┬─ Token (color, typography, rounded, spacing) ──► theme.generated.css
           ├─ design-rules ─┬─ hostOnly ──────────────┐
           │                ├─ Pairing ── Layer stack  │
           │                ├─ Decorative              ├─► check (contrast, usage, map)
           │                ├─ Tone, Component variant ┴─► widget-map.generated.json (shared roles only)
           │                └─ Elevation level
           └─ prose (rationale)
docs/design/host-surfaces.md ── Criterion ◄── cited by ── docs/threat-model.md (Phase 3 rows)
app/src/**/*.tsx ── Usage ──► must match a Pairing, a layer or a Decorative role
```
