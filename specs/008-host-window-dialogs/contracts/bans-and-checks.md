# Contract: bans and checks that fail the build

## `clippy.toml` (workspace)

`disallowed-methods`, each with the reason "the host never shows a native prompt (Principle VII,
HS-12)" or "no plugin string reaches the image loader or URL opener (FR-029)":

- `gpui::Window::prompt`, `gpui::AsyncWindowContext::prompt`, `gpui::PlatformWindow::prompt`
- `gpui::App::open_url`, `gpui::Platform::open_url`, `gpui::App::open_with_system`,
  `gpui::App::reveal_path`, `gpui::App::prompt_for_paths`, `gpui::App::prompt_for_new_path`
- `gpui::Platform::prompt_for_paths`, `gpui::Platform::prompt_for_new_path`,
  `gpui::Platform::open_with_system`, `gpui::Platform::reveal_path`: the same through the platform
  object `gpui_platform::current_platform()` returns
- `gpui::App::write_credentials`, `gpui::App::read_credentials`, `gpui::App::delete_credentials`,
  and the same three on `gpui::Platform`
- `gpui::img`, `gpui::svg`, `gpui::Svg::external_path`, `gpui::Window::use_asset`,
  `gpui::Window::get_asset`, `gpui::App::fetch_asset`, `gpui::ImageCacheItem::new`,
  `gpui::ImageCache::load`, `gpui::RetainAllImageCache::load`, `gpui::AnyImageCache::load`: allowed
  only in `host/src/media.rs`, with an inner `#[allow]` and its reason; `media` takes only bytes
  the host holds

The exact paths are confirmed by the first task (research R15); each entry is proved by
`host/ban-fixture`, a crate outside the workspace that calls every one: the "Host window" CI job
requires clippy to fail on it with exactly one "disallowed method" error per entry.

## Tests in `host/tests/checks.rs`

| Check | Fails when |
| --- | --- |
| GPUI pins | a `*gpui-unofficial*` package in `Cargo.lock` is not a direct `=` dependency of `host`, is a pre-release, or differs from the version in `host/README.md`, or `host/ban-fixture` pins another; mutations of each kind must fail it |
| clippy bans | an entry above is missing from `clippy.toml` |
| no way around them | `allow(clippy::disallowed_methods)` or `allow(clippy::disallowed_types)` appears in `host/src/` outside `media.rs` |
| no floating layer | `deferred(` or `anchored(` appears in `host/src/` (research R6) |
| host words | a string literal is passed to `child`, `aria_label`, `aria_description`, a tooltip, the window title (`TitlebarOptions`) or a text element in `host/src/` outside `text.rs` |
| development mode | `development(` appears in `host/src/` |
| deny wrappers | `deny.toml` stops limiting `host-surfaces` to `host` or stops banning `rfd` and `native-dialog` |

## Hostile cases (FR-031)

Headless ones in `host/tests/hostile.rs`; those that need a real window in `host/tests/window.rs`; all
counted in the security suite.

| Case | Rule |
| --- | --- |
| a plugin whose render output is markup, `https://` and `file://` addresses and paths | FR-027, FR-029: the frame holds only host text and the safe label; no request, no file read |
| a request through the app's HTTP client, to a remote, a loopback and a `file://` address | FR-030: each is refused |
| a native prompt asked of GPUI | FR-019: the host's builder refuses it at once, showing nothing |
| an image element from a URI string, an SVG from an external path, an asset fetch | FR-028, FR-029: none can be built in `host/src/` outside `media.rs`; `host/ban-fixture` proves clippy refuses each call |
| a plugin whose render error and manifest names imitate host text ("Remove a widget", "Rendered") | FR-008: the frame shows only the host's words for the error kind |
| a plugin that declares a `local/` connector | FR-041: it does not load; the frame says why in host words |
| a plugin whose name is long, empty, mixes scripts, carries bidi and control characters | FR-007 (HS-06, HS-07 rules) |
| a plugin that panics, traps or exceeds its limits in render | FR-009: the window keeps drawing and taking input |
| a dialog answered by input at 0, 500, 999 ms and by a held key | FR-017 |
| input sent to the chrome, frames, a panel and a second window while a dialog is open | FR-016 |
| the window's pixels while a dialog is open (`window` target) | FR-015: every pixel of every host window, chrome included, is under the scrim |
| the accessibility tree of the dialog's window and a second window while it is open (`window` target) | FR-022 |
