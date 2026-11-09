# Contract: bans and checks that fail the build

## `clippy.toml` (workspace)

`disallowed-methods`, each with the reason "the host never shows a native prompt (Principle VII,
HS-12)" or "no plugin string reaches the image loader or URL opener (FR-029)":

- `gpui::Window::prompt`, `gpui::AsyncWindowContext::prompt`, `gpui::PlatformWindow::prompt`
- `gpui::App::open_url`, `gpui::Platform::open_url`, `gpui::App::open_with_system`,
  `gpui::App::reveal_path`, `gpui::App::prompt_for_paths`, `gpui::App::prompt_for_new_path`
- `gpui::App::write_credentials`, `gpui::App::read_credentials`, `gpui::App::delete_credentials`
- `gpui::img`, allowed only in `host/src/media.rs` with an inner `#[allow]` and its reason

The exact paths are confirmed by the first task (research R15); each entry is proved by a file
that calls it and must fail clippy.

## Tests in `host/tests/checks.rs`

| Check | Fails when |
| --- | --- |
| GPUI pins | a `*gpui-unofficial*` package in `Cargo.lock` is not a direct `=` dependency of `host`, is a pre-release, or differs from the version in `host/README.md` |
| clippy bans | an entry above is missing from `clippy.toml` |
| host words | a string literal is passed to `child`, `aria_label`, `aria_description` or a text element in `host/src/` outside `text.rs` |
| HTTP client | the app's client answers a request instead of refusing it |
| prompt builder | GPUI's prompt builder is not the host's refusing one |
| deny wrappers | `deny.toml` stops limiting `host-surfaces` to `host` or stops banning `rfd` and `native-dialog` |

## Hostile cases (`host/tests/hostile/`, FR-031)

| Case | Rule |
| --- | --- |
| a plugin whose render output is markup, `https://` and `file://` addresses and paths | FR-027, FR-029: the frame holds only host text and the safe label; no request, no file read |
| an image element from a URI string, drawn in a test harness with a recording client | FR-030: no request reaches the client |
| a plugin whose name is long, empty, mixes scripts, carries bidi and control characters | FR-007 (HS-06, HS-07 rules) |
| a plugin that panics, traps or exceeds its limits in render | FR-009: the window keeps drawing and taking input |
| a dialog answered by input at 0, 500, 999 ms and by a held key | FR-017 |
| input sent to the chrome, frames, a panel and a second window while a dialog is open | FR-016 |
| the accessibility tree read while a dialog is open | FR-022 |
