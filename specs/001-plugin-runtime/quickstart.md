# Quickstart: validate the plugin runtime

Run from the repository root. Each step lists what proves the feature works.

## Prerequisites

- Rust stable with the WebAssembly targets:

  ```bash
  rustup target add wasm32-unknown-unknown wasm32-wasip2
  ```

- The development setup from [CONTRIBUTING.md](../../CONTRIBUTING.md).

## 1. Build every plugin

```bash
cargo xtask plugins
```

Expected: one component per plugin crate in `target/plugins/`, among them
`hello-widget.wasm` and one file per hostile case.

## 2. Run the example plugin (User Stories 1 and 3)

```bash
cargo run -p runtime --release -- run target/plugins/hello-widget.wasm --config '{"name":"Ada"}'
```

Expected: the widget content as JSON on stdout, a `[plugin] info: …` line on stderr, exit
status 0. Changing `--config` changes the content.

## 3. Watch hostile plugins get blocked (User Story 2)

```bash
cargo run -p runtime --release -- run target/plugins/infinite-loop.wasm
cargo run -p runtime --release -- run target/plugins/read-file.wasm
cargo run -p runtime --release -- run target/plugins/log-spoof.wasm
```

Expected: `error: cpu-budget-exceeded: …` for the first; `error: capability-refused: …` listing
`wasi:filesystem/…` imports for the second; for the third, three `[plugin]` lines whose line
breaks and control characters show escaped (`\n`, `\u{1b}`), the terminal title unchanged, and
`error: plugin-error: \nerror: forged` on a single line; exit status 1 for all three. The expected kind for each
hostile plugin is in [data-model.md](data-model.md#hostile-plugin-case).

## 4. Run the security suite (phase gate)

```bash
cargo test -p runtime --test hostile
```

Expected: every case passes, which means every hostile plugin was blocked with its expected
error kind and a well-behaved plugin rendered normally afterwards. CI runs the same suite on
Windows, macOS and Linux.

## 5. Measure performance (User Story 4)

```bash
cargo run -p runtime --release -- bench target/plugins/hello-widget.wasm
```

Expected: `load_ms`, `render_p50_us`, `render_p95_us` and `render_max_us`, following
[contracts/cli.md](contracts/cli.md). For all three systems, add the `bench` label to the pull
request: `.github/workflows/bench.yml` runs the same command, plus the cost of loading packages
near the size cap (`runtime/examples/compile_cost.rs`), and its results go to
[performance.md](performance.md).

## 6. Check the records

- `specs/001-plugin-runtime/performance.md` has measurements and targets for all three
  systems.
- `docs/decisions/0001-plugin-process-isolation.md` records the decision and its rationale.
