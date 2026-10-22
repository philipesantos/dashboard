# Contract: `runtime` command-line test tool

The tool is the only way to run plugins in this feature (FR-011). It is a binary of the
`runtime` crate: `cargo run -p runtime --release -- <command>`.

## `runtime run <plugin> [--config <json> | --config-file <path>]`

Loads the plugin, calls `render` once and prints the result.

| Stream | Content |
| --- | --- |
| stdout | On success, the widget content as pretty-printed JSON. Nothing on failure. |
| stderr | One line per plugin log message: `[plugin] <level>: <message>`. If messages were dropped: `[runtime] <n> log messages dropped`. On failure: `error: <kind>: <detail>`. Text from the plugin is escaped, so it cannot add lines or control the terminal. |

`--config` defaults to `{}`. The configuration must be valid JSON, otherwise it is a usage
error.

## `runtime bench <plugin> [--iterations <n>] [--config <json> | --config-file <path>]`

Measures performance for User Story 4. `--iterations` defaults to 1000 and must be above 0.
Plugin logs are not printed. A render that fails stops the bench with its error and exit
status 1.

stdout, one value per line:

```text
load_ms: <compile time of the plugin>
render_p50_us: <instantiate + render, median>
render_p95_us: <instantiate + render, 95th percentile>
render_max_us: <instantiate + render, maximum>
```

## Exit status

| Code | Meaning |
| --- | --- |
| 0 | Success |
| 1 | The plugin was refused, failed or was stopped |
| 2 | Usage error: unknown command, missing file, invalid configuration JSON |

## Error kinds

Every failure names its kind, so the maintainer knows which rule applied without reading
source code (SC-004).

| Kind | When |
| --- | --- |
| `invalid-package` | Not a WebAssembly component, corrupted, or larger than the package limit |
| `contract-mismatch` | Does not export `dashboard:widget/renderer@0.1.0`, or was built for another version of the `dashboard:widget` contract |
| `capability-refused` | Imports anything except `dashboard:widget/log@0.1.0`; the detail lists every refused import |
| `cpu-budget-exceeded` | Ran out of fuel |
| `time-limit-exceeded` | Hit the wall-clock time limit |
| `memory-limit-exceeded` | Tried to grow memory beyond the cap |
| `plugin-crashed` | Any other trap; the detail names it (for example, out-of-bounds memory access) |
| `plugin-error` | `render` returned an error; the detail is the plugin's message |
| `invalid-output` | The widget content is not valid JSON |
| `output-too-large` | The widget content exceeds the size cap |
