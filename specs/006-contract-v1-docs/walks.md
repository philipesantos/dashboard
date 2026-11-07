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
| 2026-10-26 | 04cbe0b | `rust:1.99.1` (Debian 13), as root, a new agent | Every command and output matched; about 7½ minutes, 5 of them `cargo install`. S1–S6 gone. Five points, none blocking, fixed: the rustup commands in an order that adds the target to another toolchain when the default is not 1.99.1 (default first); `apt-get install` asks for confirmation (`-y`); the log line and the JSON are on stderr and stdout (said); the newer-contract section did not say the widget is back as it was (said); "When something goes wrong" did not say whether to run it (optional, and which files it leaves) |
| 2026-10-26 | 8591b88 | `rust:1.99.1` (Debian 13), as root, a new agent | None: every command and output matched, the optional section included; about 7 minutes, 5m20s of them `cargo install`, under 2 minutes by hand at an agent's pace. Nits left as they are: the closing paragraph names no command (the step 5 and 6 commands, unchanged), the cleanup list does not name `target/wasm32-wasip2/`, and "about half an hour" is generous for an agent |

## The maintainer's walk

| date | docs commit | system | time (without tool downloads) | `cargo install` time | stuck points and fixes |
| --- | --- | --- | --- | --- | --- |
| 2026-11-07 | c9bcbc2 | Ubuntu (the maintainer's machine), by hand | 10 minutes, `cargo install` included | Not timed apart; inside the 10 minutes | None |

## The docs job's time

Measured on the pull request's first run. The goal is under 25 minutes on each system (plan,
Performance Goals).

| run | Linux | macOS | Windows |
| --- | --- | --- | --- |
| [42410269814](https://github.com/philipesantos/dashboard/actions/runs/42410269814), PR #11 at f574207 | 6m50s | 7m54s | 11m49s |
