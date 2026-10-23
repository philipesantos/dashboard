//! The host side of one render call: the state its store holds, the `log` capability of each
//! contract version, and the stand-in linked in place of an optional capability that is off.

use wasmtime::component::HasData;

use crate::limits::{Limits, MemoryLimiter};
use crate::{LogLevel, LogMessage, text, v0, v1};

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

    /// Keeps at most `max_log_messages` per call, each escaped and cut to
    /// `max_log_message_bytes`; the rest are dropped and counted (feature 001, FR-010, FR-011).
    fn keep_log(&mut self, level: LogLevel, message: &str) {
        if self.logs.len() >= self.max_log_messages {
            self.dropped_logs = self.dropped_logs.saturating_add(1);
            return;
        }
        self.logs.push(LogMessage {
            level,
            message: text::untrusted(message, self.max_log_message_bytes),
        });
    }
}

impl v0::dashboard::widget::log::Host for HostState {
    fn write(&mut self, level: v0::dashboard::widget::log::Level, message: String) {
        self.keep_log(level.into(), &message);
    }
}

impl v1::dashboard::widget::log::Host for HostState {
    /// Answers `ok` even when the message is dropped over the cap: the plugin was granted `log`,
    /// and the report counts what was dropped.
    fn write(
        &mut self,
        level: v1::dashboard::widget::log::Level,
        message: String,
    ) -> Result<(), v1::dashboard::widget::log::LogError> {
        self.keep_log(level.into(), &message);
        Ok(())
    }
}

/// Linked in place of contract v1's `log` when `log` is an optional permission that is off
/// (FR-019). It holds nothing and reaches nothing: every call answers `not-granted`.
pub(crate) struct LogStandIn;

impl HasData for LogStandIn {
    type Data<'a> = LogStandIn;
}

impl v1::dashboard::widget::log::Host for LogStandIn {
    fn write(
        &mut self,
        _level: v1::dashboard::widget::log::Level,
        _message: String,
    ) -> Result<(), v1::dashboard::widget::log::LogError> {
        Err(v1::dashboard::widget::log::LogError::NotGranted)
    }
}

impl From<v0::dashboard::widget::log::Level> for LogLevel {
    fn from(level: v0::dashboard::widget::log::Level) -> Self {
        use v0::dashboard::widget::log::Level;
        match level {
            Level::Debug => LogLevel::Debug,
            Level::Info => LogLevel::Info,
            Level::Warn => LogLevel::Warn,
            Level::Error => LogLevel::Error,
        }
    }
}

impl From<v1::dashboard::widget::log::Level> for LogLevel {
    fn from(level: v1::dashboard::widget::log::Level) -> Self {
        use v1::dashboard::widget::log::Level;
        match level {
            Level::Debug => LogLevel::Debug,
            Level::Info => LogLevel::Info,
            Level::Warn => LogLevel::Warn,
            Level::Error => LogLevel::Error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use v1::dashboard::widget::log::{Host as _, Level, LogError};

    fn state() -> HostState {
        HostState::new(&Limits {
            max_log_messages: 2,
            max_log_message_bytes: 4,
            ..Limits::default()
        })
    }

    #[test]
    fn v0_log_keeps_feature_001s_caps() {
        let mut state = state();
        for _ in 0..3 {
            v0::dashboard::widget::log::Host::write(
                &mut state,
                v0::dashboard::widget::log::Level::Warn,
                "ab\ncd".to_owned(),
            );
        }
        assert_eq!(
            state.logs,
            vec![
                LogMessage {
                    level: LogLevel::Warn,
                    message: r"ab\n [truncated]".to_owned()
                };
                2
            ]
        );
        assert_eq!(state.dropped_logs, 1);
    }

    #[test]
    fn v1_log_keeps_the_same_caps_and_answers_ok_when_it_drops() {
        let mut state = state();
        for _ in 0..3 {
            assert_eq!(state.write(Level::Info, "abcdef".to_owned()), Ok(()));
        }
        assert_eq!(state.logs.len(), 2);
        assert_eq!(state.logs[0].message, "abcd [truncated]");
        assert_eq!(state.logs[0].level, LogLevel::Info);
        assert_eq!(state.dropped_logs, 1);
    }

    #[test]
    fn the_stand_in_answers_not_granted() {
        let mut stand_in = LogStandIn;
        assert_eq!(
            stand_in.write(Level::Error, "anything".to_owned()),
            Err(LogError::NotGranted)
        );
    }
}
