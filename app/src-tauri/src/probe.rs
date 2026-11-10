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

/// The engine settings read back on the main thread, one case each (research R6 and R7).
fn settings(app: &AppHandle) -> Vec<Outcome> {
    let broken = |detail: String| {
        vec![Outcome {
            id: "engine-settings".into(),
            held: false,
            detail,
        }]
    };
    let Some(window) = app.get_webview_window(MAIN) else {
        return broken("the main window is gone".into());
    };
    let (sender, receiver) = mpsc::channel();
    if let Err(error) = window.with_webview(move |webview| {
        let _ = sender.send(engine::read(&webview));
    }) {
        return broken(format!("the webview could not be reached: {error}"));
    }
    receiver
        .recv_timeout(Duration::from_secs(10))
        .unwrap_or_else(|_| broken("the engine settings were not read within 10 s".into()))
}

/// Builds a setting case: held when `actual` is the release value.
fn setting(id: &str, name: &str, actual: Result<bool, String>, release: bool) -> Outcome {
    match actual {
        Ok(value) => Outcome {
            id: id.into(),
            held: value == release,
            detail: format!("{name} is {value}"),
        },
        Err(error) => Outcome {
            id: id.into(),
            held: false,
            detail: format!("{name} could not be read: {error}"),
        },
    }
}

#[cfg(windows)]
mod engine {
    use tauri::webview::PlatformWebview;
    use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Settings;
    use windows_core::BOOL;

    use super::{Outcome, setting};

    fn read_bool(
        settings: &ICoreWebView2Settings,
        get: unsafe fn(&ICoreWebView2Settings, *mut BOOL) -> windows_core::Result<()>,
    ) -> Result<bool, String> {
        let mut value = BOOL::default();
        // SAFETY: a getter of a live settings object, writing into a local BOOL
        unsafe { get(settings, &mut value) }.map_err(|error| error.to_string())?;
        Ok(value.as_bool())
    }

    pub fn read(webview: &PlatformWebview) -> Vec<Outcome> {
        // SAFETY: the controller is Tauri's own, alive on this (main) thread
        let settings = unsafe { webview.controller().CoreWebView2().and_then(|core| core.Settings()) };
        let settings = match settings {
            Ok(settings) => settings,
            Err(error) => {
                return vec![setting("devtools-setting", "the WebView2 settings", Err(error.to_string()), false)];
            }
        };
        let devtools = read_bool(&settings, ICoreWebView2Settings::AreDevToolsEnabled);
        let menus = read_bool(&settings, ICoreWebView2Settings::AreDefaultContextMenusEnabled);
        vec![
            setting("devtools-setting", "AreDevToolsEnabled", devtools, false),
            setting("context-menu-setting", "AreDefaultContextMenusEnabled", menus, false),
        ]
    }
}

#[cfg(target_os = "macos")]
mod engine {
    use objc2_web_kit::WKWebView;
    use tauri::webview::PlatformWebview;

    use super::{Outcome, setting};

    pub fn read(webview: &PlatformWebview) -> Vec<Outcome> {
        // SAFETY: Tauri's pointer to its live WKWebView, used on the main thread
        let view: &WKWebView = unsafe { &*webview.inner().cast::<WKWebView>() };
        // SAFETY: a plain property read (macOS 13.3 and later, as on the CI runners)
        let inspectable = unsafe { view.isInspectable() };
        vec![setting("devtools-setting", "isInspectable", Ok(inspectable), false)]
    }
}

// WEBKITGTK-ONLY(probe-devtools-read): the WebKitGTK settings, read through webkit2gtk
#[cfg(target_os = "linux")]
mod engine {
    use tauri::webview::PlatformWebview;
    use webkit2gtk::{SettingsExt, WebViewExt};

    use super::{Outcome, setting};

    pub fn read(webview: &PlatformWebview) -> Vec<Outcome> {
        let view = webview.inner();
        let devtools = WebViewExt::settings(&view)
            .map(|settings| settings.enables_developer_extras())
            .ok_or_else(|| "the webview has no settings".to_string());
        vec![setting("devtools-setting", "enable-developer-extras", devtools, false)]
    }
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
