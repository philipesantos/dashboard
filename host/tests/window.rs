//! The host in a real window, with real input from the operating system and the accessibility
//! tree read through the platform (research R13, layers 2 and 3): AT-SPI under Xvfb on Linux, the
//! AX API on macOS, UI Automation on Windows. GPUI's own tests never send OS events and its fake
//! window has no accessibility tree, so these need the real thing.
//!
//! Run alone, as CI's "Host window" job does: `cargo test -p host --test window`. On Linux it
//! needs an X display, `xdotool`, an AT-SPI bus with `IsEnabled` set and Python's `gi` with
//! Atspi (quickstart §6). It is `test = false`, so `cargo test --workspace` never runs it.

mod common;

use gpui::{
    AppContext as _, Bounds, SharedString, TitlebarOptions, WindowBounds, WindowHandle,
    WindowOptions, point, px, size,
};
use host::dashboard::Dashboard;
use host::plugins::{FrameStatus, start_runtime};
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const TITLE_A: &str = "Dashboard A";
const TITLE_B: &str = "Dashboard B";

/// What a window shows, as the app publishes it every few milliseconds.
#[derive(Clone, Debug, Default)]
struct WindowState {
    focused: Option<String>,
    open_menu: Option<usize>,
    open_panel: Option<usize>,
    reloads: u64,
    frames: usize,
    loading: usize,
}

#[derive(Clone, Debug, Default)]
struct Snapshot {
    a: WindowState,
    b: WindowState,
    dialog_open: bool,
    /// From the dialog shown to its first frame (SC-006).
    dialog_first_frame: Option<Duration>,
    /// From start to every frame drawn with its status (SC-001).
    all_rendered: Option<Duration>,
}

type Shared = Arc<Mutex<Snapshot>>;

fn snapshot(shared: &Shared) -> Snapshot {
    shared.lock().expect("the snapshot").clone()
}

/// Waits up to five seconds for `condition`, and says what it waited for when it gives up.
fn wait(shared: &Shared, what: &str, condition: impl Fn(&Snapshot) -> bool) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let now = snapshot(shared);
        if condition(&now) {
            return Ok(());
        }
        if Instant::now() > deadline {
            return Err(format!("{what}: gave up waiting; last state {now:?}"));
        }
        std::thread::sleep(Duration::from_millis(30));
    }
}

fn script(name: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/window")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn run(program: &str, args: &[&str]) -> String {
    let output = Command::new(program)
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("{program}: {error}"));
    // A reader the system refuses (no accessibility access, no bus) says why only here
    let errors = String::from_utf8_lossy(&output.stderr);
    if !errors.trim().is_empty() || !output.status.success() {
        eprintln!(
            "window: {program} {} ({}): {}",
            args.join(" "),
            output.status,
            errors.trim()
        );
    }
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[derive(Clone, Copy, Debug)]
enum Key {
    Tab,
    ShiftTab,
    Enter,
    Escape,
}

/// Real input and the platform's accessibility tree, on this system.
struct Os;

impl Os {
    fn activate(&self, title: &str) {
        if cfg!(target_os = "linux") {
            let ids = run("xdotool", &["search", "--name", &format!("^{title}$")]);
            if let Some(id) = ids.lines().next() {
                run("xdotool", &["windowfocus", "--sync", id]);
            }
        } else if cfg!(target_os = "macos") {
            let pid = std::process::id();
            run(
                "osascript",
                &[
                    "-e",
                    &format!(
                        "tell application \"System Events\" to tell (first process whose unix id is {pid})\nset frontmost to true\nperform action \"AXRaise\" of (first window whose name is \"{title}\")\nend tell"
                    ),
                ],
            );
        } else {
            run(
                "powershell",
                &["-NoProfile", "-File", &script("uia.ps1"), "activate", title],
            );
        }
        std::thread::sleep(Duration::from_millis(300));
    }

    fn key(&self, key: Key) {
        self.keys(&[key]);
    }

    /// Sends `keys` in one call of the system's input tool, so they arrive milliseconds apart
    /// however long the tool takes to start.
    fn keys(&self, keys: &[Key]) {
        if cfg!(target_os = "linux") {
            let names: Vec<&str> = keys
                .iter()
                .map(|key| match key {
                    Key::Tab => "Tab",
                    Key::ShiftTab => "shift+Tab",
                    Key::Enter => "Return",
                    Key::Escape => "Escape",
                })
                .collect();
            let mut args = vec!["key", "--clearmodifiers"];
            args.extend(names);
            run("xdotool", &args);
        } else if cfg!(target_os = "macos") {
            let codes: Vec<&str> = keys
                .iter()
                .map(|key| match key {
                    Key::Tab => "key code 48",
                    Key::ShiftTab => "key code 48 using {shift down}",
                    Key::Enter => "key code 36",
                    Key::Escape => "key code 53",
                })
                .collect();
            let script = format!(
                "tell application \"System Events\"\n{}\nend tell",
                codes.join("\n")
            );
            run("osascript", &["-e", &script]);
        } else {
            let sequence: String = keys
                .iter()
                .map(|key| match key {
                    Key::Tab => "{TAB}",
                    Key::ShiftTab => "+{TAB}",
                    Key::Enter => "{ENTER}",
                    Key::Escape => "{ESC}",
                })
                .collect();
            run(
                "powershell",
                &["-NoProfile", "-File", &script("uia.ps1"), "keys", &sequence],
            );
        }
        std::thread::sleep(Duration::from_millis(150));
    }

    fn click(&self, x: f64, y: f64) {
        let (x, y) = (format!("{}", x.round()), format!("{}", y.round()));
        if cfg!(target_os = "linux") {
            run("xdotool", &["mousemove", "--sync", &x, &y, "click", "1"]);
        } else if cfg!(target_os = "macos") {
            run("swift", &[&script("click.swift"), &x, &y]);
        } else {
            run(
                "powershell",
                &["-NoProfile", "-File", &script("uia.ps1"), "click", &x, &y],
            );
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    /// `role<TAB>name` of every node of the window titled `title`.
    fn dump(&self, title: &str) -> Vec<String> {
        let out = if cfg!(target_os = "linux") {
            run("python3", &[&script("atspi.py"), "dump", title])
        } else if cfg!(target_os = "macos") {
            // The AX API directly: System Events returns nothing for an AccessKit tree
            let pid = std::process::id().to_string();
            run("swift", &[&script("ax.swift"), "dump", &pid, title])
        } else {
            run(
                "powershell",
                &["-NoProfile", "-File", &script("uia.ps1"), "dump", title],
            )
        };
        out.lines().map(str::to_owned).collect()
    }

    /// The centre, in screen coordinates, of the node named `name` in the window `title`.
    fn centre(&self, title: &str, name: &str) -> Option<(f64, f64)> {
        let out = if cfg!(target_os = "linux") {
            // AT-SPI's screen coordinates rest on the window position GPUI reports, which is off
            // with no window manager: take the node's place in its window, and the window's
            // place on the screen from X itself
            let ids = run("xdotool", &["search", "--name", &format!("^{title}$")]);
            let id = ids.split_whitespace().next()?;
            let geometry = run("xdotool", &["getwindowgeometry", "--shell", id]);
            let field = |key: &str| -> Option<f64> {
                geometry
                    .lines()
                    .find_map(|line| line.strip_prefix(key))
                    .and_then(|value| value.trim().parse().ok())
            };
            let (left, top) = (field("X=")?, field("Y=")?);
            let found = run(
                "python3",
                &[&script("atspi.py"), "find-in-window", title, name],
            );
            let numbers: Vec<f64> = found
                .split_whitespace()
                .filter_map(|word| word.parse().ok())
                .collect();
            return match numbers.as_slice() {
                [x, y, w, h, ..] => Some((left + x + w / 2.0, top + y + h / 2.0)),
                _ => None,
            };
        } else if cfg!(target_os = "macos") {
            let pid = std::process::id().to_string();
            run("swift", &[&script("ax.swift"), "find", &pid, title, name])
        } else {
            run(
                "powershell",
                &[
                    "-NoProfile",
                    "-File",
                    &script("uia.ps1"),
                    "find",
                    title,
                    name,
                ],
            )
        };
        let numbers: Vec<f64> = out
            .split_whitespace()
            .filter_map(|word| word.trim_end_matches(',').parse().ok())
            .collect();
        match numbers.as_slice() {
            [x, y, w, h, ..] => Some((x + w / 2.0, y + h / 2.0)),
            _ => None,
        }
    }
}

/// Why a control could not be clicked, with the tree the platform showed.
fn missing(os: &Os, title: &str, name: &str) -> String {
    format!(
        "the platform tree of {title} has no {name:?} to click:\n{}",
        os.dump(title).join("\n")
    )
}

fn names(lines: &[String]) -> String {
    lines.join("\n").to_lowercase()
}

/// Every check, in order; the first that fails stops the run with its reason.
fn drive(shared: &Shared) -> Result<Vec<String>, String> {
    let os = Os;
    let mut report = Vec::new();
    wait(shared, "every frame drawn", |s| {
        s.a.frames == 2 && s.a.loading == 0 && s.b.frames == 2 && s.b.loading == 0
    })?;
    // The first query turns GPUI's accessibility tree on
    let _ = os.dump(TITLE_A);
    let _ = os.dump(TITLE_B);
    std::thread::sleep(Duration::from_millis(500));

    // FR-005: the first Tab reaches the first control, the chrome's "Appearance"
    os.activate(TITLE_A);
    os.key(Key::Tab);
    wait(shared, "the first Tab focuses Appearance", |s| {
        s.a.focused.as_deref() == Some("appearance")
    })?;
    os.key(Key::Tab);
    wait(shared, "the next Tab focuses Reload", |s| {
        s.a.focused.as_deref() == Some("reload")
    })?;
    report.push("FR-005: the first Tab reaches the chrome's first control".to_owned());

    // FR-010: the menu, the panel, Escape and focus back on the menu button
    os.key(Key::Tab);
    wait(shared, "Tab reaches the first frame's menu button", |s| {
        s.a.focused.as_deref() == Some("menu-0")
    })?;
    os.key(Key::Enter);
    wait(shared, "Enter opens the menu", |s| {
        s.a.open_menu == Some(0) && s.a.focused.as_deref() == Some("settings")
    })?;
    os.key(Key::Enter);
    wait(shared, "Enter on Settings opens the panel", |s| {
        s.a.open_panel == Some(0) && s.a.focused.as_deref() == Some("panel-close")
    })?;
    os.key(Key::Escape);
    wait(
        shared,
        "Escape closes the panel, focus on the menu button",
        |s| s.a.open_panel.is_none() && s.a.focused.as_deref() == Some("menu-0"),
    )?;
    report.push("FR-010: the panel opens and closes on Escape, focus back".to_owned());

    // FR-010: a click outside closes the panel and reaches its target
    os.key(Key::Enter);
    os.key(Key::Enter);
    wait(shared, "the panel opens again", |s| {
        s.a.open_panel == Some(0)
    })?;
    let reloads = snapshot(shared).a.reloads;
    let (x, y) = os
        .centre(TITLE_A, "Reload")
        .ok_or_else(|| missing(&os, TITLE_A, "Reload"))?;
    os.click(x, y);
    wait(
        shared,
        "a click on Reload closes the panel and reloads",
        |s| s.a.open_panel.is_none() && s.a.reloads > reloads,
    )?;
    report.push("FR-010: a click outside closes the panel and reaches Reload".to_owned());
    wait(shared, "the reload finishes", |s| {
        s.a.frames == 2 && s.a.loading == 0
    })?;

    // HS-16, HS-11, HS-17: the dialog from the keyboard
    os.activate(TITLE_A);
    let start = snapshot(shared).a.focused.clone();
    if start.as_deref() != Some("menu-0") {
        os.key(Key::Tab);
        if snapshot(shared).a.focused.as_deref() != Some("menu-0") {
            os.key(Key::Tab);
        }
    }
    wait(shared, "focus on the menu button", |s| {
        s.a.focused.as_deref() == Some("menu-0")
    })?;
    os.key(Key::Enter);
    os.key(Key::Tab);
    wait(shared, "Tab reaches Remove from dashboard", |s| {
        s.a.focused.as_deref() == Some("remove")
    })?;
    // Enter opens the dialog and a second Enter follows it within milliseconds, in one call of
    // the input tool: it lands as the dialog appears, before or just after its first frame
    os.keys(&[Key::Enter, Key::Enter]);
    wait(shared, "the dialog opens with focus on Cancel", |s| {
        s.dialog_open && s.a.focused.as_deref() == Some("dialog-refuse")
    })?;
    report.push("HS-16: focus starts on Cancel (keyboard)".to_owned());
    std::thread::sleep(Duration::from_millis(300));
    let after = snapshot(shared);
    if !after.dialog_open {
        return Err("HS-11: Enter right after the dialog appeared answered it".to_owned());
    }
    if after.a.open_menu.is_some() {
        return Err("HS-10: Enter right after the dialog appeared reached the menu".to_owned());
    }
    report
        .push("HS-11: a key as the dialog appears answers nothing and reaches nothing".to_owned());
    std::thread::sleep(Duration::from_millis(1200));
    for key in [Key::Tab, Key::ShiftTab] {
        for _ in 0..5 {
            os.key(key);
            let focused = snapshot(shared).a.focused;
            if !matches!(focused.as_deref(), Some("dialog-refuse" | "dialog-confirm")) {
                return Err(format!("HS-17: {key:?} left the dialog for {focused:?}"));
            }
        }
    }
    report.push("HS-17: Tab and Shift+Tab stay in the dialog".to_owned());

    // HS-18, HS-19: the platform sees only the dialog, in both windows
    let exact_a = os.dump(TITLE_A).join("\n");
    let seen_a = exact_a.to_lowercase();
    let seen_b = names(&os.dump(TITLE_B));
    // Exact case: the names come from the host's catalog, never from the capitals drawn on
    // screen, which a reader could spell out letter by letter
    for expected in ["Remove a widget", "Cancel", "Remove"] {
        if !exact_a.contains(expected) {
            return Err(format!(
                "HS-19: the dialog's {expected:?} is not exposed:\n{exact_a}"
            ));
        }
    }
    // Exactly the dialog: each window's own node, and in A the dialog's name, its subject (the
    // plugin's name, once, after the host's words) and its two buttons; nothing else
    let name_of = |line: &str| line.rsplit('\t').next().unwrap_or("").to_owned();
    let mut subject_seen = false;
    let mut dialog_seen = false;
    for line in seen_a.lines() {
        let name = name_of(line);
        let allowed = match name.as_str() {
            "dashboard a" | "cancel" | "remove" => true,
            "remove a widget" => {
                dialog_seen = true;
                true
            }
            "hello-widget" if dialog_seen && !subject_seen => {
                subject_seen = true;
                true
            }
            _ => false,
        };
        if !allowed {
            return Err(format!(
                "HS-18: {line:?} is exposed in A while the dialog is open:\nA:\n{seen_a}\nB:\n{seen_b}"
            ));
        }
    }
    for line in seen_b.lines() {
        if !matches!(name_of(line).as_str(), "dashboard b" | "") {
            return Err(format!(
                "HS-18: {line:?} is exposed in B while the dialog is open:\nA:\n{seen_a}\nB:\n{seen_b}"
            ));
        }
    }
    report.push("HS-18, HS-19: only the dialog is exposed, in both windows".to_owned());

    // HS-10, HS-11: a key in the other window answers nothing; refocusing restarts the delay
    os.activate(TITLE_B);
    os.key(Key::Enter);
    std::thread::sleep(Duration::from_millis(300));
    if !snapshot(shared).dialog_open {
        return Err("HS-10: Enter in the other window answered the dialog".to_owned());
    }
    report.push("HS-10: a key in the other window answers nothing".to_owned());
    os.activate(TITLE_A);
    std::thread::sleep(Duration::from_millis(1300));
    os.key(Key::Escape);
    wait(
        shared,
        "Escape refuses, focus back on the menu button",
        |s| !s.dialog_open && s.a.frames == 2 && s.a.focused.as_deref() == Some("menu-0"),
    )?;
    report.push("HS-17: Escape refuses and gives focus back".to_owned());

    // HS-16 from the pointer
    let (x, y) = os
        .centre(TITLE_A, "Widget menu")
        .ok_or_else(|| missing(&os, TITLE_A, "Widget menu"))?;
    os.click(x, y);
    wait(shared, "a click opens the menu", |s| {
        s.a.open_menu == Some(0)
    })?;
    let (x, y) = os
        .centre(TITLE_A, "Remove from dashboard")
        .ok_or_else(|| missing(&os, TITLE_A, "Remove from dashboard"))?;
    os.click(x, y);
    wait(
        shared,
        "the dialog opens with focus on Cancel (pointer)",
        |s| s.dialog_open && s.a.focused.as_deref() == Some("dialog-refuse"),
    )?;
    report.push("HS-16: focus starts on Cancel (pointer)".to_owned());
    std::thread::sleep(Duration::from_millis(1300));
    let (x, y) = os
        .centre(TITLE_A, "Cancel")
        .ok_or_else(|| missing(&os, TITLE_A, "Cancel"))?;
    os.click(x, y);
    wait(shared, "a click on Cancel refuses", |s| {
        !s.dialog_open && s.a.frames == 2
    })?;
    report.push("HS-11: a click after the delay answers".to_owned());

    // FR-024: the frames' names and statuses, as the widgets' content
    std::thread::sleep(Duration::from_millis(300));
    let seen = os.dump(TITLE_A).join("\n");
    for expected in [
        "hello-widget",
        "out-of-bounds",
        "Rendered",
        "Did not render",
    ] {
        if !seen.contains(expected) {
            return Err(format!("FR-024: {expected:?} is not exposed:\n{seen}"));
        }
    }
    for drawn in ["RENDERED", "DID NOT RENDER", "CANCEL"] {
        if seen.contains(drawn) {
            return Err(format!(
                "FR-024: the capitals drawn on screen reach the tree as {drawn:?}"
            ));
        }
    }
    report.push("FR-024: each frame's name and status are exposed".to_owned());

    let measured = snapshot(shared);
    report.push(format!(
        "SC-001: every frame drawn {:?} after start; SC-006: the dialog's first frame {:?} after it was shown",
        measured.all_rendered, measured.dialog_first_frame
    ));
    Ok(report)
}

fn state(view: &Dashboard, window: &gpui::Window, cx: &gpui::App) -> WindowState {
    WindowState {
        focused: view.focused_name(window, cx),
        open_menu: view.open_menu(),
        open_panel: view.open_panel(),
        reloads: view.reloads(),
        frames: view.widgets().len(),
        loading: view
            .widgets()
            .iter()
            .filter(|widget| widget.status == FrameStatus::Loading)
            .count(),
    }
}

fn open(
    cx: &mut gpui::App,
    title: &'static str,
    left: f32,
    world: &common::World,
) -> WindowHandle<Dashboard> {
    let runtime = Arc::new(start_runtime(world.dir.path()).expect("a runtime"));
    let grants = Arc::clone(&world.grants);
    let options = WindowOptions {
        titlebar: Some(TitlebarOptions {
            title: Some(SharedString::new_static(title)),
            ..Default::default()
        }),
        // Side by side: with no window manager (Xvfb), overlapping windows hide each other
        window_bounds: Some(WindowBounds::Windowed(Bounds {
            origin: point(px(left), px(40.0)),
            size: size(px(1000.0), px(700.0)),
        })),
        ..Default::default()
    };
    cx.open_window(options, |window, cx| {
        cx.new(|cx| Dashboard::new(Some(runtime), grants, window, cx))
    })
    .expect("the window opens")
}

fn main() {
    let world = Arc::new(common::World::new());
    world.approve("hello-widget");
    world.approve("out-of-bounds");
    let shared: Shared = Arc::default();
    let started = Instant::now();
    let app_world = Arc::clone(&world);
    let app_shared = Arc::clone(&shared);
    gpui_platform::application().run(move |cx| {
        host::app::install(cx).expect("installs");
        host::vendor::gpui_base::init(cx);
        let a = open(cx, TITLE_A, 20.0, &app_world);
        let b = open(cx, TITLE_B, 1060.0, &app_world);
        let publish = Arc::clone(&app_shared);
        cx.spawn(async move |cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(30))
                    .await;
                let _ = cx.update(|cx| {
                    let read = |handle: WindowHandle<Dashboard>, cx: &mut gpui::App| {
                        handle
                            .update(cx, |view, window, cx| state(view, window, cx))
                            .unwrap_or_default()
                    };
                    let a = read(a, cx);
                    let b = read(b, cx);
                    let open = host::dialog::open(cx);
                    let first_frame = open.and_then(|open| {
                        open.gate
                            .started_at()
                            .map(|at| at.saturating_sub(open.asked_at))
                    });
                    let mut shared = publish.lock().expect("the snapshot");
                    if shared.all_rendered.is_none()
                        && a.frames == 2
                        && a.loading == 0
                        && b.loading == 0
                    {
                        shared.all_rendered = Some(started.elapsed());
                    }
                    if first_frame.is_some() {
                        shared.dialog_first_frame = first_frame;
                    }
                    shared.dialog_open = open.is_some();
                    shared.a = a;
                    shared.b = b;
                });
            }
        })
        .detach();
        let drive_shared = Arc::clone(&app_shared);
        std::thread::spawn(move || {
            let outcome = drive(&drive_shared);
            match outcome {
                Ok(report) => {
                    for line in report {
                        println!("window: {line}");
                    }
                    println!("window: every check passed");
                    std::process::exit(0);
                }
                Err(problem) => {
                    eprintln!("window: FAILED: {problem}");
                    std::process::exit(1);
                }
            }
        });
    });
}
