//! The host side of one render or update call: the state its store holds, the `log` capability
//! of each contract version, the `connectors` capability of contract 1.1.0, and the stand-ins
//! linked in place of a capability that is off.

use wasmtime::component::{HasData, HasSelf, Linker};

use crate::calls::{Answer, Calls, RunLog};
use crate::connector::ArgValue;
use crate::limits::{Limits, MemoryLimiter};
use crate::permissions::Contract;
use crate::v1::dashboard::widget::connectors as wit;
use crate::{LogLevel, LogMessage, text, v0, v1};

/// What [`link`] adds for a permission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Link {
    /// The capability itself: the permission is granted.
    Real,
    /// The stand-in that answers `not-granted`: the permission is optional and off.
    StandIn,
}

/// Adds to `linker` the interface that carries `permission` in `contract`, real or stand-in.
/// Features 003–005 add an arm per capability. Contract v0 has no stand-in: it cannot answer
/// "not granted".
pub(crate) fn link(
    linker: &mut Linker<HostState>,
    contract: Contract,
    permission: &str,
    link: Link,
) -> wasmtime::Result<()> {
    match (contract, permission, link) {
        (Contract::V0, "log", Link::Real) => {
            v0::dashboard::widget::log::add_to_linker::<_, HasSelf<_>>(linker, |state| state)
        }
        (Contract::V1, "log", Link::Real) => {
            v1::dashboard::widget::log::add_to_linker::<_, HasSelf<_>>(linker, |state| state)
        }
        (Contract::V1, "log", Link::StandIn) => {
            v1::dashboard::widget::log::add_to_linker::<_, LogStandIn>(linker, |_| LogStandIn)
        }
        _ => Err(wasmtime::format_err!(
            "contract {} has no {link:?} interface for the {permission} permission",
            contract.as_str()
        )),
    }
}

/// Adds `connectors` to `linker`: the real interface over the plugin's table of granted
/// connectors, or the stand-in, which holds no table, no network and no grants, and answers
/// `not-granted` to every call (Principle II, research R12).
pub(crate) fn link_connectors(linker: &mut Linker<HostState>, link: Link) -> wasmtime::Result<()> {
    match link {
        Link::Real => wit::add_to_linker::<_, Real>(linker, |state| RealConnectors {
            run: &mut state.run,
            calls: state.calls.as_mut(),
        }),
        Link::StandIn => wit::add_to_linker::<_, StandIn>(linker, |state| ConnectorsStandIn {
            run: &mut state.run,
        }),
    }
}

/// State of one render or update call, dropped with its store when the call ends.
pub(crate) struct HostState {
    /// Messages kept, in the order the plugin wrote them, escaped and cut to size.
    pub(crate) logs: Vec<LogMessage>,
    /// Messages over `max_log_messages`, counted instead of kept.
    pub(crate) dropped_logs: u32,
    max_log_messages: usize,
    max_log_message_bytes: usize,
    /// The memory cap of this call, installed with `Store::limiter`.
    pub(crate) limiter: MemoryLimiter,
    /// The run's connector call count and request log entries, kept by both the real
    /// `connectors` and its stand-in.
    pub(crate) run: RunLog,
    /// What the real `connectors` reaches: present only when a connector is granted.
    pub(crate) calls: Option<Calls>,
}

impl HostState {
    pub(crate) fn new(limits: &Limits, run: RunLog, calls: Option<Calls>) -> Self {
        Self {
            logs: Vec::new(),
            dropped_logs: 0,
            max_log_messages: limits.max_log_messages,
            max_log_message_bytes: limits.max_log_message_bytes,
            limiter: MemoryLimiter::new(limits.max_memory_bytes),
            run,
            calls,
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

/// The real `connectors` of one call.
pub(crate) struct RealConnectors<'a> {
    run: &'a mut RunLog,
    calls: Option<&'a mut Calls>,
}

/// The marker the linker projects [`RealConnectors`] through.
pub(crate) struct Real;

impl HasData for Real {
    type Data<'a> = RealConnectors<'a>;
}

impl wit::Host for RealConnectors<'_> {
    fn call(
        &mut self,
        connector: String,
        operation: String,
        arguments: Vec<wit::Argument>,
    ) -> Result<wit::Response, wit::CallError> {
        let arguments = arguments
            .into_iter()
            .map(|argument| {
                let value = match argument.value {
                    wit::Value::Text(text) => ArgValue::Text(text),
                    wit::Value::Integer(number) => ArgValue::Integer(number),
                    wit::Value::Flag(flag) => ArgValue::Flag(flag),
                };
                (argument.name, value)
            })
            .collect();
        let answer = match self.calls.as_deref_mut() {
            Some(calls) => calls.call(self.run, &connector, &operation, arguments),
            None => self.run.not_granted(&connector, &operation),
        };
        to_wit(answer)
    }
}

/// Linked in place of `connectors` when the plugin declares connectors and none is granted.
/// It counts and logs each call, as every call is logged, and reaches nothing.
pub(crate) struct ConnectorsStandIn<'a> {
    run: &'a mut RunLog,
}

/// The marker the linker projects [`ConnectorsStandIn`] through.
pub(crate) struct StandIn;

impl HasData for StandIn {
    type Data<'a> = ConnectorsStandIn<'a>;
}

impl wit::Host for ConnectorsStandIn<'_> {
    fn call(
        &mut self,
        connector: String,
        operation: String,
        _arguments: Vec<wit::Argument>,
    ) -> Result<wit::Response, wit::CallError> {
        to_wit(self.run.not_granted(&connector, &operation))
    }
}

fn to_wit(answer: Answer) -> Result<wit::Response, wit::CallError> {
    let problem = |rule: crate::calls::Rule, detail: String| wit::Problem {
        rule: match rule {
            crate::calls::Rule::OperationNotDeclared => wit::Rule::OperationNotDeclared,
            crate::calls::Rule::ValueOutsideType => wit::Rule::ValueOutsideType,
            crate::calls::Rule::CalledOutsideUpdate => wit::Rule::CalledOutsideUpdate,
            crate::calls::Rule::ChangeNeedsUserAction => wit::Rule::ChangeNeedsUserAction,
            crate::calls::Rule::OverLimit => wit::Rule::OverLimit,
            crate::calls::Rule::NonPublicAddress => wit::Rule::NonPublicAddress,
            crate::calls::Rule::RedirectNotAllowed => wit::Rule::RedirectNotAllowed,
            crate::calls::Rule::Unavailable => wit::Rule::Unavailable,
            crate::calls::Rule::InvalidResponse => wit::Rule::InvalidResponse,
        },
        detail,
    };
    match answer {
        Answer::Ok(response) => Ok(wit::Response {
            status: response.status,
            headers: response
                .headers
                .into_iter()
                .map(|(name, value)| wit::Header { name, value })
                .collect(),
            body: response.body,
        }),
        Answer::NotGranted => Err(wit::CallError::NotGranted),
        Answer::Refused(rule, detail) => Err(wit::CallError::Refused(problem(rule, detail))),
        Answer::Failed(rule, detail) => Err(wit::CallError::Failed(problem(rule, detail))),
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
        let limits = Limits {
            max_log_messages: 2,
            max_log_message_bytes: 4,
            ..Limits::default()
        };
        let plugin = crate::manifest::PluginId::parse("local/test").expect("a valid id");
        HostState::new(&limits, RunLog::new(&plugin, None, &limits), None)
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
