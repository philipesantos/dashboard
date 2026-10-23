# Performance: Plugin Permissions

Measurements for SC-007: checking the manifest and the grants adds less than 10 ms to loading a
plugin, on every CI system (research R11).

## How it was measured

- **Run**: [Bench workflow, run 39952168630](https://github.com/philipesantos/dashboard/actions/runs/39952168630),
  2026-10-23, at commit `3d91a21`, triggered by the `bench` label on pull request #2
  (`.github/workflows/bench.yml`).
- **Build**: release profile (LTO, `codegen-units = 1`), Wasmtime 50 with parallel
  compilation, as in [feature 001](../001-plugin-runtime/performance.md).
- **Systems**: GitHub's standard runners for public repositories: `ubuntu-24.04`,
  `macos-latest` and `windows-latest`.
- **Plugin**: `hello-widget` 0.2.0, contract v1, a 64 KB package with its manifest, `log`
  required and approved in a grant record under the runner's temporary directory.
- **Commands**: `runtime approve`, then `runtime bench target/plugins/hello-widget.wasm
  --iterations 1000 --grants <record>`, and `runtime/examples/compile_cost.rs` for both package
  shapes.

Shared runners are noisy, and this is one run per system: read the numbers as an order of
magnitude, which is what the target needs.

## The permission checks

Both values are the median of 100 runs (`contracts/cli.md`):

- `checks_us`: every check of a load except the compile and the linking: finding and parsing
  the manifest section, the contract and import checks, and reading the grant record, from its
  file under its lock, to decide what to link (load steps 2, 3 and 5–8).
- `relink_us`: linking again after a grant change, as a loaded plugin does on its next call:
  reading the record, deciding, building the plugin's linker and preparing the instance (load
  steps 7–9).

| System | `checks_us` | `relink_us` |
| --- | --- | --- |
| Linux (`ubuntu-24.04`) | 20 µs | 14 µs |
| macOS (`macos-latest`) | 24 µs | 23 µs |
| Windows (`windows-latest`) | 114 µs | 104 µs |
| Reference machine (Linux, 8 cores) | 20–21 µs | 14–18 µs |

**SC-007 passes on every system.** The slowest, Windows, takes 114 µs for the checks, about 88
times under the 10 ms target, and 104 µs to link again.

## Loading and rendering hello-widget

`load_ms` now includes every check above. Next to feature 001's run, before the permission
engine, loading and rendering stay in the same range, and feature 001's targets still hold
(load under 100 ms, render p50 under 200 µs and p95 under 1 ms).

| System | `load_ms` | render p50 | render p95 | render max | Feature 001: `load_ms`, p50 |
| --- | --- | --- | --- | --- | --- |
| Linux | 42.8 ms | 75 µs | 149 µs | 1,122 µs | 42.1 ms, 80 µs |
| macOS | 46.3 ms | 40 µs | 105 µs | 354 µs | 67.8 ms, 57 µs |
| Windows | 48.0 ms | 53 µs | 94 µs | 245 µs | 50.0 ms, 59 µs |

## Loading a package near the size cap

The `compile_cost` packages now carry a manifest and are approved, so they reach the compile;
both still stop at `contract-mismatch` afterwards, since they export no `renderer`. The target
of feature 001 (under 2 s and 1 GiB) still holds.

| System | `many` (9.60 MiB): load | `huge` (7.08 MiB): load | Peak memory during the load |
| --- | --- | --- | --- |
| Linux | 706 ms | 271 ms | 593 MB (`many`), 132 MB (`huge`) |
| macOS | 318 ms | 187 ms | not reported |
| Windows | 808 ms | 303 ms | not reported |
