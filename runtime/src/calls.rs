//! Connector calls: the steps of `specs/003-plugin-connectors/data-model.md` (A connector call),
//! in order; the first rule that applies answers, and every call is logged (research R10, R12).
//!
//! What a plugin can reach is decided when it is linked: its [`ConnectorTable`] holds only the
//! connectors that were granted then (Principle II, constitution v2.3.0). A call first looks
//! its connector up there, and every later check can only take that connector away: nothing at
//! call time adds one. A plugin with no connector granted is linked to the stand-in instead,
//! which holds no table, no network and no grants, and answers "not granted" to every call.
//!
//! Calls through `accounts` (`specs/005-plugin-credentials/data-model.md`, An account call) go
//! over a table of their own, of the connectors the widget uses with an account, each with the
//! account connected when the widget was linked. Their steps add the account and its credential;
//! every one of them can only take the account away.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::accounts::{Account, AccountStore, Reason};
use crate::connector::{ArgValue, Connector, Credential, Kind};
use crate::grants::{Entry, GrantStore};
use crate::limits::Limits;
use crate::manifest::{Manifest, PluginId};
use crate::network::{self, FailureRule, RequestCredential, Session};
use crate::requests::{self, RequestLog, Summary};
use crate::signin;
use crate::vault::{Secret, Stored, Vault, VaultError, entry_user};

/// The rule behind a call that was refused or failed, as contract 1.1.0's `rule` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rule {
    OperationNotDeclared,
    ValueOutsideType,
    CalledOutsideUpdate,
    ChangeNeedsUserAction,
    OverLimit,
    NonPublicAddress,
    RedirectNotAllowed,
    Unavailable,
    InvalidResponse,
}

impl Rule {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Rule::OperationNotDeclared => "operation-not-declared",
            Rule::ValueOutsideType => "value-outside-type",
            Rule::CalledOutsideUpdate => "called-outside-update",
            Rule::ChangeNeedsUserAction => "change-needs-user-action",
            Rule::OverLimit => "over-limit",
            Rule::NonPublicAddress => "non-public-address",
            Rule::RedirectNotAllowed => "redirect-not-allowed",
            Rule::Unavailable => "unavailable",
            Rule::InvalidResponse => "invalid-response",
        }
    }
}

impl From<FailureRule> for Rule {
    fn from(rule: FailureRule) -> Rule {
        match rule {
            FailureRule::OverLimit => Rule::OverLimit,
            FailureRule::NonPublicAddress => Rule::NonPublicAddress,
            FailureRule::RedirectNotAllowed => Rule::RedirectNotAllowed,
            FailureRule::Unavailable => Rule::Unavailable,
            FailureRule::InvalidResponse => Rule::InvalidResponse,
        }
    }
}

/// What a call answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Answer {
    Ok(network::Response),
    NotGranted,
    /// Refused before anything was sent.
    Refused(Rule, String),
    /// Attempted, with no response for the widget.
    Failed(Rule, String),
    /// No account is connected, or it must be connected again (`accounts` only).
    AccountNotConnected(String),
    /// The service answered that the account lacks a paid plan (`accounts` only).
    SubscriptionRequired,
}

/// Which entry point is running.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Phase {
    Render,
    Update,
}

/// One connector a plugin can call: its checked declaration and the operations the manifest
/// declares for it.
#[derive(Debug, Clone)]
pub(crate) struct Executor {
    pub(crate) connector: Arc<Connector>,
    pub(crate) declared: Vec<String>,
    /// In the account table only: the account the widget acts as.
    pub(crate) account: Option<AccountBinding>,
}

/// The account a connector of the account table acts as, taken when the widget was linked
/// (research R11, step 0): one connected afterwards, by any process, waits for the next load.
#[derive(Debug, Clone)]
pub(crate) struct AccountBinding {
    /// `catalog`, or the canonical path of the connector's file: the account's key with its id.
    pub(crate) origin: String,
    pub(crate) account: Option<Account>,
}

/// The connectors a plugin can call, built when it is linked from the connectors granted then.
#[derive(Debug, Clone, Default)]
pub(crate) struct ConnectorTable {
    executors: BTreeMap<String, Executor>,
}

impl ConnectorTable {
    /// The table of `loaded`, the connectors `manifest` declares as loaded now, keeping only
    /// those `entry` grants; `None` when it grants none, so the stand-in is linked.
    pub(crate) fn build(
        manifest: &Manifest,
        loaded: &[(String, Arc<Connector>)],
        entry: &Entry,
    ) -> Option<ConnectorTable> {
        let executors: BTreeMap<String, Executor> = loaded
            .iter()
            .filter(|(id, _)| entry.is_connector_granted(id))
            .filter_map(|(id, connector)| {
                let request = manifest.connector(id)?;
                // A connector used with its account is in the account table instead
                if request.account && entry.uses_account(id) {
                    return None;
                }
                Some((
                    id.clone(),
                    Executor {
                        connector: Arc::clone(connector),
                        declared: request.operations.clone(),
                        account: None,
                    },
                ))
            })
            .collect();
        (!executors.is_empty()).then_some(ConnectorTable { executors })
    }

    /// The account table: the granted connectors the manifest declares with `account: true`
    /// and the approval lets the widget use with their account, each with `account(id)`, the
    /// account connected now; `None` when there is none, so the stand-in is linked.
    pub(crate) fn build_accounts(
        manifest: &Manifest,
        loaded: &[(String, String, Arc<Connector>)],
        entry: &Entry,
        account: impl Fn(&str, &str) -> Option<Account>,
    ) -> Option<ConnectorTable> {
        let executors: BTreeMap<String, Executor> = loaded
            .iter()
            .filter(|(id, _, connector)| entry.uses_account(id) && connector.credential().is_some())
            .filter_map(|(id, origin, connector)| {
                let request = manifest.connector(id).filter(|request| request.account)?;
                Some((
                    id.clone(),
                    Executor {
                        connector: Arc::clone(connector),
                        declared: request.operations.clone(),
                        account: Some(AccountBinding {
                            origin: origin.clone(),
                            account: account(id, origin),
                        }),
                    },
                ))
            })
            .collect();
        (!executors.is_empty()).then_some(ConnectorTable { executors })
    }

    /// For each connector of an account table, the account id it acts as, or `None` without
    /// one: what the kept data records it was fetched with.
    pub(crate) fn account_ids(&self) -> BTreeMap<String, Option<String>> {
        self.executors
            .iter()
            .filter_map(|(id, executor)| {
                let binding = executor.account.as_ref()?;
                Some((
                    id.clone(),
                    binding
                        .account
                        .as_ref()
                        .map(|account| account.account_id.clone()),
                ))
            })
            .collect()
    }

    /// The connectors in the table, by id: for the white-box case `connector-table`.
    #[cfg(any(test, feature = "test-network"))]
    pub(crate) fn keys(&self) -> Vec<String> {
        self.executors.keys().cloned().collect()
    }
}

/// How a run's connector calls went, for the update report.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CallCounts {
    pub calls: u64,
    pub not_granted: u64,
    pub refused: u64,
    pub failed: u64,
    /// Bytes of response bodies received, redirects included.
    pub received_bytes: u64,
    /// The connectors whose calls answered `account-not-connected`, sorted, each once.
    pub needs_connecting: Vec<String>,
}

/// The call limits of one run of `update` or `render`, and its request log entries: what both
/// the real `connectors` and the stand-in keep (step 0).
pub(crate) struct RunLog {
    plugin: String,
    log: Option<RequestLog>,
    max_calls: usize,
    calls: usize,
    summary: Summary,
    pub(crate) counts: CallCounts,
}

impl RunLog {
    pub(crate) fn new(plugin: &PluginId, log: Option<RequestLog>, limits: &Limits) -> RunLog {
        RunLog {
            plugin: plugin.to_string(),
            log,
            max_calls: limits.max_calls_per_run,
            calls: 0,
            summary: Summary::default(),
            counts: CallCounts::default(),
        }
    }

    /// Step 0: counts the call, whatever follows; true when it is past the run's limit.
    fn count(&mut self) -> bool {
        self.calls += 1;
        self.calls > self.max_calls
    }

    /// Logs a call's entries, or counts it in the summary when it was past the limit.
    fn record(
        &mut self,
        past: bool,
        connector: &str,
        answer: &Answer,
        entries: Vec<requests::Entry>,
    ) {
        self.counts.calls += 1;
        match answer {
            Answer::Ok(_) => {}
            Answer::NotGranted => self.counts.not_granted += 1,
            Answer::Refused(..) | Answer::AccountNotConnected(_) | Answer::SubscriptionRequired => {
                self.counts.refused += 1
            }
            Answer::Failed(..) => self.counts.failed += 1,
        }
        if let Answer::AccountNotConnected(_) = answer
            && let Err(at) = self
                .counts
                .needs_connecting
                .binary_search_by(|id| id.as_str().cmp(connector))
        {
            self.counts
                .needs_connecting
                .insert(at, connector.to_owned());
        }
        self.counts.received_bytes += entries
            .iter()
            .map(|entry| entry.received_bytes)
            .sum::<u64>();
        if past {
            self.summary.calls += 1;
            match answer {
                Answer::NotGranted => self.summary.not_granted += 1,
                _ => self.summary.refused += 1,
            }
            return;
        }
        if let Some(log) = &self.log {
            for entry in entries {
                // The log is the host's record; a call is never refused because it cannot be
                // written, and the CLI reads what was written
                let _ = log.append(entry);
            }
        }
    }

    /// The one summary entry of a run past its limit, written when the run ends, whatever its
    /// outcome.
    pub(crate) fn finish(&mut self) {
        if self.summary.calls == 0 {
            return;
        }
        let summary = std::mem::take(&mut self.summary);
        if let Some(log) = &self.log {
            let _ = log.append(requests::Entry {
                at_ms: now_ms(),
                plugin: self.plugin.clone(),
                connector: String::new(),
                operation: String::new(),
                kind: None,
                method: None,
                host: None,
                path: None,
                arguments: BTreeMap::new(),
                outcome: requests::Outcome::Refused {
                    refused: Rule::OverLimit.as_str().to_owned(),
                    detail: format!(
                        "{} more calls past the limit of {} in this run",
                        summary.calls, self.max_calls
                    ),
                },
                sent_bytes: 0,
                received_bytes: 0,
                duration_ms: 0,
                redirect_of: None,
                summary: Some(summary),
                account: None,
            });
        }
    }

    /// The stand-in's whole answer: counted, logged, not granted, nothing reached.
    pub(crate) fn not_granted(&mut self, connector: &str, operation: &str) -> Answer {
        let past = self.count();
        let answer = Answer::NotGranted;
        let entry = entry(&self.plugin, connector, operation, &[], &answer);
        self.record(past, connector, &answer, vec![entry]);
        answer
    }
}

/// The rate limits of one runtime: calls per minute per widget and per destination host, as
/// token buckets shared by every plugin the runtime runs (research R10).
#[derive(Debug, Default)]
pub(crate) struct Rates {
    widgets: Mutex<HashMap<String, Bucket>>,
    hosts: Mutex<HashMap<String, Bucket>>,
}

#[derive(Debug, Clone, Copy)]
struct Bucket {
    tokens: f64,
    at: Instant,
}

impl Bucket {
    fn refill(&mut self, per_minute: u32, now: Instant) {
        let elapsed = now.saturating_duration_since(self.at).as_secs_f64();
        self.tokens =
            (self.tokens + elapsed * f64::from(per_minute) / 60.0).min(f64::from(per_minute));
        self.at = now;
    }
}

impl Rates {
    /// Takes a call from both buckets, or refuses naming the limit, taking nothing.
    fn take(&self, widget: &str, host: &str, limits: &Limits) -> Result<(), String> {
        let now = Instant::now();
        let mut widgets = self.widgets.lock().unwrap_or_else(PoisonError::into_inner);
        let mut hosts = self.hosts.lock().unwrap_or_else(PoisonError::into_inner);
        let full = |per_minute: u32| Bucket {
            tokens: f64::from(per_minute),
            at: now,
        };
        let widget_bucket = widgets
            .entry(widget.to_owned())
            .or_insert_with(|| full(limits.calls_per_minute_per_widget));
        widget_bucket.refill(limits.calls_per_minute_per_widget, now);
        let host_bucket = hosts
            .entry(host.to_owned())
            .or_insert_with(|| full(limits.calls_per_minute_per_host));
        host_bucket.refill(limits.calls_per_minute_per_host, now);
        if widget_bucket.tokens < 1.0 {
            return Err(format!(
                "more than {} calls per minute from this widget",
                limits.calls_per_minute_per_widget
            ));
        }
        if host_bucket.tokens < 1.0 {
            return Err(format!(
                "more than {} calls per minute to {host}, from every widget together",
                limits.calls_per_minute_per_host
            ));
        }
        widget_bucket.tokens -= 1.0;
        host_bucket.tokens -= 1.0;
        Ok(())
    }
}

/// What a call through `accounts` reaches beyond any connector call: the accounts record, the
/// vault, and the credentials already read in this run.
pub(crate) struct AccountAccess {
    pub(crate) store: AccountStore,
    pub(crate) vault: Arc<Vault>,
    /// The record's vault id when the widget was linked; `None` when no record existed.
    pub(crate) vault_id: Option<String>,
    /// Read from the vault once per run, held until the run ends (research R11, step 4).
    pub(crate) secrets: HashMap<String, Stored>,
}

/// Why an account step answered instead of sending.
type Stop = Answer;

/// What the real `connectors` or `accounts` holds for one run.
pub(crate) struct Calls {
    pub(crate) plugin: PluginId,
    pub(crate) origin: String,
    pub(crate) grants: Arc<GrantStore>,
    pub(crate) table: Arc<ConnectorTable>,
    pub(crate) phase: Phase,
    /// The update's agents; `None` in render, which never reaches the network.
    pub(crate) session: Option<Session>,
    pub(crate) deadline: Instant,
    pub(crate) update_bytes_left: usize,
    pub(crate) limits: Limits,
    pub(crate) rates: Arc<Rates>,
    /// For the account table only.
    pub(crate) account: Option<AccountAccess>,
}

impl Calls {
    /// One call, steps 0–13 of the data model.
    pub(crate) fn call(
        &mut self,
        run: &mut RunLog,
        connector: &str,
        operation: &str,
        arguments: Vec<(String, ArgValue)>,
    ) -> Answer {
        // Step 0
        let past = run.count();
        let (answer, entries) = self.answer(connector, operation, &arguments, past);
        run.record(past, connector, &answer, entries);
        answer
    }

    fn answer(
        &mut self,
        connector_id: &str,
        operation_name: &str,
        arguments: &[(String, ArgValue)],
        past_limit: bool,
    ) -> (Answer, Vec<requests::Entry>) {
        let plugin = self.plugin.to_string();
        let logged = |answer: Answer, kind: Option<Kind>| {
            let mut entry = entry(&plugin, connector_id, operation_name, arguments, &answer);
            entry.kind = kind.map(|kind| kind.as_str().to_owned());
            (answer, vec![entry])
        };
        // Step 1: in the table built at link time, or not granted, whatever else is wrong
        let Some(executor) = self.table.executors.get(connector_id) else {
            return logged(Answer::NotGranted, None);
        };
        // Step 2: connectors are called only in an update; render reads no grant record
        if self.phase != Phase::Update {
            return logged(
                Answer::Refused(
                    Rule::CalledOutsideUpdate,
                    "connectors are called only during an update, never in render".to_owned(),
                ),
                None,
            );
        }
        // Step 3
        let connector = Arc::clone(&executor.connector);
        let declared = executor.declared.iter().any(|name| name == operation_name);
        let operation = connector.operation(operation_name).filter(|_| declared);
        let Some(operation) = operation else {
            return logged(
                Answer::Refused(
                    Rule::OperationNotDeclared,
                    format!("the manifest does not declare `{operation_name}` for {connector_id}"),
                ),
                None,
            );
        };
        let kind = Some(operation.kind());
        // Step 4
        if operation.kind() != Kind::Read {
            return logged(
                Answer::Refused(
                    Rule::ChangeNeedsUserAction,
                    format!(
                        "`{operation_name}` changes data, which needs a user action; until Phase 3, update only reads"
                    ),
                ),
                kind,
            );
        }
        // Step 5
        let checked = match operation.check_arguments(arguments, self.limits.max_arguments) {
            Ok(checked) => checked,
            Err(problem) => {
                return logged(Answer::Refused(Rule::ValueOutsideType, problem), kind);
            }
        };
        // Step 6: calls per run, then the rates
        let origin = connector
            .destination(operation.destination())
            .expect("a checked operation's destination is declared");
        if past_limit {
            return logged(
                Answer::Refused(
                    Rule::OverLimit,
                    format!(
                        "more than {} calls in this run",
                        self.limits.max_calls_per_run
                    ),
                ),
                kind,
            );
        }
        if let Err(why) = self.rates.take(&plugin, origin.host(), &self.limits) {
            return logged(Answer::Refused(Rule::OverLimit, why), kind);
        }
        // Step 7: the grant record, read again, still grants it
        if !self.still_granted(connector_id) {
            return logged(Answer::NotGranted, kind);
        }
        let mut host_entries = Vec::new();
        // Step 7a: the account and its credential (specs/005-plugin-credentials/research.md, R11)
        let binding = executor.account.clone();
        let declared_credential = connector.credential().cloned();
        let mut refreshed = false;
        let credential = match (&binding, &declared_credential) {
            (Some(binding), Some(declared)) => {
                match self.credential(
                    connector_id,
                    binding,
                    declared,
                    &mut refreshed,
                    &mut host_entries,
                ) {
                    Ok(credential) => credential,
                    Err(stop) => {
                        let (answer, mut entries) = logged(stop, kind);
                        host_entries.append(&mut entries);
                        return (answer, host_entries);
                    }
                }
            }
            _ => None,
        };
        // Step 8: build the request within its size limits
        let mut request =
            match network::build_request(&connector, operation, &checked, &self.limits) {
                Ok(request) => request,
                Err(network::Refusal(why)) => {
                    return logged(Answer::Refused(Rule::OverLimit, why), kind);
                }
            };
        // Step 8a: the credential rides along; the network module adds it per hop (R10)
        request.credential = credential;
        let Some(session) = &self.session else {
            return logged(
                Answer::Refused(
                    Rule::CalledOutsideUpdate,
                    "connectors are called only during an update".to_owned(),
                ),
                kind,
            );
        };
        let started = now_ms();
        // Steps 8–11: resolve, connect, TLS, response, redirects, media type
        let resend = request.clone();
        let mut exchange = session.send(
            &connector,
            operation,
            request,
            self.deadline,
            &mut self.update_bytes_left,
        );
        let carried = exchange.hops.iter().any(|hop| hop.with_credential);
        let unauthorized = matches!(&exchange.result, Ok(response) if response.status == 401);
        // Step 9a: a 401 to a request that carried the credential (research R10)
        let mut account_stop = None;
        if carried && unauthorized {
            let (Some(binding), Some(declared)) = (&binding, &declared_credential) else {
                unreachable!("only an account call carries a credential");
            };
            let mut retried = None;
            if !refreshed
                && declared.account().is_some()
                && let Ok(Some(credential)) = self.refreshed_credential(
                    connector_id,
                    binding,
                    declared,
                    None,
                    &mut refreshed,
                    &mut host_entries,
                )
            {
                let mut again = resend.clone();
                again.credential = Some(credential);
                let Some(session) = &self.session else {
                    unreachable!("an update has a session");
                };
                let second = session.send(
                    &connector,
                    operation,
                    again,
                    self.deadline,
                    &mut self.update_bytes_left,
                );
                retried = Some(second);
            }
            if let Some(second) = retried {
                let mut hops = std::mem::take(&mut exchange.hops);
                hops.extend(second.hops);
                exchange = network::Exchange {
                    hops,
                    result: second.result,
                };
            }
            if matches!(&exchange.result, Ok(response) if response.status == 401) {
                account_stop = Some(self.confirm_refusal(
                    connector_id,
                    binding,
                    declared,
                    &connector,
                    &mut host_entries,
                ));
            }
        }
        let mut answer = match (account_stop, exchange.result) {
            (Some(stop), _) => stop,
            (None, Ok(response)) => {
                // Step 11b: a status that means a paid plan is missing
                if declared_credential.as_ref().is_some_and(|declared| {
                    binding.is_some() && declared.subscription_required().contains(&response.status)
                }) {
                    Answer::SubscriptionRequired
                } else {
                    Answer::Ok(response)
                }
            }
            (None, Err(failure)) => Answer::Failed(failure.rule.into(), failure.detail),
        };
        // Step 12: still granted when the response arrives, or it is dropped
        if matches!(answer, Answer::Ok(_)) && !self.still_granted(connector_id) {
            answer = Answer::NotGranted;
        }
        // Step 12a: the account is still the one the widget was linked with
        if matches!(answer, Answer::Ok(_) | Answer::SubscriptionRequired)
            && let Some(binding) = &binding
            && let Some(account) = &binding.account
            && let Err(stop) = self.still_connected(connector_id, binding, account)
        {
            answer = stop;
        }
        let sent: Vec<(String, ArgValue)> = checked.into_iter().collect();
        let count = exchange.hops.len();
        let entries: Vec<requests::Entry> = exchange
            .hops
            .into_iter()
            .enumerate()
            .map(|(index, hop)| {
                let last = index + 1 == count;
                let outcome = match (&hop.outcome, last, &answer) {
                    (Err(failure), _, _) => requests::Outcome::Failed {
                        failed: Rule::from(failure.rule).as_str().to_owned(),
                        detail: failure.detail.clone(),
                    },
                    (Ok(_), true, Answer::Failed(rule, detail)) => requests::Outcome::Failed {
                        failed: rule.as_str().to_owned(),
                        detail: detail.clone(),
                    },
                    (Ok(_), true, Answer::NotGranted) => requests::Outcome::not_granted(),
                    (Ok(_), true, Answer::AccountNotConnected(detail)) => {
                        requests::Outcome::Refused {
                            refused: ACCOUNT_NOT_CONNECTED.to_owned(),
                            detail: detail.clone(),
                        }
                    }
                    (Ok(status), _, _) => requests::Outcome::Status { status: *status },
                };
                let account = hop
                    .with_credential
                    .then(|| account_name(binding.as_ref()))
                    .flatten();
                requests::Entry {
                    at_ms: if index == 0 { started } else { now_ms() },
                    plugin: plugin.clone(),
                    connector: connector_id.to_owned(),
                    operation: operation_name.to_owned(),
                    kind: kind.map(|kind| kind.as_str().to_owned()),
                    method: Some(hop.method.to_owned()),
                    host: Some(hop.host),
                    path: Some(hop.target),
                    arguments: sent
                        .iter()
                        .map(|(name, value)| (name.clone(), text_of(value)))
                        .collect(),
                    outcome,
                    sent_bytes: hop.sent_bytes,
                    received_bytes: hop.received_bytes,
                    duration_ms: u64::try_from(hop.duration.as_millis()).unwrap_or(u64::MAX),
                    redirect_of: (index > 0).then(|| u32::try_from(index).unwrap_or(u32::MAX)),
                    summary: None,
                    account,
                }
            })
            .collect();
        host_entries.extend(entries);
        (answer, host_entries)
    }

    /// Steps 7 and 12: the grant record, read again under its lock, still grants the
    /// connector. A record that cannot be read grants nothing.
    fn still_granted(&self, connector: &str) -> bool {
        self.grants
            .connector_granted(&self.plugin, &self.origin, connector)
            .unwrap_or(false)
    }

    /// Step 7a (research R11, steps 1–5): the credential to send, `None` to send without one,
    /// or the answer that stops the call before anything is sent.
    fn credential(
        &mut self,
        id: &str,
        binding: &AccountBinding,
        declared: &Credential,
        refreshed: &mut bool,
        host_entries: &mut Vec<requests::Entry>,
    ) -> Result<Option<RequestCredential>, Stop> {
        let Some(snapshot) = &binding.account else {
            // Steps 1 and 2: no account when the widget was linked
            return if declared.required() {
                Err(Answer::AccountNotConnected(format!(
                    "no account is connected to {id}, and its credential is required"
                )))
            } else {
                Ok(None)
            };
        };
        let current = self.still_connected(id, binding, snapshot)?;
        // Step 3: the connector asks for more access than the account was connected with
        let missing: Vec<String> = declared
            .access()
            .iter()
            .filter(|item| {
                !current
                    .access_asked
                    .iter()
                    .any(|held| held.name == item.name && held.level >= item.level)
            })
            .map(|item| format!("{} {}", item.name, item.level.as_str()))
            .collect();
        if !missing.is_empty() {
            self.mark(id, binding, snapshot, Reason::LessAccess);
            return Err(Answer::AccountNotConnected(format!(
                "{id} now asks for more access than its account holds ({}); connect it again",
                missing.join(", ")
            )));
        }
        // Step 4: the secret, read once per run
        let stored = self.stored(id, binding, snapshot)?;
        // Step 5: a token about to expire is refreshed first, at most once per call
        let margin = u64::try_from(self.limits.refresh_margin.as_millis()).unwrap_or(u64::MAX);
        let expiring = current
            .expires_at_ms
            .is_some_and(|expires| expires <= now_ms().saturating_add(margin));
        if expiring && declared.account().is_some() {
            return self.refreshed_credential(
                id,
                binding,
                declared,
                Some(current.refreshed_at_ms),
                refreshed,
                host_entries,
            );
        }
        request_credential(declared, &stored).map(Some)
    }

    /// The account as the record holds it now, if it is still the one the widget was linked
    /// with and needs nothing; else the answer that stops the call (steps 2 and 12a).
    fn still_connected(
        &self,
        id: &str,
        binding: &AccountBinding,
        snapshot: &Account,
    ) -> Result<Account, Stop> {
        let Some(access) = &self.account else {
            unreachable!("the account table always comes with its account access");
        };
        let current = access
            .store
            .get(id, &binding.origin)
            .map_err(|error| Answer::AccountNotConnected(error.detail().to_owned()))?;
        match current {
            Some(current) if current.account_id == snapshot.account_id => {
                match &current.needs_connecting {
                    None => Ok(current),
                    Some(needs) => Err(Answer::AccountNotConnected(format!(
                        "the account of {id} must be connected again ({})",
                        reason_text(needs.reason)
                    ))),
                }
            }
            _ => Err(Answer::AccountNotConnected(format!(
                "the account of {id} was disconnected or replaced"
            ))),
        }
    }

    /// The account's vault entry, read once per run; a missing one marks the account.
    fn stored(
        &mut self,
        id: &str,
        binding: &AccountBinding,
        snapshot: &Account,
    ) -> Result<Stored, Stop> {
        if let Some(stored) = self
            .account
            .as_ref()
            .and_then(|access| access.secrets.get(id))
        {
            return Ok(stored.clone());
        }
        let user = self.vault_user(id, binding)?;
        let read = {
            let access = self.account.as_ref().expect("account access");
            access.vault.read(&user).within(self.limits.call_time_limit)
        };
        match read {
            Ok(stored) => {
                if let Some(access) = &mut self.account {
                    access.secrets.insert(id.to_owned(), stored.clone());
                }
                Ok(stored)
            }
            Err(VaultError::NoEntry) => {
                self.mark(id, binding, snapshot, Reason::MissingFromKeychain);
                Err(Answer::AccountNotConnected(format!(
                    "the keychain has no credential for the account of {id}; connect it again"
                )))
            }
            Err(error) => Err(Answer::Failed(Rule::Unavailable, error.to_string())),
        }
    }

    fn vault_user(&self, id: &str, binding: &AccountBinding) -> Result<String, Stop> {
        self.account
            .as_ref()
            .and_then(|access| access.vault_id.as_deref())
            .map(|vault_id| entry_user(vault_id, id, &binding.origin))
            .ok_or_else(|| {
                Answer::AccountNotConnected(format!("the accounts record has no vault for {id}"))
            })
    }

    /// Marks the account as needing connecting again; a record that cannot be written leaves
    /// the call's answer as it is.
    fn mark(&self, id: &str, binding: &AccountBinding, snapshot: &Account, reason: Reason) {
        if let Some(access) = &self.account {
            let _ = access.store.mark_needs_connecting(
                id,
                &binding.origin,
                &snapshot.account_id,
                reason,
                now_ms(),
            );
        }
    }

    /// Refreshes the token under the accounts record's lock (research R6, R11 step 5), at most
    /// once per call, and returns the credential to send; a refresh another process made
    /// meanwhile is used as it is. `expiring_at` is the `refreshed_at_ms` of the record that
    /// showed the token about to expire, `None` when the service refused the token sent. A
    /// refresh that fails, or whose new token the vault does not keep, marks the account.
    fn refreshed_credential(
        &mut self,
        id: &str,
        binding: &AccountBinding,
        declared: &Credential,
        expiring_at: Option<u64>,
        refreshed: &mut bool,
        host_entries: &mut Vec<requests::Entry>,
    ) -> Result<Option<RequestCredential>, Stop> {
        let Some(snapshot) = binding.account.clone() else {
            return Err(Answer::AccountNotConnected(format!(
                "no account is connected to {id}"
            )));
        };
        if *refreshed {
            return Err(Answer::AccountNotConnected(format!(
                "the account of {id} was refused after a refresh; connect it again"
            )));
        }
        *refreshed = true;
        let user = self.vault_user(id, binding)?;
        let Some(account_decl) = declared.account().cloned() else {
            return Err(Answer::AccountNotConnected(format!(
                "a key of {id} cannot be refreshed"
            )));
        };
        let (Some(access), Some(session)) = (&self.account, &self.session) else {
            unreachable!("an account call in an update has its access and its session");
        };
        let vault = Arc::clone(&access.vault);
        let deadline = self.deadline;
        let wait = deadline.saturating_duration_since(Instant::now());
        let max_bytes = self.limits.max_sign_in_response_bytes;
        let plugin = self.plugin.to_string();
        let cached = access.secrets.get(id).cloned();
        let result = access
            .store
            .with_account_locked(id, &binding.origin, |current| {
                let Some(current) =
                    current.filter(|current| current.account_id == snapshot.account_id)
                else {
                    return Ok((
                        Err(format!("the account of {id} was disconnected or replaced")),
                        None,
                    ));
                };
                let stored = match vault.read(&user).within(wait) {
                    Ok(stored) => stored,
                    Err(error) => return Ok((Err(error.to_string()), None)),
                };
                // Another process refreshed since the record showed the token expiring, even
                // before this run read the secret, or since the token refused was read: use
                // what it kept
                let elsewhere = match expiring_at {
                    Some(seen) => current.refreshed_at_ms > seen,
                    None => {
                        current.refreshed_at_ms > snapshot.refreshed_at_ms
                            && Some(&stored) != cached.as_ref()
                    }
                };
                if elsewhere {
                    return Ok((Ok(stored), None));
                }
                let mut failed = current.clone();
                failed.needs_connecting = Some(crate::accounts::NeedsConnecting {
                    reason: Reason::RefreshFailed,
                    at_ms: now_ms(),
                });
                let Stored::Account {
                    refresh: Some(refresh_token),
                    ..
                } = &stored
                else {
                    return Ok((
                        Err(format!("the account of {id} has no refresh token")),
                        Some(failed),
                    ));
                };
                let Some(client) = &current.client else {
                    return Ok((
                        Err(format!("the account of {id} has no client registration")),
                        Some(failed),
                    ));
                };
                let started = now_ms();
                let answer = session.host_post(
                    account_decl.token(),
                    &[
                        ("grant_type", "refresh_token"),
                        ("refresh_token", refresh_token.expose()),
                        ("client_id", &client.client_id),
                    ],
                    deadline,
                    max_bytes,
                );
                host_entries.push(host_entry(
                    &plugin,
                    id,
                    "refresh",
                    account_decl.token(),
                    started,
                    &answer,
                ));
                let tokens = match answer {
                    Ok(answer) => {
                        signin::tokens(answer.status, &answer.body, &[refresh_token.expose()])
                    }
                    Err(failure) => Err(failure.detail),
                };
                let tokens = match tokens {
                    Ok(tokens) => tokens,
                    Err(why) => return Ok((Err(why), Some(failed))),
                };
                let renewed = Stored::Account {
                    access: tokens.access,
                    refresh: tokens.refresh.or_else(|| Some(refresh_token.clone())),
                };
                // The lock is held until the vault answered: the old refresh token may be dead now
                if let Err(error) = vault.write(&user, &renewed).within(wait) {
                    return Ok((Err(error.to_string()), Some(failed)));
                }
                let now = now_ms();
                let mut updated = current.clone();
                updated.refreshed_at_ms = now;
                updated.expires_at_ms = tokens
                    .expires_in_s
                    .map(|seconds| now.saturating_add(seconds.saturating_mul(1000)));
                if !tokens.scopes.is_empty() {
                    updated.access_reported = tokens.scopes;
                }
                Ok((Ok(renewed), Some(updated)))
            });
        match result {
            Ok(Ok(stored)) => {
                if let Some(access) = &mut self.account {
                    access.secrets.insert(id.to_owned(), stored.clone());
                }
                request_credential(declared, &stored).map(Some)
            }
            Ok(Err(why)) => Err(Answer::AccountNotConnected(format!(
                "the token of {id} could not be refreshed ({why}); connect the account again"
            ))),
            Err(error) => Err(Answer::AccountNotConnected(error.detail().to_owned())),
        }
    }

    /// Step 9a's end: a `401` that a refresh did not cure is confirmed with the identity
    /// operation. Refused too, the account is marked; answered, only this call stops, so one
    /// widget cannot lock the shared account out for the others (FR-011).
    fn confirm_refusal(
        &mut self,
        id: &str,
        binding: &AccountBinding,
        declared: &Credential,
        connector: &Arc<Connector>,
        host_entries: &mut Vec<requests::Entry>,
    ) -> Answer {
        let Some(snapshot) = binding.account.clone() else {
            return Answer::AccountNotConnected(format!("no account is connected to {id}"));
        };
        let refused_too = match declared.account().and_then(|account| account.identity()) {
            None => true,
            Some(identity) => {
                let stored = self
                    .account
                    .as_ref()
                    .and_then(|access| access.secrets.get(id))
                    .cloned();
                let operation = connector.operation(identity.operation());
                match (stored, operation, &self.session) {
                    (Some(stored), Some(operation), Some(session)) => {
                        match (
                            network::build_request(
                                connector,
                                operation,
                                &BTreeMap::new(),
                                &self.limits,
                            ),
                            request_credential(declared, &stored),
                        ) {
                            (Ok(mut request), Ok(credential)) => {
                                let url = request.url.clone();
                                request.credential = Some(credential);
                                let started = now_ms();
                                let exchange = session.send(
                                    connector,
                                    operation,
                                    request,
                                    self.deadline,
                                    &mut self.update_bytes_left,
                                );
                                let status =
                                    exchange.result.as_ref().map(|response| response.status);
                                host_entries.push(identity_entry(
                                    id,
                                    &url,
                                    started,
                                    &exchange,
                                    account_name(Some(binding)),
                                ));
                                matches!(status, Ok(401))
                            }
                            _ => true,
                        }
                    }
                    _ => true,
                }
            }
        };
        if refused_too {
            self.mark(id, binding, &snapshot, Reason::Refused);
            Answer::AccountNotConnected(format!(
                "the service refused the account of {id} (401); connect it again"
            ))
        } else {
            Answer::AccountNotConnected(format!(
                "the service refused this request as unauthorized (401), while the account of {id} still works"
            ))
        }
    }
}

/// The plugin name of the host's own entries in the request log.
pub(crate) const HOST: &str = "host";
const ACCOUNT_NOT_CONNECTED: &str = "account-not-connected";
const SUBSCRIPTION_REQUIRED: &str = "subscription-required";

fn reason_text(reason: Reason) -> &'static str {
    match reason {
        Reason::Refused => "the service refused it",
        Reason::RefreshFailed => "its token could not be refreshed",
        Reason::MissingFromKeychain => "the keychain has no credential for it",
        Reason::LessAccess => "the connector asks for more access than it holds",
    }
}

/// The name the request log gives the account a request acted as: its name, or `key`.
fn account_name(binding: Option<&AccountBinding>) -> Option<String> {
    binding
        .and_then(|binding| binding.account.as_ref())
        .map(|account| account.name.clone())
}

/// The credential to send from what the vault keeps.
fn request_credential(declared: &Credential, stored: &Stored) -> Result<RequestCredential, Stop> {
    let secret: &Secret = match stored {
        Stored::Account { access, .. } => access,
        Stored::Key { key } => key,
    };
    RequestCredential::new(declared, secret)
        .map_err(|why| Answer::Failed(Rule::Unavailable, why.to_owned()))
}

/// The request log's entry of the host's own identity request, made with the account's
/// credential: never a body, and no query.
pub(crate) fn identity_entry(
    connector: &str,
    url: &url::Url,
    started: u64,
    exchange: &network::Exchange,
    account: Option<String>,
) -> requests::Entry {
    requests::Entry {
        at_ms: started,
        plugin: HOST.to_owned(),
        connector: connector.to_owned(),
        operation: "identity".to_owned(),
        kind: None,
        method: Some("GET".to_owned()),
        host: url.host_str().map(str::to_owned),
        path: Some(url.path().to_owned()),
        arguments: BTreeMap::new(),
        outcome: match &exchange.result {
            Ok(response) => requests::Outcome::Status {
                status: response.status,
            },
            Err(failure) => requests::Outcome::Failed {
                failed: Rule::from(failure.rule).as_str().to_owned(),
                detail: failure.detail.clone(),
            },
        },
        sent_bytes: exchange.hops.iter().map(|hop| hop.sent_bytes).sum(),
        received_bytes: exchange.hops.iter().map(|hop| hop.received_bytes).sum(),
        duration_ms: 0,
        redirect_of: None,
        summary: None,
        account,
    }
}

/// The request log's entry of one of the host's own requests: never a body, and no query.
pub(crate) fn host_entry(
    plugin: &str,
    connector: &str,
    operation: &str,
    url: &url::Url,
    started: u64,
    answer: &Result<network::HostAnswer, network::Failure>,
) -> requests::Entry {
    let _ = plugin;
    requests::Entry {
        at_ms: started,
        plugin: HOST.to_owned(),
        connector: connector.to_owned(),
        operation: operation.to_owned(),
        kind: None,
        method: Some("POST".to_owned()),
        host: url.host_str().map(str::to_owned),
        path: Some(url.path().to_owned()),
        arguments: BTreeMap::new(),
        outcome: match answer {
            Ok(answer) => requests::Outcome::Status {
                status: answer.status,
            },
            Err(failure) => requests::Outcome::Failed {
                failed: Rule::from(failure.rule).as_str().to_owned(),
                detail: failure.detail.clone(),
            },
        },
        sent_bytes: answer.as_ref().map_or(0, |answer| answer.sent_bytes),
        received_bytes: answer.as_ref().map_or(0, |answer| answer.body.len() as u64),
        duration_ms: answer.as_ref().map_or(0, |answer| {
            u64::try_from(answer.duration.as_millis()).unwrap_or(u64::MAX)
        }),
        redirect_of: None,
        summary: None,
        account: None,
    }
}

/// A log entry for a call that sent nothing.
fn entry(
    plugin: &str,
    connector: &str,
    operation: &str,
    arguments: &[(String, ArgValue)],
    answer: &Answer,
) -> requests::Entry {
    let outcome = match answer {
        Answer::Ok(response) => requests::Outcome::Status {
            status: response.status,
        },
        Answer::NotGranted => requests::Outcome::not_granted(),
        Answer::Refused(rule, detail) => requests::Outcome::Refused {
            refused: rule.as_str().to_owned(),
            detail: detail.clone(),
        },
        Answer::Failed(rule, detail) => requests::Outcome::Failed {
            failed: rule.as_str().to_owned(),
            detail: detail.clone(),
        },
        Answer::AccountNotConnected(detail) => requests::Outcome::Refused {
            refused: ACCOUNT_NOT_CONNECTED.to_owned(),
            detail: detail.clone(),
        },
        Answer::SubscriptionRequired => requests::Outcome::Refused {
            refused: SUBSCRIPTION_REQUIRED.to_owned(),
            detail: "the service answered that the account lacks a paid plan".to_owned(),
        },
    };
    requests::Entry {
        at_ms: now_ms(),
        plugin: plugin.to_owned(),
        connector: connector.to_owned(),
        operation: operation.to_owned(),
        kind: None,
        method: None,
        host: None,
        path: None,
        arguments: arguments
            .iter()
            .map(|(name, value)| (name.clone(), text_of(value)))
            .collect(),
        outcome,
        sent_bytes: 0,
        received_bytes: 0,
        duration_ms: 0,
        redirect_of: None,
        summary: None,
        account: None,
    }
}

fn text_of(value: &ArgValue) -> String {
    match value {
        ArgValue::Text(text) => text.clone(),
        ArgValue::Integer(number) => number.to_string(),
        ArgValue::Flag(flag) => flag.to_string(),
    }
}

/// Milliseconds since the Unix epoch.
pub(crate) fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::json;
    use tempfile::TempDir;

    use super::*;
    use crate::connector::Source;
    use crate::grants::{self, ConnectorState};
    use crate::network::{Network, TestNetwork};

    const ORIGIN: &str = "/plugins/net-probe.wasm";
    const DIGEST: &str = "sha256:00";

    /// `local/echo`, with a read `get`, a read `other` the manifest never declares, and a change
    /// `post`; its destination has no seam route, so a call that gets that far fails as
    /// unavailable without touching any network.
    fn echo(id: &str) -> Arc<Connector> {
        let connector = json!({
            "connector_version": 1, "id": id, "version": "1.0.0", "author": "Tests",
            "description": "Echo",
            "destinations": { "api": "https://api.echo.test" },
            "operations": {
                "get": { "kind": "read", "method": "GET", "destination": "api", "path": "/{v}",
                    "parameters": { "v": { "type": "text", "max_length": 4 } },
                    "response": { "format": "json" } },
                "other": { "kind": "read", "method": "GET", "destination": "api", "path": "/other",
                    "response": { "format": "json" } },
                "post": { "kind": "change", "method": "POST", "destination": "api", "path": "/post",
                    "response": { "format": "json" } }
            }
        });
        Arc::new(Connector::parse(connector.to_string().as_bytes(), Source::Local).unwrap())
    }

    fn manifest(connectors: &[(&str, bool)]) -> Manifest {
        let connectors: Vec<serde_json::Value> = connectors
            .iter()
            .map(|(id, required)| json!({ "id": id, "major": 1, "operations": ["get", "post"], "required": required }))
            .collect();
        Manifest::parse(
            json!({
                "manifest_version": 1, "id": "local/net-probe", "version": "0.1.0", "author": "A",
                "permissions": [], "login_providers": [], "connectors": connectors
            })
            .to_string()
            .as_bytes(),
        )
        .unwrap()
    }

    fn states(manifest: &Manifest, loaded: &[(String, Arc<Connector>)]) -> Vec<ConnectorState> {
        loaded
            .iter()
            .map(|(id, connector)| ConnectorState {
                id: id.clone(),
                origin: format!("/dev/{id}.json"),
                version: connector.version().to_string(),
                required: manifest.connector(id).unwrap().required,
                destinations: connector
                    .destinations()
                    .iter()
                    .map(|o| o.serialization())
                    .collect(),
                operations: manifest
                    .connector(id)
                    .unwrap()
                    .operations
                    .iter()
                    .map(|name| (name.clone(), connector.operation_digest(name).unwrap()))
                    .collect(),
                account: None,
            })
            .collect()
    }

    struct Fixture {
        _dir: TempDir,
        grants: Arc<GrantStore>,
        manifest: Manifest,
        loaded: Vec<(String, Arc<Connector>)>,
        log: RequestLog,
        rates: Arc<Rates>,
    }

    fn fixture(connectors: &[(&str, bool)]) -> Fixture {
        let dir = TempDir::new().unwrap();
        let grants = Arc::new(GrantStore::open(dir.path().join("grants.json")));
        let manifest = manifest(connectors);
        let loaded: Vec<(String, Arc<Connector>)> = connectors
            .iter()
            .map(|(id, _)| ((*id).to_owned(), echo(id)))
            .collect();
        let states = states(&manifest, &loaded);
        let expected = grants::approval_digest(DIGEST, &manifest, &states);
        grants
            .approve_connected(&manifest, ORIGIN, DIGEST, &states, &expected)
            .unwrap();
        let log = RequestLog::new(dir.path(), &Limits::default());
        Fixture {
            _dir: dir,
            grants,
            manifest,
            loaded,
            log,
            rates: Arc::default(),
        }
    }

    impl Fixture {
        fn table(&self) -> Option<ConnectorTable> {
            let entry = self.grants.entry(self.manifest.id()).unwrap().unwrap();
            ConnectorTable::build(&self.manifest, &self.loaded, &entry)
        }

        fn calls(&self, phase: Phase, limits: Limits) -> (Calls, RunLog) {
            let table = Arc::new(self.table().expect("a granted connector"));
            let run = RunLog::new(self.manifest.id(), Some(self.log.clone()), &limits);
            let calls = Calls {
                plugin: self.manifest.id().clone(),
                origin: ORIGIN.to_owned(),
                grants: Arc::clone(&self.grants),
                table,
                phase,
                session: (phase == Phase::Update)
                    .then(|| Network::with_seam(TestNetwork::default()).session(&limits)),
                deadline: Instant::now() + limits.update_time_limit,
                update_bytes_left: limits.max_update_response_bytes,
                limits,
                rates: Arc::clone(&self.rates),
                account: None,
            };
            (calls, run)
        }

        fn entries(&self) -> Vec<requests::Entry> {
            self.log.read_last(10_000, None).unwrap()
        }
    }

    fn text(value: &str) -> Vec<(String, ArgValue)> {
        vec![("v".to_owned(), ArgValue::Text(value.to_owned()))]
    }

    fn refused(answer: &Answer) -> Rule {
        match answer {
            Answer::Refused(rule, _) => *rule,
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_connector_missing_from_the_table_is_not_granted_whatever_else_is_wrong() {
        let fixture = fixture(&[("local/echo", true)]);
        for phase in [Phase::Update, Phase::Render] {
            let (mut calls, mut run) = fixture.calls(phase, Limits::default());
            for (connector, operation, arguments) in [
                ("local/undeclared", "get", text("ok")),
                ("local/undeclared", "nothing", text("far too long")),
                ("", "", Vec::new()),
            ] {
                assert_eq!(
                    calls.call(&mut run, connector, operation, arguments),
                    Answer::NotGranted
                );
            }
        }
        assert!(
            fixture
                .entries()
                .iter()
                .all(|entry| entry.outcome == requests::Outcome::not_granted())
        );
    }

    #[test]
    fn a_call_from_render_is_refused_without_reading_the_grant_record() {
        let fixture = fixture(&[("local/echo", true)]);
        let (mut calls, mut run) = fixture.calls(Phase::Render, Limits::default());
        // A damaged record would answer not-granted if render read it
        std::fs::write(fixture.grants.path().unwrap(), "{").unwrap();
        let answer = calls.call(&mut run, "local/echo", "get", text("ok"));
        assert_eq!(refused(&answer), Rule::CalledOutsideUpdate);
    }

    #[test]
    fn each_rule_answers_in_the_order_of_the_data_model() {
        let fixture = fixture(&[("local/echo", true)]);
        let (mut calls, mut run) = fixture.calls(Phase::Update, Limits::default());
        let mut call =
            |operation: &str, arguments| calls.call(&mut run, "local/echo", operation, arguments);
        assert_eq!(
            refused(&call("other", text("ok"))),
            Rule::OperationNotDeclared
        );
        assert_eq!(
            refused(&call("missing", text("ok"))),
            Rule::OperationNotDeclared
        );
        assert_eq!(
            refused(&call("post", Vec::new())),
            Rule::ChangeNeedsUserAction
        );
        assert_eq!(
            refused(&call("get", text("toolong"))),
            Rule::ValueOutsideType
        );
        assert_eq!(refused(&call("get", Vec::new())), Rule::ValueOutsideType);
        let many: Vec<(String, ArgValue)> = (0..17)
            .map(|i| (format!("a{i}"), ArgValue::Flag(true)))
            .collect();
        assert_eq!(refused(&call("get", many)), Rule::ValueOutsideType);
        // A valid call goes as far as the network, where no route leads anywhere
        assert!(matches!(
            call("get", text("ok")),
            Answer::Failed(Rule::Unavailable, _)
        ));
    }

    #[test]
    fn past_the_runs_call_limit_calls_are_refused_and_counted_in_one_summary_entry() {
        let fixture = fixture(&[("local/echo", true)]);
        let (mut calls, mut run) = fixture.calls(Phase::Update, Limits::default());
        for _ in 0..20 {
            calls.call(&mut run, "local/echo", "get", text("toolong"));
        }
        let answer = calls.call(&mut run, "local/echo", "get", text("ok"));
        assert_eq!(refused(&answer), Rule::OverLimit);
        calls.call(&mut run, "local/undeclared", "get", Vec::new());
        assert_eq!(
            fixture.entries().len(),
            20,
            "past the limit, nothing more is logged"
        );
        run.finish();
        let entries = fixture.entries();
        assert_eq!(entries.len(), 21);
        assert_eq!(
            entries.last().unwrap().summary,
            Some(Summary {
                calls: 2,
                not_granted: 1,
                refused: 1
            })
        );
        assert_eq!(run.counts.calls, 22);
        // A second finish writes nothing more
        run.finish();
        assert_eq!(fixture.entries().len(), 21);
    }

    #[test]
    fn the_rates_per_widget_and_per_host_are_shared_by_one_runtime() {
        let limits = Limits {
            max_calls_per_run: 1000,
            calls_per_minute_per_widget: 3,
            calls_per_minute_per_host: 5,
            ..Limits::default()
        };
        let fixture = fixture(&[("local/echo", true)]);
        let (mut calls, mut run) = fixture.calls(Phase::Update, limits.clone());
        for _ in 0..3 {
            assert!(matches!(
                calls.call(&mut run, "local/echo", "get", text("ok")),
                Answer::Failed(..)
            ));
        }
        let Answer::Refused(Rule::OverLimit, why) =
            calls.call(&mut run, "local/echo", "get", text("ok"))
        else {
            panic!("the widget's rate should refuse")
        };
        assert!(why.contains("this widget"), "{why}");

        // Another widget, same host, same runtime: the host's bucket has 2 left. It has no
        // approval, so a call the rates let through stops at step 7, after them
        let mut other = fixture.calls(Phase::Update, limits).0;
        other.plugin = PluginId::parse("local/other").unwrap();
        let mut other_run = RunLog::new(&other.plugin.clone(), None, &other.limits.clone());
        for _ in 0..2 {
            assert_eq!(
                other.call(&mut other_run, "local/echo", "get", text("ok")),
                Answer::NotGranted
            );
        }
        let Answer::Refused(Rule::OverLimit, why) =
            other.call(&mut other_run, "local/echo", "get", text("ok"))
        else {
            panic!("the host's rate should refuse")
        };
        assert!(why.contains("api.echo.test"), "{why}");
    }

    #[test]
    fn a_connector_no_longer_granted_answers_not_granted_and_sends_nothing() {
        let fixture = fixture(&[("local/echo", true), ("local/weather", false)]);
        fixture
            .grants
            .set_connector(&fixture.manifest, ORIGIN, "local/weather", true)
            .unwrap();
        let (mut calls, mut run) = fixture.calls(Phase::Update, Limits::default());
        // Turned off from another store, as another process would
        let other = GrantStore::open(fixture.grants.path().unwrap());
        other
            .set_connector(&fixture.manifest, ORIGIN, "local/weather", false)
            .unwrap();
        assert_eq!(
            calls.call(&mut run, "local/weather", "get", text("ok")),
            Answer::NotGranted
        );
        other.remove(fixture.manifest.id()).unwrap();
        assert_eq!(
            calls.call(&mut run, "local/echo", "get", text("ok")),
            Answer::NotGranted
        );
        let entries = fixture.entries();
        assert!(
            entries.iter().all(|entry| entry.host.is_none()),
            "nothing was built or sent"
        );
    }

    #[test]
    fn the_table_holds_exactly_the_granted_connectors_or_is_none() {
        let fixture = fixture(&[("local/echo", true), ("local/weather", false)]);
        assert_eq!(fixture.table().unwrap().keys(), ["local/echo"]);
        fixture
            .grants
            .set_connector(&fixture.manifest, ORIGIN, "local/weather", true)
            .unwrap();
        assert_eq!(
            fixture.table().unwrap().keys(),
            ["local/echo", "local/weather"]
        );

        let optional = fixture_optional_only();
        assert!(
            optional.table().is_none(),
            "none granted: the stand-in is linked"
        );
    }

    fn fixture_optional_only() -> Fixture {
        fixture(&[("local/weather", false)])
    }

    #[test]
    fn the_stand_in_counts_and_logs_every_call_and_answers_not_granted() {
        let dir = TempDir::new().unwrap();
        let log = RequestLog::new(Path::new(dir.path()), &Limits::default());
        let plugin = PluginId::parse("local/net-probe").unwrap();
        let mut run = RunLog::new(&plugin, Some(log.clone()), &Limits::default());
        for _ in 0..25 {
            assert_eq!(run.not_granted("local/weather", "get"), Answer::NotGranted);
        }
        run.finish();
        let entries = log.read_last(100, None).unwrap();
        assert_eq!(entries.len(), 21);
        assert_eq!(
            entries
                .last()
                .unwrap()
                .summary
                .as_ref()
                .unwrap()
                .not_granted,
            5
        );
    }

    // Feature 005: the account table and the account steps that answer before anything is sent
    // (`specs/005-plugin-credentials/data-model.md`, An account call)

    use crate::accounts::{AccountKind, Client, Connected, NewAccount};
    use crate::connector::{Access, Level};
    use crate::grants::AccountState;
    use crate::vault::{Backend, Vault};

    const NOTES_ORIGIN: &str = "/dev/local/notes.json";

    /// `local/notes`: `echo`'s operations on `api.echo.test`, with an account credential.
    fn notes(required: bool) -> Arc<Connector> {
        let connector = json!({
            "connector_version": 1, "id": "local/notes", "version": "1.0.0", "author": "Tests",
            "description": "Notes",
            "destinations": { "api": "https://api.echo.test" },
            "credential": {
                "kind": "account", "required": required, "destinations": ["api"],
                "header": "Authorization", "scheme": "Bearer",
                "account": {
                    "flow": "authorization-code", "client": "notes",
                    "authorize": "https://api.echo.test/authorize",
                    "token": "https://api.echo.test/token",
                    "access": [{ "name": "notes", "level": "read" }]
                }
            },
            "operations": {
                "get": { "kind": "read", "method": "GET", "destination": "api", "path": "/{v}",
                    "parameters": { "v": { "type": "text", "max_length": 4 } },
                    "response": { "format": "json" } },
                "post": { "kind": "change", "method": "POST", "destination": "api", "path": "/post",
                    "response": { "format": "json" } }
            }
        });
        Arc::new(Connector::parse(connector.to_string().as_bytes(), Source::Local).unwrap())
    }

    struct AccountFixture {
        dir: TempDir,
        grants: Arc<GrantStore>,
        manifest: Manifest,
        loaded: Vec<(String, String, Arc<Connector>)>,
        vault: Arc<Vault>,
    }

    /// A widget declaring `local/notes` with its account and `local/echo` without one, approved.
    fn account_fixture(required: bool) -> AccountFixture {
        let dir = TempDir::new().unwrap();
        let grants = Arc::new(GrantStore::open(dir.path().join("grants.json")));
        let manifest = Manifest::parse(
            json!({
                "manifest_version": 1, "id": "local/account-probe", "version": "0.1.0", "author": "A",
                "permissions": [],
                "connectors": [
                    { "id": "local/notes", "major": 1, "operations": ["get"], "required": true, "account": true },
                    { "id": "local/echo", "major": 1, "operations": ["get"], "required": true }
                ]
            })
            .to_string()
            .as_bytes(),
        )
        .unwrap();
        let loaded = vec![
            (
                "local/notes".to_owned(),
                NOTES_ORIGIN.to_owned(),
                notes(required),
            ),
            (
                "local/echo".to_owned(),
                "/dev/local/echo.json".to_owned(),
                echo("local/echo"),
            ),
        ];
        let states: Vec<ConnectorState> = loaded
            .iter()
            .map(|(id, origin, connector)| ConnectorState {
                id: id.clone(),
                origin: origin.clone(),
                version: "1.0.0".to_owned(),
                required: true,
                destinations: vec!["https://api.echo.test".to_owned()],
                operations: BTreeMap::from([(
                    "get".to_owned(),
                    connector.operation_digest("get").unwrap(),
                )]),
                account: connector.credential().map(|credential| AccountState {
                    uses: manifest.connector(id).unwrap().account,
                    required: credential.required(),
                    credential: credential.digest(),
                    access: credential.access().to_vec(),
                }),
            })
            .collect();
        let expected = grants::approval_digest(DIGEST, &manifest, &states);
        grants
            .approve_connected(&manifest, ORIGIN, DIGEST, &states, &expected)
            .unwrap();
        AccountFixture {
            dir,
            grants,
            manifest,
            loaded,
            vault: Arc::new(Vault::new(Backend::mock(), 2560)),
        }
    }

    impl AccountFixture {
        fn store(&self) -> AccountStore {
            AccountStore::open(self.dir.path())
        }

        /// Connects an account to `local/notes`, asking for `access`, with its token in the vault.
        fn connect(&self, access: Vec<Access>) -> Account {
            let vault_id = self.store().vault_id().unwrap();
            let new = NewAccount {
                connector: "local/notes".to_owned(),
                origin: NOTES_ORIGIN.to_owned(),
                kind: AccountKind::Account,
                flow: Some("authorization-code".to_owned()),
                client: Some(Client {
                    client_id: "dev".to_owned(),
                    from: "option".to_owned(),
                }),
                name: "ada".to_owned(),
                service_id: Some("1".to_owned()),
                access_asked: access,
                access_reported: Vec::new(),
                now_ms: now_ms(),
                expires_at_ms: None,
            };
            let vault = Arc::clone(&self.vault);
            let user = entry_user(&vault_id, "local/notes", NOTES_ORIGIN);
            let connected = self
                .store()
                .connect(new, |_, _| {
                    vault
                        .write(
                            &user,
                            &Stored::Account {
                                access: Secret::new("t0ken".to_owned()),
                                refresh: None,
                            },
                        )
                        .within(std::time::Duration::from_secs(5))
                        .map_err(|error| {
                            crate::RuntimeError::new(
                                crate::ErrorKind::KeychainUnavailable,
                                error.to_string(),
                            )
                        })
                })
                .unwrap();
            match connected {
                Connected::New(account) | Connected::Again(account) => account,
            }
        }

        fn read_notes() -> Vec<Access> {
            vec![Access {
                name: "notes".to_owned(),
                level: Level::Read,
            }]
        }

        /// The account table, with the accounts connected now: the widget linked now.
        fn account_table(&self) -> Option<ConnectorTable> {
            let entry = self.grants.entry(self.manifest.id()).unwrap().unwrap();
            let store = self.store();
            ConnectorTable::build_accounts(&self.manifest, &self.loaded, &entry, |id, origin| {
                store.get(id, origin).ok().flatten()
            })
        }

        fn calls(&self) -> (Calls, RunLog) {
            let limits = Limits::default();
            let table = Arc::new(self.account_table().expect("an account connector"));
            let run = RunLog::new(self.manifest.id(), None, &limits);
            let calls = Calls {
                plugin: self.manifest.id().clone(),
                origin: ORIGIN.to_owned(),
                grants: Arc::clone(&self.grants),
                table,
                phase: Phase::Update,
                session: Some(Network::with_seam(TestNetwork::default()).session(&limits)),
                deadline: Instant::now() + limits.update_time_limit,
                update_bytes_left: limits.max_update_response_bytes,
                limits,
                rates: Arc::default(),
                account: Some(AccountAccess {
                    store: self.store(),
                    vault: Arc::clone(&self.vault),
                    vault_id: self.store().existing_vault_id().unwrap(),
                    secrets: HashMap::new(),
                }),
            };
            (calls, run)
        }
    }

    fn account_not_connected(answer: &Answer) -> &str {
        match answer {
            Answer::AccountNotConnected(detail) => detail,
            other => panic!("expected account-not-connected, got {other:?}"),
        }
    }

    #[test]
    fn a_connector_is_in_the_account_table_or_the_other_never_both() {
        let fixture = account_fixture(false);
        let entry = fixture
            .grants
            .entry(fixture.manifest.id())
            .unwrap()
            .unwrap();
        let plain: Vec<(String, Arc<Connector>)> = fixture
            .loaded
            .iter()
            .map(|(id, _, connector)| (id.clone(), Arc::clone(connector)))
            .collect();
        assert_eq!(
            ConnectorTable::build(&fixture.manifest, &plain, &entry)
                .unwrap()
                .keys(),
            ["local/echo"]
        );
        assert_eq!(fixture.account_table().unwrap().keys(), ["local/notes"]);
        // wrong-interface: through accounts, a connector declared without an account misses
        let (mut calls, mut run) = fixture.calls();
        assert_eq!(
            calls.call(&mut run, "local/echo", "get", text("a")),
            Answer::NotGranted
        );
    }

    #[test]
    fn without_an_account_a_required_credential_answers_not_connected_and_an_optional_one_sends_without()
     {
        let fixture = account_fixture(true);
        let (mut calls, mut run) = fixture.calls();
        let answer = calls.call(&mut run, "local/notes", "get", text("a"));
        assert!(
            account_not_connected(&answer).contains("required"),
            "{answer:?}"
        );

        let fixture = account_fixture(false);
        let (mut calls, mut run) = fixture.calls();
        // No seam route: the request is attempted, without a credential, and cannot connect
        match calls.call(&mut run, "local/notes", "get", text("a")) {
            Answer::Failed(Rule::Unavailable, _) => {}
            other => panic!("expected an attempt without a credential, got {other:?}"),
        }
    }

    /// connect-mid-run: an account connected after the widget was linked is not used by this run.
    #[test]
    fn an_account_connected_after_linking_waits_for_the_next_load() {
        let fixture = account_fixture(true);
        let (mut calls, mut run) = fixture.calls();
        fixture.connect(AccountFixture::read_notes());
        let answer = calls.call(&mut run, "local/notes", "get", text("a"));
        assert!(
            account_not_connected(&answer).contains("no account"),
            "{answer:?}"
        );
        // Linked again, it is used
        let (mut calls, mut run) = fixture.calls();
        assert!(matches!(
            calls.call(&mut run, "local/notes", "get", text("a")),
            Answer::Failed(Rule::Unavailable, _)
        ));
    }

    #[test]
    fn an_account_marked_or_disconnected_since_linking_answers_not_connected() {
        let fixture = account_fixture(true);
        let account = fixture.connect(AccountFixture::read_notes());
        let (mut calls, mut run) = fixture.calls();
        fixture
            .store()
            .mark_needs_connecting(
                "local/notes",
                NOTES_ORIGIN,
                &account.account_id,
                Reason::Refused,
                1,
            )
            .unwrap();
        let answer = calls.call(&mut run, "local/notes", "get", text("a"));
        assert!(
            account_not_connected(&answer).contains("connected again"),
            "{answer:?}"
        );

        let fixture = account_fixture(true);
        fixture.connect(AccountFixture::read_notes());
        let (mut calls, mut run) = fixture.calls();
        fixture
            .store()
            .disconnect("local/notes", NOTES_ORIGIN, |_, _| Ok(()))
            .unwrap();
        let answer = calls.call(&mut run, "local/notes", "get", text("a"));
        assert!(
            account_not_connected(&answer).contains("disconnected"),
            "{answer:?}"
        );
    }

    #[test]
    fn less_access_than_the_connector_asks_for_marks_the_account() {
        let fixture = account_fixture(true);
        fixture.connect(Vec::new());
        let (mut calls, mut run) = fixture.calls();
        let answer = calls.call(&mut run, "local/notes", "get", text("a"));
        assert!(
            account_not_connected(&answer).contains("notes read"),
            "{answer:?}"
        );
        let account = fixture
            .store()
            .get("local/notes", NOTES_ORIGIN)
            .unwrap()
            .unwrap();
        assert_eq!(account.needs_connecting.unwrap().reason, Reason::LessAccess);
    }

    #[test]
    fn a_credential_missing_from_the_keychain_marks_the_account() {
        let fixture = account_fixture(true);
        let account = fixture.connect(AccountFixture::read_notes());
        let user = entry_user(
            &fixture.store().vault_id().unwrap(),
            "local/notes",
            NOTES_ORIGIN,
        );
        fixture
            .vault
            .delete(&user)
            .within(std::time::Duration::from_secs(5))
            .unwrap();
        let (mut calls, mut run) = fixture.calls();
        let answer = calls.call(&mut run, "local/notes", "get", text("a"));
        assert!(
            account_not_connected(&answer).contains("keychain"),
            "{answer:?}"
        );
        let marked = fixture
            .store()
            .get("local/notes", NOTES_ORIGIN)
            .unwrap()
            .unwrap();
        assert_eq!(marked.account_id, account.account_id);
        assert_eq!(
            marked.needs_connecting.unwrap().reason,
            Reason::MissingFromKeychain
        );
    }

    /// keychain_locked_call: a keychain that refuses access fails the call as unavailable,
    /// naming the keychain; nothing is sent, with or without a credential.
    #[test]
    fn a_locked_keychain_fails_the_call_and_sends_nothing() {
        let fixture = account_fixture(false);
        fixture.connect(AccountFixture::read_notes());
        let user = entry_user(
            &fixture.store().vault_id().unwrap(),
            "local/notes",
            NOTES_ORIGIN,
        );
        fixture
            .vault
            .fail_next(&user, crate::vault::Failure::NoStorageAccess);
        let (mut calls, mut run) = fixture.calls();
        match calls.call(&mut run, "local/notes", "get", text("a")) {
            Answer::Failed(Rule::Unavailable, detail) => {
                assert!(detail.contains("keychain"), "{detail}")
            }
            other => panic!("expected the keychain to be unavailable, got {other:?}"),
        }
    }
}
