//! The dashboard window's root view (US1): the chrome above a content area that scrolls and
//! clips everything it holds, so no frame or panel can cover the chrome (HS-01, research R6), one
//! frame per approved plugin, the empty state, and the appearance screen in place of the frames
//! while it is open (US7). It reloads the approvals when the window
//! regains focus and from the chrome's "Reload" (FR-040), and draws a host dialog's scrim over
//! everything while one is open (HS-02).

use crate::a11y::Modality;
use crate::appearance::{self, Pick};
use crate::chrome::{Controls, chrome};
use crate::dialog::view::{Shown, overlay};
use crate::dialog::{self, Answer, DialogHost, Request};
use crate::frame::frame;
use crate::panel::{MenuItem, attached, menu, panel};
use crate::plugins::{FrameStatus, Outcome, Widget, approvals, load_and_render};
use crate::text::Msg;
use crate::theme::{Theme, body_md, resolve_theme, title_sm};
use crate::ui::text;
use crate::vendor::gpui_base::tab::{CONTEXT, Tab, TabPrev, advance_focus};
use design_tokens::Refusal;
use gpui::{
    AnyWindowHandle, Context, FocusHandle, InteractiveElement, IntoElement, KeyDownEvent,
    KeyUpEvent, ParentElement, Render, Role, ScrollHandle, SharedString,
    StatefulInteractiveElement, Styled, Subscription, Task, Window, div, px,
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
    widgets: Vec<Widget>,
    /// One menu focus handle per frame, kept across renders.
    menus: Vec<FocusHandle>,
    root: FocusHandle,
    reload: FocusHandle,
    appearance_button: FocusHandle,
    /// The appearance screen's controls, and whether it is open with the choice it last refused.
    appearance_focus: appearance::Focus,
    appearance_open: bool,
    refused: Option<Refusal>,
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
    /// The reload running now; a new one drops it, which stops its loads.
    reloading: Option<Task<()>>,
    /// The host's notice about the record or a removal, until the next reload that reads it.
    notice: Option<Msg>,
    handle: AnyWindowHandle,
    _activation: Subscription,
    _dialogs: Subscription,
    _appearance: Subscription,
    _system_theme: Subscription,
}

impl Dashboard {
    /// The window's root, drawn in the app's appearance (`appearance::init`, or the default).
    /// With no runtime (it did not start), every frame says why.
    pub fn new(
        runtime: Option<Arc<Runtime>>,
        grants: Arc<GrantStore>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Dashboard {
        if !cx.has_global::<appearance::Settings>() {
            appearance::init(cx, appearance::Settings::default());
        }
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
            // A dialog closes every menu and panel, in every window, so nothing behind the scrim
            // can take focus back when it closes (HS-10, HS-17)
            if dialog::is_open(cx) {
                this.open_menu = None;
                this.open_panel = None;
            }
            if this.pending_reload && !dialog::is_open(cx) {
                this.pending_reload = false;
                this.reload(cx);
            }
            cx.notify();
        });
        // A choice made in any window redraws this one (FR-036)
        let appearance_changed = cx.observe_global::<appearance::Settings>(|_, cx| cx.notify());
        // With the system's theme picked, a switch of the system's redraws it (FR-037)
        let system_theme = cx.observe_window_appearance(window, |_, _, cx| cx.notify());
        // Closing the window answers an open dialog as a refusal first, and the dialogs waiting
        // to show in it (HS-10)
        let closing = window.window_handle();
        window.on_window_should_close(cx, move |_, cx| {
            dialog::refuse_for_closing(cx, closing);
            true
        });
        let mut dashboard = Dashboard {
            runtime,
            grants,
            widgets: Vec::new(),
            menus: Vec::new(),
            root,
            reload: cx.focus_handle().tab_stop(true),
            appearance_button: cx.focus_handle().tab_stop(true),
            appearance_focus: appearance::Focus::new(cx),
            appearance_open: false,
            refused: None,
            open_menu: None,
            open_panel: None,
            item_settings: cx.focus_handle().tab_stop(true),
            item_remove: cx.focus_handle().tab_stop(true),
            panel_close: cx.focus_handle().tab_stop(true),
            scroll: ScrollHandle::new(),
            epoch: 0,
            pending_reload: false,
            reloading: None,
            notice: None,
            handle: window.window_handle(),
            _activation: activation,
            _dialogs: dialogs,
            _appearance: appearance_changed,
            _system_theme: system_theme,
        };
        dashboard.reload(cx);
        dashboard
    }

    /// How many reloads have started: for tests that check a click reached "Reload".
    pub fn reloads(&self) -> u64 {
        self.epoch
    }

    /// The host's notice about the record or the last removal, if it shows one.
    pub fn notice(&self) -> Option<Msg> {
        self.notice
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

    pub fn appearance_button_focus(&self) -> &FocusHandle {
        &self.appearance_button
    }

    /// The appearance screen's controls: its "Close" and one per choice, in `appearance::groups`'
    /// order.
    pub fn appearance_focus(&self) -> &appearance::Focus {
        &self.appearance_focus
    }

    pub fn appearance_open(&self) -> bool {
        self.appearance_open
    }

    /// The choice the appearance screen last refused, while it shows the refusal.
    pub fn refused(&self) -> Option<Refusal> {
        self.refused
    }

    /// The colors, type and radius this window draws with now.
    pub fn theme(&self, window: &Window, cx: &gpui::App) -> Theme {
        let appearance = appearance::current(cx);
        Theme::new(
            &appearance,
            resolve_theme(appearance.theme, window.appearance()),
        )
    }

    /// Opens the appearance screen in place of the frames, focus on the first current choice; or
    /// closes it if it is open.
    pub fn toggle_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.appearance_open {
            self.close_appearance(window, cx);
            return;
        }
        self.open_menu = None;
        self.open_panel = None;
        self.appearance_open = true;
        self.refused = None;
        let current = appearance::current(cx);
        window.focus(self.appearance_focus.first_current(&current), cx);
        cx.notify();
    }

    /// Closes the appearance screen; focus goes back to the chrome's "Appearance".
    pub fn close_appearance(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.appearance_open {
            self.appearance_open = false;
            self.refused = None;
            window.focus(&self.appearance_button, cx);
            cx.notify();
        }
    }

    /// Makes a choice of the appearance screen; a refused one stays shown with the host's reason
    /// and changes nothing (FR-035).
    pub fn pick(&mut self, pick: Pick, cx: &mut Context<Self>) {
        self.refused = appearance::choose(cx, pick).err();
        cx.notify();
    }

    /// The name of the host control focused in `window`, for tests that drive a real window:
    /// `root`, `reload`, `appearance`, `appearance-close`, `appearance-<choice>`, `menu-<frame>`,
    /// `settings`, `remove`, `panel-close`, `dialog-refuse` or `dialog-confirm`.
    pub fn focused_name(&self, window: &Window, cx: &gpui::App) -> Option<String> {
        if let Some(open) = dialog::open(cx) {
            if open.refuse.is_focused(window) {
                return Some("dialog-refuse".to_owned());
            }
            if open.confirm.is_focused(window) {
                return Some("dialog-confirm".to_owned());
            }
        }
        if self.appearance_focus.close.is_focused(window) {
            return Some("appearance-close".to_owned());
        }
        if let Some(index) = self
            .appearance_focus
            .picks
            .iter()
            .position(|handle| handle.is_focused(window))
        {
            return Some(format!("appearance-{index}"));
        }
        let named = [
            (&self.root, "root"),
            (&self.reload, "reload"),
            (&self.appearance_button, "appearance"),
            (&self.item_settings, "settings"),
            (&self.item_remove, "remove"),
            (&self.panel_close, "panel-close"),
        ];
        if let Some((_, name)) = named.iter().find(|(handle, _)| handle.is_focused(window)) {
            return Some((*name).to_owned());
        }
        self.menus
            .iter()
            .position(|handle| handle.is_focused(window))
            .map(|index| format!("menu-{index}"))
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
            // While a dialog is open, focus stays where the dialog put it
            if !dialog::is_open(cx)
                && let Some(button) = self.menus.get(index)
            {
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
        self.notice = None;
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
            // As the development tool's `remove`: the approval first, and the kept data only once
            // it is gone (or the tool removed it meanwhile)
            let notice = cx
                .background_executor()
                .spawn(async move {
                    if grants.remove(&id).is_err() {
                        return Some(Msg::RemoveFailed);
                    }
                    match runtime.map(|runtime| runtime.discard_kept_data(&id)) {
                        Some(Err(_)) => Some(Msg::RemoveDataKept),
                        _ => None,
                    }
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                this.reload(cx);
                this.notice = notice;
                cx.notify();
            });
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
            if !dialog::is_open(cx)
                && let Some(button) = self.menus.get(index)
            {
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
        // Replacing the task drops the reload before it: its loads stop at their next await
        self.reloading = Some(cx.spawn(async move |this, cx| {
            let listed = cx
                .background_executor()
                .spawn({
                    let grants = Arc::clone(&grants);
                    async move { approvals(&grants) }
                })
                .await;
            let origins: Vec<(usize, String)> = this
                .update(cx, |this, cx| {
                    // A newer reload started meanwhile: its list wins
                    if this.epoch != epoch {
                        return Vec::new();
                    }
                    let Some(listed) = listed else {
                        // The record cannot be read now: keep the frames, say so, unless a
                        // removal's notice already says more
                        this.notice.get_or_insert(Msg::RecordUnreadable);
                        cx.notify();
                        return Vec::new();
                    };
                    if this.notice == Some(Msg::RecordUnreadable) {
                        this.notice = None;
                    }
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
                            widget.load_time = outcome.load_time;
                        }
                        cx.notify();
                        true
                    })
                    .unwrap_or(false);
                if !still_current {
                    break;
                }
            }
        }));
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
        let theme = self.theme(window, cx);
        let this = cx.weak_entity();
        let on_reload = move |_: &mut Window, cx: &mut gpui::App| {
            let _ = this.update(cx, |this, cx| this.reload(cx));
        };
        let this = cx.weak_entity();
        let on_appearance = move |window: &mut Window, cx: &mut gpui::App| {
            let _ = this.update(cx, |this, cx| this.toggle_appearance(window, cx));
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
        if let Some(shown) = shown.as_ref().filter(|shown| shown.here) {
            if let Some(refuse) = dialog::start_gate_if_needed(cx) {
                window.focus(&refuse, cx);
            } else if !shown.refuse.is_focused(window) && !shown.confirm.is_focused(window) {
                // Focus is never anywhere but the dialog's controls while it is open (HS-17)
                window.focus(&shown.refuse, cx);
            }
        }
        let here = self.handle;
        let body = if self.appearance_open {
            let picker = cx.weak_entity();
            let closer = cx.weak_entity();
            div().child(appearance::screen(
                &theme,
                window,
                &appearance::current(cx),
                &self.appearance_focus,
                self.refused,
                std::rc::Rc::new(move |pick, _, cx| {
                    let _ = picker.update(cx, |this, cx| this.pick(pick, cx));
                }),
                std::rc::Rc::new(move |window, cx| {
                    let _ = closer.update(cx, |this, cx| this.close_appearance(window, cx));
                }),
            ))
        } else if frames.is_empty() {
            div().child(Self::empty_state(&theme))
        } else {
            div().flex().flex_wrap().gap(px(12.0)).children(frames)
        };
        let notices = [
            appearance::notice(cx)
                .map(|notice| appearance::notice_line("appearance-notice", &theme, notice)),
            self.notice
                .map(|notice| appearance::notice_line("host-notice", &theme, notice)),
        ];
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
            .capture_key_down(move |event: &KeyDownEvent, window, cx| {
                dialog::key_down(cx, &event.keystroke.key);
                // While a dialog is open, a key reaches only its controls: in another window
                // nothing, and in its own window nothing until its first frame has put focus on
                // Cancel (HS-10)
                let outside = dialog::open(cx).is_some_and(|open| {
                    open.window != here
                        || !(open.refuse.is_focused(window) || open.confirm.is_focused(window))
                });
                if outside {
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
                    .child(chrome(
                        &theme,
                        window,
                        Controls {
                            appearance: self.appearance_button.clone(),
                            on_appearance,
                            reload: self.reload.clone(),
                            on_reload,
                        },
                    ))
                    .child(
                        div()
                            .id("content")
                            .debug_selector(|| "content".to_owned())
                            .flex_1()
                            .min_h_0()
                            .overflow_y_scroll()
                            .track_scroll(&self.scroll)
                            .p(px(CONTENT_PADDING))
                            .children(notices.into_iter().flatten())
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
