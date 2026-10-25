//! Kept data: the last data each widget's update returned, with when it was fetched and how the
//! last update went (`specs/003-plugin-connectors/data-model.md`, Kept data; research R14).
//!
//! Host files in the development data directory, which no plugin can reach: one per package
//! origin, `widget-data/<hex SHA-256 of "<id>\n<origin>">.json`, written atomically. A file
//! that no longer matches the package or its approval is discarded, so no version or approval
//! ever receives data another one produced (clarification 4).

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::NamedTempFile;

/// The directory of kept data inside the data directory.
const DIR: &str = "widget-data";

/// What a kept data file belongs to: a package from an origin, under one approval.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Key {
    pub(crate) id: String,
    pub(crate) origin: String,
    /// The package digest.
    pub(crate) digest: String,
    /// The approval digest of what was approved (`data-model.md`, Approval digest).
    pub(crate) approval: String,
    /// The connectors that are on, sorted.
    pub(crate) connectors_on: Vec<String>,
    /// For each connector the widget uses with an account, the account id it acts as, or `None`
    /// without one (`specs/005-plugin-credentials/data-model.md`, Kept data).
    pub(crate) accounts: BTreeMap<String, Option<String>>,
}

/// One kept data file, as stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Kept {
    pub(crate) id: String,
    pub(crate) origin: String,
    pub(crate) digest: String,
    pub(crate) approval: KeptApproval,
    /// When the last successful update finished, in milliseconds since the Unix epoch.
    pub(crate) fetched_at_ms: Option<u64>,
    /// The JSON document the last successful update returned.
    pub(crate) data: Option<Value>,
    pub(crate) last_update: Option<LastUpdate>,
    /// For each connector the widget uses with an account, the account id the data was fetched
    /// with, or `None` without one. Absent for widgets that use no account.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub(crate) accounts: BTreeMap<String, Option<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct KeptApproval {
    pub(crate) digest: String,
    pub(crate) connectors_on: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LastUpdate {
    pub(crate) at_ms: u64,
    pub(crate) outcome: Outcome,
    /// `<kind>: <detail>`, when the update failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Outcome {
    Ok,
    Failed,
}

/// What reading a widget's kept data found.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Found {
    /// Data that matches the package and its approval, or `None` when there is none yet.
    Current(Option<Kept>),
    /// A file that no longer matched, now deleted: why.
    Discarded(String),
    /// A file that cannot be read, counted as no data: why. It stays for the user to look at,
    /// until the next update replaces it.
    Damaged(String),
}

/// The kept data of every widget, in one data directory.
#[derive(Debug, Clone)]
pub(crate) struct KeptStore {
    dir: PathBuf,
    max_data_bytes: usize,
}

impl KeptStore {
    pub(crate) fn new(data_dir: &Path, max_data_bytes: usize) -> KeptStore {
        KeptStore {
            dir: data_dir.join(DIR),
            max_data_bytes,
        }
    }

    /// `widget-data/<hex SHA-256 of "<id>\n<origin>">.json`
    pub(crate) fn path(&self, id: &str, origin: &str) -> PathBuf {
        let digest = Sha256::digest(format!("{id}\n{origin}").as_bytes());
        let name: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
        self.dir.join(format!("{name}.json"))
    }

    /// The kept data of `key`'s package, discarding a file whose package digest or approval
    /// differs, or that recorded a connector as on that is now off.
    pub(crate) fn read(&self, key: &Key) -> Found {
        let path = self.path(&key.id, &key.origin);
        let kept = match self.read_file(&path) {
            Ok(None) => return Found::Current(None),
            Ok(Some(kept)) => kept,
            Err(reason) => return Found::Damaged(reason),
        };
        let why = if kept.id != key.id || kept.origin != key.origin {
            Some("it belongs to another package")
        } else if kept.digest != key.digest {
            Some("the package changed")
        } else if kept.approval.digest != key.approval {
            Some("the approval changed")
        } else if kept
            .approval
            .connectors_on
            .iter()
            .any(|id| !key.connectors_on.contains(id))
        {
            Some("a connector it used is off")
        } else if kept.accounts.iter().any(|(connector, fetched_with)| {
            fetched_with.is_some() && key.accounts.get(connector) != Some(fetched_with)
        }) {
            // Data read as an account never outlives it: another account, or none (FR-013)
            Some("the account it was fetched with is not connected")
        } else {
            None
        };
        match why {
            None => Found::Current(Some(kept)),
            Some(why) => {
                let _ = fs::remove_file(&path);
                Found::Discarded(why.to_owned())
            }
        }
    }

    /// A successful update: replaces the data and when it was fetched.
    pub(crate) fn record_success(&self, key: &Key, data: Value, at_ms: u64) -> io::Result<()> {
        let mut kept = self.current(key);
        // The data was fetched under this approval, with these connectors on
        kept.approval = KeptApproval {
            digest: key.approval.clone(),
            connectors_on: key.connectors_on.clone(),
        };
        kept.fetched_at_ms = Some(at_ms);
        kept.data = Some(data);
        kept.accounts = key.accounts.clone();
        kept.last_update = Some(LastUpdate {
            at_ms,
            outcome: Outcome::Ok,
            error: None,
        });
        self.write(&kept)
    }

    /// A failed update: changes only how the last update went, leaving the previous data.
    pub(crate) fn record_failure(&self, key: &Key, error: &str, at_ms: u64) -> io::Result<()> {
        let mut kept = self.current(key);
        kept.last_update = Some(LastUpdate {
            at_ms,
            outcome: Outcome::Failed,
            error: Some(error.to_owned()),
        });
        self.write(&kept)
    }

    /// Removes every kept data file fetched as the account `account_id` of `connector`, at once
    /// (FR-013): the ids of the widgets whose data went.
    pub(crate) fn discard_account(&self, connector: &str, account_id: &str) -> io::Result<Vec<String>> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let mut discarded = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(Some(kept)) = self.read_file(&path) else {
                continue;
            };
            if kept.accounts.get(connector).is_some_and(|fetched| fetched.as_deref() == Some(account_id)) {
                fs::remove_file(&path)?;
                discarded.push(kept.id);
            }
        }
        discarded.sort();
        discarded.dedup();
        Ok(discarded)
    }

    /// Removes every kept data file of the plugin `id`, whatever its origin. Returns how many.
    pub(crate) fn discard(&self, id: &str) -> io::Result<usize> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(error),
        };
        let mut removed = 0;
        for entry in entries {
            let path = entry?.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
                && self
                    .read_file(&path)
                    .ok()
                    .flatten()
                    .is_some_and(|kept| kept.id == id)
            {
                fs::remove_file(&path)?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// The file of `key`, current, or a new one under `key`'s approval.
    fn current(&self, key: &Key) -> Kept {
        match self.read(key) {
            Found::Current(Some(kept)) => kept,
            _ => Kept {
                id: key.id.clone(),
                origin: key.origin.clone(),
                digest: key.digest.clone(),
                approval: KeptApproval {
                    digest: key.approval.clone(),
                    connectors_on: key.connectors_on.clone(),
                },
                fetched_at_ms: None,
                data: None,
                last_update: None,
                accounts: BTreeMap::new(),
            },
        }
    }

    fn read_file(&self, path: &Path) -> Result<Option<Kept>, String> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        let kept: Kept = serde_json::from_slice(&bytes)
            .map_err(|error| format!("{} is damaged: {error}", path.display()))?;
        if let Some(data) = &kept.data
            && data.to_string().len() > self.max_data_bytes
        {
            return Err(format!(
                "{} holds data over the limit of {} bytes",
                path.display(),
                self.max_data_bytes
            ));
        }
        Ok(Some(kept))
    }

    /// Writes `kept` beside its path and renames it over it.
    fn write(&self, kept: &Kept) -> io::Result<()> {
        fs::create_dir_all(&self.dir)?;
        let path = self.path(&kept.id, &kept.origin);
        let mut temporary = NamedTempFile::new_in(&self.dir)?;
        let mut json = serde_json::to_vec_pretty(kept).expect("kept data always serializes");
        json.push(b'\n');
        temporary.write_all(&json)?;
        temporary.as_file().sync_all()?;
        temporary.persist(&path).map_err(|error| error.error)?;
        Ok(())
    }
}

/// The configuration `render` receives for a widget that exports `updater`
/// (`data-model.md`, What render receives): `data` and `fetched_at_ms` are `null` before the
/// first successful update, and `last_update` before the first update.
pub(crate) fn render_document(config: Value, kept: Option<&Kept>) -> Value {
    json!({
        "config": config,
        "data": kept.and_then(|kept| kept.data.clone()),
        "fetched_at_ms": kept.and_then(|kept| kept.fetched_at_ms),
        "last_update": kept.and_then(|kept| kept.last_update.clone()),
    })
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn key() -> Key {
        Key {
            id: "local/github-releases".to_owned(),
            origin: "/plugins/github-releases.wasm".to_owned(),
            digest: "sha256:aa".to_owned(),
            approval: "sha256:bb".to_owned(),
            connectors_on: vec!["catalog/github".to_owned()],
            accounts: BTreeMap::new(),
        }
    }

    fn store() -> (TempDir, KeptStore) {
        let dir = TempDir::new().unwrap();
        let store = KeptStore::new(dir.path(), 256 * 1024);
        (dir, store)
    }

    #[test]
    fn the_file_is_named_after_the_id_and_origin() {
        let (dir, store) = store();
        let path = store.path("local/a", "/x");
        assert!(path.starts_with(dir.path().join("widget-data")));
        let name = path.file_name().unwrap().to_str().unwrap();
        assert_eq!(name.len(), 64 + ".json".len());
        assert_ne!(path, store.path("local/a", "/y"));
        assert_ne!(path, store.path("local/b", "/x"));
        // The line break separates the two only because an id never holds one
        assert!(crate::manifest::PluginId::parse("local/a\nb").is_err());
    }

    #[test]
    fn no_file_is_no_data() {
        let (_dir, store) = store();
        assert_eq!(store.read(&key()), Found::Current(None));
    }

    #[test]
    fn a_success_replaces_the_data_and_a_failure_only_the_last_update() {
        let (_dir, store) = store();
        store
            .record_success(&key(), json!({"releases": [1]}), 1000)
            .unwrap();
        let Found::Current(Some(kept)) = store.read(&key()) else {
            panic!("kept data")
        };
        assert_eq!(kept.data, Some(json!({"releases": [1]})));
        assert_eq!(kept.fetched_at_ms, Some(1000));
        assert_eq!(kept.last_update.as_ref().unwrap().outcome, Outcome::Ok);
        assert_eq!(kept.approval.digest, "sha256:bb");
        assert_eq!(kept.approval.connectors_on, ["catalog/github"]);

        store
            .record_failure(&key(), "plugin-error: GitHub answered 500", 2000)
            .unwrap();
        let Found::Current(Some(kept)) = store.read(&key()) else {
            panic!("kept data")
        };
        assert_eq!(kept.data, Some(json!({"releases": [1]})));
        assert_eq!(kept.fetched_at_ms, Some(1000));
        assert_eq!(
            kept.last_update,
            Some(LastUpdate {
                at_ms: 2000,
                outcome: Outcome::Failed,
                error: Some("plugin-error: GitHub answered 500".to_owned()),
            })
        );

        store
            .record_success(&key(), json!({"releases": [2]}), 3000)
            .unwrap();
        let Found::Current(Some(kept)) = store.read(&key()) else {
            panic!("kept data")
        };
        assert_eq!(kept.data, Some(json!({"releases": [2]})));
        assert_eq!(kept.last_update.unwrap().error, None);
    }

    #[test]
    fn a_failure_before_any_success_keeps_no_data() {
        let (_dir, store) = store();
        store
            .record_failure(&key(), "plugin-error: down", 5)
            .unwrap();
        let Found::Current(Some(kept)) = store.read(&key()) else {
            panic!("kept data")
        };
        assert_eq!(kept.data, None);
        assert_eq!(kept.fetched_at_ms, None);
        assert_eq!(kept.last_update.unwrap().outcome, Outcome::Failed);
    }

    #[test]
    fn data_that_no_longer_matches_is_discarded() {
        let edits: [fn(&mut Key); 3] = [
            |key| key.digest = "sha256:new-version".to_owned(),
            |key| key.approval = "sha256:new-approval".to_owned(),
            |key| key.connectors_on.clear(),
        ];
        for edit in edits {
            let (_dir, store) = store();
            store.record_success(&key(), json!({"a": 1}), 1).unwrap();
            let mut changed = key();
            edit(&mut changed);
            assert!(matches!(store.read(&changed), Found::Discarded(_)));
            assert!(!store.path(&key().id, &key().origin).exists());
            assert_eq!(store.read(&key()), Found::Current(None));
        }
        // A connector turned on since keeps the data: nothing it was fetched with went away
        let (_dir, store) = store();
        store.record_success(&key(), json!({"a": 1}), 1).unwrap();
        let mut more = key();
        more.connectors_on.push("local/weather".to_owned());
        assert!(matches!(store.read(&more), Found::Current(Some(_))));
        // Data fetched with it on records it, so turning it off again discards that data
        store.record_success(&more, json!({"a": 2}), 2).unwrap();
        assert!(matches!(store.read(&key()), Found::Discarded(_)));
    }

    #[test]
    fn discard_removes_every_file_of_the_id() {
        let (_dir, store) = store();
        let mut other_origin = key();
        other_origin.origin = "/elsewhere/github-releases.wasm".to_owned();
        let mut other_id = key();
        other_id.id = "local/other".to_owned();
        for key in [key(), other_origin.clone(), other_id.clone()] {
            store.record_success(&key, json!(1), 1).unwrap();
        }
        assert_eq!(store.discard("local/github-releases").unwrap(), 2);
        assert_eq!(store.read(&key()), Found::Current(None));
        assert_eq!(store.read(&other_origin), Found::Current(None));
        assert!(matches!(store.read(&other_id), Found::Current(Some(_))));
        assert_eq!(store.discard("local/none").unwrap(), 0);
        let (_dir, empty) = self::store();
        assert_eq!(empty.discard("local/x").unwrap(), 0);
    }

    #[test]
    fn a_damaged_file_counts_as_no_data_and_is_reported() {
        let (_dir, store) = store();
        let path = store.path(&key().id, &key().origin);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        for text in ["{", "[]", r#"{"id":"x"}"#] {
            fs::write(&path, text).unwrap();
            assert!(matches!(store.read(&key()), Found::Damaged(_)), "{text}");
        }
        // Data over the limit, written by hand, is not data either
        let small = KeptStore::new(path.parent().unwrap().parent().unwrap(), 8);
        store
            .record_success(&key(), json!({"a": "long enough"}), 1)
            .unwrap();
        assert!(matches!(small.read(&key()), Found::Damaged(_)));
        // The next update replaces a damaged file
        fs::write(&path, "{").unwrap();
        store.record_success(&key(), json!(2), 2).unwrap();
        assert!(matches!(store.read(&key()), Found::Current(Some(_))));
    }

    #[test]
    fn render_receives_the_document_of_the_data_model() {
        assert_eq!(
            render_document(json!({"owner": "rust-lang"}), None),
            json!({"config": {"owner": "rust-lang"}, "data": null, "fetched_at_ms": null, "last_update": null})
        );
        let (_dir, store) = store();
        store
            .record_success(&key(), json!({"releases": []}), 1792761600000)
            .unwrap();
        let Found::Current(kept) = store.read(&key()) else {
            panic!("kept data")
        };
        assert_eq!(
            render_document(json!({"owner": "rust-lang"}), kept.as_ref()),
            json!({
                "config": {"owner": "rust-lang"},
                "data": {"releases": []},
                "fetched_at_ms": 1792761600000_u64,
                "last_update": {"at_ms": 1792761600000_u64, "outcome": "ok"}
            })
        );
    }

    #[test]
    fn writes_are_atomic_and_leave_no_temporary_file() {
        let (_dir, store) = store();
        for at in 0..5 {
            store.record_success(&key(), json!(at), at).unwrap();
        }
        let files: Vec<_> = fs::read_dir(store.path(&key().id, &key().origin).parent().unwrap())
            .unwrap()
            .collect();
        assert_eq!(files.len(), 1);
    }
}
