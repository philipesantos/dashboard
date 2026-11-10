# Implementation Plan: Host Visual System and the Design of the Host Frame and Dialogs

**Branch**: `007-host-visual-system` | **Date**: 2026-11-10 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/007-host-visual-system/spec.md`

## Summary

`DESIGN.md` at the repository root becomes the only source of the host's look. It uses Google's
open format at version 0.4.0, with oklch colors and `-dark` twins, plus `design-rules` blocks for
host-only roles, contrast pairings, tones and variants. A new workspace package, `design/`, holds
the TypeScript tooling, run by Node.js 24 without a build step. It does four things:

- generates the app's theme, `app/src/theme.generated.css`, and the widget map the future kit
  reads;
- checks the format with its pinned checker, plus contrast, color usage in the app, the
  host-only split, drift and criteria coverage;
- draws a static preview that CI publishes;
- runs on every change that touches `DESIGN.md`, `docs/design/` or the threat model, once
  `ci.yml` treats those as code.

The frame and dialog design is written as numbered criteria in `docs/design/host-surfaces.md`.
The threat model's Phase 3 rows cite them. The starting values are the Ardósia + Musgo theme,
with the contrast fixes research R4 measured: input edges, a solid focus ring, two chart colors
and `destructive` in light.

## Technical Context

**Language/Version**: TypeScript run by Node.js 24 type stripping, for `design/`. CSS (Tailwind
v4) and React 19 in `app/`, unchanged.

**Primary Dependencies**:
- `@google/design.md` 0.4.0, the format checker;
- `yaml` 2.9.1.

Both are development dependencies pinned to exact versions. The fonts `@fontsource-variable/figtree`,
`@fontsource-variable/bricolage-grotesque` and `@fontsource/ibm-plex-mono`, all OFL-1.1, are
already in `app/`.

**Storage**: files only:
- `DESIGN.md`;
- `app/src/theme.generated.css` and `design/widget-map.generated.json`, generated and committed;
- `docs/design/host-surfaces.md`.

**Testing**: `node --test` in `design/`, covering:
- the color math against reference values;
- the parser and each checker, with failing fixtures;
- drift.

`pnpm --filter ./design check` runs in CI on Windows, macOS and Linux.

**Target Platform**: CI runners and contributors' machines. The output runs in the Tauri webview
on all three systems.

**Project Type**: build-time tooling and design documents for a desktop app.

**Performance Goals**: `check` completes in under 10 seconds on a CI runner, so it adds nothing
noticeable to the build job.

**Constraints**:
- no runtime dependency added to `app/`;
- no font or asset fetched at run time;
- generated files byte-identical on all three systems (`eol=lf`);
- no change to `runtime/`, `wit/`, `plugins/` or the Tauri crate.

**Scale/Scope**:
- about 50 color roles × 2 themes;
- about 60 pairings;
- 6 tones and about 20 component variants;
- about 25 criteria.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle or rule | Status | How |
| --- | --- | --- |
| I. WebAssembly-only plugins | Pass | No plugin code involved. |
| II. Zero capabilities by default | Pass | No capability added. |
| III. External data only through connectors | Pass | Nothing fetched at run time; fonts bundled (R7). |
| IV. Plugins never see secrets | Pass | Not touched. |
| V. Plugins stay inside the host frame | Pass, strengthened | The criteria make "inside the frame, no dialogs" checkable (HS- criteria, R10, R11). The host-only split keeps the frame's and dialogs' roles out of widget content (R6). |
| VI. Human confirmation for AI actions | Pass | Not touched. |
| VII. Trust decisions only in host dialogs | Pass, strengthened | The dialog's structure is specified so a widget's picture of one cannot pass for it and its controls cannot reach a trust decision (R11). |
| VIII. Every security rule has a breaking test | Pass | The host-only map check fails the build on a violation (FR-015). The dialog and frame rules are written as criteria now. Their breaking tests, named in each criterion's `Check:` line, land with the features that draw the frame and dialogs, since nothing draws them yet. |
| IX. Human review of the security core | Pass | No security-core file changes. |
| Fixed decisions (Tauri 2 + React, licenses) | Pass | Unchanged. Node.js tooling is build-time only. The fonts are OFL-1.1, bundled beside Apache-2.0 code under their own license. |
| CI on three systems; docs-only skip | Pass | `DESIGN.md`, `docs/design/` and `docs/threat-model.md` become code paths (R9), as the constitution's "Markdown the build uses as code" rule requires. |
| Phase gates | Pass | Phases 1 and 2 have passed (the guide, rev 147), so Phase 3 UI work may start. |

**Re-check after Phase 1 design**: unchanged, all pass. No complexity to justify.

## Project Structure

### Documentation (this feature)

```text
specs/007-host-visual-system/
├── plan.md              # This file
├── research.md          # Phase 0: R1–R12
├── data-model.md        # Phase 1
├── quickstart.md        # Phase 1
├── contracts/
│   ├── design-md.md     # DESIGN.md's two layers
│   └── outputs.md       # commands, generated files, the criteria document
├── checklists/
│   └── requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
DESIGN.md                          # new: the visual system
.gitattributes                     # eol=lf for DESIGN.md and generated files
pnpm-workspace.yaml                # + design
design/                            # new workspace package (private)
├── package.json                   # scripts gen, check, preview, test; pinned devDependencies
├── src/
│   ├── parse.ts                   # front matter + design-rules blocks + bold rationale
│   ├── color.ts                   # oklch → sRGB, compositing, WCAG ratio
│   ├── checks/                    # format, schema, contrast, coverage, usage, host-only, drift, criteria
│   ├── generate.ts                # theme.generated.css, widget-map.generated.json
│   ├── preview.ts                 # preview/index.html
│   └── cli.ts                     # gen | check | preview
├── test/                          # node --test; fixtures/ with failing DESIGN.md files
├── widget-map.generated.json      # generated, committed
└── preview/                       # generated, ignored
app/
├── src/index.css                  # imports + @layer base only; imports theme.generated.css
├── src/theme.generated.css        # generated, committed
├── src/App.tsx                    # starts in the system's theme (R12)
└── src/components/ui/*.tsx        # ring-ring/50 → ring-ring, other undeclared opacities fixed
docs/
├── design/host-surfaces.md        # new: HS- criteria
└── threat-model.md                # Phase 3 rows cite HS- criteria; new imitation row
.github/workflows/ci.yml           # filter paths; design check + test; preview artifact
```

**Structure Decision**: the tooling is its own pnpm workspace package beside `app/`, so its
dependencies stay out of the app's bundle and `pnpm --filter` runs it in CI and locally. The
criteria live in `docs/design/` because later features cite them as a standing rule (R10).

## Complexity Tracking

No violations to justify.
