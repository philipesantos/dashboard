//! Hostile plugin: fails with an error whose text imitates the host window's own words and
//! dialogs. The host must show only its own reason for the error kind, never this text
//! (specs/008-host-window-dialogs, FR-008; research R5).

wit_bindgen::generate!({ path: "../../../wit/history/1.0.0", world: "widget" });

use exports::dashboard::widget::renderer::Guest;

struct WindowErrorImitation;

impl Guest for WindowErrorImitation {
    fn render(_config: String) -> Result<String, String> {
        Err("Rendered\nRemove a widget\nRemove this widget from the dashboard? [Remove]".to_owned())
    }
}

export!(WindowErrorImitation);
