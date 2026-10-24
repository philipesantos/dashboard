# Performance: Plugin Connectors

Measurements for SC-007: checking a widget's connectors when it loads adds less than 10 ms on
every CI system. Also for SC-004: a connected widget renders from its kept data in under 1 s.
And the limits of research R10, as the tests measure them (research R19).

## How it was measured

- **Run**: [Bench workflow, run 40088708812](https://github.com/philipesantos/dashboard/actions/runs/40088708812),
  2026-10-24, at commit `d291846`, triggered by the `bench` label on pull request #3
  (`.github/workflows/bench.yml`).
- **Build**: release profile (LTO, `codegen-units = 1`), Wasmtime 50 with parallel
  compilation, Rust 1.99.1, as in [feature 001](../001-plugin-runtime/performance.md).
- **Systems**: GitHub's standard runners for public repositories: `ubuntu-24.04`,
  `macos-latest` and `windows-latest`.
- **Plugins**:
  - `hello-widget` 0.2.0, contract v1 (1.0.0), with `log` required, as in feature 002.
  - `github-releases` 0.1.0, contract v1 (1.1.0), which declares `catalog/github` 1. It was
    approved with the digest `permissions` printed (`approve --expect`), in a grant record
    under the runner's temporary directory.
- **Commands**:
  - `runtime bench <plugin> --iterations 1000` for each plugin. For `github-releases` it adds
    `--config '{"owner":"rust-lang","repo":"rust"}' --kept-data
    plugins/examples/github-releases/bench-data.json` (five releases, 773 bytes).
  - `dashboard-bench`'s `compile_cost` for both package shapes.
- **Network**: nothing reached the network. The bench ran no update, and the kept data is held
  in memory.

Shared runners are noisy, and this is one run per system: read the numbers as an order of
magnitude, which is what the targets need.

## The connector checks

`checks_us` is the median of 100 runs (`contracts/cli.md`). It covers every check of a load
except the compile and the linking:
- the manifest section and the contract and import checks;
- for each connector the manifest declares: finding it in the catalog, or among the local
  connectors, and checking it against the manifest;
- reading the grant record to decide what to link, including the connector approvals.

`relink_us` is linking again after a grant change, including the connector table.

| System | `github-releases`: `checks_us` | `relink_us` | `hello-widget`: `checks_us` | `relink_us` |
| --- | --- | --- | --- | --- |
| Linux (`ubuntu-24.04`) | 23 µs | 26 µs | 9 µs | 5 µs |
| macOS (`macos-latest`) | 41 µs | 72 µs | 35 µs | 21 µs |
| Windows (`windows-latest`) | 142 µs | 162 µs | 65 µs | 57 µs |
| Reference machine (Linux, 8 cores) | 53–66 µs | — | — | — |

**SC-007 passes on every system.** The slowest, Windows, takes 142 µs to check
`github-releases` and its connector, about 70 times under the 10 ms target. A widget without
connectors checks as fast as in feature 002 (20–114 µs then, 9–65 µs now).

## Rendering from kept data

`render_with_data_us` is the median of 100 renders of `github-releases` from the kept data.
Each render gets the five releases, the time they were fetched and the last outcome. No update
runs and no request is made. That a render never waits on a slow service is tested apart, in
`runtime/tests/updates.rs` (`a_slow_service_never_slows_a_render`).

| System | `render_with_data_us` |
| --- | --- |
| Linux | 70 µs |
| macOS | 66 µs |
| Windows | 102 µs |
| Reference machine | 101–111 µs |

**SC-004 passes on every system**: the slowest render from kept data, 102 µs on Windows, is
about 10,000 times under 1 s.

## Loading and rendering

The render columns are the 1000 renders of `bench`. For `github-releases` there is no kept data
in that loop, so they render its "No data yet" state. Loading a widget with a connector costs a
few milliseconds more than `hello-widget`. Feature 001's targets still hold: load under 100 ms,
render p50 under 200 µs and p95 under 1 ms.

| System | Plugin | `load_ms` | render p50 | render p95 | render max |
| --- | --- | --- | --- | --- | --- |
| Linux | `hello-widget` | 29.1 ms | 33 µs | 65 µs | 662 µs |
| Linux | `github-releases` | 39.6 ms | 66 µs | 114 µs | 1,081 µs |
| macOS | `hello-widget` | 34.6 ms | 29 µs | 61 µs | 212 µs |
| macOS | `github-releases` | 39.1 ms | 93 µs | 142 µs | 536 µs |
| Windows | `hello-widget` | 52.3 ms | 59 µs | 67 µs | 234 µs |
| Windows | `github-releases` | 84.4 ms | 188 µs | 285 µs | 591 µs |

Feature 002's run, for `hello-widget`: 42.8, 46.3 and 48.0 ms to load, and 75, 40 and 53 µs for
the p50 (Linux, macOS, Windows).

## Loading a package near the size cap

Both `compile_cost` packages still stop at `contract-mismatch` after the compile, since they
export no `renderer`. Feature 001's target still holds: under 2 s and 1 GiB.

| System | `many` (9.60 MiB): load | `huge` (7.08 MiB): load | Peak memory during the load |
| --- | --- | --- | --- |
| Linux | 386 ms | 193 ms | 596 MB (`many`), 132 MB (`huge`) |
| macOS | 572 ms | 324 ms | not reported |
| Windows | 717 ms | 298 ms | not reported |

## The limits of research R10

The limits keep the values of R10 (`Limits::default()`, checked in `runtime/src/limits.rs`).
Each one is exercised by a test that passed on all three systems in
[CI run 40082922654](https://github.com/philipesantos/dashboard/actions/runs/40082922654), at
the same commit. Where a limit is a time or a rate, the test lowers it so the suite stays quick.
The rule it checks is the same.

| Limit | Value | Measured by | Value in the test |
| --- | --- | --- | --- |
| Update wall-clock time | 10 s | `an_update_past_its_time_limit_is_stopped_and_its_calls_abandoned`: stopped in under 4 s, its render still shows "No data yet" | 2 s |
| A service slower than render | — | `a_service_slower_than_render_allows_still_completes_within_the_update`: a 3 s answer completes | default |
| One call's time | 5 s | `a_slow_service_fails_at_the_calls_time_limit` and `over_limits`: a 3 s answer fails as `over-limit` in under 1.5 s | 1 s |
| Calls per run | 20 | `over_limits`: the 21st valid call is refused. `log_flood`: 10,000 calls in one update and one render, past 20 counted in one summary entry each | default |
| Response body per call | 1 MiB | `over_limits` and `network.rs`: exactly 1 MiB passes, 1 MiB + 1 byte fails, and the widget gets no part of it | default |
| Response bodies per update | 4 MiB | `over_limits`: four bodies of 1 MiB pass, the fifth fails | default |
| Response headers per call | 16 KiB | `over_limits`: a 17 KiB header fails | default |
| Request address | 8 KiB | `over_limits`: refused before sending, and the server receives nothing | default |
| Request body | 64 KiB | `over_limits`: refused before sending, and the server receives nothing | default |
| Redirects per call | 5 | `over_limits` and `past_five_redirects_the_host_follows_no_more`: the first request and five redirects reach the server, then `redirect-not-allowed` | default |
| Update output | 256 KiB | `over_limits`: one byte over fails the update with `output-too-large`, and the previous data stays | default |
| Rate per widget | 30 per minute | `over_limits_rates`: the burst past the bucket is refused | 5 |
| Rate per destination host | 60 per minute | `over_limits_rates`: two widgets share the bucket | 8 |
| Request log | 2 MiB, plus one previous file | `log_complete`: the log rotates into `requests.1.jsonl`, neither file passes the limit, and the oldest entries go first | 4 KiB |
| Parameter value in the log | 256 characters | The unit tests of `runtime/src/requests.rs`: cut at 256 characters, then ` [truncated]` | default |

No limit needed to change. The update's fuel and memory are the render's (500 M units,
32 MiB). Four bodies of 1 MiB passed inside that memory on every system.
