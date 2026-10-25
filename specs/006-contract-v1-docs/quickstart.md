# Quickstart: Contract v1 Closed and Docs for Plugin Authors

How to check feature 006 end to end. Formats follow
[contracts/docs-format.md](contracts/docs-format.md); CLI changes follow
[contracts/cli.md](contracts/cli.md).

## Prerequisites

- Rust 1.99.1 with `wasm32-unknown-unknown`; on Linux `libdbus-1-dev` and `pkg-config`.
- PowerShell 7 on Windows, for the runner's `PS>` blocks.
- Docker, for the agent's walk only.

## 1. The contract

```sh
cargo test -p runtime --test contract_v1   # 1.0.0–1.3.0 pinned; v1 equals its frozen copy
```

Expected: passes; editing one byte of any `wit/history/*/widget.wit`, or of `wit/v1/widget.wit`,
makes it fail, naming the version. `review.md` lists every finding with an outcome, none open.

## 2. The reference

```sh
cargo test -p runtime --test docs_reference
```

Expected: passes. Delete one marker from `docs/authors/reference/contract.md`, or add
`<!-- ref: error no-such-error -->`, or change a limit's default in `limits.md`: each fails,
naming the item.

## 3. The walkthrough and the guides

```sh
cargo xtask plugins --locked
cargo test -p runtime --test docs_walk --locked -- --ignored
```

Expected: every page's blocks run and match, including the connectors and accounts guides
against the test services. Change one command in `getting-started.md` to fail, or one line of an
expected output: the page fails, showing expected and actual lines.

## 4. `pack` on a core module

Follow `docs/authors/getting-started.md` by hand in an empty folder outside the repository.
Expected: `runtime pack` turns the cargo build into a package, and `runtime run` prints the
widget's content after `runtime approve`.

## 5. CI

On the pull request: the `docs` job runs on the three systems in parallel with `build`, and
`CI result` requires it. A commit that changes only `docs/authors/getting-started.md` runs every
job; one that changes only `README.md` skips the build, as before.

## 6. The walks

`walks.md` records the agent's walk in a `rust:1.99.1` container with no stuck point left, and
the maintainer's timed walk on Linux, under 30 minutes without tool downloads.
