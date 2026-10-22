//! Plugin runtime: loads plugins packaged as WebAssembly components and runs them in a sandbox,
//! under limits set by the host.

mod error;
mod limits;

use wasmtime::Engine;

pub use error::{ErrorKind, RuntimeError};
pub use limits::Limits;

// Bindings for the `widget` world of plugin contract v0, the only copy of which is
// wit/widget.wit.
wasmtime::component::bindgen!({ path: "../wit", world: "widget" });

/// Severity of a plugin log message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    /// The level's name, as the CLI prints it.
    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::Debug => "debug",
            LogLevel::Info => "info",
            LogLevel::Warn => "warn",
            LogLevel::Error => "error",
        }
    }
}

/// A message a plugin wrote through the `log` capability during one render call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LogMessage {
    pub level: LogLevel,
    pub message: String,
}

/// The outcome of one render call.
#[derive(Debug)]
pub struct RenderReport {
    /// The widget content, or why the call was refused or stopped. On error there is no
    /// content, even if the plugin produced some before it stopped.
    pub result: Result<serde_json::Value, RuntimeError>,
    /// Log messages written before the call ended, in order.
    pub logs: Vec<LogMessage>,
    /// Messages over the per-call cap, dropped instead of kept.
    pub dropped_logs: u32,
}

/// Loads plugins and runs them under the host's limits.
#[expect(dead_code, reason = "the fields are used from User Story 1 on")]
pub struct Runtime {
    engine: Engine,
    limits: Limits,
}

/// A loaded plugin, compiled once and instantiated afresh for every render call.
pub struct Plugin {}

impl Runtime {
    pub fn new(limits: Limits) -> Result<Runtime, RuntimeError> {
        let _ = limits;
        todo!("User Story 1 (T016)")
    }

    /// Checks and compiles a plugin package.
    pub fn load(&self, bytes: &[u8]) -> Result<Plugin, RuntimeError> {
        let _ = bytes;
        todo!("User Story 1 (T016)")
    }
}

impl Plugin {
    /// Calls the plugin's `render` with a JSON configuration, in a fresh instance.
    pub fn render(&self, config: &str) -> RenderReport {
        let _ = config;
        todo!("User Story 1 (T017)")
    }
}
