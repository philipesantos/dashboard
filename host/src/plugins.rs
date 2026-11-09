//! The widgets the window shows (research R5): the approvals of the development grant record,
//! each package read from its origin and loaded and rendered by the runtime, never in development
//! mode, never on the UI thread. What a render returns is dropped: until the widget kit exists a
//! frame shows only whether it rendered (FR-008).

use crate::label::{Label, label, monogram};
use crate::text::Msg;
use runtime::grants::GrantStore;
use runtime::manifest::PluginId;
use runtime::{Limits, Runtime, RuntimeOptions};
use std::io::Read;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// What a frame says about its widget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FrameStatus {
    Loading,
    Rendered,
    /// Why it did not load or render, in the host's words for the error kind (research R5).
    Failed(Msg),
}

/// One approved plugin, as its frame shows it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Widget {
    pub id: PluginId,
    pub origin: String,
    pub label: Label,
    pub monogram: String,
    pub status: FrameStatus,
    /// How long loading (compiling) its package took on the last reload, once it has loaded
    /// (decision 0001's third trigger, research R14).
    pub load_time: Option<std::time::Duration>,
}

/// A plugin's name: its id's last segment (`local/<name>`, `github.com/<owner>/<repository>`).
/// A manifest has no other name (research R8).
pub fn name_of(id: &PluginId) -> &str {
    id.as_str().rsplit('/').next().unwrap_or(id.as_str())
}

impl Widget {
    /// A frame for the approval of `id` from `origin`, before its package is read.
    pub fn new(id: PluginId, origin: String) -> Widget {
        let name = name_of(&id).to_owned();
        Widget {
            label: label(&name),
            monogram: monogram(&name),
            id,
            origin,
            status: FrameStatus::Loading,
            load_time: None,
        }
    }
}

/// The runtime the window loads plugins with: development mode off, so local connectors never
/// load here (FR-041).
pub fn start_runtime(data_dir: &Path) -> Result<Runtime, String> {
    Runtime::new(Limits::default(), RuntimeOptions::new(data_dir))
        .map_err(|error| format!("the plugin runtime did not start: {error:#}"))
}

/// The approvals to show, in the record's order (by plugin id), or `None` when the record cannot
/// be read now (locked by the development tool, or damaged).
pub fn approvals(grants: &GrantStore) -> Option<Vec<Widget>> {
    grants.approvals().ok().map(|approvals| {
        approvals
            .into_iter()
            .map(|(id, origin)| Widget::new(id, origin))
            .collect()
    })
}

/// Reads at most one byte more than the package limit, so an oversized file is refused by the
/// runtime's own check without the window reading all of it.
fn read_capped(path: &str, max_bytes: usize) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take((max_bytes as u64).saturating_add(1))
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// The outcome of loading and rendering one widget, with how long loading (compiling) took.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Outcome {
    pub status: FrameStatus,
    pub load_time: Option<Duration>,
}

/// Reads the package at `origin`, loads it and renders it once with an empty configuration.
/// Blocking: call it from the background executor.
pub fn load_and_render(runtime: &Runtime, grants: &Arc<GrantStore>, origin: &str) -> Outcome {
    let bytes = match read_capped(origin, Limits::default().max_package_bytes) {
        Ok(bytes) => bytes,
        Err(_) => {
            return Outcome {
                status: FrameStatus::Failed(Msg::ReasonUnreadable),
                load_time: None,
            };
        }
    };
    let started = Instant::now();
    let plugin = match runtime.load(&bytes, origin, Arc::clone(grants)) {
        Ok(plugin) => plugin,
        Err(error) => {
            return Outcome {
                status: FrameStatus::Failed(Msg::reason(error.kind())),
                load_time: None,
            };
        }
    };
    let load_time = Some(started.elapsed());
    let status = match plugin.render("{}").result {
        Ok(_) => FrameStatus::Rendered,
        Err(error) => FrameStatus::Failed(Msg::reason(error.kind())),
    };
    Outcome { status, load_time }
}
