//! The host side of one render call: the state its store holds and the `log` capability, the
//! only host function of contract v0.

use crate::dashboard::widget::log::{Host, Level};
use crate::limits::{Limits, MemoryLimiter};
use crate::{LogLevel, LogMessage, text};

/// State of one render call, dropped with its store when the call ends.
pub(crate) struct HostState {
    /// Messages kept, in the order the plugin wrote them, escaped and cut to size.
    pub(crate) logs: Vec<LogMessage>,
    /// Messages over `max_log_messages`, counted instead of kept.
    pub(crate) dropped_logs: u32,
    max_log_messages: usize,
    max_log_message_bytes: usize,
    /// The memory cap of this call, installed with `Store::limiter`.
    pub(crate) limiter: MemoryLimiter,
}

impl HostState {
    pub(crate) fn new(limits: &Limits) -> Self {
        Self {
            logs: Vec::new(),
            dropped_logs: 0,
            max_log_messages: limits.max_log_messages,
            max_log_message_bytes: limits.max_log_message_bytes,
            limiter: MemoryLimiter::new(limits.max_memory_bytes),
        }
    }
}

impl Host for HostState {
    /// Keeps at most `max_log_messages` per call, each escaped and cut to
    /// `max_log_message_bytes`; the rest are dropped and counted (FR-010, FR-011).
    fn write(&mut self, level: Level, message: String) {
        if self.logs.len() >= self.max_log_messages {
            self.dropped_logs = self.dropped_logs.saturating_add(1);
            return;
        }
        self.logs.push(LogMessage {
            level: level.into(),
            message: text::untrusted(&message, self.max_log_message_bytes),
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
