//! The chrome: the host's own bar at the top of every host window, holding its menus. No frame
//! or panel covers it; only a host dialog's scrim does (HS-01, HS-02). The system's title bar
//! and window controls stay above it (research R6).

use crate::text::Msg;
use crate::theme::{Theme, label_md, label_mono};
use crate::ui::{control, text};
use gpui::{App, Div, FocusHandle, InteractiveElement, ParentElement, Styled, Window, div, px};

/// The chrome's height: `DESIGN.md`'s `components.chrome`.
pub const HEIGHT: f32 = 40.0;

/// The chrome, with its "Reload" control.
pub fn chrome(
    theme: &Theme,
    window: &Window,
    reload: &FocusHandle,
    on_reload: impl Fn(&mut Window, &mut App) + 'static,
) -> Div {
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
        .child(control(
            "chrome-reload",
            reload,
            theme,
            window,
            Msg::Reload.text(),
            div()
                .px(px(8.0))
                .py(px(2.0))
                .rounded(theme.radius)
                .bg(theme.neutral_soft)
                .border(px(1.0))
                .border_color(theme.border_control)
                .child(text(label_mono(), theme.text, Msg::Reload.text())),
            on_reload,
        ))
}
