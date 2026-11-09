//! The dashboard window's root view (US1): the chrome above a content area that scrolls and
//! clips everything it holds, so no frame or panel can cover the chrome (HS-01, research R6), one
//! frame per approved plugin, and the empty state. It reloads the approvals when the window
//! regains focus and from the chrome's "Reload" (FR-040), and draws a host dialog's scrim over
//! everything while one is open (HS-02).

use crate::a11y::Modality;
use crate::chrome::chrome;
use crate::dialog::view::{Shown, overlay};
use crate::dialog::{self, Answer, DialogHost, Request};
use crate::frame::frame;
use crate::panel::{MenuItem, attached, menu, panel};
use crate::plugins::{FrameStatus, Outcome, Widget, approvals, load_and_render};
use crate::text::Msg;
use crate::theme::{Theme, body_md, resolve_theme, title_sm};
use crate::ui::text;
use crate::vendor::gpui_base::tab::{CONTEXT, Tab, TabPrev, advance_focus};
use design_tokens::Appearance;
use gpui::{
    AnyWindowHandle, Context, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    KeyUpEvent, ParentElement, Render, Role, ScrollHandle, SharedString,
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
    /// The frame whose menu is open, and the frame whose panel is open.
    open_menu: Option<usize>,
    open_panel: Option<usize>,
    item_settings: FocusHandle,
    item_remove: FocusHandle,
    panel_close: FocusHandle,
    scroll: ScrollHandle,
    /// Bumped on every reload, so a slow load from an earlier one cannot overwrite a newer frame.
    epoch: u64,
    /// A reload asked for while a dialog was open, run when it is answered (FR-040).
    pending_reload: bool,
    handle: AnyWindowHandle,
    _activation: Subscription,
    _dialogs: Subscription,
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
            if !window.is_window_active() {
                return;
            }
            // The dialog's window regained focus: its 1000 ms start over (HS-11)
            if dialog::open(cx).is_some_and(|open| open.window == this.handle) {
                dialog::restart_gate(cx);
            }
            this.reload(cx);
        });
        let dialogs = cx.observe_global::<DialogHost>(|this, cx| {
            if this.pending_reload && !dialog::is_open(cx) {
                this.pending_reload = false;
                this.reload(cx);
            }
            cx.notify();
        });
        // Closing the window answers an open dialog as a refusal first (HS-10)
        window.on_window_should_close(cx, |_, cx| {
            dialog::refuse_if_open(cx);
            true
        });
        let mut dashboard = Dashboard {
            runtime,
            grants,
            appearance,
            widgets: Vec::new(),
            menus: Vec::new(),
            root,
            reload: cx.focus_handle().tab_stop(true),
            open_menu: None,
            open_panel: None,
            item_settings: cx.focus_handle().tab_stop(true),
            item_remove: cx.focus_handle().tab_stop(true),
            panel_close: cx.focus_handle().tab_stop(true),
            scroll: ScrollHandle::new(),
            epoch: 0,
            pending_reload: false,
            handle: window.window_handle(),
            _activation: activation,
            _dialogs: dialogs,
        };
        dashboard.reload(cx);
        dashboard
    }

    /// How many reloads have started: for tests that check a click reached "Reload".
    pub fn reloads(&self) -> u64 {
        self.epoch
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

    pub fn menu_focus(&self, index: usize) -> Option<&FocusHandle> {
        self.menus.get(index)
    }

    pub fn open_menu(&self) -> Option<usize> {
        self.open_menu
    }

    pub fn open_panel(&self) -> Option<usize> {
        self.open_panel
    }

    pub fn panel_close_focus(&self) -> &FocusHandle {
        &self.panel_close
    }

    pub fn settings_item_focus(&self) -> &FocusHandle {
        &self.item_settings
    }

    /// Opens frame `index`'s menu, focus on its first entry; or closes it if it is open.
    pub fn toggle_menu(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        if self.open_menu == Some(index) {
            self.close_menu(window, cx);
        } else {
            self.open_menu = Some(index);
            window.focus(&self.item_settings, cx);
            cx.notify();
        }
    }

    /// Closes the open menu; focus goes back to its frame's menu button.
    pub fn close_menu(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.open_menu.take() {
            if let Some(button) = self.menus.get(index) {
                window.focus(button, cx);
            }
            cx.notify();
        }
    }

    /// "Remove from dashboard" (FR-038): asks in a host dialog; confirmed, removes the approval and
    /// the data the host kept for the plugin, as the development tool's `remove` does, then
    /// reloads. Focus goes back to the frame's menu button whatever the answer.
    pub fn ask_remove(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.close_menu(window, cx);
        let Some(widget) = self.widgets.get(index).cloned() else {
            return;
        };
        // The dialog gives focus back to what held it: the frame's menu button
        if let Some(button) = self.menus.get(index) {
            window.focus(button, cx);
        }
        let answer = dialog::ask(
            cx,
            self.handle,
            Request {
                name: Msg::RemoveDialogName.text(),
                question: Msg::RemoveDialogQuestion.text(),
                subject: Some(widget.label.clone()),
                refuse: Msg::Cancel.text(),
                confirm: Msg::Remove.text(),
            },
        );
        let grants = Arc::clone(&self.grants);
        let runtime = self.runtime.clone();
        cx.spawn(async move |this, cx| {
            if answer.await != Ok(Answer::Confirmed) {
                return;
            }
            let id = widget.id.clone();
            cx.background_executor()
                .spawn(async move {
                    // Nothing more to do if the tool removed it meanwhile
                    let _ = grants.remove(&id);
                    if let Some(runtime) = runtime {
                        let _ = runtime.discard_kept_data(&id);
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| this.reload(cx));
        })
        .detach();
    }

    /// Opens frame `index`'s panel, attached to it, focus on its close control.
    pub fn open_panel_of(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
        self.open_menu = None;
        self.open_panel = Some(index);
        window.focus(&self.panel_close, cx);
        cx.notify();
    }

    /// Closes the open panel; focus goes back to its frame's menu button.
    pub fn close_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(index) = self.open_panel.take() {
            if let Some(button) = self.menus.get(index) {
                window.focus(button, cx);
            }
            cx.notify();
        }
    }

    /// Reads the approvals again, keeps the frames of plugins still approved from the same
    /// origin, adds the new ones and drops the rest, then loads and renders every widget off the
    /// UI thread.
    pub fn reload(&mut self, cx: &mut Context<Self>) {
        if dialog::is_open(cx) {
            self.pending_reload = true;
            return;
        }
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
                    // A menu or panel of a frame that may have moved closes
                    this.open_menu = None;
                    this.open_panel = None;
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
        let mut frames = Vec::with_capacity(self.widgets.len());
        for (index, (widget, button)) in self.widgets.iter().zip(&self.menus).enumerate() {
            let entity = cx.weak_entity();
            let on_menu = move |window: &mut Window, cx: &mut gpui::App| {
                let _ = entity.update(cx, |this, cx| this.toggle_menu(index, window, cx));
            };
            let open_menu = (self.open_menu == Some(index)).then(|| {
                let settings = cx.weak_entity();
                let remove = cx.weak_entity();
                let close = cx.weak_entity();
                menu(
                    &theme,
                    window,
                    index,
                    vec![
                        MenuItem {
                            name: Msg::WidgetSettings.text(),
                            focus: self.item_settings.clone(),
                            on_press: Box::new(move |window, cx| {
                                let _ = settings
                                    .update(cx, |this, cx| this.open_panel_of(index, window, cx));
                            }),
                        },
                        MenuItem {
                            name: Msg::RemoveFromDashboard.text(),
                            focus: self.item_remove.clone(),
                            on_press: Box::new(move |window, cx| {
                                let _ = remove
                                    .update(cx, |this, cx| this.ask_remove(index, window, cx));
                            }),
                        },
                    ],
                    move |window, cx| {
                        let _ = close.update(cx, |this, cx| this.close_menu(window, cx));
                    },
                )
                .into_any_element()
            });
            let drawn = frame(&theme, window, index, widget, button, on_menu, open_menu);
            if self.open_panel == Some(index) {
                let close = cx.weak_entity();
                frames.push(
                    attached(
                        drawn,
                        panel(
                            &theme,
                            window,
                            index,
                            &self.panel_close,
                            move |window, cx| {
                                let _ = close.update(cx, |this, cx| this.close_panel(window, cx));
                            },
                        ),
                    )
                    .into_any_element(),
                );
            } else {
                frames.push(drawn.into_any_element());
            }
        }
        let shown = dialog::open(cx).map(|open| Shown {
            request: open.request.clone(),
            refuse: open.refuse.clone(),
            confirm: open.confirm.clone(),
            trap: open.trap.clone(),
            here: open.window == self.handle,
        });
        if shown.as_ref().is_some_and(|shown| shown.here)
            && let Some(refuse) = dialog::start_gate_if_needed(cx)
        {
            window.focus(&refuse, cx);
        }
        let here = self.handle;
        let body = if frames.is_empty() {
            div().child(Self::empty_state(&theme))
        } else {
            div().flex().flex_wrap().gap(px(12.0)).children(frames)
        };
        div()
            .id("dashboard")
            .key_context(CONTEXT)
            .track_focus(&self.root)
            .on_action(cx.listener(move |_, _: &Tab, window, cx| {
                if !dialog_elsewhere(here, cx) {
                    advance_focus(window, cx, Window::focus_next)
                }
            }))
            .on_action(cx.listener(move |_, _: &TabPrev, window, cx| {
                if !dialog_elsewhere(here, cx) {
                    advance_focus(window, cx, Window::focus_prev)
                }
            }))
            // Every key is seen for the gate's held keys; while a dialog is open in another
            // window, this one drops every key (HS-10)
            .capture_key_down(move |event: &KeyDownEvent, _, cx| {
                dialog::key_down(cx, &event.keystroke.key);
                if dialog_elsewhere(here, cx) {
                    cx.stop_propagation();
                }
            })
            .on_key_up(|event: &KeyUpEvent, _, cx| dialog::key_up(cx, &event.keystroke.key))
            .relative()
            .size_full()
            .bg(theme.canvas)
            .child(
                // The window's background, one node for assistive technology: hidden while a
                // dialog is open, so only the dialog is exposed (HS-18)
                div()
                    .id("background")
                    .role(Role::Pane)
                    .aria_label(SharedString::from(Msg::AppName.text()))
                    .hidden_while(shown.is_some())
                    .size_full()
                    .flex()
                    .flex_col()
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
                    ),
            )
            .children(shown.map(|shown| overlay(&theme, window, &shown)))
    }
}

/// Whether a dialog is open in another host window, so this one drops its input.
fn dialog_elsewhere(here: AnyWindowHandle, cx: &gpui::App) -> bool {
    dialog::open(cx).is_some_and(|open| open.window != here)
}
