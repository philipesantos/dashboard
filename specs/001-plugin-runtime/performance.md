# Performance: Plugin Runtime

Measurements for User Story 4 (FR-015, SC-005), the targets set from them, and the default
limits they confirm.

## How it was measured

- **Run**: [Bench workflow, run 39815527335](https://github.com/philipesantos/dashboard/actions/runs/39815527335),
  2026-10-22, at commit `741a150`, triggered by the `bench` label on pull request #1
  (`.github/workflows/bench.yml`).
- **Build**: release profile (LTO, `codegen-units = 1`), Wasmtime 50 with parallel
  compilation.
- **Systems**: GitHub's standard runners for public repositories: `ubuntu-24.04` and
  `windows-latest` with 4 CPUs and 16 GB, `macos-latest` with 3 M1 CPUs and 7 GB.
- **Plugin**: `hello-widget`, a 63 KB component (Rust, `serde_json`), configuration `{}`.
- **Commands**: `runtime bench target/plugins/hello-widget.wasm --iterations 1000` and
  `runtime/examples/compile_cost.rs` for both package shapes.

Shared runners are noisy, and this is one run per system: read the numbers as an order of
magnitude, which is what the targets need.

## Loading and rendering hello-widget

`load_ms` is compiling the plugin; each render includes instantiating it in a fresh store.

| System | `load_ms` | render p50 | render p95 | render max |
| --- | --- | --- | --- | --- |
| Linux (`ubuntu-24.04`) | 42.1 ms | 80 µs | 122 µs | 968 µs |
| macOS (`macos-latest`) | 67.8 ms | 57 µs | 255 µs | 667 µs |
| Windows (`windows-latest`) | 50.0 ms | 59 µs | 99 µs | 246 µs |
| Reference machine (Linux, 8 cores) | 22 ms | 49 µs | 75 µs | 669 µs |

Before parallel compilation, the reference machine took about 100 ms to load the same plugin.

## Loading a package near the size cap

Compiling happens at load, outside every per-call limit and bounded only by the 10 MiB package
cap (`docs/threat-model.md`, decision [0001](../../docs/decisions/0001-plugin-process-isolation.md)).
Two shapes: `many` is 9.60 MiB of functions of about 1.2 KiB, like a large ordinary program;
`huge` is one 7.08 MiB function, as large as validation allows, with 4,000 locals in use at
once. Both stop at `contract-mismatch` after compiling, because they export no `renderer`.

| System | `many`: load | `huge`: load | Peak memory during the load |
| --- | --- | --- | --- |
| Linux | 718 ms | 280 ms | 593 MB (`many`), 130 MB (`huge`) |
| macOS | 540 ms | 286 ms | not reported |
| Windows | 790 ms | 299 ms | not reported |
| Reference machine | 325–349 ms | 466–490 ms | ~590 MB (`many`), ~110 MB (`huge`) |

## Targets

Measured against the CI systems above, on every system:

| Measure | Target | Measured, worst system |
| --- | --- | --- |
| Load of a plugin up to 100 KB | under 100 ms | 67.8 ms (macOS) |
| Render, instantiation included, p50 | under 200 µs | 80 µs (Linux) |
| Render, instantiation included, p95 | under 1 ms | 255 µs (macOS) |
| Load of a package near the 10 MiB cap | under 2 s and 1 GiB | 790 ms (Windows), 593 MB (Linux) |

The last target is a revisit trigger of decision 0001: passing it reopens the question of
compiling outside the host process. The plan's provisional goals from the Linux spike (load
under 50 ms, instantiate and render under 1 ms) are replaced by these.

## Default limits

The measurements confirm every default in `runtime/src/limits.rs` and research R9; none
changes.

| Limit | Default | Why it stays |
| --- | --- | --- |
| Package | 10 MiB | Loading a package near it stays under the 2 s and 1 GiB target; hello-widget is 63 KB |
| CPU budget | 500,000,000 fuel | About 0.36 s of tight computation on the reference machine; a render like hello-widget's uses a tiny fraction of it |
| Time limit | 1 s | Over 3,000 times the slowest p95 render; stops a runaway call 3–9 ms after the limit |
| Memory | 32 MiB per call | Enough for hello-widget with `serde_json`; growth beyond it fails at once |
| Widget content | 64 KiB | Far above any widget's JSON |
| Log messages | 100 per call, 1 KiB each | FR-010 |
