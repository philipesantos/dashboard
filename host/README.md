# host

The dashboard's host window: the chrome, the frame around every widget, the panel a frame opens
and the host's dialogs, drawn with GPUI. Design and decisions:
[specs/008-host-window-dialogs](../specs/008-host-window-dialogs/).

## GPUI

GPUI comes from [gpui-unofficial](https://github.com/gpui-archipelago/gpui-unofficial), a third
party's automated republish of Zed's GPUI at Zed's release tags (constitution, Fixed Technical
Decisions). Every crate of the line is a direct dependency of this crate, pinned with `=`.

| Pin | Value |
| --- | --- |
| Version | `1.25.3` |
| Zed tag | `v1.25.3`, commit `8dd255c3eafb5231056fe4e6f601cedd011f1c93` |
| Published | 2026-10-23 |
| Patch list reviewed | `docs/upstream-patches.md` at commit `58b5a3814e` |

`host/tests/checks.rs` fails when a gpui-unofficial package in `Cargo.lock` is not a direct `=`
dependency of this crate, is a pre-release, or differs from the version above.

To upgrade, at most once a month, or at once for a security advisory: take a version published
from a Zed stable tag at least two weeks old; read the patch list's changes since the commit above;
move all nineteen pins together; update this table; let the "Host window" CI job run on the three
systems.
