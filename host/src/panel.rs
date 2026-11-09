//! A frame's menu and the panel a frame opens (FR-010, research R6). Both are drawn in the
//! content area's own flow, never in a floating layer: the menu inside its frame under the
//! header, the panel attached to its frame's side, or under it when there is no room. Neither
//! dims or blocks anything; Escape or a click outside closes it, the click still reaching what it
//! was aimed at, and focus goes back to the control that opened it.

use crate::text::{HostText, Msg};
use crate::theme::{Theme, body_md, label_mono, title_sm};
use crate::ui::{control, text};
use gpui::{
    App, Div, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, ParentElement, Role,
    SharedString, Stateful, StatefulInteractiveElement, Styled, Window, div, px,
};

/// The panel's width, beside a frame.
pub const WIDTH: f32 = 280.0;

/// One entry of a frame's menu.
pub struct MenuItem {
    pub name: HostText,
    pub focus: FocusHandle,
    pub on_press: Box<dyn Fn(&mut Window, &mut App)>,
}

/// A frame's menu, inside the frame under its header. `on_close` runs on Escape and on a click
/// outside it.
pub fn menu(
    theme: &Theme,
    window: &Window,
    index: usize,
    items: Vec<MenuItem>,
    on_close: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let on_close = std::rc::Rc::new(on_close);
    let on_escape = on_close.clone();
    div()
        .id(("frame-menu-list", index))
        .debug_selector(move || format!("menu-{index}"))
        .role(Role::Menu)
        .aria_label(SharedString::from(Msg::WidgetMenu.text()))
        .flex()
        .flex_col()
        .gap(px(2.0))
        .p(px(6.0))
        .border_b(px(1.0))
        .border_color(theme.divider)
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if event.keystroke.key == "escape" {
                on_escape(window, cx);
                cx.stop_propagation();
            }
        })
        .on_mouse_down_out(move |_, window, cx| on_close(window, cx))
        .children(items.into_iter().enumerate().map(|(item, entry)| {
            control(
                ("frame-menu-item", index * 16 + item),
                &entry.focus,
                theme,
                window,
                entry.name,
                div()
                    .px(px(6.0))
                    .py(px(2.0))
                    .child(text(label_mono(), theme.text, entry.name)),
                entry.on_press,
            )
        }))
}

/// The panel of frame `index`: the host's title and, until declared settings exist, the host's
/// note that they will appear here.
pub fn panel(
    theme: &Theme,
    window: &Window,
    index: usize,
    close: &FocusHandle,
    on_close: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let on_close = std::rc::Rc::new(on_close);
    let on_escape = on_close.clone();
    let on_outside = on_close.clone();
    let on_button = on_close.clone();
    div()
        .id(("panel", index))
        .debug_selector(move || format!("panel-{index}"))
        .role(Role::Group)
        .aria_label(SharedString::from(Msg::WidgetSettings.text()))
        .w(px(WIDTH))
        .flex_none()
        .flex()
        .flex_col()
        .gap(px(8.0))
        .p(px(16.0))
        .bg(theme.panel)
        .border(px(1.0))
        .border_color(theme.divider)
        .rounded(theme.radius)
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if event.keystroke.key == "escape" {
                on_escape(window, cx);
                cx.stop_propagation();
            }
        })
        .on_mouse_down_out(move |_, window, cx| on_outside(window, cx))
        .child(text(title_sm(), theme.text, Msg::WidgetSettings.text()))
        .child(text(
            body_md(),
            theme.text_secondary,
            Msg::PanelEmpty.text(),
        ))
        .child(
            div().flex().child(control(
                ("panel-close", index),
                close,
                theme,
                window,
                Msg::Close.text(),
                div()
                    .px(px(8.0))
                    .py(px(2.0))
                    .rounded(theme.radius)
                    .bg(theme.neutral_soft)
                    .border(px(1.0))
                    .border_color(theme.border_control)
                    .child(text(label_mono(), theme.text, Msg::Close.text())),
                move |window, cx| on_button(window, cx),
            )),
        )
}

/// A frame with its open panel attached: side by side, or the panel under the frame when the row
/// has no room.
pub fn attached(frame: impl IntoElement, panel: impl IntoElement) -> Div {
    div()
        .flex()
        .flex_wrap()
        .items_start()
        .child(frame)
        .child(panel)
}
