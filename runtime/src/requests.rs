//! The request log: every connector call, allowed or refused, and every redirect the host
//! followed (`specs/003-plugin-connectors/data-model.md`, Request log entry; FR-009, R14).
//!
//! One JSON object per line of `<data>/requests.jsonl`, appended under a lock file, so two
//! processes lose nothing. Past its size it is renamed to `requests.1.jsonl`, replacing the
//! older one: the oldest entries go first, and recording never stops. A plugin can neither read
//! nor change it, and cannot push earlier entries out: past a run's call limit, its calls are
//! counted in one summary entry. Every text a plugin or a connector wrote is cut here and
//! escaped by whoever shows it.

use std::collections::BTreeMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::limits::Limits;

const FILE: &str = "requests.jsonl";
const PREVIOUS: &str = "requests.1.jsonl";
const LOCK: &str = "requests.jsonl.lock";
/// Follows a value that was cut.
const TRUNCATED: &str = " [truncated]";

/// One line of the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// When the call started, in milliseconds since the Unix epoch.
    pub at_ms: u64,
    pub plugin: String,
    pub connector: String,
    pub operation: String,
    /// `read` or `change`, when the operation is known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    /// The request as built; absent when the call was refused before building it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    /// Each argument as sent, its value cut to a maximum length.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub arguments: BTreeMap<String, String>,
    pub outcome: Outcome,
    #[serde(default)]
    pub sent_bytes: u64,
    #[serde(default)]
    pub received_bytes: u64,
    #[serde(default)]
    pub duration_ms: u64,
    /// For a redirect the host followed: the number of the redirect in its call, from 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub redirect_of: Option<u32>,
    /// Only in the one entry written past a run's call limit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<Summary>,
    /// The account a request acted as: its name, or `key` (feature 005); never the credential.
    /// Absent when the request carried none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub account: Option<String>,
}

/// How a call ended: `{ "status": 200 }`, `{ "not-granted": true }`,
/// `{ "refused": "<rule>", "detail": "…" }` or `{ "failed": "<rule>", "detail": "…" }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged, deny_unknown_fields)]
pub enum Outcome {
    Status {
        status: u16,
    },
    NotGranted {
        #[serde(rename = "not-granted")]
        not_granted: bool,
    },
    Refused {
        refused: String,
        detail: String,
    },
    Failed {
        failed: String,
        detail: String,
    },
}

impl Outcome {
    pub(crate) fn not_granted() -> Outcome {
        Outcome::NotGranted { not_granted: true }
    }
}

/// The calls of a run past its limit, by answer. Past the limit every call steps 1–5 let
/// through is refused as `over-limit`, so none is sent.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    /// Calls past the limit.
    pub calls: u64,
    #[serde(rename = "not-granted")]
    pub not_granted: u64,
    pub refused: u64,
}

/// The request log of one data directory.
#[derive(Debug, Clone)]
pub(crate) struct RequestLog {
    dir: PathBuf,
    max_bytes: usize,
    max_value_chars: usize,
    max_name_chars: usize,
    max_arguments: usize,
    max_argument_name_chars: usize,
}

impl RequestLog {
    pub(crate) fn new(data_dir: &Path, limits: &Limits) -> RequestLog {
        RequestLog {
            dir: data_dir.to_owned(),
            max_bytes: limits.max_request_log_bytes,
            max_value_chars: limits.max_logged_value_chars,
            max_name_chars: limits.max_logged_name_chars,
            max_arguments: limits.max_arguments,
            max_argument_name_chars: limits.max_argument_name_chars,
        }
    }

    /// Appends `entry`, after cutting every text a plugin chose to its limit, rotating the file
    /// first when it would pass its size.
    pub(crate) fn append(&self, mut entry: Entry) -> io::Result<()> {
        entry.plugin = cut(&entry.plugin, self.max_name_chars);
        entry.connector = cut(&entry.connector, self.max_name_chars);
        entry.operation = cut(&entry.operation, self.max_name_chars);
        entry.arguments = entry
            .arguments
            .into_iter()
            .take(self.max_arguments)
            .map(|(name, value)| {
                (
                    cut(&name, self.max_argument_name_chars),
                    cut(&value, self.max_value_chars),
                )
            })
            .collect();
        let mut line = serde_json::to_vec(&entry).expect("an entry always serializes");
        line.push(b'\n');

        fs::create_dir_all(&self.dir)?;
        let lock = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.dir.join(LOCK))?;
        lock.lock()?;
        let path = self.dir.join(FILE);
        let size = fs::metadata(&path).map_or(0, |metadata| metadata.len()) as usize;
        if size > 0 && size + line.len() > self.max_bytes {
            fs::rename(&path, self.dir.join(PREVIOUS))?;
        }
        let mut file = OpenOptions::new().create(true).append(true).open(&path)?;
        file.write_all(&line)?;
        file.sync_data()?;
        drop(lock);
        Ok(())
    }

    /// The last `n` entries, oldest first, of `plugin` only when given. Lines that are not
    /// entries, such as one cut by a crash, are skipped.
    pub(crate) fn read_last(&self, n: usize, plugin: Option<&str>) -> io::Result<Vec<Entry>> {
        let lock = match OpenOptions::new()
            .read(true)
            .write(true)
            .create(false)
            .open(self.dir.join(LOCK))
        {
            Ok(lock) => Some(lock),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        };
        if let Some(lock) = &lock {
            lock.lock_shared()?;
        }
        let mut entries = Vec::new();
        for name in [PREVIOUS, FILE] {
            let file = match File::open(self.dir.join(name)) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
                Err(error) => return Err(error),
            };
            for line in BufReader::new(file).lines() {
                if let Ok(entry) = serde_json::from_str::<Entry>(&line?)
                    && plugin.is_none_or(|plugin| entry.plugin == plugin)
                {
                    entries.push(entry);
                }
            }
        }
        drop(lock);
        let skip = entries.len().saturating_sub(n);
        Ok(entries.split_off(skip))
    }
}

/// `text` cut to `max` characters, followed by ` [truncated]` when cut.
fn cut(text: &str, max: usize) -> String {
    match text.char_indices().nth(max) {
        None => text.to_owned(),
        Some((end, _)) => format!("{}{TRUNCATED}", &text[..end]),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;

    use serde_json::{Value, json};
    use tempfile::TempDir;

    use super::*;

    fn entry(plugin: &str, at_ms: u64) -> Entry {
        Entry {
            at_ms,
            plugin: plugin.to_owned(),
            connector: "catalog/github".to_owned(),
            operation: "list-releases".to_owned(),
            kind: Some("read".to_owned()),
            method: Some("GET".to_owned()),
            host: Some("api.github.com".to_owned()),
            path: Some("/repos/rust-lang/rust/releases?per_page=5".to_owned()),
            arguments: BTreeMap::from([
                ("owner".to_owned(), "rust-lang".to_owned()),
                ("repo".to_owned(), "rust".to_owned()),
            ]),
            outcome: Outcome::Status { status: 200 },
            sent_bytes: 120,
            received_bytes: 3412,
            duration_ms: 184,
            redirect_of: None,
            summary: None,
            account: None,
        }
    }

    fn log(dir: &TempDir) -> RequestLog {
        RequestLog::new(dir.path(), &Limits::default())
    }

    #[test]
    fn each_entry_is_one_json_object_per_line_with_the_fields_of_the_data_model() {
        let dir = TempDir::new().unwrap();
        log(&dir).append(entry("local/a", 1)).unwrap();
        let text = fs::read_to_string(dir.path().join("requests.jsonl")).unwrap();
        assert_eq!(text.lines().count(), 1);
        let line: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
        assert_eq!(
            line,
            json!({
                "at_ms": 1,
                "plugin": "local/a",
                "connector": "catalog/github",
                "operation": "list-releases",
                "kind": "read",
                "method": "GET",
                "host": "api.github.com",
                "path": "/repos/rust-lang/rust/releases?per_page=5",
                "arguments": { "owner": "rust-lang", "repo": "rust" },
                "outcome": { "status": 200 },
                "sent_bytes": 120,
                "received_bytes": 3412,
                "duration_ms": 184
            })
        );
    }

    #[test]
    fn every_outcome_form_round_trips() {
        let dir = TempDir::new().unwrap();
        let log = log(&dir);
        let outcomes = [
            Outcome::Status { status: 404 },
            Outcome::not_granted(),
            Outcome::Refused {
                refused: "value-outside-type".to_owned(),
                detail: "`owner`: missing".to_owned(),
            },
            Outcome::Failed {
                failed: "unavailable".to_owned(),
                detail: "api.github.com was not found".to_owned(),
            },
        ];
        for outcome in &outcomes {
            log.append(Entry {
                outcome: outcome.clone(),
                ..entry("local/a", 1)
            })
            .unwrap();
        }
        let read: Vec<Outcome> = log
            .read_last(10, None)
            .unwrap()
            .into_iter()
            .map(|entry| entry.outcome)
            .collect();
        assert_eq!(read, outcomes);
        let text = fs::read_to_string(dir.path().join("requests.jsonl")).unwrap();
        assert!(text.contains(r#""outcome":{"not-granted":true}"#), "{text}");
    }

    #[test]
    fn texts_a_plugin_chose_are_cut_before_they_are_written() {
        let dir = TempDir::new().unwrap();
        let log = log(&dir);
        let arguments: BTreeMap<String, String> = (0..20)
            .map(|i| (format!("{i:02}{}", "n".repeat(40)), "v".repeat(300)))
            .collect();
        log.append(Entry {
            connector: "c".repeat(200),
            operation: "o".repeat(161),
            arguments,
            ..entry("local/a", 1)
        })
        .unwrap();
        let entry = log.read_last(1, None).unwrap().remove(0);
        assert_eq!(entry.connector, format!("{} [truncated]", "c".repeat(160)));
        assert_eq!(entry.operation, format!("{} [truncated]", "o".repeat(160)));
        assert_eq!(entry.arguments.len(), 16);
        for (name, value) in &entry.arguments {
            assert_eq!(name.chars().count(), 32 + " [truncated]".len());
            assert_eq!(value, &format!("{} [truncated]", "v".repeat(256)));
        }
        // Under the limit, nothing is marked
        assert_eq!(cut("abc", 3), "abc");
        assert_eq!(cut("ééé", 2), "éé [truncated]");
    }

    #[test]
    fn a_summary_entry_counts_the_calls_past_the_limit() {
        let dir = TempDir::new().unwrap();
        let log = log(&dir);
        log.append(Entry {
            outcome: Outcome::Refused {
                refused: "over-limit".to_owned(),
                detail: "past 20 calls".to_owned(),
            },
            summary: Some(Summary {
                calls: 9980,
                not_granted: 5000,
                refused: 4980,
            }),
            ..entry("local/a", 1)
        })
        .unwrap();
        let text = fs::read_to_string(dir.path().join("requests.jsonl")).unwrap();
        assert!(
            text.contains(r#""summary":{"calls":9980,"not-granted":5000,"refused":4980}"#),
            "{text}"
        );
    }

    #[test]
    fn past_its_size_the_log_rotates_keeping_the_newest() {
        let dir = TempDir::new().unwrap();
        let log = RequestLog::new(
            dir.path(),
            &Limits {
                max_request_log_bytes: 2048,
                ..Limits::default()
            },
        );
        for at in 0..100 {
            log.append(entry("local/a", at)).unwrap();
        }
        let current = fs::metadata(dir.path().join("requests.jsonl"))
            .unwrap()
            .len();
        let previous = fs::metadata(dir.path().join("requests.1.jsonl"))
            .unwrap()
            .len();
        assert!(current <= 2048 && previous <= 2048, "{current} {previous}");
        let entries = log.read_last(1000, None).unwrap();
        assert_eq!(entries.last().unwrap().at_ms, 99);
        // Oldest first, in order
        assert!(entries.windows(2).all(|pair| pair[0].at_ms < pair[1].at_ms));
        assert!(entries.first().unwrap().at_ms > 0, "the oldest went first");
    }

    #[test]
    fn read_last_returns_the_newest_n_oldest_first_and_filters_by_plugin() {
        let dir = TempDir::new().unwrap();
        let log = log(&dir);
        for at in 0..10 {
            let plugin = if at % 2 == 0 { "local/a" } else { "local/b" };
            log.append(entry(plugin, at)).unwrap();
        }
        let ats = |entries: Vec<Entry>| entries.iter().map(|entry| entry.at_ms).collect::<Vec<_>>();
        assert_eq!(ats(log.read_last(3, None).unwrap()), [7, 8, 9]);
        assert_eq!(ats(log.read_last(2, Some("local/a")).unwrap()), [6, 8]);
        assert_eq!(
            ats(log.read_last(50, Some("local/none")).unwrap()),
            Vec::<u64>::new()
        );
        // A line cut by a crash is skipped
        let mut file = OpenOptions::new()
            .append(true)
            .open(dir.path().join("requests.jsonl"))
            .unwrap();
        file.write_all(b"{\"at_ms\":").unwrap();
        assert_eq!(ats(log.read_last(1, None).unwrap()), [9]);
        let empty = TempDir::new().unwrap();
        assert!(self::log(&empty).read_last(5, None).unwrap().is_empty());
    }

    #[test]
    fn appends_from_two_threads_lose_nothing() {
        let dir = TempDir::new().unwrap();
        let log = Arc::new(log(&dir));
        let threads: Vec<_> = (0..2)
            .map(|thread| {
                let log = Arc::clone(&log);
                thread::spawn(move || {
                    for at in 0..200 {
                        log.append(entry(&format!("local/t{thread}"), at)).unwrap();
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(log.read_last(1000, Some("local/t0")).unwrap().len(), 200);
        assert_eq!(log.read_last(1000, Some("local/t1")).unwrap().len(), 200);
    }
}
