# Contract: Commands, Generated Files and the Criteria Document

## Commands

Every command runs from the repository root, through the `design/` workspace package.

| Command | Does | Exit |
| --- | --- | --- |
| `pnpm --filter ./design gen` | Writes `app/src/theme.generated.css` and `design/widget-map.generated.json` from `DESIGN.md`. | 0, or 1 with the project checker's errors (nothing written) |
| `pnpm --filter ./design check` | Runs every check below without writing. | 0 when all pass, 1 otherwise |
| `pnpm --filter ./design preview` | Writes `design/preview/index.html` (not committed). | 0, or 1 as `gen` |
| `pnpm --filter ./design test` | Runs `node --test` on the package's tests. | `node --test`'s |
| `pnpm --filter ./design run licenses` | Checks the `license` of every installed npm package against the allowlist (research R13). | 0, or 1 naming each package and license |

`check` runs these, in order, and reports every failure before exiting:

1. **Format**: `design.md lint DESIGN.md` at 0.4.0. Its errors fail, and its warnings print.
2. **Project layer**:
   - the schema;
   - the `-dark` twins;
   - the bold rationale;
   - duplicate `design-rules` keys;
   - the format's section order and duplicate `##` headings, which the 0.4.0 checker only warns
     about or misses (research R1).
3. **Contrast**: every pairing, both themes.
4. **Coverage**: every color role is in a pairing or decorative.
5. **Usage**: every color utility in `app/src/**/*.tsx` matches a declared role and opacity.
6. **Host-only**: no tone or variant reaches a host-only role.
7. **Drift**: both generated files equal what `DESIGN.md` produces now.
8. **Criteria**: `docs/design/host-surfaces.md` and the threat model's B4 rows in Phase 3.
9. **Font licenses**: each family in `DESIGN.md`'s typography has its license text in
   `app/public/licenses/fonts/`.

Each failure is one line, prefixed with its file:

```text
DESIGN.md: pairing input on card (boundary, dark): 1.52 < 3.00
app/src/components/ui/button.tsx: ring-ring/50 is not declared (ring is declared at 100 only)
app/src/theme.generated.css: --primary differs from DESIGN.md (run pnpm --filter ./design gen)
docs/threat-model.md: row "Widget opens a window or dialog (phishing)" cites no HS- criterion
docs/design/host-surfaces.md: HS-07 has no "Feature:" line
```

## `app/src/theme.generated.css`

```css
/* Generated from DESIGN.md by `pnpm --filter ./design gen`. Do not edit. */
@theme inline {
    --font-heading: 'Bricolage Grotesque Variable', <fallback stack>;
    --font-sans: 'Figtree Variable', <fallback stack>;
    --font-mono: 'IBM Plex Mono', <fallback stack>;
    --color-background: var(--background);
    /* … one --color-<role> per color role, then --radius-sm … --radius-4xl from --radius */
    --spacing: 0.25rem;               /* DESIGN.md spacing.unit: every p-*, m-*, gap-* follows */
    --text-sm: 0.875rem;              /* … the type scale, with --text-*--line-height */
    --shadow-frame: var(--elevation-frame);  /* … one per elevation level */
}

:root {
    --background: oklch(0.99 0.003 250);
    /* … every role's light value, then --radius, then each level's light shadow */
    --elevation-frame: 0 1px 3px 0 rgb(0 0 0 / 0.1), …;
}

.dark {
    --background: oklch(0.17 0.012 255);
    /* … every role's dark value, then each level's dark shadow */
}
```

- The roles appear in `DESIGN.md`'s order.
- A shadow differs by theme, so `--shadow-<level>` points at `--elevation-<level>`, which
  `:root` and `.dark` set.
- `DESIGN.md` has one radius token, `rounded.base`, so a contributor changes every corner in
  one place. The generator derives the steps `sm` to `4xl` with the multipliers `index.css` uses
  today (0.6, 0.8, 1, 1.4, 1.8, 2.2, 2.6). The Shapes section's prose documents them. They are
  not separate tokens. The widget map names these derived steps.
- `app/src/index.css` imports this file and holds no token value.

## `design/widget-map.generated.json`

```json
{
  "generatedFrom": "DESIGN.md",
  "unknownTone": "default",
  "tones": {
    "default": { "text": "foreground" },
    "ok": { "text": "ok", "icon": "circle-check" }
  },
  "variants": {
    "button.primary": { "text": "primary-foreground", "fill": "primary", "rounded": "lg" }
  },
  "elevation": ["flat"]
}
```

It contains role and token names only, never values and never a host-only role or level.
`gen` refuses to write it when the host-only step fails. The future kit
turns a role into `var(--<role>)`.

## `docs/design/host-surfaces.md`

A Markdown document with one `###` heading per criterion:

```md
### HS-03 Widget content cannot cover the frame's header

Widget content is drawn only inside the frame's body. The body is a containing block with paint
containment and clipping, so no element of widget content, positioned or not, is drawn over the
header or outside the frame.

Check: hostile case: a widget whose content is wider and taller than its frame, and one that
asks for every tone and variant; the frame's header stays fully visible and hit-testable.
Feature: Moldura do host em cada widget com nome e ícone do plugin
```

- The `Check:` line starts with `test`, `hostile case` or `review`.
- The `Feature:` line names a Phase 3 item of the development guide, by the start of its text.

**The minimum set** (FR-017 and FR-018), refined in implementation:

- chrome and frame:
  - the frame's header is the host's;
  - the name and icon come from the manifest, escaped, with a length limit and an empty-name
    placeholder;
  - content is clipped to the body;
  - content cannot cover the header.
- dialogs:
  - a dialog covers the chrome and every frame;
  - clicks and keys are ignored for a short moment after it appears;
  - it names the plugin by name and repository id, escaped, with a length limit;
  - the rest of the window is inert for pointer, keyboard and assistive technology;
  - the dialog names the plugin or connector the decision is about;
  - the safe choice is the default;
  - long content scrolls inside the dialog.
- dialog accessibility:
  - focus moves in and is trapped;
  - Escape takes the safe choice;
  - focus returns to the opener;
  - the dialog is announced with its name and description;
  - nothing behind it is reachable by assistive technology.
- widget controls:
  - a control in widget content triggers only that widget's declared actions;
  - widget content has no field for a color, font, size, class or style, and a tone the host
    does not know draws as the default tone and is recorded in the plugin's log (FR-014);
  - the form has no password or masked field, and its inputs are never offered for autofill;
  - no plugin string reaches the opener or anything that opens a link, a file or another app;
  - widget content cannot produce a layer above the frames, a scrim or an inert window;
  - status is never shown by color alone.
- forced colors:
  - every property that marks a dialog still holds when the system replaces colors.

**Threat model citation.** The mitigation cell of every row on boundary B4 in Phase 3 ends with
`(HS-NN, …)`. Each criterion cited there has a `Check:` of `test` or `hostile case`.
