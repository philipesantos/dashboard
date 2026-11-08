# Implementation Plan: Host Visual System and the Design of the Host Frame and Dialogs

**Branch**: `007-host-visual-design`, merged into `master` through one pull request (#13),
reviewed commit by commit | **Date**: 2026-11-08 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/007-host-visual-design/spec.md`

## Summary

Give the host one visual system, checked by the build, before any window draws with it. A root
`DESIGN.md` in Google's format (pinned at 0.4.0) holds the tokens the format knows in its front
matter and, in `design-roles` blocks, what it does not: each color role as a step of a Radix
scale per theme, elevation, the radius picks, the fixed status scales, the host-surface marks
and a rationale for every token (R1). Radix Colors 3.0.0 is vendored as upstream data (R2). A
Rust library, `design-gen`, checks the file, generates two crates' tokens and a DTCG export,
runs the contrast lint over every appearance a person can pick and the 30° hue rule, and draws
an SVG preview; `cargo xtask design` drives it and its tests run in every `cargo test` (R4, R7,
R8). Measured on the real data, the hue rule leaves ten widget colors and the lint forces
concrete choices (no normal text on solid steps, step 9 for control boundaries, a host focus
style) (R3, R5). The host's surfaces live in a crate of their own that `cargo deny` lets only the
future host crate use (R6). The frame and dialog design becomes numbered criteria in
`docs/design/host-surfaces.md`, with a test that every Phase 3 threat row on boundary B4 is
answered or owned elsewhere (R10). CI counts these files as code and publishes the preview (R12).
No window, no hostile case: those arrive with the host features, which must pass the criteria.

## Technical Context

**Language/Version**: Rust (edition 2024, stable; MSRV 1.97, as the workspace)

**Primary Dependencies**: `yaml-rust2` `=0.13.0` in `design-gen` (R4); vendored data, not crates:
Radix Colors 3.0.0 `light.ts`/`dark.ts` (MIT, R2), Inter (OFL 1.1, R9); the DESIGN.md format
spec at tag 0.4.0 is a reference, not vendored code (R1); `actions/upload-artifact` pinned to a
commit SHA (R12)

**Storage**: files only: `DESIGN.md`; generated `design/tokens/src/generated.rs`,
`design/host-surfaces/src/generated.rs`, `design/tokens.dtcg.json` (committed, R7); the preview
in `target/` and as a CI artifact (R8)

**Testing**: `cargo test` in `design-gen` (format and project-layer checks, drift, contrast lint
over every allowed appearance, hue rule against an independent computation, Radix and spec
pins, criteria coverage against the threat model), in `design-tokens` (appearance validation,
tone resolution, the `compile_fail` doc test) and `cargo deny check bans` (the `host-surfaces`
ban); the existing security suite unchanged (FR-022)

**Target Platform**: the build and tests on Windows, macOS and Linux; the preview job on Linux

**Project Type**: Rust libraries and a repository task inside the existing Cargo workspace, plus
design documentation

**Performance Goals**: `cargo xtask design --check` under 5 s on the reference machine (about
2,800 contrast combinations × the declared pairings is arithmetic); no new CI job

**Constraints**: no Node.js in CI (spec, R4); no HTML and no script in anything generated (R8);
no image block or plugin-supplied icon (FR-023); the format's tags are the only upstream change
that can alter the checker, and moving them is its own reviewed change

**Scale/Scope**: 31 Radix scales × 12 steps × 2 themes; about 25 color roles and 20 pairings;
5 tones; 9 type levels, about 9 spacing steps, 5 radii, 4 elevation levels; 3 new workspace
crates (`design-gen`, `design-tokens`, `host-surfaces`) and one `xtask` command; about 20
criteria

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

Checked against constitution v4.0.0.

| Rule | Status | How the plan complies |
| --- | --- | --- |
| I. WebAssembly-only plugins | Pass (N/A) | No plugin code is loaded or run; the new crates are host-side libraries and a repository task |
| II. Zero capabilities by default | Pass (N/A) | No capability, import or grant changes |
| III. External data only through connectors | Pass | Nothing generated reaches the network: the preview embeds its font and references nothing outside the file (R8); the vendored data is read from the repository, never fetched by the build (R2) |
| IV. Plugins never see secrets | Pass (N/A) | No secret is involved |
| V. Plugins stay inside the host frame | Pass | The host's surfaces (chrome, dialog, scrim, panel attachment, focus) are tokens of a crate the kit cannot depend on (R6); the criteria state, testably, that nothing a plugin supplies covers the chrome, takes a dialog role or draws outside its frame, that panels stay attached and never dim or block, and that every typed field carries the host's note bound for assistive technology (R10) |
| VI. Human confirmation for AI actions | Pass (N/A) | No AI; the criteria leave AI output's presentation to Phase 4 |
| VII. Trust decisions only in host dialogs | Pass | The criteria define the host dialog's form (covers the chrome, blocks every host window, keeps focus, ignores outside input, sole node for assistive technology, never a native prompt) for the dialog features to test (R10, R11) |
| VIII. Every security rule has a breaking test | Pass | This feature adds no security rule of its own: the kit-and-host split is defence in depth, tested as an ordinary test (spec, Clarifications); the rules of V and VII it designs get their hostile cases in the features that build the window, as the criteria's **Tested by** says; the suite runs unchanged (FR-022) |
| IX. Human review of the security core | Pass | No change to `runtime/`, `wit/` or `connectors/`; `deny.toml` changes (a ban), and is reviewed with the rest |
| Fixed decisions: host app, UI | **Open** | Colors only from Radix scales; typography, spacing, radius and shadow are the project's own, in `DESIGN.md` (R13); no GPUI dependency yet. But "each step used for its role" collides with WCAG 1.4.11: Radix's border steps (7, 8) reach only 1.86:1 on step 1, so a control boundary at 3:1 needs a step Radix assigns to another role (9, solid backgrounds, or 11, text). Brought to the maintainer (R5) |
| Fixed decision: licenses | Pass | Client code Apache-2.0; vendored Radix data MIT and Inter OFL 1.1 kept with their licenses; `yaml-rust2` MIT or Apache-2.0, inside `deny.toml`'s list |
| Workflow: CI on three systems, clippy, audit, deny | Pass | The new crates are workspace members, so the build job's Clippy, `cargo test`, `cargo deny` and `cargo audit` cover them on three systems (R12) |
| Workflow: documentation-only rule | Pass | `DESIGN.md`, `design/`, `docs/design/` and `docs/threat-model.md` are read by the checks, so the `changes` job counts them as code (FR-006, R12) |
| Workflow: phase gates | Pass | Phases 1 and 2 passed; the Phase 3 gate (every Phase 3 row of the security table blocked) is not this feature's to pass, and the criteria map those rows to the features that will |
| Workflow: Spec Kit, Constitution Check | Pass | This table, re-checked below, and an independent review before `/speckit-tasks` |

**Post-design re-check**: Pass on the plan as written; the independent review before
`/speckit-tasks` re-checks it.

## Project Structure

### Documentation (this feature)

```text
specs/007-host-visual-design/
├── plan.md              # This file
├── research.md          # Phase 0: decisions R1–R13
├── data-model.md        # Phase 1: scales, roles, pairings, tones, appearance, tokens, criteria
├── quickstart.md        # Phase 1: how to check the feature end to end
├── contracts/
│   ├── design-md.md              # the two layers of DESIGN.md and the checker's rules
│   ├── tokens-api.md             # design-tokens, host-surfaces, cargo xtask design, the DTCG file
│   └── host-surfaces-criteria.md # the criteria file and its coverage rules
├── checklists/requirements.md
└── tasks.md             # Phase 2 (/speckit-tasks)
```

### Source Code (repository root)

```text
DESIGN.md                        # the visual system (R1, R13)

design/
├── radix/3.0.0/                 # light.ts, dark.ts, LICENSE, pinned by SHA-256 (R2)
├── fonts/inter/                 # Inter variable WOFF2 and TTF, OFL.txt (R9)
├── gen/                         # crate design-gen: checker, generator, lint, preview (R4)
│   ├── src/
│   └── tests/                   # drift, lint, hue, pins, criteria coverage
├── tokens/                      # crate design-tokens (R6)
│   └── src/{lib.rs,generated.rs}
├── host-surfaces/               # crate host-surfaces, banned except for `host` (R6)
│   └── src/{lib.rs,generated.rs}
└── tokens.dtcg.json             # generated (R7)

docs/design/host-surfaces.md     # the frame and dialog criteria (R10)

xtask/src/main.rs                # `design`, `design --check`, `design --preview <dir>`
Cargo.toml                       # workspace members: design/gen, design/tokens, design/host-surfaces
deny.toml                        # ban host-surfaces, wrappers = ["host"]
.github/workflows/ci.yml         # code paths; design --check; preview + upload on Linux (R12)
CONTRIBUTING.md                  # how to change DESIGN.md and regenerate
```

**Structure Decision**: the visual system is its own top-level `design/` directory beside
`runtime/`, because it is shared by the future host and kit and depends on neither. The generator
is a library so its checks are ordinary tests that run on three systems; `xtask` only calls it.
The host's surfaces are a separate crate because the boundary the spec asks for (the kit cannot
reach them) is only enforceable between crates (R6).

## Complexity Tracking

| Choice | Why needed | Simpler alternative rejected because |
| --- | --- | --- |
| Three crates instead of one | FR-015: the kit must fail to build against a host surface | One crate with modules or features cannot stop a dependent crate from reaching them (R6) |
| A project layer inside `DESIGN.md` | The format has no themes, steps, shadows or per-token rationale (R1) | A second file would be a second source (FR-001); unknown front-matter keys risk the format's future versions |
| Our own checker instead of the format's tool | No Node.js in CI; the tool's contrast check is one pass on declared pairs, as a warning (R4) | Bringing Node.js back for one check adds a runtime and its packages to the build |
