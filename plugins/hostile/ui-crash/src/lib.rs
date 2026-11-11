//! Hostile widget for the window probe (specs/009-declarative-widget-ui/research.md, R14): its
//! render traps. The frame must show the host's error state, and the widgets beside it must
//! still draw (FR-005).

wit_bindgen::generate!({ path: "../../../wit/history/1.4.0", world: "widget" });

use exports::dashboard::widget::renderer::Guest;

struct UiCrash;

impl Guest for UiCrash {
    fn render(_config: String) -> Result<String, String> {
        core::arch::wasm32::unreachable()
    }
}

export!(UiCrash);
