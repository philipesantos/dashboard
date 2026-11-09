//! The host dialog (FR-014 to FR-018, FR-020, FR-021; contracts/dialog.md; research R9). Every
//! host decision is asked through [`ask`]; at most one dialog is shown, the rest wait in order.
//! While one is open, every host window draws a scrim over all of it, chrome included, that takes
//! every pointer event; windows other than the dialog's drop every key and lose their focus,
//! given back when it is answered. Closing a window or quitting answers it as a refusal.

pub mod gate;
pub mod view;

use crate::label::Label;
use crate::text::HostText;
use futures::channel::oneshot;
use gate::{Clock, Gate, SystemClock};
use gpui::{AnyWindowHandle, App, FocusHandle, Global};
use std::collections::{HashSet, VecDeque};
use std::sync::Arc;

/// What the host asks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    /// The host's name for the decision, such as "Remove a widget".
    pub name: HostText,
    /// The question, in the host's words.
    pub question: HostText,
    /// A plugin's name, shown and exposed after the host's words.
    pub subject: Option<Label>,
    /// The first control and the initial focus; never the default.
    pub refuse: HostText,
    /// The last control; never the default.
    pub confirm: HostText,
}

/// The answer. Closing a window, quitting and Escape answer `Refused`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Confirmed,
    Refused,
}

struct Pending {
    request: Request,
    window: AnyWindowHandle,
    answer: oneshot::Sender<Answer>,
}

/// The dialog on screen.
pub struct Open {
    pub request: Request,
    /// The window the dialog is drawn in; every other host window shows only the scrim.
    pub window: AnyWindowHandle,
    pub gate: Gate,
    pub refuse: FocusHandle,
    pub confirm: FocusHandle,
    /// The trap that keeps keyboard focus inside the dialog.
    pub trap: FocusHandle,
    /// Increases with every dialog shown, so a view can tell a new dialog from the same one.
    pub generation: u64,
    /// When it was asked for, on the dialogs' clock, to measure how soon it shows (SC-006).
    pub asked_at: std::time::Duration,
    answer: Option<oneshot::Sender<Answer>>,
    saved_focus: Vec<(AnyWindowHandle, Option<FocusHandle>)>,
}

/// The host's dialogs, one global per app.
pub struct DialogHost {
    clock: Arc<dyn Clock>,
    queue: VecDeque<Pending>,
    open: Option<Open>,
    generation: u64,
    /// Keys down right now, as the host windows saw them, for the gate's held keys.
    pressed: HashSet<String>,
}

impl Global for DialogHost {}

impl Default for DialogHost {
    fn default() -> Self {
        DialogHost {
            clock: Arc::new(SystemClock::default()),
            queue: VecDeque::new(),
            open: None,
            generation: 0,
            pressed: HashSet::new(),
        }
    }
}

/// Installs the dialog host on the app; [`crate::app::install`] calls it.
pub fn init(cx: &mut App) {
    cx.set_global(DialogHost::default());
}

/// Replaces the clock, for tests.
pub fn set_clock(cx: &mut App, clock: Arc<dyn Clock>) {
    cx.global_mut::<DialogHost>().clock = clock;
}

pub fn now(cx: &App) -> std::time::Duration {
    cx.global::<DialogHost>().clock.now()
}

/// The dialog on screen, if any.
pub fn open(cx: &App) -> Option<&Open> {
    cx.try_global::<DialogHost>()
        .and_then(|host| host.open.as_ref())
}

pub fn is_open(cx: &App) -> bool {
    open(cx).is_some()
}

/// Asks `request` in `window`. Shown at once when no other dialog is open, else after the ones
/// before it (FR-018).
pub fn ask(cx: &mut App, window: AnyWindowHandle, request: Request) -> oneshot::Receiver<Answer> {
    let (sender, receiver) = oneshot::channel();
    cx.global_mut::<DialogHost>().queue.push_back(Pending {
        request,
        window,
        answer: sender,
    });
    if !is_open(cx) {
        show_next(cx);
    }
    receiver
}

fn show_next(cx: &mut App) {
    let Some(pending) = cx.global_mut::<DialogHost>().queue.pop_front() else {
        return;
    };
    let refuse = cx.focus_handle().tab_stop(true);
    let confirm = cx.focus_handle().tab_stop(true);
    let trap = cx.focus_handle();
    let host = cx.global_mut::<DialogHost>();
    host.generation += 1;
    host.open = Some(Open {
        request: pending.request,
        window: pending.window,
        gate: Gate::default(),
        refuse,
        confirm,
        trap,
        generation: host.generation,
        asked_at: host.clock.now(),
        answer: Some(pending.answer),
        saved_focus: Vec::new(),
    });
    let generation = host.generation;
    // Outside whatever window update asked: remember every window's focus and take it away;
    // the dialog's window gives it to the refusing control on the dialog's first frame
    // (start_gate_if_needed), so no focused node outside the dialog stays exposed (HS-16, HS-18)
    cx.defer(move |cx| {
        let mut saved = Vec::new();
        for handle in cx.windows() {
            let focused = handle
                .update(cx, |_, window, cx| {
                    let focused = window.focused(cx);
                    window.blur(cx);
                    focused
                })
                .ok()
                .flatten();
            saved.push((handle, focused));
        }
        if let Some(open) = cx.global_mut::<DialogHost>().open.as_mut()
            && open.generation == generation
        {
            open.saved_focus = saved;
        }
        refresh_windows(cx);
    });
}

/// Answers the open dialog, gives every window its focus back and shows the next one.
pub fn answer(cx: &mut App, answer: Answer) {
    let Some(mut open) = cx.global_mut::<DialogHost>().open.take() else {
        return;
    };
    if let Some(sender) = open.answer.take() {
        let _ = sender.send(answer);
    }
    // Outside whatever window update answered, before the next dialog saves focus again
    let saved = std::mem::take(&mut open.saved_focus);
    cx.defer(move |cx| {
        for (handle, focused) in saved {
            let _ = handle.update(cx, |_, window, cx| match &focused {
                Some(focus) => window.focus(focus, cx),
                None => window.blur(cx),
            });
        }
        refresh_windows(cx);
    });
    show_next(cx);
}

/// Answers the open dialog as a refusal, if there is one: a window's close request and the app's
/// quit call it before they proceed (HS-10).
pub fn refuse_if_open(cx: &mut App) {
    if is_open(cx) {
        answer(cx, Answer::Refused);
    }
}

/// On the first frame that shows the dialog: starts the gate and puts focus on the refusing
/// control (HS-16). Returns that control when it should take focus now.
pub fn start_gate_if_needed(cx: &mut App) -> Option<FocusHandle> {
    // Read first: changing the global redraws the windows, and this runs while drawing
    if open(cx).is_none_or(|open| open.gate.is_started()) {
        return None;
    }
    let host = cx.global_mut::<DialogHost>();
    let now = host.clock.now();
    let pressed = host.pressed.clone();
    let open = host.open.as_mut()?;
    open.gate.start(now, &pressed);
    Some(open.refuse.clone())
}

/// Starts the 1000 ms over: the dialog's window regained focus (HS-11).
pub fn restart_gate(cx: &mut App) {
    let host = cx.global_mut::<DialogHost>();
    let now = host.clock.now();
    let pressed = host.pressed.clone();
    if let Some(open) = host.open.as_mut() {
        open.gate.start(now, &pressed);
    }
}

/// A key went down in a host window: remembered for the gate's held keys.
pub fn key_down(cx: &mut App, key: &str) {
    let known = cx
        .try_global::<DialogHost>()
        .is_none_or(|host| host.pressed.contains(key));
    if !known {
        cx.global_mut::<DialogHost>().pressed.insert(key.to_owned());
    }
}

/// A key went up in a host window.
pub fn key_up(cx: &mut App, key: &str) {
    if !cx.has_global::<DialogHost>() {
        return;
    }
    let host = cx.global_mut::<DialogHost>();
    host.pressed.remove(key);
    if let Some(open) = host.open.as_mut() {
        open.gate.key_released(key);
    }
}

/// A key press on the dialog: answers with `answer` when the gate accepts it.
pub fn key_press(cx: &mut App, key: &str, is_repeat: bool, answer_with: Answer) -> bool {
    let host = cx.global::<DialogHost>();
    let now = host.clock.now();
    let accepted = host
        .open
        .as_ref()
        .is_some_and(|open| open.gate.accepts_key(now, key, is_repeat));
    if accepted {
        answer(cx, answer_with);
    }
    accepted
}

/// A pointer press on one of the dialog's controls.
pub fn pointer_down(cx: &mut App) {
    let host = cx.global_mut::<DialogHost>();
    let now = host.clock.now();
    if let Some(open) = host.open.as_mut() {
        open.gate.pointer_down(now);
    }
}

/// A click on one of the dialog's controls: answers with `answer_with` when the gate accepts it.
pub fn click(cx: &mut App, answer_with: Answer) -> bool {
    let host = cx.global::<DialogHost>();
    let now = host.clock.now();
    let accepted = host
        .open
        .as_ref()
        .is_some_and(|open| open.gate.accepts_click(now));
    if accepted {
        answer(cx, answer_with);
    }
    accepted
}

/// Redraws every host window, so each shows or drops the scrim.
fn refresh_windows(cx: &mut App) {
    for handle in cx.windows() {
        let _ = handle.update(cx, |_, window, _| window.refresh());
    }
}
