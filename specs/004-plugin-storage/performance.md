# Performance: Plugin Storage

Measurements for SC-006: storage calls add less than 1 ms each on every CI system, and a render
that reads 100 values finishes within render's limits. Also the file a store takes when filled
to its quota, against the file cap of research R10 (research R16).

## How it was measured

- **Run**: [Bench workflow, run 40133415501](https://github.com/philipesantos/dashboard/actions/runs/40133415501),
  2026-10-24, at commit `f14dd96`, triggered by the `bench` label on pull request #4
  (`.github/workflows/bench.yml`). The commits after it change only tests.
- **Build**: release profile (LTO, `codegen-units = 1`), Rust 1.99.1, rusqlite 0.41.0 with its
  bundled SQLite 3.54.0.
- **Systems**: GitHub's standard runners for public repositories: `ubuntu-24.04`,
  `macos-latest` and `windows-latest`, each on its own disk.
- **Plugin**: the `store-probe` fixture, contract 1.2.0, packed by `storage_cost` with a
  manifest that declares storage, approved with its digest in a grant record file. Every
  storage call reads that file again, as in production (research R9).
- **Command**: `cargo run -p dashboard-bench --release --locked --bin storage_cost`, 100 runs
  per median. Per-call costs are differences of medians between runs that differ only in their
  storage calls (research R16), so no timing code enters the host.

Shared runners are noisy, and this is one run per system: read the numbers as an order of
magnitude, which is what the targets need.

## Storage calls

| System | First call of a run | `get` | `set` | Update writing 100 values | Render reading 100 values |
| --- | --- | --- | --- | --- | --- |
| Linux (`ubuntu-24.04`) | 36 µs | 27 µs | 30 µs | 3.1 ms | 3.1 ms |
| macOS (`macos-latest`) | 18 µs | 38 µs | 32 µs | 3.7 ms | 4.1 ms |
| Windows (`windows-latest`) | 190 µs | 220 µs | 224 µs | 20.6 ms | 22.9 ms |

**SC-006 passes on every system.** The slowest call, a `set` on Windows, takes 224 µs, under a
quarter of the 1 ms target; on Linux and macOS a call takes 27–38 µs. Most of each call is the
grant check, which opens and reads the record file under its lock: Windows opens files several
times slower, as feature 003 measured for its connector checks. A render that reads 100 values
takes at most 22.9 ms, about 2% of render's 1 s limit.

An update that writes 100 values and commits them costs about what its 100 writes cost: the
commit itself is under the noise. It cannot be measured against a rollback, which costs about
the same, so the bench measures the whole update beyond one with no storage calls.

## Files at the quota

A store filled to the 8 MiB host cap, as many values per update as the limits allow, until a
write is refused as over quota. The quota came before the file cap in every case, and the sizes
are the same, byte for byte, on the three systems.

| Values | Usage at the quota | File | File ÷ usage | Share of the cap (40.06 MiB) |
| --- | --- | --- | --- | --- |
| 32 B | 8,388,571 bytes | 4,038,656 bytes | 0.5 | 10% |
| 1,000 B | 8,387,920 bytes | 36,593,664 bytes | 4.4 | 87% |
| 4,096 B | 8,385,278 bytes | 9,408,512 bytes | 1.1 | 22% |

Values of about 1,000 bytes are the worst case research R6 measured: each one is just too large
to share a page, so it takes one of its own. Their file, 4.4 times their usage, stays at 87% of
the cap of five times the quota plus 64 KiB. The `many-entries` and `churn` cases of the hostile
suite check the same on every change, 1,500 B values included.

## The limits of research R10

Every limit stands as planned:

| Limit | Value | Measured |
| --- | --- | --- |
| Host cap | 8 MiB | Filled on three systems in seconds; the worst file is 35 MiB |
| Entry cost | 64 B | The `many-entries` case fills 64 KiB of quota with 999 empty values |
| Name, value | 256 B, 256 KiB | The largest entry is kept, one byte more refused (`over-limits`) |
| Calls per run | 1,000 | At about 220 µs on Windows, 1,000 calls take about 0.2 s of an update's 10 s |
| Bytes per update | 1 MiB | An update of 1 MiB spills pages past the 256 KiB cache, which `killed-mid-update` needs |
| List page | 100 names | A render reading 100 values takes at most 22.9 ms |
| File cap | 5 × quota + 64 KiB | The worst file reaches 87% of it, so the quota is what a widget meets |
