//! Hostile plugin: writes one log message, then loops forever. The CPU budget or the time limit
//! must stop it, and the log message must survive the error.

wit_bindgen::generate!({ path: "../../../wit", world: "widget" });

use dashboard::widget::log::{Level, write};
use exports::dashboard::widget::renderer::Guest;

struct InfiniteLoop;

impl Guest for InfiniteLoop {
    fn render(_config: String) -> Result<String, String> {
        write(Level::Info, "looping");
        loop {
            std::hint::black_box(());
        }
    }
}

export!(InfiniteLoop);
