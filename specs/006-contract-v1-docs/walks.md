# Walks of the docs for plugin authors

SC-001 is checked in three ways:
- CI walks the walkthrough on Windows, macOS and Linux;
- an agent with no context walks it in a clean Linux container (research R16);
- the maintainer walks it once by hand on Linux, timed (research R17).

Each stuck point is fixed in the docs, and the walk is repeated until one has none.

## The agent's walks

| date | docs commit | system | stuck points and fixes |
| --- | --- | --- | --- |

## The maintainer's walk

| date | docs commit | system | time (without tool downloads) | `cargo install` time | stuck points and fixes |
| --- | --- | --- | --- | --- | --- |

## The docs job's time

Measured on the pull request's first run. The goal is under 25 minutes on each system (plan,
Performance Goals).

| run | Linux | macOS | Windows |
| --- | --- | --- | --- |
