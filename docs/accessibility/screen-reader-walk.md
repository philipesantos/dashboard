# Screen-reader walk of the host's dialogs

A person walks every host dialog with each system's screen reader before a release
(specs/008-host-window-dialogs, FR-026). It adds to the automated checks of the accessibility tree
(`host/tests/window.rs`), which run in CI on every change; it never replaces them. Record each walk
in the log at the end of this file.

## Setup

1. Build the host: `cargo build -p host --release`.
2. Approve two plugins with the development tool, one that renders and one that fails, so the
   dashboard has frames to cover: `cargo xtask plugins`, then
   `cargo run -p runtime --bin runtime -- approve target/plugins/hello-widget.wasm` and the same
   with `target/plugins/out-of-bounds.wasm`.
3. Start the screen reader, then `target/release/dashboard`.

| System | Screen reader | Start it |
| --- | --- | --- |
| macOS | VoiceOver | Command + F5 |
| Windows | Narrator | Windows + Control + Enter |
| Linux (GNOME) | Orca | Super + Alt + S |

## The dialogs

| Dialog | How to open it |
| --- | --- |
| Remove a widget | A frame's menu button, then "Remove from dashboard" |

## The walk, for each dialog

Open it once from the keyboard (Tab to the frame's menu button, Enter, Down or Tab to "Remove
from dashboard", Enter) and once with the pointer.

1. **It announces itself as the host's** (HS-19): the reader says it is a dialog, its name ("Remove
   a widget") and its question in the host's words; the plugin's name comes after them.
2. **Focus starts on Cancel** (HS-16): the reader announces "Cancel" as the focused button, not
   "Remove".
3. **Nothing outside it can be reached** (HS-18): with the reader's own navigation (VoiceOver: VO +
   arrows and the rotor; Narrator: Caps Lock + arrows and scan mode; Orca: flat review and
   structural navigation), try to reach the chrome's "Reload", a frame's label or status, and a
   second dashboard window. Nothing outside the dialog is read.
4. **Tab stays inside** (HS-17): Tab and Shift + Tab, more times than the dialog has buttons, move
   only between Cancel and Remove.
5. **The first second takes nothing** (HS-11): pressing Enter or Space right as it appears does not
   answer it.
6. **Escape refuses** (HS-17): after a second, Escape closes it as a refusal, the frame stays, and
   the reader announces the frame's menu button, where focus returned.
7. **The frames read as the widget's content** (FR-024): with no dialog open, each frame is read
   with its name and its status ("Rendered", or "Did not render" and the host's reason).

A step that fails is a bug for the release: record it below and open an issue before releasing.

## Log

| Date | Release | System | Reader and version | Steps that passed | Notes |
| --- | --- | --- | --- | --- | --- |
