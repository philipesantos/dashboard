# Performance: the host window and its dialogs

Measurements for SC-001 (every approved plugin's frame drawn within 2 s of starting, with 5
plugins, on the reference machine) and SC-006 (a host dialog within 100 ms of being asked), and
the compile time and memory of the packages the window loads, which decision
[0001](../../docs/decisions/0001-plugin-process-isolation.md)'s third trigger watches (research
R14).

## On the three systems (CI)

- **Run**: [CI, run 42695343870](https://github.com/philipesantos/dashboard/actions/runs/42695343870),
  2026-11-09, at commit `b31e6fc`, the "Host window" job (`cargo test -p host --test window`).
- **Build**: the test profile (unoptimized), as CI runs it.
- **Systems**: GitHub's standard runners. Linux draws with Mesa's software Vulkan (lavapipe)
  under Xvfb; macOS and Windows on their runners' displays.
- **Window**: two dashboard windows side by side, each with 2 approved plugins (`hello-widget`,
  which renders, and `out-of-bounds`, which traps).
- **Measured by the test**: from the program's start to the first moment every frame in both
  windows has its status (SC-001's measure), and from a host dialog being asked to its first frame
  (SC-006's).

| System | Every frame with its status | Dialog's first frame |
| --- | --- | --- |
| Linux (`ubuntu-24.04`) | 2.12 s | 12.4 ms |
| macOS (`macos-latest`) | 2.54 s | 2.5 ms |
| Windows (`windows-latest`) | 2.09 s | 15.5 ms |

Earlier runs of the same job measured 1.40 s and 3.30 s on Linux and 2.72 s on Windows, and
dialogs at 4.5–12 ms: shared runners and an unoptimized build make the start time noisy and
slow. Every dialog appeared well within SC-006's 100 ms, on every system and every run.

## On the reference machine (SC-001)

- **Date**: 2026-11-09, at commit `b31e6fc` plus the `--time` flag of the screenshot example.
- **Machine**: the maintainer's Linux machine, 8 cores.
- **Command**: `host/examples/screenshot.rs --time`, which starts the runtime and opens the
  dashboard window offscreen with GPUI's headless renderer on the GPU (wgpu), and prints when
  every frame had its status and how long each package took to load (compile). Five plugins
  approved with the development tool: `hello-widget`, `log-flood`, `state-leak` (they render),
  `log-spoof` and `out-of-bounds` (they fail, and their frames say why).
- **Build**: `--release` with `opt-level = 3`, but without fat LTO and with 16 codegen units
  (`CARGO_PROFILE_RELEASE_LTO=false`, `CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16`): the shipped
  profile's single-unit LTO over GPUI and Wasmtime does not fit the machine's 8 GiB build cap.
  It is at most slightly slower than the shipped build.
- **Memory**: peak resident memory of the whole process, from `/usr/bin/time`.

Three runs:

| Run | Every frame with its status | Peak memory | Package loads (compile) |
| --- | --- | --- | --- |
| 1 (cold) | 612 ms | 212 MiB | 23–72 ms |
| 2 | 434 ms | 204 MiB | 25–67 ms |
| 3 | 393 ms | 207 MiB | 21–67 ms |

SC-001 holds with room to spare: under 0.65 s against 2 s. `hello-widget` (63 KB) is the slowest
load at 67–72 ms; the window loads packages one after another, so five ordinary packages add
about 150–200 ms.

The same five in an unoptimized build took 2.22–2.30 s, each load 200–540 ms: Wasmtime's
compiler unoptimized is about ten times slower, which is why CI's start times above are near
2 s with only two plugins.

## Against decision 0001's third trigger

The trigger is a package near the 10 MiB cap taking more than 2 s or 1 GiB to load on any CI
system. The window loads with the same `Runtime::load` the development tool and the bench use, so
the bench's near-cap measurements
([001's performance.md](../001-plugin-runtime/performance.md)) stay the measure of record; the
window adds no compile work of its own. The packages above, ordinary ones, compile in under
75 ms optimized, and the whole window stays near 210 MiB.
