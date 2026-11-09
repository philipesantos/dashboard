//! Pieces every host screen draws with: text in a type level of `DESIGN.md`, and a host control
//! with the host's focus ring (HS-24).

use crate::text::HostText;
use crate::theme::{Theme, Type};
use gpui::{
    App, Div, ElementId, FocusHandle, Hsla, InteractiveElement, IntoElement, KeyDownEvent,
    ParentElement, Role, SharedString, Stateful, StatefulInteractiveElement, Styled, Window, div,
    px, transparent_black,
};

/// Text in a type level, in capitals when the level says so (`label-mono`).
pub fn text(level: Type, color: Hsla, content: impl Into<SharedString>) -> Div {
    let content: SharedString = content.into();
    let content: SharedString = if level.capitals {
        content.to_uppercase().into()
    } else {
        content
    };
    div()
        .font_family(level.family)
        .text_size(level.size)
        .font_weight(level.weight)
        .line_height(level.line_height)
        .text_color(color)
        .child(content)
}

/// The focus ring's width and its distance from the control (HS-24, `contracts/dialog.md`).
pub const RING: f32 = 2.0;

/// A host control: a button-like element the keyboard reaches, named for assistive technology,
/// with the host's focus ring 2 px wide, 2 px outside its face. `face` is what the control shows.
/// Enter and Space press it, as a click does.
pub fn control(
    id: impl Into<ElementId>,
    focus: &FocusHandle,
    theme: &Theme,
    window: &Window,
    name: HostText,
    face: impl IntoElement,
    on_press: impl Fn(&mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let on_press = std::rc::Rc::new(on_press);
    let on_key = on_press.clone();
    let ring = if focus.is_focused(window) {
        theme.focus
    } else {
        transparent_black()
    };
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(SharedString::from(name))
        .track_focus(focus)
        .p(px(RING))
        .border(px(RING))
        .border_color(ring)
        .rounded(theme.radius + px(2.0 * RING))
        .on_click(move |_, window, cx| on_press(window, cx))
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if matches!(event.keystroke.key.as_str(), "enter" | "space") {
                on_key(window, cx);
                cx.stop_propagation();
            }
        })
        .child(face)
}
