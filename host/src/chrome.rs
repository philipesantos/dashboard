//! The chrome: the host's own bar at the top of every host window, holding its controls. No frame
//! or panel covers it; only a host dialog's scrim does (HS-01, HS-02). The system's title bar
//! and window controls stay above it (research R6).

use crate::text::Msg;
use crate::theme::{Theme, label_md, label_mono};
use crate::ui::{control, text};
use gpui::{App, Div, FocusHandle, InteractiveElement, ParentElement, Styled, Window, div, px};

/// The chrome's height: `DESIGN.md`'s `components.chrome`.
pub const HEIGHT: f32 = 40.0;

/// The chrome's controls and what pressing them does.
pub struct Controls<R, A> {
    pub appearance: FocusHandle,
    pub on_appearance: A,
    pub reload: FocusHandle,
    pub on_reload: R,
}

/// A button of the chrome: its name on a soft face.
fn button(theme: &Theme, name: Msg) -> Div {
    div()
        .px(px(8.0))
        .py(px(2.0))
        .rounded(theme.radius)
        .bg(theme.neutral_soft)
        .border(px(1.0))
        .border_color(theme.border_control)
        .child(text(label_mono(), theme.text, name.text()))
}

/// The chrome, with its "Appearance" and "Reload" controls.
pub fn chrome<R, A>(theme: &Theme, window: &Window, controls: Controls<R, A>) -> Div
where
    R: Fn(&mut Window, &mut App) + 'static,
    A: Fn(&mut Window, &mut App) + 'static,
{
    div()
        .debug_selector(|| "chrome".to_owned())
        .flex_none()
        .h(px(HEIGHT))
        .px(px(12.0))
        .flex()
        .items_center()
        .gap(px(8.0))
        .bg(theme.chrome)
        .border_b(px(1.0))
        .border_color(theme.divider)
        .child(text(label_md(), theme.chrome_text, Msg::AppName.text()))
        .child(div().flex_1())
        .child(
            control(
                "chrome-appearance",
                &controls.appearance,
                theme,
                window,
                Msg::Appearance.text(),
                button(theme, Msg::Appearance),
                controls.on_appearance,
            )
            .debug_selector(|| "appearance-button".to_owned()),
        )
        .child(
            control(
                "chrome-reload",
                &controls.reload,
                theme,
                window,
                Msg::Reload.text(),
                button(theme, Msg::Reload),
                controls.on_reload,
            )
            .debug_selector(|| "reload".to_owned()),
        )
}
