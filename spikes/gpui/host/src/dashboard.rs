//! The main window: toolbar, the widget grid, each widget's host frame, and the host dialogs.
//!
//! Only this module draws host chrome, with the host palette. Widget content is drawn by
//! widget-kit with the widget palette, inside the frame's body, which clips it.

use std::cell::Cell as StdCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use gpui_kit::base::{Button as BaseButton, FocusTrapElement as _, TestSupportExt as _};
use gpui_kit::prelude::*;
use gpui_kit::{
    AnyElement, App, Bounds, Context, ElementId, FocusHandle, FontWeight, KeyBinding, MouseButton,
    MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels, Point, Render, Role, SharedString,
    Window, WindowAppearance, canvas, div, point, px, relative,
};
use runtime::Plugin;
use serde::{Deserialize, Serialize};
use widget_kit::model::Content;

use crate::layout::{Cell, Layout};
use crate::plugins::{self, CATALOG, Entry, Loaded, PluginHost, Request};
use crate::theme::{self, Mode, Theme};

/// Height of one grid row.
const ROW: f32 = 64.0;
/// Space between frames.
const GAP: f32 = 8.0;

gpui_kit::actions!(
    dashboard,
    [
        MoveLeft,
        MoveRight,
        MoveUp,
        MoveDown,
        Narrower,
        Wider,
        Shorter,
        Taller,
        RemoveWidget,
        CloseDialog
    ]
);

pub fn bind_keys(cx: &mut App) {
    let frame = Some("WidgetFrame");
    cx.bind_keys([
        KeyBinding::new("left", MoveLeft, frame),
        KeyBinding::new("right", MoveRight, frame),
        KeyBinding::new("up", MoveUp, frame),
        KeyBinding::new("down", MoveDown, frame),
        KeyBinding::new("shift-left", Narrower, frame),
        KeyBinding::new("shift-right", Wider, frame),
        KeyBinding::new("shift-up", Shorter, frame),
        KeyBinding::new("shift-down", Taller, frame),
        KeyBinding::new("delete", RemoveWidget, frame),
        KeyBinding::new("backspace", RemoveWidget, frame),
        KeyBinding::new("escape", CloseDialog, Some("HostDialog")),
    ]);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Settings {
    mode: Mode,
    /// The name of the person's visual secret, among `theme::secret_choices`.
    secret: Option<String>,
}

enum WidgetState {
    Loading,
    Ready(Content),
    Failed(String),
}

struct WidgetView {
    focus: FocusHandle,
    /// The plugin's id, from its manifest, as the runtime read it.
    plugin_id: SharedString,
    state: WidgetState,
}

enum Dialog {
    /// The approval dialog for `entry`, shown before a package without an approval is added.
    Approval {
        entry: &'static Entry,
        request: Request,
    },
    Settings,
}

#[derive(Clone, Copy, PartialEq)]
enum DragKind {
    Move,
    Resize,
}

struct Drag {
    id: u64,
    kind: DragKind,
    start_mouse: Point<Pixels>,
    start_cell: Cell,
    mouse: Point<Pixels>,
}

/// What `SPIKE_DRAG_BENCH` records: one entry per frame while a widget is dragged by script.
struct DragBench {
    started: Instant,
    last: Instant,
    frames: Vec<f32>,
    cpu_start: Option<f64>,
}

pub struct Dashboard {
    host: PluginHost,
    data_dir: PathBuf,
    layout: Layout,
    views: HashMap<u64, WidgetView>,
    plugins: HashMap<u64, Arc<Plugin>>,
    settings: Settings,
    theme: Theme,
    dialog: Option<Dialog>,
    dialog_focus: FocusHandle,
    /// The window's own focus, given at start so the keyboard works before anything is clicked.
    root_focus: FocusHandle,
    drag: Option<Drag>,
    grid_bounds: Rc<StdCell<Bounds<Pixels>>>,
    status: Option<SharedString>,
    started: Instant,
    first_frame: bool,
    bench: Option<DragBench>,
    font: SharedString,
}

impl Dashboard {
    pub fn new(
        data_dir: PathBuf,
        started: Instant,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Dashboard {
        std::fs::create_dir_all(&data_dir).ok();
        let host = PluginHost::new(&data_dir).expect("the runtime starts");
        let settings: Settings = read_json(&data_dir.join("settings.json")).unwrap_or(Settings {
            mode: Mode::System,
            secret: None,
        });
        let layout: Layout = read_json(&data_dir.join("layout.json")).unwrap_or_default();
        let dark = is_dark(settings.mode, window);
        let theme = theme::theme(dark);
        let installed = cx.text_system().all_font_names();
        let font: SharedString = theme
            .typography
            .font_families
            .iter()
            .find(|family| installed.iter().any(|name| name == *family))
            .cloned()
            .unwrap_or_else(|| ".SystemUIFont".into())
            .into();
        let mut dashboard = Dashboard {
            host,
            data_dir,
            layout,
            views: HashMap::new(),
            plugins: HashMap::new(),
            settings,
            theme,
            dialog: None,
            dialog_focus: cx.focus_handle(),
            root_focus: cx.focus_handle(),
            drag: None,
            grid_bounds: Rc::new(StdCell::new(Bounds::default())),
            status: None,
            started,
            first_frame: true,
            bench: None,
            font,
        };
        sync_component_theme(dark, window, cx);
        cx.observe_window_appearance(window, |this, window, cx| this.apply_mode(window, cx))
            .detach();
        let ids: Vec<u64> = dashboard.layout.widgets.iter().map(|w| w.id).collect();
        for id in ids {
            dashboard.start_widget(id, window, cx);
        }
        dashboard.root_focus.focus(window, cx);
        if dashboard.settings.secret.is_none() {
            dashboard.open_dialog(Dialog::Settings, window, cx);
        }
        dashboard
    }

    fn apply_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let dark = is_dark(self.settings.mode, window);
        self.theme = theme::theme(dark);
        sync_component_theme(dark, window, cx);
        cx.notify();
    }

    fn save(&self) {
        write_json(&self.data_dir.join("layout.json"), &self.layout);
        write_json(&self.data_dir.join("settings.json"), &self.settings);
    }

    /// Adds `entry`, asking for approval first in a host dialog when its package has none.
    pub fn request_add(
        &mut self,
        entry: &'static Entry,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match self.host.load(entry.package) {
            Loaded::Ready(_) => self.add(entry, window, cx),
            Loaded::NeedsApproval(request) => {
                self.open_dialog(Dialog::Approval { entry, request }, window, cx)
            }
            Loaded::Refused(error) => {
                self.status = Some(format!("{} was refused: {error}", entry.name).into())
            }
        }
        cx.notify();
    }

    fn add(&mut self, entry: &'static Entry, window: &mut Window, cx: &mut Context<Self>) {
        let id = self.layout.add(entry.key, entry.size.0, entry.size.1);
        self.save();
        self.start_widget(id, window, cx);
        if let Some(view) = self.views.get(&id) {
            view.focus.focus(window, cx);
        }
    }

    fn start_widget(&mut self, id: u64, _window: &mut Window, cx: &mut Context<Self>) {
        let Some(placed) = self.layout.get(id) else {
            return;
        };
        let entry = plugins::entry(&placed.entry);
        let focus = cx.focus_handle().tab_stop(true);
        let mut plugin_id = SharedString::from("unknown plugin");
        let state = match entry.map(|entry| self.host.load(entry.package)) {
            None => WidgetState::Failed(format!("unknown catalog entry {:?}", placed.entry)),
            Some(Loaded::Ready(plugin)) => {
                plugin_id = plugin.manifest().id().as_str().to_owned().into();
                self.plugins.insert(id, plugin);
                WidgetState::Loading
            }
            Some(Loaded::NeedsApproval(_)) => {
                WidgetState::Failed("this widget's plugin has no approval".into())
            }
            Some(Loaded::Refused(error)) => WidgetState::Failed(error),
        };
        let loading = matches!(state, WidgetState::Loading);
        self.views.insert(
            id,
            WidgetView {
                focus,
                plugin_id,
                state,
            },
        );
        if loading {
            self.render_widget(id, cx);
        }
    }

    /// Calls the plugin's `render` on a background thread and keeps the validated content.
    fn render_widget(&mut self, id: u64, cx: &mut Context<Self>) {
        let (Some(plugin), Some(placed)) = (self.plugins.get(&id).cloned(), self.layout.get(id))
        else {
            return;
        };
        let Some(entry) = plugins::entry(&placed.entry) else {
            return;
        };
        let config = (entry.config)(placed.clicks);
        let task = cx
            .background_executor()
            .spawn(async move { plugins::render(&plugin, &config) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            this.update(cx, |this, cx| {
                if let Some(view) = this.views.get_mut(&id) {
                    view.state = match result {
                        Ok(content) => WidgetState::Ready(content),
                        Err(error) => WidgetState::Failed(error),
                    };
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    /// A widget's button asks for `action`. The host acts only on what it knows for that
    /// widget; anything else is reported and ignored. A widget cannot approve, install or open
    /// anything.
    fn widget_action(&mut self, id: u64, action: &str, cx: &mut Context<Self>) {
        let entry = self.layout.get(id).map(|w| w.entry.clone());
        match (entry.as_deref(), action) {
            (Some("sampler"), "increment") => {
                if let Some(widget) = self.layout.widgets.iter_mut().find(|w| w.id == id) {
                    widget.clicks += 1;
                }
                self.save();
                self.render_widget(id, cx);
            }
            (_, other) => {
                self.status = Some(
                    format!(
                        "A widget asked for \"{other}\"; the host ignores actions it does not know."
                    )
                    .into(),
                )
            }
        }
        cx.notify();
    }

    fn open_dialog(&mut self, dialog: Dialog, window: &mut Window, cx: &mut Context<Self>) {
        self.dialog = Some(dialog);
        self.dialog_focus.focus(window, cx);
        // Move focus into the dialog on the next frame, once its buttons exist
        let focus = self.dialog_focus.clone();
        window.on_next_frame(move |window, cx| {
            focus.focus(window, cx);
            window.focus_next(cx);
        });
        cx.notify();
    }

    fn close_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dialog = None;
        if let Some(view) = self
            .layout
            .widgets
            .last()
            .and_then(|w| self.views.get(&w.id))
        {
            view.focus.focus(window, cx);
        }
        cx.notify();
    }

    fn focused_widget(&self, window: &Window) -> Option<u64> {
        self.views
            .iter()
            .find(|(_, view)| view.focus.is_focused(window))
            .map(|(id, _)| *id)
    }

    fn keyboard(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        change: impl FnOnce(&mut Layout, u64) -> bool,
    ) {
        let Some(id) = self.focused_widget(window) else {
            return;
        };
        if change(&mut self.layout, id) {
            self.save();
        } else {
            self.status = Some("That place is taken or off the grid.".into());
        }
        cx.notify();
    }

    fn remove(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.focused_widget(window) else {
            return;
        };
        self.layout.remove(id);
        self.views.remove(&id);
        self.plugins.remove(&id);
        self.save();
        window.focus_next(cx);
        cx.notify();
    }

    // Mouse dragging: the frame follows the pointer; the grid cell under it is outlined and is
    // where the widget lands if it is free.

    fn cell_size(&self) -> (f32, f32) {
        let bounds = self.grid_bounds.get();
        (
            f32::from(bounds.size.width) / crate::layout::COLUMNS as f32,
            ROW,
        )
    }

    fn drag_target(&self, drag: &Drag) -> Cell {
        let (cw, ch) = self.cell_size();
        let dx = ((f32::from(drag.mouse.x) - f32::from(drag.start_mouse.x)) / cw).round() as i32;
        let dy = ((f32::from(drag.mouse.y) - f32::from(drag.start_mouse.y)) / ch).round() as i32;
        let c = drag.start_cell;
        match drag.kind {
            DragKind::Move => Cell {
                x: c.x + dx,
                y: c.y + dy,
                ..c
            },
            DragKind::Resize => Cell {
                w: c.w + dx,
                h: c.h + dy,
                ..c
            },
        }
    }

    pub fn start_drag_at(
        &mut self,
        id: u64,
        resize: bool,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(placed) = self.layout.get(id) else {
            return;
        };
        let kind = if resize {
            DragKind::Resize
        } else {
            DragKind::Move
        };
        self.drag = Some(Drag {
            id,
            kind,
            start_mouse: position,
            start_cell: placed.cell,
            mouse: position,
        });
        cx.notify();
    }

    pub fn layout(&self) -> &Layout {
        &self.layout
    }

    pub fn dialog_open(&self) -> bool {
        self.dialog.is_some()
    }

    pub fn widget_ready(&self, id: u64) -> bool {
        matches!(
            self.views.get(&id).map(|v| &v.state),
            Some(WidgetState::Ready(_))
        )
    }

    pub fn widget_error(&self, id: u64) -> Option<String> {
        match self.views.get(&id).map(|v| &v.state) {
            Some(WidgetState::Failed(error)) => Some(error.clone()),
            _ => None,
        }
    }

    fn start_drag(
        &mut self,
        id: u64,
        kind: DragKind,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(placed) = self.layout.get(id) else {
            return;
        };
        self.drag = Some(Drag {
            id,
            kind,
            start_mouse: event.position,
            start_cell: placed.cell,
            mouse: event.position,
        });
        if let Some(view) = self.views.get(&id) {
            view.focus.focus(window, cx);
        }
        cx.stop_propagation();
        cx.notify();
    }

    pub fn drag_move(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        if let Some(drag) = self.drag.as_mut() {
            drag.mouse = position;
            cx.notify();
        }
    }

    pub fn end_drag(&mut self, cx: &mut Context<Self>) {
        let Some(drag) = self.drag.take() else { return };
        let target = self.drag_target(&drag);
        if target != drag.start_cell && !self.layout.place(drag.id, target) {
            self.status = Some("That place is taken or off the grid.".into());
        }
        self.save();
        cx.notify();
    }

    /// `SPIKE_DRAG_BENCH`: drags the first widget in a circle for five seconds, one step per
    /// frame, through the same code path as the mouse, and prints frame times and CPU.
    pub fn start_bench(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(first) = self.layout.widgets.first() else {
            eprintln!("spike: drag bench needs a widget on the grid");
            cx.quit();
            return;
        };
        let bounds = self.grid_bounds.get();
        let start = point(bounds.origin.x + px(40.0), bounds.origin.y + px(20.0));
        self.drag = Some(Drag {
            id: first.id,
            kind: DragKind::Move,
            start_mouse: start,
            start_cell: first.cell,
            mouse: start,
        });
        let now = Instant::now();
        self.bench = Some(DragBench {
            started: now,
            last: now,
            frames: Vec::new(),
            cpu_start: cpu_seconds(),
        });
        window.request_animation_frame();
    }

    fn bench_frame(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(bench) = self.bench.as_mut() else {
            return;
        };
        let now = Instant::now();
        bench.frames.push((now - bench.last).as_secs_f32() * 1000.0);
        bench.last = now;
        let t = (now - bench.started).as_secs_f32();
        if t < 5.0 {
            let (cw, _) = self.cell_size();
            if let Some(drag) = self.drag.as_mut() {
                let radius = 2.5 * cw;
                drag.mouse = point(
                    drag.start_mouse.x + px(radius * (t * 2.0).cos() - radius + radius),
                    drag.start_mouse.y + px(radius * 0.6 * (t * 2.0).sin()),
                );
            }
            window.request_animation_frame();
            return;
        }
        let bench = self.bench.take().unwrap();
        self.drag = None;
        let mut frames = bench.frames[1..].to_vec();
        frames.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let pick = |q: f32| frames[((frames.len() - 1) as f32 * q) as usize];
        let slow = frames.iter().filter(|f| **f > 1000.0 / 30.0).count();
        let cpu = match (bench.cpu_start, cpu_seconds()) {
            (Some(a), Some(b)) => format!("{:.0}%", (b - a) / t as f64 * 100.0),
            _ => "n/a".into(),
        };
        eprintln!(
            "spike: drag bench {} frames in {t:.1}s ({:.1} fps); frame ms p50 {:.1} p95 {:.1} p99 {:.1} max {:.1}; over 33 ms: {slow}; CPU {cpu}",
            frames.len(),
            frames.len() as f32 / t,
            pick(0.5),
            pick(0.95),
            pick(0.99),
            frames.last().copied().unwrap_or_default(),
        );
        cx.quit();
    }
}

impl Render for Dashboard {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.first_frame {
            self.first_frame = false;
            let started = self.started;
            window.on_next_frame(move |_, _| {
                eprintln!(
                    "spike: first frame presented {} ms after start",
                    started.elapsed().as_millis()
                )
            });
        }
        self.bench_frame(window, cx);
        let theme = self.theme.clone();
        let h = &theme.host;
        let s = &theme.spacing;

        let toolbar = div()
            .id("toolbar")
            .role(Role::Toolbar)
            .aria_label("Dashboard")
            .flex()
            .items_center()
            .gap(px(s.sm))
            .px(px(s.lg))
            .py(px(s.sm))
            .bg(h.toolbar.hsla())
            .text_color(h.frame_text.hsla())
            .child(
                div()
                    .font_weight(FontWeight::SEMIBOLD)
                    .mr(px(s.md))
                    .child("Dashboard (GPUI spike)"),
            )
            .children(CATALOG.iter().map(|entry| {
                host_button(
                    &theme,
                    ElementId::Name(format!("add-{}", entry.key).into()),
                    format!("Add {}", entry.name).into(),
                    false,
                )
                .on_click(
                    cx.listener(move |this, _, window, cx| this.request_add(entry, window, cx)),
                )
            }))
            .child(div().flex_1())
            .child(
                host_button(&theme, "settings".into(), "Settings".into(), false).on_click(
                    cx.listener(|this, _, window, cx| {
                        this.open_dialog(Dialog::Settings, window, cx)
                    }),
                ),
            );

        let status = self.status.clone().map(|text| {
            div()
                .id("status")
                .role(Role::Status)
                .test_support()
                .aria_label(text.clone())
                .px(px(s.lg))
                .py(px(s.xs))
                .bg(h.toolbar.hsla())
                .text_color(h.frame_text.hsla())
                .text_size(px(theme.typography.size_small))
                .child(text)
        });

        let grid_bounds = self.grid_bounds.clone();
        let rows = self.layout.rows();
        let target = self
            .drag
            .as_ref()
            .map(|drag| (drag.id, self.drag_target(drag)));
        let (cell_w, _) = self.cell_size();
        let frames: Vec<AnyElement> = self
            .layout
            .widgets
            .clone()
            .into_iter()
            .map(|placed| self.frame(&placed, cell_w, window, cx))
            .collect();

        let grid =
            div()
                .id("grid-scroll")
                .flex_1()
                .overflow_y_scroll()
                .p(px(GAP))
                .child(
                    div()
                        .id("grid")
                        .role(Role::Group)
                        .aria_label("Widgets")
                        .relative()
                        .w_full()
                        .h(px(rows as f32 * ROW))
                        .child(
                            canvas(move |bounds, _, _| grid_bounds.set(bounds), |_, _, _, _| {})
                                .absolute()
                                .size_full(),
                        )
                        .when_some(target, |this, (id, cell)| {
                            let free = self.layout.is_free(&cell, Some(id));
                            this.child(
                                div()
                                    .absolute()
                                    .left(relative(cell.x as f32 / 12.0))
                                    .top(px(cell.y as f32 * ROW))
                                    .w(relative(cell.w as f32 / 12.0))
                                    .h(px(cell.h as f32 * ROW))
                                    .p(px(GAP / 2.0))
                                    .child(
                                        div()
                                            .size_full()
                                            .rounded(px(s.radius))
                                            .border_2()
                                            .border_dashed()
                                            .border_color(if free {
                                                h.drop_target.hsla()
                                            } else {
                                                h.frame_border.hsla().opacity(0.5)
                                            }),
                                    ),
                            )
                        })
                        .children(frames),
                )
                .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                    this.drag_move(event.position, cx)
                }))
                .on_mouse_up(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, _, cx| this.end_drag(cx)),
                )
                .on_mouse_up_out(
                    MouseButton::Left,
                    cx.listener(|this, _: &MouseUpEvent, _, cx| this.end_drag(cx)),
                );

        let dialog = self
            .dialog
            .as_ref()
            .map(|dialog| self.dialog(dialog, &theme, cx));

        div()
            .id("dashboard")
            .track_focus(&self.root_focus)
            .size_full()
            .relative()
            .flex()
            .flex_col()
            .bg(h.canvas.hsla())
            .font_family(self.font.clone())
            .text_size(px(theme.typography.size_body))
            .on_action(cx.listener(|this, _: &MoveLeft, w, cx| {
                this.keyboard(w, cx, |l, id| l.nudge(id, -1, 0))
            }))
            .on_action(cx.listener(|this, _: &MoveRight, w, cx| {
                this.keyboard(w, cx, |l, id| l.nudge(id, 1, 0))
            }))
            .on_action(cx.listener(|this, _: &MoveUp, w, cx| {
                this.keyboard(w, cx, |l, id| l.nudge(id, 0, -1))
            }))
            .on_action(cx.listener(|this, _: &MoveDown, w, cx| {
                this.keyboard(w, cx, |l, id| l.nudge(id, 0, 1))
            }))
            .on_action(cx.listener(|this, _: &Narrower, w, cx| {
                this.keyboard(w, cx, |l, id| l.grow(id, -1, 0))
            }))
            .on_action(
                cx.listener(|this, _: &Wider, w, cx| {
                    this.keyboard(w, cx, |l, id| l.grow(id, 1, 0))
                }),
            )
            .on_action(cx.listener(|this, _: &Shorter, w, cx| {
                this.keyboard(w, cx, |l, id| l.grow(id, 0, -1))
            }))
            .on_action(
                cx.listener(|this, _: &Taller, w, cx| {
                    this.keyboard(w, cx, |l, id| l.grow(id, 0, 1))
                }),
            )
            .on_action(cx.listener(|this, _: &RemoveWidget, w, cx| this.remove(w, cx)))
            .child(toolbar)
            .children(status)
            .child(grid)
            .children(dialog)
    }
}

impl Dashboard {
    /// The host frame of one widget: a header the host draws (icon, name, plugin id, remove),
    /// and a body that clips the widget's content.
    fn frame(
        &mut self,
        placed: &crate::layout::Placed,
        cell_w: f32,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let theme = &self.theme;
        let h = &theme.host;
        let s = &theme.spacing;
        let id = placed.id;
        let entry = plugins::entry(&placed.entry);
        let name = entry.map(|e| e.name).unwrap_or("Unknown");
        let Some(view) = self.views.get(&id) else {
            return div().into_any_element();
        };
        let plugin_id = view.plugin_id.clone();
        let focused = view.focus.is_focused(window);
        let c = placed.cell;

        // While dragged by the mouse, the frame follows the pointer in pixels
        let dragging = self.drag.as_ref().filter(|d| d.id == id);
        let (left, top, width, height) = match dragging {
            Some(drag) if drag.kind == DragKind::Move => {
                let dx = f32::from(drag.mouse.x) - f32::from(drag.start_mouse.x);
                let dy = f32::from(drag.mouse.y) - f32::from(drag.start_mouse.y);
                (
                    px(c.x as f32 * cell_w + dx).into(),
                    px(c.y as f32 * ROW + dy),
                    relative(c.w as f32 / 12.0),
                    px(c.h as f32 * ROW),
                )
            }
            Some(drag) => {
                let dx = f32::from(drag.mouse.x) - f32::from(drag.start_mouse.x);
                let dy = f32::from(drag.mouse.y) - f32::from(drag.start_mouse.y);
                (
                    relative(c.x as f32 / 12.0),
                    px(c.y as f32 * ROW),
                    px((c.w as f32 * cell_w + dx).max(cell_w)).into(),
                    px((c.h as f32 * ROW + dy).max(ROW)),
                )
            }
            None => (
                relative(c.x as f32 / 12.0),
                px(c.y as f32 * ROW),
                relative(c.w as f32 / 12.0),
                px(c.h as f32 * ROW),
            ),
        };

        let widget_id: SharedString = format!("w{id}").into();
        let body: AnyElement = match &view.state {
            WidgetState::Loading => div()
                .text_color(theme.widget.palette.muted.hsla())
                .child("Loading…")
                .into_any_element(),
            WidgetState::Failed(error) => div()
                .id(("widget-error", id as usize))
                .role(Role::Alert)
                .test_support()
                .aria_label(SharedString::from(format!(
                    "This widget could not be shown: {error}"
                )))
                .text_color(theme.widget.palette.danger.hsla())
                .text_size(px(theme.typography.size_small))
                .child(SharedString::from(format!(
                    "This widget could not be shown: {error}"
                )))
                .into_any_element(),
            WidgetState::Ready(content) => {
                let view = cx.entity().downgrade();
                let on_action: widget_kit::render::OnAction =
                    Rc::new(move |action: &str, _window: &mut Window, cx: &mut App| {
                        let action = action.to_owned();
                        view.update(cx, |this, cx| this.widget_action(id, &action, cx))
                            .ok();
                    });
                widget_kit::render::content(&widget_id, content, &theme.widget, on_action)
            }
        };

        let label = format!(
            "{name}, plugin {plugin_id}. Column {}, row {}, {} by {} cells.",
            c.x + 1,
            c.y + 1,
            c.w,
            c.h
        );

        div()
            .absolute()
            .left(left)
            .top(top)
            .w(width)
            .h(height)
            .p(px(GAP / 2.0))
            .when(dragging.is_some(), |this| this.opacity(0.92))
            .child(
                div()
                    .id(("frame", id as usize))
                    .role(Role::Group)
                    .test_support()
                    .aria_label(SharedString::from(label))
                    .aria_description("Arrow keys move the widget, Shift and the arrow keys resize it, Delete removes it.")
                    .track_focus(&view.focus)
                    .key_context("WidgetFrame")
                    .size_full()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .rounded(px(s.radius + 2.0))
                    .bg(h.frame.hsla())
                    .border_2()
                    .border_color(if focused { h.frame_focus.hsla() } else { h.frame_border.hsla() })
                    // Header: drawn by the host, from the catalog and the manifest, never by the plugin
                    .child(
                        div()
                            .id(("frame-header", id as usize))
                            .flex()
                            .flex_none()
                            .items_center()
                            .gap(px(s.sm))
                            .px(px(s.sm))
                            .py(px(s.xs))
                            .text_color(h.frame_text.hsla())
                            .text_size(px(theme.typography.size_small))
                            .cursor_grab()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &MouseDownEvent, window, cx| this.start_drag(id, DragKind::Move, event, window, cx)),
                            )
                            .child(icon(name, theme))
                            .child(div().font_weight(FontWeight::SEMIBOLD).child(name))
                            .child(div().flex_1().min_w_0().overflow_hidden().opacity(0.8).child(plugin_id.clone()))
                            .child(
                                host_button(theme, ElementId::NamedInteger("remove".into(), id), "Remove".into(), true)
                                    .accessibility_label(SharedString::from(format!("Remove {name}")))
                                    .tab_stop(false)
                                    .on_click(cx.listener(move |this, _, window, cx| {
                                        this.layout.remove(id);
                                        this.views.remove(&id);
                                        this.save();
                                        window.focus_next(cx);
                                        cx.notify();
                                    })),
                            ),
                    )
                    // Body: the widget's content, on the widget surface, clipped and scrollable
                    .child(
                        div()
                            .id(("frame-body", id as usize))
                            .role(Role::Group)
                            .test_support()
                            .aria_label(SharedString::from(format!("{name} content")))
                            .flex_1()
                            .min_h_0()
                            .mx(px(2.0))
                            .mb(px(2.0))
                            .rounded(px(s.radius))
                            .bg(theme.widget.palette.surface.hsla())
                            .overflow_hidden()
                            .overflow_y_scroll()
                            .p(px(s.md))
                            .child(body),
                    )
                    // Resize handle
                    .child(
                        div()
                            .id(("frame-resize", id as usize))
                            .absolute()
                            .right_0()
                            .bottom_0()
                            .size(px(14.0))
                            .cursor_nwse_resize()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &MouseDownEvent, window, cx| this.start_drag(id, DragKind::Resize, event, window, cx)),
                            )
                            .child(div().absolute().right(px(3.0)).bottom(px(3.0)).size(px(6.0)).bg(h.frame_border.hsla())),
                    ),
            )
            .into_any_element()
    }

    fn dialog(&self, dialog: &Dialog, theme: &Theme, cx: &mut Context<Self>) -> AnyElement {
        let h = &theme.host;
        let s = &theme.spacing;
        let secret = self.settings.secret.as_ref().and_then(|name| {
            theme::secret_choices()
                .get(name)
                .map(|c| (name.clone(), *c))
        });
        let (title, body, buttons): (SharedString, AnyElement, AnyElement) = match dialog {
            Dialog::Approval { entry, request } => {
                let entry: &'static Entry = entry;
                let permissions = div()
                    .id("approval-permissions")
                    .role(Role::List)
                    .aria_label("Permissions")
                    .flex()
                    .flex_col()
                    .gap(px(s.xs))
                    .children(request.permissions.iter().enumerate().map(
                        |(i, (name, required))| {
                            let text = format!(
                                "{name} ({})",
                                if *required {
                                    "required"
                                } else {
                                    "optional, off until you turn it on"
                                }
                            );
                            div()
                                .id(("permission", i))
                                .role(Role::ListItem)
                                .test_support()
                                .aria_label(SharedString::from(text.clone()))
                                .child(format!("• {text}"))
                        },
                    ));
                let body = div()
                    .flex()
                    .flex_col()
                    .gap(px(s.sm))
                    .child(label(
                        "approval-what",
                        format!(
                            "{} {} by {}, built for contract {}.",
                            request.id, request.version, request.author, request.contract
                        ),
                    ))
                    .child(
                        label("approval-from", format!("From: {}", entry.package.origin))
                            .opacity(0.85),
                    )
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("It asks for:"),
                    )
                    .child(permissions)
                    .into_any_element();
                let buttons = div()
                    .flex()
                    .justify_end()
                    .gap(px(s.sm))
                    .child(
                        host_button(theme, "approval-cancel".into(), "Cancel".into(), false)
                            .on_click(cx.listener(|this, _, w, cx| this.close_dialog(w, cx))),
                    )
                    .child(
                        host_button(
                            theme,
                            "approval-approve".into(),
                            "Approve and add".into(),
                            true,
                        )
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                match this.host.approve(entry.package) {
                                    Ok(()) => {
                                        this.dialog = None;
                                        this.add(entry, window, cx);
                                    }
                                    Err(error) => {
                                        this.status =
                                            Some(format!("Approval failed: {error}").into())
                                    }
                                }
                                cx.notify();
                            },
                        )),
                    )
                    .into_any_element();
                (format!("Add {}?", entry.name).into(), body, buttons)
            }
            Dialog::Settings => {
                let current = self.settings.secret.clone();
                let swatches = div()
                    .id("secret-choices")
                    .role(Role::RadioGroup)
                    .aria_label("Your color")
                    .flex()
                    .flex_wrap()
                    .gap(px(s.sm))
                    .children(theme::secret_choices().into_iter().map(|(name, color)| {
                        let selected = current.as_deref() == Some(name.as_str());
                        let chosen = name.clone();
                        BaseButton::new(ElementId::Name(format!("secret-{name}").into()))
                            .role(Role::RadioButton)
                            .selected(selected)
                            .accessibility_label(SharedString::from(name.clone()))
                            .flex()
                            .items_center()
                            .gap(px(s.xs))
                            .px(px(s.sm))
                            .py(px(s.xs))
                            .rounded(px(s.radius))
                            .border_2()
                            .border_color(if selected {
                                h.dialog_text.hsla()
                            } else {
                                h.dialog_border.hsla().opacity(0.3)
                            })
                            .focus_visible(|style| style.border_color(h.frame_focus.hsla()))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.settings.secret = Some(chosen.clone());
                                this.save();
                                cx.notify();
                            }))
                            .child(div().size(px(14.0)).rounded_full().bg(color.hsla()))
                            .child(SharedString::from(name))
                    }));
                let modes = div().flex().gap(px(s.sm)).children(
                    [
                        (Mode::System, "System"),
                        (Mode::Light, "Light"),
                        (Mode::Dark, "Dark"),
                    ]
                    .map(|(mode, label)| {
                        host_button(
                            theme,
                            ElementId::Name(format!("mode-{label}").into()),
                            label.into(),
                            self.settings.mode == mode,
                        )
                        .selected(self.settings.mode == mode)
                        .on_click(cx.listener(
                            move |this, _, window, cx| {
                                this.settings.mode = mode;
                                this.save();
                                this.apply_mode(window, cx);
                            },
                        ))
                    }),
                );
                let body = div()
                    .flex()
                    .flex_col()
                    .gap(px(s.md))
                    .child(div().child(
                        "Choose your color. Dashboard shows it at the top of every one of its dialogs; no widget can see it or draw it, so a dialog without it is not from Dashboard.",
                    ))
                    .child(swatches)
                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Appearance"))
                    .child(modes)
                    .into_any_element();
                let buttons = div()
                    .flex()
                    .justify_end()
                    .child(
                        host_button(theme, "settings-done".into(), "Done".into(), true)
                            .disabled(self.settings.secret.is_none())
                            .on_click(cx.listener(|this, _, w, cx| {
                                if this.settings.secret.is_some() {
                                    this.close_dialog(w, cx)
                                }
                            })),
                    )
                    .into_any_element();
                ("Settings".into(), body, buttons)
            }
        };

        let can_close = self.settings.secret.is_some() || !matches!(dialog, Dialog::Settings);
        div()
            .id("dialog-layer")
            .absolute()
            .inset_0()
            .flex()
            .items_center()
            .justify_center()
            .occlude()
            .child(div().absolute().inset_0().bg(h.scrim.hsla().opacity(0.55)))
            .child(
                div()
                    .id("host-dialog")
                    .role(Role::Dialog)
                    .test_support()
                    .aria_label(title.clone())
                    .child(
                        div()
                            .key_context("HostDialog")
                            .track_focus(&self.dialog_focus)
                            .on_action(cx.listener(move |this, _: &CloseDialog, w, cx| {
                                if can_close {
                                    this.close_dialog(w, cx)
                                }
                            }))
                            .relative()
                            .w(px(460.0))
                            .flex()
                            .flex_col()
                            .overflow_hidden()
                            .rounded(px(s.radius + 4.0))
                            .bg(h.dialog.hsla())
                            .border_2()
                            .border_color(h.dialog_border.hsla())
                            .text_color(h.dialog_text.hsla())
                            // The visual secret: a band in the person's color, and its name
                            .child(match &secret {
                                Some((name, color)) => div()
                                    .id("visual-secret")
                                    .role(Role::Label)
                                    .test_support()
                                    .aria_label(SharedString::from(format!(
                                        "Your color, {name}: this dialog is from Dashboard"
                                    )))
                                    // AT-SPI names a label by its value
                                    .aria_value(SharedString::from(format!(
                                        "Your color, {name}: this dialog is from Dashboard"
                                    )))
                                    .flex()
                                    .items_center()
                                    .gap(px(s.sm))
                                    .px(px(s.lg))
                                    .py(px(s.xs))
                                    .bg(color.hsla())
                                    .text_color(if color.luminance() > 0.35 {
                                        gpui_kit::black()
                                    } else {
                                        gpui_kit::white()
                                    })
                                    .text_size(px(theme.typography.size_small))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(SharedString::from(format!("Your color: {name}")))
                                    .into_any_element(),
                                None => div()
                                    .px(px(s.lg))
                                    .py(px(s.xs))
                                    .bg(h.toolbar.hsla())
                                    .text_size(px(theme.typography.size_small))
                                    .child("No color chosen yet")
                                    .into_any_element(),
                            })
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap(px(s.md))
                                    .p(px(s.lg))
                                    .child(
                                        div()
                                            .id("dialog-title")
                                            .role(Role::Heading)
                                            .aria_label(title.clone())
                                            .text_size(px(theme.typography.size_heading))
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .child(title),
                                    )
                                    .child(body)
                                    .child(buttons),
                            )
                            .focus_trap("host-dialog-trap", &self.dialog_focus),
                    ),
            )
            .into_any_element()
    }
}

/// Host text that screen readers read: AT-SPI names a label by its value, not its label.
fn label(id: &'static str, text: String) -> gpui_kit::Stateful<gpui_kit::Div> {
    let text = SharedString::from(text);
    div()
        .id(id)
        .role(Role::Label)
        .aria_value(text.clone())
        .child(text)
}

/// A host button, drawn with host tokens on gpui-base's unstyled button.
fn host_button(theme: &Theme, id: ElementId, label: SharedString, strong: bool) -> BaseButton {
    let h = &theme.host;
    let s = &theme.spacing;
    BaseButton::new(id)
        .accessibility_label(label.clone())
        .px(px(s.md))
        .py(px(s.xs))
        .rounded(px(s.radius))
        .text_size(px(theme.typography.size_small))
        .border_2()
        .when(strong, |this| {
            this.bg(h.host_accent.hsla())
                .text_color(h.host_accent_text.hsla())
                .border_color(h.host_accent.hsla())
        })
        .when(!strong, |this| {
            this.bg(h.frame.hsla())
                .text_color(h.frame_text.hsla())
                .border_color(h.frame_border.hsla().opacity(0.6))
        })
        .focus_visible(|style| style.border_color(h.frame_focus.hsla()))
        .styles(|styles| {
            styles
                .selected(|style| style.border_color(h.frame_text.hsla()))
                .disabled(|style| style.opacity(0.5))
        })
        .child(label)
}

/// The plugin's icon. Manifests have no icon yet, so the host draws a monogram, in host colors.
fn icon(name: &str, theme: &Theme) -> impl IntoElement {
    let letter: String = name
        .chars()
        .next()
        .map(|c| c.to_uppercase().collect())
        .unwrap_or_default();
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(18.0))
        .rounded(px(4.0))
        .bg(theme.host.host_accent.hsla())
        .text_color(theme.host.host_accent_text.hsla())
        .text_size(px(11.0))
        .font_weight(FontWeight::BOLD)
        .child(letter)
}

fn is_dark(mode: Mode, window: &Window) -> bool {
    match mode {
        Mode::Light => false,
        Mode::Dark => true,
        Mode::System => matches!(
            window.appearance(),
            WindowAppearance::Dark | WindowAppearance::VibrantDark
        ),
    }
}

/// The gpui-component theme follows the host's, for the components the host uses from it.
fn sync_component_theme(dark: bool, window: &mut Window, cx: &mut App) {
    use gpui_kit::component::{Theme as ComponentTheme, ThemeMode};
    ComponentTheme::change(
        if dark {
            ThemeMode::Dark
        } else {
            ThemeMode::Light
        },
        Some(window),
        cx,
    );
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &std::path::Path) -> Option<T> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn write_json<T: Serialize>(path: &std::path::Path, value: &T) {
    if let Ok(text) = serde_json::to_vec_pretty(value) {
        let _ = std::fs::write(path, text);
    }
}

/// This process's CPU time (user + system), for the drag bench.
fn cpu_seconds() -> Option<f64> {
    #[cfg(target_os = "linux")]
    {
        let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
        let fields: Vec<&str> = stat.rsplit_once(')')?.1.split_whitespace().collect();
        let ticks: f64 =
            fields.get(11)?.parse::<f64>().ok()? + fields.get(12)?.parse::<f64>().ok()?;
        Some(ticks / 100.0)
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}
