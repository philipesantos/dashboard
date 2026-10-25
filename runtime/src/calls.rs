//! Connector calls: the steps of `specs/003-plugin-connectors/data-model.md` (A connector call),
//! in order; the first rule that applies answers, and every call is logged (research R10, R12).
//!
//! What a plugin can reach is decided when it is linked: its [`ConnectorTable`] holds only the
//! connectors that were granted then (Principle II, constitution v2.3.0). A call first looks
//! its connector up there, and every later check can only take that connector away: nothing at
//! call time adds one. A plugin with no connector granted is linked to the stand-in instead,
//! which holds no table, no network and no grants, and answers "not granted" to every call.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::connector::{ArgValue, Connector, Kind};
use crate::grants::{Entry, GrantStore};
use crate::limits::Limits;
use crate::manifest::{Manifest, PluginId};
use crate::network::{self, FailureRule, Session};
use crate::requests::{self, RequestLog, Summary};

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
                Some((
                    id.clone(),
                    Executor {
                        connector: Arc::clone(connector),
                        declared: request.operations.clone(),
                    },
                ))
            })
            .collect();
        (!executors.is_empty()).then_some(ConnectorTable { executors })
    }

    /// The connectors in the table, by id: for the white-box case `connector-table`.
    #[cfg(any(test, feature = "test-network"))]
    pub(crate) fn keys(&self) -> Vec<String> {
        self.executors.keys().cloned().collect()
    }
}

/// How a run's connector calls went, for the update report.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CallCounts {
    pub calls: u64,
    pub not_granted: u64,
    pub refused: u64,
    pub failed: u64,
    /// Bytes of response bodies received, redirects included.
    pub received_bytes: u64,
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
    fn record(&mut self, past: bool, answer: &Answer, entries: Vec<requests::Entry>) {
        self.counts.calls += 1;
        match answer {
            Answer::Ok(_) => {}
            Answer::NotGranted => self.counts.not_granted += 1,
            Answer::Refused(..) => self.counts.refused += 1,
            Answer::Failed(..) => self.counts.failed += 1,
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
            });
        }
    }

    /// The stand-in's whole answer: counted, logged, not granted, nothing reached.
    pub(crate) fn not_granted(&mut self, connector: &str, operation: &str) -> Answer {
        let past = self.count();
        let answer = Answer::NotGranted;
        let entry = entry(&self.plugin, connector, operation, &[], &answer);
        self.record(past, &answer, vec![entry]);
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

/// What the real `connectors` holds for one run.
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
        run.record(past, &answer, entries);
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
        // Step 8: build the request within its size limits
        let request = match network::build_request(&connector, operation, &checked, &self.limits) {
            Ok(request) => request,
            Err(network::Refusal(why)) => {
                return logged(Answer::Refused(Rule::OverLimit, why), kind);
            }
        };
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
        let exchange = session.send(
            &connector,
            operation,
            request,
            self.deadline,
            &mut self.update_bytes_left,
        );
        let mut answer = match exchange.result {
            Ok(response) => Answer::Ok(response),
            Err(failure) => Answer::Failed(failure.rule.into(), failure.detail),
        };
        // Step 12: still granted when the response arrives, or it is dropped
        if matches!(answer, Answer::Ok(_)) && !self.still_granted(connector_id) {
            answer = Answer::NotGranted;
        }
        let sent: Vec<(String, ArgValue)> = checked.into_iter().collect();
        let count = exchange.hops.len();
        let entries = exchange
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
                    (Ok(status), _, _) => requests::Outcome::Status { status: *status },
                };
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
                }
            })
            .collect();
        (answer, entries)
    }

    /// Steps 7 and 12: the grant record, read again under its lock, still grants the
    /// connector. A record that cannot be read grants nothing.
    fn still_granted(&self, connector: &str) -> bool {
        self.grants
            .connector_granted(&self.plugin, &self.origin, connector)
            .unwrap_or(false)
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
}
