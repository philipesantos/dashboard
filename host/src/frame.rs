//! The frame the host draws around every widget (FR-007, FR-008): the plugin's monogram and
//! label, its status in the host's words, and the menu the frame opens. None of the plugin's
//! content is drawn in this feature.

use crate::label::Label;
use crate::media::status_icon;
use crate::plugins::{FrameStatus, Widget};
use crate::text::Msg;
use crate::theme::{Theme, caption, label_mono, label_sm};
use crate::ui::{control, text};
use design_tokens::Icon;
use gpui::{
    AnyElement, App, Div, FocusHandle, InteractiveElement, ParentElement, Role, SharedString,
    Stateful, StatefulInteractiveElement, Styled, Window, div, px,
};

/// A frame's width while frames are laid out in fixed columns, before the dashboard grid.
pub const WIDTH: f32 = 320.0;

fn status(theme: &Theme, widget: &Widget) -> Div {
    let row = div().flex().items_center().gap(px(6.0));
    match widget.status {
        FrameStatus::Loading => div().child(row.child(text(
            label_mono(),
            theme.text_secondary,
            Msg::Loading.text(),
        ))),
        FrameStatus::Rendered => div().child(
            row.child(status_icon(Icon::Check, theme.ok_icon, px(14.0)))
                .child(text(label_mono(), theme.text, Msg::Rendered.text())),
        ),
        FrameStatus::Failed(reason) => div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(
                row.child(status_icon(Icon::Cross, theme.error_icon, px(14.0)))
                    .child(text(label_mono(), theme.text, Msg::DidNotRender.text())),
            )
            .child(text(caption(), theme.text_secondary, reason.text())),
    }
}

/// One widget's frame. `menu` is the focus handle of its menu control; `open_menu`, when the
/// menu is open, is drawn inside the frame under the header (research R6).
pub fn frame(
    theme: &Theme,
    window: &Window,
    index: usize,
    widget: &Widget,
    menu: &FocusHandle,
    on_menu: impl Fn(&mut Window, &mut App) + 'static,
    open_menu: Option<AnyElement>,
) -> Stateful<Div> {
    let label = match &widget.label {
        Label::Plugin(text) => SharedString::from(text.as_str().to_owned()),
        Label::Unnamed(text) => SharedString::from(*text),
    };
    let header = div()
        .flex()
        .items_center()
        .gap(px(8.0))
        .h(px(36.0))
        .px(px(8.0))
        .border_b(px(1.0))
        .border_color(theme.divider)
        .child(
            div()
                .size(px(20.0))
                .flex_none()
                .rounded(Theme::full_radius())
                .bg(theme.ink)
                .flex()
                .items_center()
                .justify_center()
                .child(text(label_sm(), theme.on_ink, widget.monogram.clone())),
        )
        .child(text(label_mono(), theme.text_secondary, label.clone()).flex_1())
        .child(control(
            ("frame-menu", index),
            menu,
            theme,
            window,
            Msg::WidgetMenu.text(),
            div()
                .size(px(24.0))
                .flex()
                .items_center()
                .justify_center()
                .child(text(label_mono(), theme.text, Msg::MenuGlyph.text())),
            on_menu,
        ));
    div()
        .id(("frame", index))
        .debug_selector(move || format!("frame-{index}"))
        .role(Role::Group)
        .aria_label(label)
        .aria_description(SharedString::from(widget.id.as_str().to_owned()))
        .w(px(WIDTH))
        .flex_none()
        .bg(theme.surface)
        .border(px(1.0))
        .border_color(theme.divider)
        .rounded(theme.radius)
        .child(header)
        .children(open_menu)
        .child(div().p(px(12.0)).child(status(theme, widget)))
}
