# Quickstart: Validating the Host Visual System

These steps prove each success criterion on a checkout of the feature branch. Commands run from
the repository root, and need Node.js 24 or later and pnpm 12.

## 1. Install

```sh
pnpm install --frozen-lockfile
```

**Expected**: the `design` package installs `@google/design.md` 0.4.0 and `yaml` 2.9.1 at
exactly those versions.

## 2. Everything passes on the committed state

```sh
pnpm --filter ./design check
pnpm --filter ./design test
```

**Expected**:
- Both exit 0. The format checker reports zero errors (warnings may print).
- The contrast step lists every pairing with its light and dark ratios, none below its minimum
  (SC-002).
- The first time this runs, record the format checker's warnings in `research.md` R1.

## 3. One source (SC-001, User Story 1)

1. In `DESIGN.md` only, change three tokens:
   - `primary` to another hue, such as oklch(0.5 0.13 280) and its `-dark` twin;
   - one spacing step;
   - `rounded.base` to `0.375rem`.
2. Run `pnpm --filter ./design gen`, then `pnpm --filter ./app dev`, and open the page.

**Expected**: the buttons, spacing and corners change in light and dark. `git diff` shows
changes only in `DESIGN.md`, `app/src/theme.generated.css` and, if a variant changed,
`design/widget-map.generated.json`. Time it: under five minutes. Revert.

## 4. Drift is caught (FR-005)

Edit one value in `app/src/theme.generated.css` by hand, then run
`pnpm --filter ./design check`.

**Expected**: exit 1, with the line
`app/src/theme.generated.css: --<role> differs from DESIGN.md …`. Revert.

## 5. Contrast is enforced (SC-002, User Story 2)

1. Set `chart-2` back to oklch(0.68 0.1 200), then run `check`.

   **Expected**: `DESIGN.md: pairing chart-2 on card (chart, light): 2.76 < 3.00`.

2. Revert it. Then in `app/src/components/ui/button.tsx`, change one `ring-ring` back to
   `ring-ring/50` and run `check`.

   **Expected**: the usage line names the file and `ring-ring/50`. Revert.

## 6. Host-only roles stay out of reach (SC-003, User Story 3)

In `DESIGN.md`'s Components `design-rules` block, map `button.secondary`'s `fill` to
`frame-header`, then run `check`.

**Expected**: `DESIGN.md: variant button.secondary maps fill to host-only role
"frame-header"`. Revert. Then open `design/widget-map.generated.json` and confirm that no role in
it is host-only.

## 7. A DESIGN.md-only change runs all of CI (SC-004)

Push a commit that touches only `DESIGN.md`, for example a word of prose. Then push another that
touches only a fixture under `design/test/fixtures/`.

**Expected**: in both cases "Detect code changes" outputs `code=true`, and the build jobs run on
Windows, macOS and Linux.

## 8. Criteria coverage (SC-005, User Stories 4–6)

1. Delete the `Feature:` line of one criterion in `docs/design/host-surfaces.md`, run `check`,
   and revert.
2. Remove the `HS-` citation from the threat model's "Widget opens a window or dialog
   (phishing)" row, run `check`, and revert.

**Expected**: each fails, naming the criterion or the row.

## 9. The look is unchanged except for the fixes (SC-006)

Run `pnpm --filter ./app dev` on this branch and on `master`, and compare the demo in light and
dark.

**Expected**: the only differences are the ones research R4 lists:
- darker input edges, and in dark mode slightly lighter field and switch fills;
- the solid focus ring;
- `chart-2` and `chart-3` darker in light;
- `destructive` slightly darker in light;
- the submenu's shadow matching the menu's.

## 10. The preview (SC-007)

1. Run `pnpm --filter ./design preview` and open `design/preview/index.html`.
2. On the pull request's CI run, download the `design-preview` artifact from the Linux job.

**Expected**: both themes, the scales, tones with icons, the components, and sketches of the
chrome, a frame and a dialog. The page loads no script.
