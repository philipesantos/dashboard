# Quickstart: checking feature 007

How to see that the feature works, end to end. Commands run from the repository root. Long
builds run under the memory cap the maintainer uses
(`systemd-run --user --scope -p MemoryMax=8G …`).

## 1. Everything passes

```sh
cargo xtask design --check
cargo test -p design-gen -p design-tokens -p host-surfaces
cargo deny check bans
```

Expected: all three exit 0. `--check` prints nothing on success.

## 2. Drift is caught (US1, FR-004)

Change one value in `DESIGN.md`, for example the `body-md` font size, without regenerating:

```sh
cargo xtask design --check
```

Expected: exit 1, one line naming `typography.body-md` and `design/tokens/src/generated.rs`.
Then `cargo xtask design` regenerates, and `--check` passes again. Undo the change.

## 3. A failing pairing is caught (US2, FR-008)

In `DESIGN.md`, make `border-control` use gray step 8 in the light theme, and its front-matter
color `#bbbbbb`, gray 8, so only the contrast rule can refuse it:

```sh
cargo xtask design --check
```

Expected: exit 1, naming `roles.border-control` on `surface` (non-text, light theme), a ratio
under 3:1 and the appearance where it is worst (R5). Undo the change.

## 4. The hue rule (US4, FR-011)

```sh
cargo test -p design-tokens --test appearance
cargo test -p design-gen --test it hue_rule::
```

Expected: the default passes; blue with an indigo host (15° apart) is refused as `TooClose`;
bronze as the widget color is refused as `NotChromatic`; the accepted widget colors with a
monochrome host are exactly lime, sky, cyan, blue, indigo, iris, violet, purple, plum and pink
(R3).

## 5. The kit cannot reach a host surface (US3, FR-015)

```sh
cargo test -p design-gen --test it host_surfaces::
cargo deny --manifest-path design/ban-fixture/Cargo.toml check bans   # must fail
```

Expected: the test passes (every host-surface token is absent from `design-tokens` and present in
`host-surfaces`, and no file `include!`s them); `cargo deny` on the fixture fails naming the
`host-surfaces` ban.

## 6. The criteria cover the threat model (US5, SC-005)

```sh
cargo test -p design-gen --test it criteria::
```

Expected: pass. Removing a criterion's **Answers** line, or adding a Phase 3 B4 row to
`docs/threat-model.md` that no criterion names, makes it fail with the row's title.

## 7. The preview (FR-026, SC-007)

```sh
cargo xtask design --preview target/design-preview
```

Expected: `index.svg`, `palette-light.svg`, `palette-dark.svg`, `default-light.svg` and
`default-dark.svg` in `target/design-preview/`, opening in a browser with the host's fonts (Barlow, IBM Plex Mono) and no
network request. On a pull request, the Linux build job's run has a `design-preview` artifact
with the same files.

## 8. CI treats `DESIGN.md` as code (FR-006, SC-006)

On a branch, change only `DESIGN.md` and open a pull request. Expected: the `changes` job prints
`code=true`, and every job runs on the three systems.
