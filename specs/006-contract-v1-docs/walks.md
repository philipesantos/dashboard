# Walks of the docs for plugin authors

SC-001 is checked in three ways:
- CI walks the walkthrough on Windows, macOS and Linux;
- an agent with no context walks it in a clean Linux container (research R16);
- the maintainer walks it once by hand on Linux, timed (research R17).

Each stuck point is fixed in the docs, and the walk is repeated until one has none.

## The agent's walks

| date | docs commit | system | stuck points and fixes |
| --- | --- | --- | --- |
| 2026-10-26 | 1f817d4 | `rust:1.99.1` (Debian 13), as root | Every command and output matched; the widget ran in about 7 minutes, 5 of them `cargo install`. Six points, none blocking, fixed in `getting-started.md`: S1 `apt-get install` fails on a system whose package lists were never fetched (adds `apt-get update`); S2 nothing says to make 1.99.1 the default toolchain (says so); S3 the `login providers` line of `permissions` is never explained (explained); S4 the newer-contract section ends without a command and leaves `wit/widget.wit.bak` (a block copies the contract back and removes it); S5 "the approval belongs to the package's file" said twice and unclear about paths (said once: the path); S6 step 3 depends on running beside the clone (says so). The harness's `bash -lc` lost the image's `PATH`; later walks use `bash -c` |

## The maintainer's walk

| date | docs commit | system | time (without tool downloads) | `cargo install` time | stuck points and fixes |
| --- | --- | --- | --- | --- | --- |

## The docs job's time

Measured on the pull request's first run. The goal is under 25 minutes on each system (plan,
Performance Goals).

| run | Linux | macOS | Windows |
| --- | --- | --- | --- |
