# Provenance

Code adapted from [gpui-base](https://crates.io/crates/gpui-base) 0.9.0
(repository `longbridge/gpui-kit`), licensed Apache-2.0; `LICENSE-APACHE` is the crate's own file,
unchanged. gpui-base builds on `gpui-pre` (a snapshot of Zed's main branch); this copy builds on
gpui-unofficial 1.25.3, which has every API it uses.

| File here | Source in gpui-base 0.9.0 | SHA-256 of the source file | What changed |
| --- | --- | --- | --- |
| `focus_trap.rs` | `src/focus_trap.rs` | `10fe3e8516e80bc2948c11ba76f60e9661802dbd0f9baba8dfe1e06635120f08` | The notice added at the top; its test, which used gpui-base's `TestSupportExt`, left out; `init` and `active_focus_trap` made crate-visible. |
| `tab.rs` | `src/root.rs`, lines 11–51 (`actions!`, `advance_focus`, `init`) | `9a32e3bb2668d0ee5a21f8ad22f306a4156446c5540150db625823fc90b95203` | Only the Tab and Shift+Tab parts: the copy binding and the rest of the root left out; the key context renamed `HostRoot`; `advance_focus` public in the crate. |
| `LICENSE-APACHE` | `LICENSE-APACHE` | — | Unchanged. |
