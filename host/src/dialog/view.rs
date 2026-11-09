//! What a host window draws while a dialog is open (HS-02, contracts/dialog.md): a scrim over the
//! whole window, chrome included, that takes every pointer event, and, in the dialog's window, the
//! dialog above it, drawn only with the host's components and host-surface tokens.

use super::{Answer, Request};
use crate::a11y::Modality;
use crate::label::Label;
use crate::text::HostText;
use crate::theme::{Theme, body_md, label_mono, title_md};
use crate::ui::{RING, text};
use crate::vendor::gpui_base::FocusTrapElement;
use gpui::{
    AnyElement, Div, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent, MouseButton,
    ParentElement, Role, SharedString, Stateful, StatefulInteractiveElement, Styled, Window, div,
    px, transparent_black,
};

/// The dialog's width: `DESIGN.md`'s `components.dialog`.
pub const WIDTH: f32 = 480.0;

/// What the view needs of the open dialog.
#[derive(Clone)]
pub struct Shown {
    pub request: Request,
    pub refuse: FocusHandle,
    pub confirm: FocusHandle,
    pub trap: FocusHandle,
    /// Whether the dialog is drawn in this window, or only its scrim.
    pub here: bool,
}

fn button(
    id: &'static str,
    focus: &FocusHandle,
    theme: &Theme,
    window: &Window,
    name: HostText,
    inked: bool,
    answer: Answer,
) -> Stateful<Div> {
    let ring = if focus.is_focused(window) {
        theme.focus
    } else {
        transparent_black()
    };
    // The confirming button in the host's ink, never the widgets' color: the dialog is the
    // host's, and nothing in it borrows what widgets draw with
    let (face, ink, edge) = if inked {
        (theme.ink, theme.on_ink, theme.ink)
    } else {
        (theme.neutral_soft, theme.text, theme.border_control)
    };
    div()
        .id(id)
        .debug_selector(move || id.to_owned())
        .role(Role::Button)
        .aria_label(SharedString::from(name))
        .track_focus(focus)
        .p(px(RING))
        .border(px(RING))
        .border_color(ring)
        .rounded(theme.radius + px(2.0 * RING))
        .on_mouse_down(MouseButton::Left, |_, _, cx| super::pointer_down(cx))
        .on_click(move |event, _, cx| {
            // Enter and Space answer only through `key_press`, which applies the gate's held
            // keys; GPUI also turns their release into a click, which is ignored here
            if !matches!(event, gpui::ClickEvent::Keyboard(_)) {
                super::click(cx, answer);
            }
        })
        .on_key_down(move |event: &KeyDownEvent, _, cx| {
            let key = event.keystroke.key.as_str();
            if matches!(key, "enter" | "space") {
                super::key_press(cx, key, event.is_held, answer);
                cx.stop_propagation();
            }
        })
        .child(
            div()
                .px(px(12.0))
                .py(px(4.0))
                .rounded(theme.radius)
                .bg(face)
                .border(px(1.0))
                .border_color(edge)
                .child(text(label_mono(), ink, name)),
        )
}

fn dialog(theme: &Theme, window: &Window, shown: &Shown) -> impl IntoElement {
    let request = &shown.request;
    let subject = request.subject.as_ref().map(|label| {
        let name = match label {
            Label::Plugin(text) => SharedString::from(text.as_str().to_owned()),
            Label::Unnamed(text) => SharedString::from(*text),
        };
        div()
            .id("dialog-subject")
            .role(Role::Label)
            .aria_label(name.clone())
            // AccessKit names a label from its value on Windows
            .aria_value(name.clone())
            .child(text(label_mono(), theme.text_secondary, name))
    });
    div()
        .id("dialog")
        .debug_selector(|| "dialog".to_owned())
        .role(Role::Dialog)
        .modal()
        .aria_label(SharedString::from(request.name))
        .aria_description(SharedString::from(request.question))
        .w(px(WIDTH))
        .p(px(24.0))
        .flex()
        .flex_col()
        .gap(px(12.0))
        .bg(theme.dialog)
        .border(px(1.0))
        .border_color(theme.divider)
        .rounded(theme.radius)
        .on_key_down(|event: &KeyDownEvent, _, cx| {
            if event.keystroke.key == "escape" {
                super::key_press(cx, "escape", event.is_held, Answer::Refused);
                cx.stop_propagation();
            }
        })
        .child(text(title_md(), theme.text, request.name))
        .child(text(body_md(), theme.text, request.question))
        .children(subject)
        .child(
            div()
                .flex()
                .justify_end()
                .gap(px(8.0))
                .child(button(
                    "dialog-refuse",
                    &shown.refuse,
                    theme,
                    window,
                    request.refuse,
                    false,
                    Answer::Refused,
                ))
                .child(button(
                    "dialog-confirm",
                    &shown.confirm,
                    theme,
                    window,
                    request.confirm,
                    true,
                    Answer::Confirmed,
                )),
        )
        .focus_trap("dialog-trap", &shown.trap)
}

/// The scrim over the whole window, and the dialog when it is drawn here.
pub fn overlay(theme: &Theme, window: &Window, shown: &Shown) -> AnyElement {
    div()
        .id("scrim")
        .debug_selector(|| "scrim".to_owned())
        .absolute()
        .top_0()
        .left_0()
        .size_full()
        .occlude()
        .bg(theme.scrim)
        .flex()
        .items_center()
        .justify_center()
        .children(shown.here.then(|| dialog(theme, window, shown)))
        .into_any_element()
}
