//! Hostile plugin: counts its own calls in a static. Every call must see a count of 1, because
//! no state may survive from one call to the next, nor pass between plugins (FR-008).

wit_bindgen::generate!({ path: "../../../wit", world: "widget" });

use std::sync::atomic::{AtomicU32, Ordering};

use exports::dashboard::widget::renderer::Guest;

static CALLS: AtomicU32 = AtomicU32::new(0);

struct StateLeak;

impl Guest for StateLeak {
    fn render(_config: String) -> Result<String, String> {
        let calls = CALLS.fetch_add(1, Ordering::Relaxed) + 1;
        Ok(format!(r#"{{"calls":{calls}}}"#))
    }
}

export!(StateLeak);
