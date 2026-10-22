//! The host side of one render call: the state its store holds and the `log` capability, the
//! only host function of contract v0.

use crate::dashboard::widget::log::{Host, Level};
use crate::{LogLevel, LogMessage};

/// State of one render call, dropped with its store when the call ends.
#[derive(Default)]
pub(crate) struct HostState {
    /// Messages the plugin wrote, in order. Caps come in T037.
    pub(crate) logs: Vec<LogMessage>,
}

impl Host for HostState {
    fn write(&mut self, level: Level, message: String) {
        self.logs.push(LogMessage {
            level: level.into(),
            message,
        });
    }
}

impl From<Level> for LogLevel {
    fn from(level: Level) -> Self {
        match level {
            Level::Debug => LogLevel::Debug,
            Level::Info => LogLevel::Info,
            Level::Warn => LogLevel::Warn,
            Level::Error => LogLevel::Error,
        }
    }
}
