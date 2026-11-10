//! The shell probe: breaking tests run inside the real page of a release build
//! (spec 008 FR-018, `specs/008-tauri-shell/contracts/probe.md`).
//!
//! Compiled only with the `shell-probe` feature. The page module `src/probe/run.ts` tries every
//! forbidden action and reports through `shell_probe_report`. This side adds what the page cannot
//! see: the handlers' refusals, the window count and the engine settings. It prints one line per
//! case and exits 0 (every case held), 1 (a case broke) or 2 (no report within 60 s).
//!
//! Part of the security core (constitution, Principle IX).

use std::io::Write;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::ipc::CapabilityBuilder;
use tauri::{AppHandle, Manager, State};

use crate::window::MAIN;

/// How long the probe waits for the page's report.
const TIMEOUT: Duration = Duration::from_secs(60);

/// A refusal by one of the window's handlers.
#[derive(Clone, Copy)]
#[expect(dead_code, reason = "window.rs's handlers report refusals from T017 and T018")]
pub enum Refusal {
    Navigation,
    NewWindow,
    Permission,
    Download,
}

static NAVIGATION: AtomicU32 = AtomicU32::new(0);
static NEW_WINDOW: AtomicU32 = AtomicU32::new(0);
static PERMISSION: AtomicU32 = AtomicU32::new(0);
static DOWNLOAD: AtomicU32 = AtomicU32::new(0);

/// Counts a refusal. `window.rs` calls it from each handler; the handlers never depend on it.
#[expect(dead_code, reason = "window.rs's handlers report refusals from T017 and T018")]
pub fn refused(refusal: Refusal) {
    let counter = match refusal {
        Refusal::Navigation => &NAVIGATION,
        Refusal::NewWindow => &NEW_WINDOW,
        Refusal::Permission => &PERMISSION,
        Refusal::Download => &DOWNLOAD,
    };
    counter.fetch_add(1, Ordering::SeqCst);
}

/// The handlers' refusals so far. The page diffs two snapshots around a case.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counters {
    navigation: u32,
    new_window: u32,
    permission: u32,
    download: u32,
}

fn counters() -> Counters {
    Counters {
        navigation: NAVIGATION.load(Ordering::SeqCst),
        new_window: NEW_WINDOW.load(Ordering::SeqCst),
        permission: PERMISSION.load(Ordering::SeqCst),
        download: DOWNLOAD.load(Ordering::SeqCst),
    }
}

/// One case as the page judged it.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Case {
    id: String,
    held: bool,
    detail: String,
    /// The case holds only if the app still has exactly one window.
    #[serde(default)]
    single_window: bool,
}

/// One line of the report.
struct Outcome {
    id: String,
    held: bool,
    detail: String,
}

/// Where the report command hands the page's cases to the waiting thread.
pub struct Inbox(Mutex<Option<Sender<Vec<Case>>>>);

/// Refuses to start unless `DASHBOARD_PROBE=1`, so a build shipped with the feature by mistake
/// does not run.
pub fn gate() {
    if std::env::var("DASHBOARD_PROBE").as_deref() != Ok("1") {
        eprintln!("probe: this is a shell-probe build, which runs only with DASHBOARD_PROBE=1");
        std::process::exit(3);
    }
}

/// Grants the report command to the window at run time and starts waiting for the report.
pub fn setup(app: &tauri::App) -> tauri::Result<()> {
    app.add_capability(
        CapabilityBuilder::new("shell-probe")
            .window(MAIN)
            .permission("allow-shell-probe-report"),
    )?;
    let (sender, receiver) = mpsc::channel();
    app.manage(Inbox(Mutex::new(Some(sender))));
    let handle = app.handle().clone();
    std::thread::spawn(move || finish(&handle, &receiver));
    Ok(())
}

/// With `cases`, the page's final report. Either way, returns the handlers' counters.
#[tauri::command]
pub fn shell_probe_report(inbox: State<'_, Inbox>, cases: Option<Vec<Case>>) -> Counters {
    if let Some(cases) = cases
        && let Some(sender) = inbox.0.lock().expect("probe inbox").take()
    {
        let _ = sender.send(cases);
    }
    counters()
}

/// Registered, and granted by nothing: the page must be refused by the ACL.
#[tauri::command]
pub fn shell_probe_ungranted() -> &'static str {
    "reached"
}

/// The engine settings read back, one case each. Filled by the US3 and US5 tasks.
fn settings(_app: &AppHandle) -> Vec<Outcome> {
    Vec::new()
}

fn finish(app: &AppHandle, receiver: &Receiver<Vec<Case>>) {
    let Ok(cases) = receiver.recv_timeout(TIMEOUT) else {
        println!("probe: no report within 60 s: the page unloaded, which means a navigation succeeded, or it hung");
        exit(2);
    };
    let windows = app.webview_windows().len();
    let mut outcomes: Vec<Outcome> = cases
        .into_iter()
        .map(|case| {
            if case.single_window && windows != 1 {
                Outcome {
                    id: case.id,
                    held: false,
                    detail: format!("{}; {windows} windows are open", case.detail),
                }
            } else {
                Outcome {
                    id: case.id,
                    held: case.held,
                    detail: case.detail,
                }
            }
        })
        .collect();
    outcomes.extend(settings(app));

    let mut held = 0;
    let mut broken = 0;
    for outcome in &outcomes {
        let word = if outcome.held { "held" } else { "broken" };
        if outcome.held {
            held += 1;
        } else {
            broken += 1;
        }
        println!("probe: {word} {} ({})", outcome.id, outcome.detail);
    }
    println!("probe: {held} held, {broken} broken");

    if std::env::var("DASHBOARD_PROBE_HOLD").as_deref() == Ok("1") {
        let _ = std::io::stdout().flush();
        return;
    }
    exit(if broken == 0 { 0 } else { 1 });
}

fn exit(code: i32) -> ! {
    let _ = std::io::stdout().flush();
    std::process::exit(code);
}
