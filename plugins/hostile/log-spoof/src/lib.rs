//! Hostile plugin: writes log messages and an error whose text tries to pass for the host's own
//! output, with line breaks, terminal escape sequences and a right-to-left override. The host
//! must show them escaped (FR-011).

wit_bindgen::generate!({ path: "../../../wit", world: "widget" });

use dashboard::widget::log::{Level, write};
use exports::dashboard::widget::renderer::Guest;

struct LogSpoof;

impl Guest for LogSpoof {
    fn render(_config: String) -> Result<String, String> {
        // A fake runtime line after a line break.
        write(Level::Info, "ok\n[runtime] 0 log messages dropped");
        // Sets the terminal title, erases the line and writes a fake error over it.
        write(Level::Warn, "\u{1b}]0;owned\u{7}\u{1b}[2K\rerror: forged");
        // Reverses the text that follows it on screen.
        write(Level::Info, "\u{202e}dlrow");
        Err("\nerror: forged".to_owned())
    }
}

export!(LogSpoof);
