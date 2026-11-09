//! The dashboard window's root view (US1): the chrome above a content area that scrolls and
//! clips everything it holds, so no frame or panel can cover the chrome (HS-01, research R6), one
//! frame per approved plugin, and the empty state. It reloads the approvals when the window
//! regains focus and from the chrome's "Reload" (FR-040).

use crate::chrome::chrome;
use crate::frame::frame;
use crate::plugins::{FrameStatus, Outcome, Widget, approvals, load_and_render};
use crate::text::Msg;
use crate::theme::{Theme, body_md, resolve_theme, title_sm};
use crate::ui::text;
use crate::vendor::gpui_base::tab::{CONTEXT, Tab, TabPrev, advance_focus};
use design_tokens::Appearance;
use gpui::{
    Context, FocusHandle, InteractiveElement, IntoElement, ParentElement, Render, ScrollHandle,
    StatefulInteractiveElement, Styled, Subscription, Window, div, px,
};
use runtime::Runtime;
use runtime::grants::GrantStore;
use std::sync::Arc;

/// The padding inside the content area: more than the focus ring's 4 px, so a ring next to the
/// clipping edge is never cut (HS-24, research R6).
pub const CONTENT_PADDING: f32 = 16.0;

pub struct Dashboard {
    runtime: Option<Arc<Runtime>>,
    grants: Arc<GrantStore>,
    appearance: Appearance,
    widgets: Vec<Widget>,
    /// One menu focus handle per frame, kept across renders.
    menus: Vec<FocusHandle>,
    root: FocusHandle,
    reload: FocusHandle,
    scroll: ScrollHandle,
    /// Bumped on every reload, so a slow load from an earlier one cannot overwrite a newer frame.
    epoch: u64,
    _activation: Subscription,
}

impl Dashboard {
    /// The window's root. With no runtime (it did not start), every frame says why.
    pub fn new(
        runtime: Option<Arc<Runtime>>,
        grants: Arc<GrantStore>,
        appearance: Appearance,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Dashboard {
        let root = cx.focus_handle();
        // The window focuses its root, so the first Tab reaches the first control (FR-005)
        window.focus(&root, cx);
        let activation = cx.observe_window_activation(window, |this, window, cx| {
            if window.is_window_active() {
                this.reload(cx);
            }
        });
        let mut dashboard = Dashboard {
            runtime,
            grants,
            appearance,
            widgets: Vec::new(),
            menus: Vec::new(),
            root,
            reload: cx.focus_handle().tab_stop(true),
            scroll: ScrollHandle::new(),
            epoch: 0,
            _activation: activation,
        };
        dashboard.reload(cx);
        dashboard
    }

    pub fn widgets(&self) -> &[Widget] {
        &self.widgets
    }

    pub fn scroll_handle(&self) -> &ScrollHandle {
        &self.scroll
    }

    pub fn root_focus(&self) -> &FocusHandle {
        &self.root
    }

    pub fn reload_focus(&self) -> &FocusHandle {
        &self.reload
    }

    /// Reads the approvals again, keeps the frames of plugins still approved from the same
    /// origin, adds the new ones and drops the rest, then loads and renders every widget off the
    /// UI thread.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        self.epoch += 1;
        let epoch = self.epoch;
        let grants = Arc::clone(&self.grants);
        let runtime = self.runtime.clone();
        cx.spawn(async move |this, cx| {
            let listed = cx
                .background_executor()
                .spawn({
                    let grants = Arc::clone(&grants);
                    async move { approvals(&grants) }
                })
                .await;
            let origins: Vec<(usize, String)> = this
                .update(cx, |this, cx| {
                    this.widgets = listed;
                    while this.menus.len() < this.widgets.len() {
                        this.menus.push(cx.focus_handle().tab_stop(true));
                    }
                    this.menus.truncate(this.widgets.len());
                    cx.notify();
                    this.widgets
                        .iter()
                        .map(|widget| widget.origin.clone())
                        .enumerate()
                        .collect()
                })
                .unwrap_or_default();
            for (index, origin) in origins {
                let outcome = match &runtime {
                    Some(runtime) => {
                        let runtime = Arc::clone(runtime);
                        let grants = Arc::clone(&grants);
                        cx.background_executor()
                            .spawn(async move { load_and_render(&runtime, &grants, &origin) })
                            .await
                    }
                    None => Outcome {
                        status: FrameStatus::Failed(Msg::ReasonPackage),
                        load_time: None,
                    },
                };
                let still_current = this
                    .update(cx, |this, cx| {
                        if this.epoch != epoch {
                            return false;
                        }
                        if let Some(widget) = this.widgets.get_mut(index) {
                            widget.status = outcome.status;
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !still_current {
                    break;
                }
            }
        })
        .detach();
    }

    fn empty_state(theme: &Theme) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .gap(px(4.0))
            .child(text(title_sm(), theme.text, Msg::NothingApproved.text()))
            .child(text(
                body_md(),
                theme.text_secondary,
                Msg::NothingApprovedHint.text(),
            ))
    }
}

impl Render for Dashboard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = Theme::new(
            &self.appearance,
            resolve_theme(self.appearance.theme, window.appearance()),
        );
        let this = cx.weak_entity();
        let on_reload = move |_: &mut Window, cx: &mut gpui::App| {
            let _ = this.update(cx, |this, cx| this.reload(cx));
        };
        let frames: Vec<_> = self
            .widgets
            .iter()
            .zip(&self.menus)
            .enumerate()
            .map(|(index, (widget, menu))| {
                frame(&theme, window, index, widget, menu, |_, _| {}).into_any_element()
            })
            .collect();
        let body = if frames.is_empty() {
            div().child(Self::empty_state(&theme))
        } else {
            div().flex().flex_wrap().gap(px(12.0)).children(frames)
        };
        div()
            .id("dashboard")
            .key_context(CONTEXT)
            .track_focus(&self.root)
            .on_action(
                cx.listener(|_, _: &Tab, window, cx| advance_focus(window, cx, Window::focus_next)),
            )
            .on_action(cx.listener(|_, _: &TabPrev, window, cx| {
                advance_focus(window, cx, Window::focus_prev)
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(theme.canvas)
            .child(chrome(&theme, window, &self.reload, on_reload))
            .child(
                div()
                    .id("content")
                    .debug_selector(|| "content".to_owned())
                    .flex_1()
                    .min_h_0()
                    .overflow_y_scroll()
                    .track_scroll(&self.scroll)
                    .p(px(CONTENT_PADDING))
                    .child(body),
            )
    }
}
