//! Draws validated widget content with the host's components, built on gpui-base.
//!
//! Every element here is laid out in normal flow: nothing is positioned absolutely, deferred or
//! anchored, so nothing can paint outside the box the host gives the content, and the host
//! clips that box. Colors come only from the [`WidgetTheme`] passed in.

use std::rc::Rc;

use gpui_kit::base::{
    Button, Table, TableBody, TableCell, TableHead, TableHeader, TableRow, TestSupportExt as _,
};
use gpui_kit::prelude::*;
use gpui_kit::{AnyElement, App, ElementId, FontWeight, Role, SharedString, Text, Window, div, px};

use crate::model::{Block, Content, TextStyle, Tone};
use crate::tokens::WidgetTheme;

/// Called with a button's action when the person presses it. The host decides what it means.
pub type OnAction = Rc<dyn Fn(&str, &mut Window, &mut App)>;

/// Draws `content` for the widget `widget_id` (unique per window).
pub fn content(
    widget_id: &SharedString,
    content: &Content,
    theme: &WidgetTheme,
    on_action: OnAction,
) -> AnyElement {
    let palette = &theme.palette;
    let spacing = &theme.spacing;
    let typography = &theme.typography;
    let id = |part: &str, index: usize| {
        ElementId::from((SharedString::from(format!("{widget_id}-{part}")), index))
    };

    div()
        .flex()
        .flex_col()
        .gap(px(spacing.sm))
        .w_full()
        .min_w_0()
        .text_size(px(typography.size_body))
        .text_color(palette.text.hsla())
        .children(content.blocks.iter().enumerate().map(|(index, block)| {
            match block {
                Block::Text { text, style, tone } => {
                    let color = match tone {
                        Tone::Default => palette.text,
                        Tone::Muted => palette.muted,
                        Tone::Accent => palette.accent,
                        Tone::Positive => palette.positive,
                        Tone::Warning => palette.warning,
                        Tone::Danger => palette.danger,
                    };
                    let heading = *style == TextStyle::Heading;
                    let text = SharedString::from(text.clone());
                    // A heading is named by its label; body text is a GPUI `Text` with an id,
                    // which screen readers read through its value (AT-SPI ignores a label's name)
                    div()
                        .id(id("text", index))
                        .when(heading, |this| {
                            this.role(Role::Heading).aria_label(text.clone())
                        })
                        .test_support()
                        .w_full()
                        .min_w_0()
                        .text_color(color.hsla())
                        .when(heading, |this| {
                            this.text_size(px(typography.size_heading))
                                .font_weight(FontWeight::SEMIBOLD)
                        })
                        .child(if heading {
                            Text::new_inaccessible(text)
                        } else {
                            Text::new(id("text-value", index), text)
                        })
                        .into_any_element()
                }
                Block::List { items } => div()
                    .id(id("list", index))
                    .role(Role::List)
                    .test_support()
                    .flex()
                    .flex_col()
                    .gap(px(spacing.xs))
                    .children(items.iter().enumerate().map(|(n, item)| {
                        div()
                            .id(id(&format!("list{index}-item"), n))
                            .role(Role::ListItem)
                            .test_support()
                            .aria_label(SharedString::from(item.clone()))
                            .flex()
                            .gap(px(spacing.sm))
                            .min_w_0()
                            .child(div().text_color(palette.muted.hsla()).child("•"))
                            .child(div().min_w_0().child(SharedString::from(item.clone())))
                    }))
                    .into_any_element(),
                Block::Table { columns, rows } => {
                    let cell = |text: &str, text_id: ElementId| {
                        div()
                            .px(px(spacing.sm))
                            .py(px(spacing.xs))
                            .min_w_0()
                            .flex_1()
                            .child(Text::new(text_id, SharedString::from(text.to_owned())))
                    };
                    Table::new(id("table", index))
                        .accessibility_label(SharedString::from(format!(
                            "Table: {}",
                            columns.join(", ")
                        )))
                        .row_count(rows.len() + 1)
                        .column_count(columns.len())
                        .flex()
                        .flex_col()
                        .w_full()
                        .border_1()
                        .border_color(palette.border.hsla())
                        .rounded(px(spacing.radius))
                        .overflow_hidden()
                        .child(
                            TableHeader::new(id(&format!("table{index}-head"), 0)).child(
                                TableRow::new(id(&format!("table{index}-row"), 0), 0)
                                    .flex()
                                    .bg(palette.row_alt.hsla())
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .children(columns.iter().enumerate().map(|(c, column)| {
                                        TableHead::new(id(&format!("table{index}-h"), c), c)
                                            .flex_1()
                                            .min_w_0()
                                            .child(cell(
                                                column,
                                                id(&format!("table{index}-h{c}-text"), 0),
                                            ))
                                    })),
                            ),
                        )
                        .child(
                            TableBody::new(id(&format!("table{index}-body"), 0)).children(
                                rows.iter().enumerate().map(|(r, row)| {
                                    TableRow::new(id(&format!("table{index}-row"), r + 1), r + 1)
                                        .flex()
                                        .border_t_1()
                                        .border_color(palette.border.hsla())
                                        .children(row.iter().enumerate().map(|(c, text)| {
                                            TableCell::new(
                                                id(&format!("table{index}-r{r}-c"), c),
                                                c,
                                            )
                                            .flex_1()
                                            .min_w_0()
                                            .child(
                                                cell(
                                                    text,
                                                    id(&format!("table{index}-r{r}-c{c}-text"), 0),
                                                ),
                                            )
                                        }))
                                }),
                            ),
                        )
                        .into_any_element()
                }
                Block::Button { label, action } => {
                    let action = action.clone();
                    let on_action = on_action.clone();
                    div()
                        .flex()
                        .child(
                            Button::new(id("button", index))
                                .accessibility_label(SharedString::from(label.clone()))
                                .px(px(spacing.md))
                                .py(px(spacing.xs + 2.0))
                                .rounded(px(spacing.radius))
                                .bg(palette.accent.hsla())
                                .text_color(palette.accent_text.hsla())
                                .border_2()
                                .border_color(gpui_kit::transparent_black())
                                .focus_visible(|style| {
                                    style.border_color(palette.focus_ring.hsla())
                                })
                                .on_click(move |_, window, cx| on_action(&action, window, cx))
                                .child(SharedString::from(label.clone())),
                        )
                        .into_any_element()
                }
            }
        }))
        .into_any_element()
}
