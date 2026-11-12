//! The app's widget host (specs/009-declarative-widget-ui/research.md, R1, R2, R3, R9, R11, R12;
//! `contracts/window-commands.md`).
//!
//! The app runs the packages it was built with, embedded by `build.rs`, in its own runtime: its
//! data under `<app data dir>/host`, its own keychain service, development mode off. It approves
//! a bundled package in memory only when its manifest declares no permission, no connector and
//! no storage (constitution v2.6.3, Principle VII); that approval grants it nothing. The page
//! knows each widget by a handle (`w0`, `w1`…) and calls it through three commands; everything it
//! sends is checked here before a plugin runs.
//!
//! Part of the security core (constitution, Principle IX).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError, TryLockError};

use runtime::grants::GrantStore;
use runtime::{Limits, LogMessage, Plugin, Runtime, RuntimeError, RuntimeOptions};
use serde::Serialize;
use serde_json::Value;
use widget_content::{Component, Content, ContentError};

use crate::name::drawn_name;

/// The app's keychain service, apart from the development tool's (research R3).
pub const APP_VAULT_SERVICE: &str = "dev.philsantos.dashboard.app";

/// The configuration every widget gets: widgets have none yet.
const CONFIG: &str = "{}";

/// How many log entries the app keeps per widget (research R12).
pub const LOG_ENTRIES: usize = 100;

/// A form sends at most this many values.
pub const MAX_VALUES: usize = 20;

/// A form value has at most this many characters.
pub const MAX_VALUE_CHARS: usize = 500;

/// A form field's id has at most this many characters, as the content format allows.
pub const MAX_FIELD_ID_CHARS: usize = 32;

const UNKNOWN_WIDGET: &str = "unknown-widget";
const WIDGET_BUSY: &str = "widget-busy";
const UNDECLARED_ACTION: &str = "undeclared-action";
const BAD_VALUES: &str = "bad-values";
const SECRET_IN_FORM: &str = "secret-in-form";

/// A package the app was built with.
#[derive(Debug, Clone, Copy)]
pub struct Bundled<'a> {
    /// Its file name in `target/plugins/`; its origin is `bundled:<file>`.
    pub file: &'a str,
    pub bytes: &'a [u8],
}

// The packages this build embeds: `BUNDLED`, written by build.rs
include!(concat!(env!("OUT_DIR"), "/bundled.rs"));

/// The app's runtime options: its data under `<app data dir>/host`, its own keychain service,
/// development mode off (research R3).
pub fn app_options(app_data_dir: &Path) -> RuntimeOptions {
    RuntimeOptions::new(app_data_dir.join("host")).with_vault_service(APP_VAULT_SERVICE)
}

/// What the page gets to draw a widget's frame.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FrameInfo {
    pub handle: String,
    /// Already sanitised (`name.rs`).
    pub name: String,
    pub icon: Option<IconPixels>,
    /// The plugin's id, or its file when it was refused before its manifest was read: given only
    /// when another frame draws the same name, so the person can tell them apart by something
    /// the plugin does not choose freely.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// An icon, decoded by the runtime from the manifest's PNG.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct IconPixels {
    pub size: u32,
    /// `size × size` pixels, four bytes each.
    pub rgba: Vec<u8>,
}

/// The answer to `widget_render` and `widget_act`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Rendered {
    /// The normalised content to draw.
    Content(Content),
    /// Why there is nothing to draw, or why the action failed.
    Error(Failure),
    /// Refused before any plugin ran.
    Refused(&'static str),
}

/// A failure the frame shows: a kind, and a short reason the host wrote.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Failure {
    /// `not-loaded`, `render-failed`, `action-failed`, `bad-shape` or `over-bound`.
    pub kind: &'static str,
    /// The runtime's error kind, or the content checker's reason; never the plugin's text.
    pub reason: String,
    /// Where in the content, for `bad-shape` and `over-bound`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

/// One entry of a widget's log in the app.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogEntry {
    pub level: &'static str,
    /// `plugin`, `content` or `host`.
    pub source: &'static str,
    pub message: String,
}

/// One widget the host shows.
struct Shown {
    handle: String,
    frame: FrameInfo,
    /// The loaded plugin, or the runtime's error kind when it was refused at load.
    plugin: Result<Plugin, String>,
    /// Held while a render or an action runs: a call that finds it held is refused as busy.
    busy: Mutex<()>,
    log: Mutex<VecDeque<LogEntry>>,
    /// The field ids of each form in the content last drawn, in the order the person sees them.
    forms: Mutex<Vec<Vec<String>>>,
}

/// The widgets the app shows, and the runtime they run in.
pub struct Widgets {
    runtime: Runtime,
    shown: Vec<Shown>,
}

impl Widgets {
    /// Loads each of `bundled` in `runtime`, approved in memory when it declares nothing. A
    /// package that is refused keeps its frame, in the not-loaded state, with the reason in its
    /// log.
    pub fn start(runtime: Runtime, bundled: &[Bundled<'_>]) -> Widgets {
        let grants = Arc::new(GrantStore::in_memory());
        let mut shown: Vec<Shown> = bundled
            .iter()
            .enumerate()
            .map(|(index, package)| {
                let handle = format!("w{index}");
                let mut log = VecDeque::new();
                let (frame, plugin) = load(&runtime, &grants, package, &handle, &mut log);
                Shown {
                    handle,
                    frame,
                    plugin,
                    busy: Mutex::new(()),
                    log: Mutex::new(log),
                    forms: Mutex::new(Vec::new()),
                }
            })
            .collect();
        same_names(&mut shown, bundled);
        let widgets = Widgets { runtime, shown };
        #[cfg(debug_assertions)]
        for shown in &widgets.shown {
            for entry in lock(&shown.log).iter() {
                print_entry(&shown.handle, entry);
            }
        }
        widgets
    }

    /// Every widget's frame, in order.
    pub fn list(&self) -> Vec<FrameInfo> {
        self.shown.iter().map(|shown| shown.frame.clone()).collect()
    }

    /// Renders the widget `handle`, and checks its content.
    pub fn render(&self, handle: &str) -> Rendered {
        let Some(shown) = self.find(handle) else {
            return Rendered::Refused(UNKNOWN_WIDGET);
        };
        let Some(_running) = try_hold(&shown.busy) else {
            shown.note("warn", "host", "refused a render: the widget is busy");
            return Rendered::Refused(WIDGET_BUSY);
        };
        shown.draw()
    }

    /// Runs the action `action` of the widget `handle` with the form's `values`, then renders
    /// it. Refused, in this order and before the plugin runs: a handle the host is not showing,
    /// a busy widget, an action the manifest does not declare, values out of bounds, a value
    /// holding a secret the host holds.
    pub fn act(&self, handle: &str, action: &str, values: &Value) -> Rendered {
        let Some(shown) = self.find(handle) else {
            return Rendered::Refused(UNKNOWN_WIDGET);
        };
        let Some(_running) = try_hold(&shown.busy) else {
            shown.note("warn", "host", "refused an action: the widget is busy");
            return Rendered::Refused(WIDGET_BUSY);
        };
        let plugin = match &shown.plugin {
            Ok(plugin) if plugin.manifest().declares_action(action) => plugin,
            _ => {
                shown.note(
                    "warn",
                    "host",
                    &format!(
                        "refused the action \"{}\": not declared",
                        runtime::escape(action)
                    ),
                );
                return Rendered::Refused(UNDECLARED_ACTION);
            }
        };
        let Some(values) = form_values(values) else {
            shown.note(
                "warn",
                "host",
                "refused an action: its values are out of bounds",
            );
            return Rendered::Refused(BAD_VALUES);
        };
        let candidates = secret_candidates(&values, &lock(&shown.forms));
        for (id, text) in candidates {
            match self.runtime.holds_secret_in(&text) {
                Ok(false) => {}
                Ok(true) => {
                    shown.note(
                        "warn",
                        "host",
                        &format!(
                            "refused an action: {} holds a secret the app holds",
                            match id {
                                Some(id) => format!("the field \"{}\"", runtime::escape(&id)),
                                None => "the fields read together".to_owned(),
                            }
                        ),
                    );
                    return Rendered::Refused(SECRET_IN_FORM);
                }
                Err(error) => {
                    shown.note(
                        "warn",
                        "host",
                        &format!(
                            "refused an action: the form could not be checked for secrets ({})",
                            error.kind().as_str()
                        ),
                    );
                    return Rendered::Refused(SECRET_IN_FORM);
                }
            }
        }
        #[cfg(feature = "shell-probe")]
        crate::probe::action_ran();
        let report = plugin.act(action, CONFIG, &values);
        shown.plugin_logs(&report.logs, report.dropped_logs);
        if let Err(error) = report.result {
            shown.note("error", "host", &format!("the action failed: {error}"));
            return Rendered::Error(failure("action-failed", &error));
        }
        shown.draw()
    }

    /// The widget's log in the app, oldest first; empty for a handle the host is not showing.
    pub fn log(&self, handle: &str) -> Vec<LogEntry> {
        self.find(handle)
            .map(|shown| lock(&shown.log).iter().cloned().collect())
            .unwrap_or_default()
    }

    /// The widget the page means by `handle`: only a handle the host gave out.
    fn find(&self, handle: &str) -> Option<&Shown> {
        self.shown.iter().find(|shown| shown.handle == handle)
    }
}

impl Shown {
    /// Renders and checks the content. The caller holds `busy`.
    fn draw(&self) -> Rendered {
        let plugin = match &self.plugin {
            Ok(plugin) => plugin,
            Err(kind) => {
                return Rendered::Error(Failure {
                    kind: "not-loaded",
                    reason: kind.clone(),
                    path: None,
                });
            }
        };
        let report = plugin.render(CONFIG);
        self.plugin_logs(&report.logs, report.dropped_logs);
        let value = match report.result {
            Ok(value) => value,
            Err(error) => {
                self.note("error", "host", &format!("the render failed: {error}"));
                return Rendered::Error(failure("render-failed", &error));
            }
        };
        match widget_content::parse(&value) {
            Ok((content, notes)) => {
                for note in notes {
                    self.note("warn", "content", &note);
                }
                *lock(&self.forms) = form_orders(&content.body);
                Rendered::Content(content)
            }
            Err(error) => {
                self.note("error", "content", &error.to_string());
                Rendered::Error(content_failure(error))
            }
        }
    }

    fn plugin_logs(&self, logs: &[LogMessage], dropped: u32) {
        for message in logs {
            self.note(message.level.as_str(), "plugin", &message.message);
        }
        if dropped > 0 {
            self.note(
                "warn",
                "host",
                &format!("{dropped} log messages over the cap were dropped"),
            );
        }
    }

    /// Adds an entry to the log, dropping the oldest past [`LOG_ENTRIES`].
    fn note(&self, level: &'static str, source: &'static str, message: &str) {
        let entry = LogEntry {
            level,
            source,
            message: message.to_owned(),
        };
        #[cfg(debug_assertions)]
        print_entry(&self.handle, &entry);
        push(&mut lock(&self.log), entry);
    }
}

/// Gives each frame that draws the same name as another, ignoring case, its plugin's id.
fn same_names(shown: &mut [Shown], bundled: &[Bundled<'_>]) {
    let names: Vec<String> = shown
        .iter()
        .map(|shown| shown.frame.name.to_lowercase())
        .collect();
    for (index, each) in shown.iter_mut().enumerate() {
        let shared = names
            .iter()
            .enumerate()
            .any(|(other, name)| other != index && *name == names[index]);
        if shared {
            each.frame.id = Some(match &each.plugin {
                Ok(plugin) => plugin.manifest().id().to_string(),
                Err(_) => bundled[index].file.to_owned(),
            });
        }
    }
}

/// Checks and loads one bundled package: its frame, and the plugin or why it was refused.
fn load(
    runtime: &Runtime,
    grants: &Arc<GrantStore>,
    package: &Bundled<'_>,
    handle: &str,
    log: &mut VecDeque<LogEntry>,
) -> (FrameInfo, Result<Plugin, String>) {
    let mut refuse = |frame: FrameInfo, message: String, kind: &str| {
        push(
            log,
            LogEntry {
                level: "error",
                source: "host",
                message,
            },
        );
        (frame, Err(kind.to_owned()))
    };
    let stem = package.file.trim_end_matches(".wasm");
    let bare = FrameInfo {
        handle: handle.to_owned(),
        name: drawn_name(None, stem),
        icon: None,
        id: None,
    };
    let origin = format!("bundled:{}", package.file);
    let checked = match runtime.check(package.bytes) {
        Ok(checked) => checked,
        Err(error) => {
            let kind = error.kind().as_str();
            return refuse(bare, format!("refused at load: {error}"), kind);
        }
    };
    let manifest = checked.manifest();
    let frame = FrameInfo {
        handle: handle.to_owned(),
        name: drawn_name(manifest.name(), manifest.id().as_str()),
        icon: manifest.icon().map(|icon| IconPixels {
            size: icon.size(),
            rgba: icon.rgba().to_vec(),
        }),
        id: None,
    };
    // Principle VII (v2.6.3): only a bundled package that declares nothing is part of the app
    let declared = if manifest.storage().is_some() {
        Some("storage")
    } else if !manifest.connectors().is_empty() {
        Some("a connector")
    } else if !manifest.permissions().is_empty() {
        Some("a permission")
    } else {
        None
    };
    if let Some(declared) = declared {
        return refuse(
            frame,
            format!("refused at load: a bundled package declares {declared}"),
            "bundled-declares-permissions",
        );
    }
    let loaded = checked
        .approve(grants, &origin, None)
        .and_then(|_| runtime.load(package.bytes, &origin, Arc::clone(grants)));
    match loaded {
        Ok(plugin) => (frame, Ok(plugin)),
        Err(error) => {
            let kind = error.kind().as_str();
            refuse(frame, format!("refused at load: {error}"), kind)
        }
    }
}

/// The field ids of every form in `body`, each in the order the form draws them.
fn form_orders(body: &[Component]) -> Vec<Vec<String>> {
    let mut forms = Vec::new();
    for component in body {
        match component {
            Component::Form { fields, .. } => {
                forms.push(fields.iter().map(|field| field.id.clone()).collect());
            }
            Component::Stack { children } => forms.extend(form_orders(children)),
            _ => {}
        }
    }
    forms
}

/// The texts checked for a secret the host holds, before a form's values reach the plugin: each
/// value, named by its field; and, for a secret typed in pieces, the values read together in the
/// order of each form last drawn with those fields, and in the order of their ids. A piece too
/// short to be a secret on its own is caught with the others.
fn secret_candidates(
    values: &serde_json::Map<String, Value>,
    forms: &[Vec<String>],
) -> Vec<(Option<String>, String)> {
    let text = |id: &str| values.get(id).and_then(Value::as_str).unwrap_or_default();
    let mut candidates: Vec<(Option<String>, String)> = values
        .keys()
        .map(|id| (Some(id.clone()), text(id).to_owned()))
        .collect();
    let mut orders: Vec<Vec<&str>> = forms
        .iter()
        .map(|order| {
            order
                .iter()
                .map(String::as_str)
                .filter(|id| values.contains_key(*id))
                .collect()
        })
        .collect();
    orders.push(values.keys().map(String::as_str).collect());
    for order in orders {
        let pieces: Vec<&str> = order
            .into_iter()
            .map(text)
            .filter(|piece| !piece.is_empty())
            .collect();
        if pieces.len() < 2 {
            continue;
        }
        let joined = pieces.concat();
        if !candidates.iter().any(|(_, seen)| *seen == joined) {
            candidates.push((None, joined));
        }
    }
    candidates
}

/// The form's values as the plugin gets them: an object of at most [`MAX_VALUES`] strings, each
/// at most [`MAX_VALUE_CHARS`] characters, keyed by field ids of at most
/// [`MAX_FIELD_ID_CHARS`]; anything else is `None`.
fn form_values(values: &Value) -> Option<serde_json::Map<String, Value>> {
    let values = values.as_object()?;
    if values.len() > MAX_VALUES {
        return None;
    }
    for (id, value) in values {
        let text = value.as_str()?;
        if id.chars().count() > MAX_FIELD_ID_CHARS || text.chars().count() > MAX_VALUE_CHARS {
            return None;
        }
    }
    Some(values.clone())
}

fn failure(kind: &'static str, error: &RuntimeError) -> Failure {
    Failure {
        kind,
        reason: error.kind().as_str().to_owned(),
        path: None,
    }
}

fn content_failure(error: ContentError) -> Failure {
    let kind = match error.kind {
        widget_content::ErrorKind::BadShape => "bad-shape",
        widget_content::ErrorKind::OverBound => "over-bound",
    };
    Failure {
        kind,
        reason: error.reason,
        path: Some(error.path),
    }
}

fn push(log: &mut VecDeque<LogEntry>, entry: LogEntry) {
    if log.len() == LOG_ENTRIES {
        log.pop_front();
    }
    log.push_back(entry);
}

/// The lock, or `None` when a call already holds it.
fn try_hold(busy: &Mutex<()>) -> Option<MutexGuard<'_, ()>> {
    match busy.try_lock() {
        Ok(guard) => Some(guard),
        Err(TryLockError::Poisoned(poisoned)) => Some(poisoned.into_inner()),
        Err(TryLockError::WouldBlock) => None,
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// A development build prints each entry to its console (research R12), escaped.
#[cfg(debug_assertions)]
fn print_entry(handle: &str, entry: &LogEntry) {
    eprintln!(
        "widget {handle} [{}] {}: {}",
        entry.source,
        entry.level,
        runtime::escape(&entry.message)
    );
}

/// The widget host as the app holds it: started on a thread of its own after the window is
/// built, so compiling the packages never delays the window. A command that comes earlier waits
/// for it.
pub struct Host {
    data_dir: PathBuf,
    widgets: OnceLock<Result<Widgets, String>>,
}

impl Host {
    pub fn new(app_data_dir: PathBuf) -> Host {
        Host {
            data_dir: app_data_dir,
            widgets: OnceLock::new(),
        }
    }

    /// The widgets, started on the first call; or why the runtime could not start.
    pub fn widgets(&self) -> Result<&Widgets, String> {
        self.widgets
            .get_or_init(|| {
                let runtime = Runtime::new(Limits::default(), app_options(&self.data_dir))
                    .map_err(|error| format!("the widget runtime cannot start: {error}"))?;
                Ok(Widgets::start(runtime, BUNDLED))
            })
            .as_ref()
            .map_err(Clone::clone)
    }
}
