//! The host side of one render or update call: the state its store holds, the `log` capability
//! of each contract version, the `connectors` capability of contract 1.1.0, the `storage`
//! capability of contract 1.2.0, the `accounts` capability of contract 1.3.0, and the stand-ins
//! linked in place of a capability that is off.

use wasmtime::component::{HasData, HasSelf, Linker};

use crate::calls::{Answer, Calls, RunLog};
use crate::connector::ArgValue;
use crate::limits::{Limits, MemoryLimiter};
use crate::permissions::Contract;
use crate::storage::{CallError, Rule, StorageRun};
use crate::v1::dashboard::widget::accounts as accounts_wit;
use crate::v1::dashboard::widget::connectors as wit;
use crate::v1::dashboard::widget::storage as storage_wit;
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
        (Contract::V1, "storage", Link::Real) => {
            storage_wit::add_to_linker::<_, HasSelf<_>>(linker, |state| state)
        }
        (Contract::V1, "storage", Link::StandIn) => {
            storage_wit::add_to_linker::<_, StorageStandIn>(linker, |_| StorageStandIn)
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

/// Adds `accounts` to `linker`: the real interface over the plugin's table of connectors it uses
/// with an account, or the stand-in, which holds no table, no vault and no network, and answers
/// `not-granted` to every call (`specs/005-plugin-credentials/research.md`, R9).
pub(crate) fn link_accounts(linker: &mut Linker<HostState>, link: Link) -> wasmtime::Result<()> {
    match link {
        Link::Real => {
            accounts_wit::add_to_linker::<_, RealAccountsMarker>(linker, |state| RealAccounts {
                run: &mut state.run,
                calls: state.account_calls.as_mut(),
            })
        }
        Link::StandIn => accounts_wit::add_to_linker::<_, AccountsStandInMarker>(linker, |state| {
            AccountsStandIn {
                run: &mut state.run,
            }
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
    /// What the real `accounts` reaches: present only when a connector the plugin uses with an
    /// account is granted.
    pub(crate) account_calls: Option<Calls>,
    /// What the real `storage` reaches: the run of the store bound at link time, present only
    /// when storage is granted.
    pub(crate) storage: Option<StorageRun>,
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
            account_calls: None,
            storage: None,
        }
    }

    /// The run's storage; with none, the call reaches nothing and answers `not-granted`.
    fn storage(&mut self) -> Result<&mut StorageRun, storage_wit::StorageError> {
        self.storage
            .as_mut()
            .ok_or(storage_wit::StorageError::NotGranted)
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

/// The real `accounts` of one call.
pub(crate) struct RealAccounts<'a> {
    run: &'a mut RunLog,
    calls: Option<&'a mut Calls>,
}

/// The marker the linker projects [`RealAccounts`] through.
pub(crate) struct RealAccountsMarker;

impl HasData for RealAccountsMarker {
    type Data<'a> = RealAccounts<'a>;
}

impl accounts_wit::Host for RealAccounts<'_> {
    fn call(
        &mut self,
        connector: String,
        operation: String,
        arguments: Vec<wit::Argument>,
    ) -> Result<wit::Response, accounts_wit::AccountCallError> {
        let answer = match self.calls.as_deref_mut() {
            Some(calls) => calls.call(self.run, &connector, &operation, arguments_of(arguments)),
            None => self.run.not_granted(&connector, &operation),
        };
        account_to_wit(answer)
    }
}

/// Linked in place of `accounts` when the plugin declares connectors with an account and none
/// of them is granted. It counts and logs each call, and reaches nothing.
pub(crate) struct AccountsStandIn<'a> {
    run: &'a mut RunLog,
}

/// The marker the linker projects [`AccountsStandIn`] through.
pub(crate) struct AccountsStandInMarker;

impl HasData for AccountsStandInMarker {
    type Data<'a> = AccountsStandIn<'a>;
}

impl accounts_wit::Host for AccountsStandIn<'_> {
    fn call(
        &mut self,
        connector: String,
        operation: String,
        _arguments: Vec<wit::Argument>,
    ) -> Result<wit::Response, accounts_wit::AccountCallError> {
        account_to_wit(self.run.not_granted(&connector, &operation))
    }
}

fn arguments_of(arguments: Vec<wit::Argument>) -> Vec<(String, ArgValue)> {
    arguments
        .into_iter()
        .map(|argument| {
            let value = match argument.value {
                wit::Value::Text(text) => ArgValue::Text(text),
                wit::Value::Integer(number) => ArgValue::Integer(number),
                wit::Value::Flag(flag) => ArgValue::Flag(flag),
            };
            (argument.name, value)
        })
        .collect()
}

fn account_to_wit(answer: Answer) -> Result<wit::Response, accounts_wit::AccountCallError> {
    match answer {
        Answer::AccountNotConnected(_) => Err(accounts_wit::AccountCallError::AccountNotConnected),
        Answer::SubscriptionRequired => Err(accounts_wit::AccountCallError::SubscriptionRequired),
        other => to_wit(other).map_err(|error| match error {
            wit::CallError::NotGranted => accounts_wit::AccountCallError::NotGranted,
            wit::CallError::Refused(problem) => accounts_wit::AccountCallError::Refused(problem),
            wit::CallError::Failed(problem) => accounts_wit::AccountCallError::Failed(problem),
        }),
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
        // Only the account table answers these, and `connectors` never reaches it
        Answer::NotGranted | Answer::AccountNotConnected(_) | Answer::SubscriptionRequired => {
            Err(wit::CallError::NotGranted)
        }
        Answer::Refused(rule, detail) => Err(wit::CallError::Refused(problem(rule, detail))),
        Answer::Failed(rule, detail) => Err(wit::CallError::Failed(problem(rule, detail))),
    }
}

/// The real `storage`: every call goes to the run's [`StorageRun`], which acts only on the store
/// the plugin was linked with (`specs/004-plugin-storage/data-model.md`, A storage call).
impl storage_wit::Host for HostState {
    fn get(&mut self, name: String) -> Result<Option<Vec<u8>>, storage_wit::StorageError> {
        self.storage()?.get(&name).map_err(storage_error)
    }

    fn set(&mut self, name: String, value: Vec<u8>) -> Result<(), storage_wit::StorageError> {
        self.storage()?.set(&name, &value).map_err(storage_error)
    }

    fn delete(&mut self, name: String) -> Result<(), storage_wit::StorageError> {
        self.storage()?.delete(&name).map_err(storage_error)
    }

    fn list(
        &mut self,
        prefix: String,
        after: Option<String>,
    ) -> Result<storage_wit::Page, storage_wit::StorageError> {
        let (names, next) = self
            .storage()?
            .list(&prefix, after.as_deref())
            .map_err(storage_error)?;
        Ok(storage_wit::Page { names, next })
    }

    fn usage(&mut self) -> Result<storage_wit::StoreUsage, storage_wit::StorageError> {
        let (used_bytes, quota_bytes) = self.storage()?.usage().map_err(storage_error)?;
        Ok(storage_wit::StoreUsage {
            used_bytes,
            quota_bytes,
        })
    }
}

fn storage_error(error: CallError) -> storage_wit::StorageError {
    let problem = |rule: Rule, detail: String| storage_wit::Problem {
        rule: match rule {
            Rule::RenderOnlyReads => storage_wit::Rule::RenderOnlyReads,
            Rule::OverQuota => storage_wit::Rule::OverQuota,
            Rule::OverLimit => storage_wit::Rule::OverLimit,
            Rule::InvalidName => storage_wit::Rule::InvalidName,
            Rule::Damaged => storage_wit::Rule::Damaged,
            Rule::Busy => storage_wit::Rule::Busy,
            Rule::Unavailable => storage_wit::Rule::Unavailable,
            Rule::RunAborted => storage_wit::Rule::RunAborted,
        },
        detail,
    };
    match error {
        CallError::NotGranted => storage_wit::StorageError::NotGranted,
        CallError::Refused(rule, detail) => {
            storage_wit::StorageError::Refused(problem(rule, detail))
        }
        CallError::Failed(rule, detail) => storage_wit::StorageError::Failed(problem(rule, detail)),
    }
}

/// Linked in place of `storage` when it is an optional permission that is off (FR-001). It
/// holds no store binding and reaches nothing: every call answers `not-granted`, and nothing is
/// counted, read or written.
pub(crate) struct StorageStandIn;

impl HasData for StorageStandIn {
    type Data<'a> = StorageStandIn;
}

impl storage_wit::Host for StorageStandIn {
    fn get(&mut self, _name: String) -> Result<Option<Vec<u8>>, storage_wit::StorageError> {
        Err(storage_wit::StorageError::NotGranted)
    }

    fn set(&mut self, _name: String, _value: Vec<u8>) -> Result<(), storage_wit::StorageError> {
        Err(storage_wit::StorageError::NotGranted)
    }

    fn delete(&mut self, _name: String) -> Result<(), storage_wit::StorageError> {
        Err(storage_wit::StorageError::NotGranted)
    }

    fn list(
        &mut self,
        _prefix: String,
        _after: Option<String>,
    ) -> Result<storage_wit::Page, storage_wit::StorageError> {
        Err(storage_wit::StorageError::NotGranted)
    }

    fn usage(&mut self) -> Result<storage_wit::StoreUsage, storage_wit::StorageError> {
        Err(storage_wit::StorageError::NotGranted)
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

    /// Step 0 of a storage call (`specs/004-plugin-storage/data-model.md`): the stand-in of
    /// storage that is off answers every call `not-granted`, and so does a run without storage.
    #[test]
    fn storage_off_answers_not_granted_to_every_call() {
        use storage_wit::{Host as _, StorageError};
        let not_granted = |answer: Result<(), StorageError>| {
            assert!(matches!(answer, Err(StorageError::NotGranted)));
        };
        let mut stand_in = StorageStandIn;
        not_granted(stand_in.get("a".to_owned()).map(drop));
        not_granted(stand_in.set("a".to_owned(), b"b".to_vec()));
        not_granted(stand_in.delete("a".to_owned()));
        not_granted(stand_in.list(String::new(), None).map(drop));
        not_granted(stand_in.usage().map(drop));
        let mut state = state();
        not_granted(storage_wit::Host::get(&mut state, "a".to_owned()).map(drop));
    }
}
