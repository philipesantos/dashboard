//! The grant record: every approval the user gave, kept by the host
//! (`specs/002-plugin-permissions/`, data-model.md and research R4).
//!
//! Plugins have no file access, so they can neither read nor change it (FR-009). In this phase
//! the only record is the development one, which the CLI changes for local package files.
//! Every approval belongs to the origin it was approved from, so another package that claims
//! the same id gets none of its grants (FR-020).

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, PoisonError};

use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

use crate::error::{ErrorKind, RuntimeError};
use crate::manifest::{Manifest, PluginId};

/// The record format this host reads and writes.
pub const RECORD_VERSION: u64 = 1;

/// The app's directory inside the system's data directory. Provisional: it moves with the
/// project's final name, before the first release.
const APP_DIR: &str = "dev.philsantos.dashboard";
/// The development record's file name.
const RECORD_FILE: &str = "development-grants.json";

/// One plugin's approval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    /// Where the approved package came from: in the CLI, the canonical path of its file. The
    /// entry grants nothing to a package from any other origin.
    pub origin: String,
    /// Sorted and unique: the required permissions approved, plus the optional ones turned
    /// on. A name the host does not know is kept and never linked.
    pub granted: Vec<String>,
    /// The package last approved, for display only.
    pub approved_version: String,
    pub approved_digest: String,
}

impl Entry {
    pub fn is_granted(&self, permission: &str) -> bool {
        self.granted.iter().any(|name| name == permission)
    }
}

/// What [`GrantStore::approve`] did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Approval {
    /// The required permissions granted, in the manifest's order.
    pub required: Vec<String>,
    /// The origin of the approval this one replaced, when it came from another origin.
    pub replaced: Option<String>,
}

/// The record as stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct GrantRecord {
    record_version: u64,
    plugins: BTreeMap<String, Entry>,
}

impl GrantRecord {
    fn empty() -> GrantRecord {
        GrantRecord {
            record_version: RECORD_VERSION,
            plugins: BTreeMap::new(),
        }
    }

    /// Reads a record, refusing anything this host would not have written.
    fn parse(bytes: &[u8]) -> Result<GrantRecord, String> {
        // The version first, so a newer record is named as such, whatever else it holds
        let value: serde_json::Value =
            serde_json::from_slice(bytes).map_err(|error| format!("not JSON: {error}"))?;
        match value
            .get("record_version")
            .and_then(serde_json::Value::as_u64)
        {
            Some(RECORD_VERSION) => {}
            Some(version) if version > RECORD_VERSION => {
                return Err(format!(
                    "record_version {version} is newer than this host's {RECORD_VERSION}"
                ));
            }
            _ => return Err(format!("record_version must be {RECORD_VERSION}")),
        }
        // Parsed again from the bytes, which refuses duplicate fields
        let record: GrantRecord =
            serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
        for (id, entry) in &record.plugins {
            PluginId::parse(id)?;
            if !entry.granted.windows(2).all(|pair| pair[0] < pair[1]) {
                return Err(format!("the grants of {id} are not sorted and unique"));
            }
        }
        Ok(record)
    }
}

/// The runtime's view of one grant record: where it lives, and a generation that every change
/// through this store increments, so a loaded plugin knows to link again (research R6).
pub struct GrantStore {
    backing: Backing,
    generation: AtomicU64,
}

enum Backing {
    /// For tests and examples: a record that lives as long as the store.
    Memory(Mutex<GrantRecord>),
    /// A JSON file, read on every use and changed under its lock file.
    File(PathBuf),
}

impl GrantStore {
    /// A store whose record lives in memory, empty at first.
    pub fn in_memory() -> GrantStore {
        GrantStore {
            backing: Backing::Memory(Mutex::new(GrantRecord::empty())),
            generation: AtomicU64::new(0),
        }
    }

    /// A store backed by the record at `path`. Nothing is read or written until it is used; a
    /// missing file counts as an empty record.
    pub fn open(path: impl Into<PathBuf>) -> GrantStore {
        GrantStore {
            backing: Backing::File(path.into()),
            generation: AtomicU64::new(0),
        }
    }

    /// The development record in the system's data directory, if the system has one.
    pub fn default_development_path() -> Option<PathBuf> {
        dirs::data_dir().map(|dir| dir.join(APP_DIR).join(RECORD_FILE))
    }

    /// The record's file, for a store backed by one.
    pub fn path(&self) -> Option<&Path> {
        match &self.backing {
            Backing::Memory(_) => None,
            Backing::File(path) => Some(path),
        }
    }

    /// Increases with every change made through this store.
    pub fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    /// The plugin's approval, if it has one, whatever its origin. Refused as `not-granted` when
    /// the record cannot be used.
    pub fn entry(&self, id: &PluginId) -> Result<Option<Entry>, RuntimeError> {
        Ok(self.read()?.plugins.remove(id.as_str()))
    }

    /// Approves the manifest's required permissions, all together, for packages from `origin`
    /// (FR-018). From the entry's own origin, `granted` becomes (`granted` ∩ declared) ∪
    /// required. With no entry, or one from another origin, the entry is replaced: only the
    /// required permissions are granted (FR-020).
    pub fn approve(
        &self,
        manifest: &Manifest,
        origin: &str,
        digest: &str,
    ) -> Result<Approval, RuntimeError> {
        let required: Vec<String> = manifest
            .permissions()
            .iter()
            .filter(|permission| permission.required)
            .map(|permission| permission.name.clone())
            .collect();
        self.change(|record| {
            let (mut granted, replaced) = match record.plugins.get(manifest.id().as_str()) {
                Some(entry) if entry.origin == origin => {
                    let declared = entry
                        .granted
                        .iter()
                        .filter(|name| manifest.permission(name).is_some())
                        .cloned()
                        .collect();
                    (declared, None)
                }
                Some(entry) => (Vec::new(), Some(entry.origin.clone())),
                None => (Vec::new(), None),
            };
            granted.extend(required.iter().cloned());
            granted.sort();
            granted.dedup();
            record.plugins.insert(
                manifest.id().to_string(),
                Entry {
                    origin: origin.to_owned(),
                    granted,
                    approved_version: manifest.version().to_owned(),
                    approved_digest: digest.to_owned(),
                },
            );
            let approval = Approval {
                required: required.clone(),
                replaced,
            };
            Ok((approval, true))
        })
    }

    /// Turns one optional permission on or off. Refused as `not-declared` when the manifest
    /// does not request it, `required-together` when it is required, and `not-granted` when
    /// the plugin has no approval from `origin`.
    pub fn set_optional(
        &self,
        manifest: &Manifest,
        origin: &str,
        permission: &str,
        on: bool,
    ) -> Result<(), RuntimeError> {
        let names = || vec![permission.to_owned()];
        match manifest.permission(permission) {
            None => {
                return Err(RuntimeError::with_names(
                    ErrorKind::NotDeclared,
                    names(),
                    Some("the manifest does not request it"),
                ));
            }
            Some(request) if request.required => {
                return Err(RuntimeError::with_names(
                    ErrorKind::RequiredTogether,
                    names(),
                    Some(
                        "it is required: required permissions are approved together, and change only with a new approval or by removing it",
                    ),
                ));
            }
            Some(_) => {}
        }
        self.change(|record| {
            let entry = approved_entry(record, manifest.id(), origin)?;
            if entry.is_granted(permission) == on {
                return Ok(((), false));
            }
            if on {
                entry.granted.push(permission.to_owned());
                entry.granted.sort();
            } else {
                entry.granted.retain(|name| name != permission);
            }
            Ok(((), true))
        })
    }

    /// Removes the plugin's approval and every grant with it. Returns whether there was one.
    pub fn remove(&self, id: &PluginId) -> Result<bool, RuntimeError> {
        self.change(|record| {
            let removed = record.plugins.remove(id.as_str()).is_some();
            Ok((removed, removed))
        })
    }

    /// Removes from the plugin's approval every permission this manifest no longer declares
    /// (FR-012), when the approval is from `origin`. Returns the names removed; writes nothing
    /// when there are none.
    pub fn prune(&self, manifest: &Manifest, origin: &str) -> Result<Vec<String>, RuntimeError> {
        self.change(|record| {
            let Some(entry) = record
                .plugins
                .get_mut(manifest.id().as_str())
                .filter(|entry| entry.origin == origin)
            else {
                return Ok((Vec::new(), false));
            };
            let (kept, removed): (Vec<String>, Vec<String>) = entry
                .granted
                .drain(..)
                .partition(|name| manifest.permission(name).is_some());
            entry.granted = kept;
            let changed = !removed.is_empty();
            Ok((removed, changed))
        })
    }

    /// The record as it is now.
    fn read(&self) -> Result<GrantRecord, RuntimeError> {
        match &self.backing {
            Backing::Memory(record) => Ok(record
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone()),
            Backing::File(path) => {
                // A record that does not exist is empty, and reading it creates nothing
                match fs::metadata(path) {
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {
                        return Ok(GrantRecord::empty());
                    }
                    Err(error) => return Err(damaged(path, error)),
                    Ok(metadata) if !metadata.is_file() => return Err(damaged(path, "not a file")),
                    Ok(_) => {}
                }
                let _lock = lock(path, false)?;
                load(path)
            }
        }
    }

    /// Applies `apply` to the record under its lock, and writes the result when `apply` says it
    /// changed it. A refusal, or a record that cannot be used, leaves it as it was.
    fn change<T>(
        &self,
        apply: impl FnOnce(&mut GrantRecord) -> Result<(T, bool), RuntimeError>,
    ) -> Result<T, RuntimeError> {
        let (value, changed) = match &self.backing {
            Backing::Memory(record) => {
                let mut record = record.lock().unwrap_or_else(PoisonError::into_inner);
                let mut changed_record = record.clone();
                let (value, changed) = apply(&mut changed_record)?;
                if changed {
                    *record = changed_record;
                }
                (value, changed)
            }
            Backing::File(path) => {
                // Read, change and write under one exclusive lock, so an update made by another
                // process in between is never lost (research R4).
                let _lock = lock(path, true)?;
                let mut record = load(path)?;
                let (value, changed) = apply(&mut record)?;
                if changed {
                    write(path, &record)?;
                }
                (value, changed)
            }
        };
        if changed {
            self.generation.fetch_add(1, Ordering::AcqRel);
        }
        Ok(value)
    }
}

/// The plugin's entry, if it was approved from `origin`; otherwise a `not-granted` error that
/// says why.
fn approved_entry<'a>(
    record: &'a mut GrantRecord,
    id: &PluginId,
    origin: &str,
) -> Result<&'a mut Entry, RuntimeError> {
    match record.plugins.get_mut(id.as_str()) {
        Some(entry) if entry.origin == origin => Ok(entry),
        Some(entry) => Err(RuntimeError::new(
            ErrorKind::NotGranted,
            format!(
                "{id} is approved from {}, not from {origin}; approve this package to replace that approval",
                entry.origin
            ),
        )),
        None => Err(RuntimeError::new(
            ErrorKind::NotGranted,
            format!("{id} has no approval; approve it first"),
        )),
    }
}

/// Takes the record's lock file, `<record>.lock`: shared to read, exclusive to change. The lock
/// is released when the returned file is dropped.
fn lock(path: &Path, exclusive: bool) -> Result<File, RuntimeError> {
    let mut lock_path = OsString::from(path.as_os_str());
    lock_path.push(".lock");
    let lock_path = PathBuf::from(lock_path);
    if exclusive && let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|error| unwritable(path, error))?;
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| damaged(&lock_path, error))?;
    let locked = if exclusive {
        file.lock()
    } else {
        file.lock_shared()
    };
    locked.map_err(|error| damaged(&lock_path, error))?;
    Ok(file)
}

/// Reads the record at `path`, under a lock the caller holds.
fn load(path: &Path) -> Result<GrantRecord, RuntimeError> {
    match fs::read(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(GrantRecord::empty()),
        Err(error) => Err(damaged(path, error)),
        Ok(bytes) => GrantRecord::parse(&bytes).map_err(|reason| damaged(path, reason)),
    }
}

/// Writes the record beside `path`, syncs it, then renames it over `path`, so a crash leaves
/// either the old record or the new one, never part of each.
fn write(path: &Path, record: &GrantRecord) -> Result<(), RuntimeError> {
    let dir = path.parent().unwrap_or(Path::new(""));
    let mut temporary = NamedTempFile::new_in(dir).map_err(|error| unwritable(path, error))?;
    let mut json = serde_json::to_vec_pretty(record).expect("a grant record always serializes");
    json.push(b'\n');
    temporary
        .write_all(&json)
        .and_then(|()| temporary.as_file().sync_all())
        .map_err(|error| unwritable(path, error))?;
    temporary
        .persist(path)
        .map_err(|error| unwritable(path, error.error))?;
    Ok(())
}

/// A record that cannot be used grants nothing, and is never written over (research R4).
fn damaged(path: &Path, reason: impl std::fmt::Display) -> RuntimeError {
    RuntimeError::new(
        ErrorKind::NotGranted,
        format!(
            "the grant record {} cannot be used ({reason}); it grants nothing until it is fixed or deleted",
            path.display()
        ),
    )
}

fn unwritable(path: &Path, error: io::Error) -> RuntimeError {
    RuntimeError::new(
        ErrorKind::NotGranted,
        format!("cannot write the grant record {}: {error}", path.display()),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::sync::Arc;
    use std::thread;

    use serde_json::{Value, json};
    use tempfile::TempDir;

    use super::*;

    const ORIGIN: &str = "/plugins/probe.wasm";
    const OTHER_ORIGIN: &str = "/downloads/probe.wasm";
    const DIGEST: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";

    /// A manifest for `id` with `log` required, optional or not declared.
    fn manifest(id: &str, version: &str, log: Option<bool>) -> Manifest {
        let permissions: Vec<Value> = log
            .map(|required| json!({ "name": "log", "required": required }))
            .into_iter()
            .collect();
        let manifest = json!({
            "manifest_version": 1,
            "id": id,
            "version": version,
            "author": "A",
            "permissions": permissions,
            "login_providers": []
        });
        Manifest::parse(&serde_json::to_vec(&manifest).expect("serializes")).expect("valid")
    }

    fn id(id: &str) -> PluginId {
        PluginId::parse(id).expect("a valid id")
    }

    /// A file-backed store in a fresh directory, which the store must create.
    fn file_store() -> (TempDir, GrantStore) {
        let dir = TempDir::new().expect("a temporary directory");
        let store = GrantStore::open(dir.path().join("nested").join("grants.json"));
        (dir, store)
    }

    /// Runs `check` on an in-memory store and on a file-backed one.
    fn for_each_store(check: impl Fn(&GrantStore)) {
        check(&GrantStore::in_memory());
        let (_dir, store) = file_store();
        check(&store);
    }

    fn entry(store: &GrantStore, plugin: &str) -> Option<Entry> {
        store.entry(&id(plugin)).expect("the record is readable")
    }

    fn granted(store: &GrantStore, plugin: &str) -> Vec<String> {
        entry(store, plugin)
            .map(|entry| entry.granted)
            .unwrap_or_default()
    }

    /// A file store over a record with this exact text.
    fn store_with_text(text: &str) -> (TempDir, GrantStore, PathBuf) {
        let dir = TempDir::new().expect("a temporary directory");
        let path = dir.path().join("grants.json");
        fs::write(&path, text).expect("the record is written");
        (dir, GrantStore::open(&path), path)
    }

    #[test]
    fn writes_the_record_format_of_the_data_model() {
        let (_dir, store) = file_store();
        store
            .approve(
                &manifest("local/probe", "1.2.0", Some(true)),
                ORIGIN,
                DIGEST,
            )
            .expect("approved");
        let path = store.path().expect("a file store");
        let record: Value =
            serde_json::from_slice(&fs::read(path).expect("written")).expect("JSON");
        assert_eq!(
            record,
            json!({
                "record_version": 1,
                "plugins": {
                    "local/probe": {
                        "origin": ORIGIN,
                        "granted": ["log"],
                        "approved_version": "1.2.0",
                        "approved_digest": DIGEST
                    }
                }
            })
        );
    }

    #[test]
    fn the_default_record_is_in_the_data_directory() {
        let path = GrantStore::default_development_path();
        match dirs::data_dir() {
            Some(data_dir) => {
                let path = path.expect("a path when the system has a data directory");
                assert!(path.starts_with(data_dir), "{}", path.display());
                assert!(
                    path.ends_with(
                        Path::new("dev.philsantos.dashboard").join("development-grants.json")
                    ),
                    "{}",
                    path.display()
                );
            }
            None => assert_eq!(path, None),
        }
    }

    #[test]
    fn a_missing_record_is_empty() {
        let (_dir, store) = file_store();
        assert_eq!(entry(&store, "local/probe"), None);
        assert!(
            !store.path().expect("a file store").exists(),
            "reading wrote nothing"
        );
        assert_eq!(GrantStore::in_memory().entry(&id("local/probe")), Ok(None));
    }

    #[test]
    fn reads_a_record_with_a_permission_the_host_does_not_know() {
        let (_dir, store, _path) = store_with_text(&format!(
            r#"{{"record_version":1,"plugins":{{"local/probe":{{"origin":"{ORIGIN}","granted":["log","telepathy"],"approved_version":"1.0.0","approved_digest":"{DIGEST}"}}}}}}"#
        ));
        assert_eq!(granted(&store, "local/probe"), ["log", "telepathy"]);
    }

    #[test]
    fn a_damaged_record_grants_nothing_and_is_never_overwritten() {
        let damaged = [
            ("not JSON", "{".to_owned()),
            ("not an object", "[]".to_owned()),
            (
                "a newer record version",
                r#"{"record_version":2,"plugins":{},"more":1}"#.to_owned(),
            ),
            (
                "an older record version",
                r#"{"record_version":0,"plugins":{}}"#.to_owned(),
            ),
            (
                "an unknown field",
                r#"{"record_version":1,"plugins":{},"x":1}"#.to_owned(),
            ),
            (
                "a malformed id",
                format!(
                    r#"{{"record_version":1,"plugins":{{"Local/Probe":{{"origin":"{ORIGIN}","granted":[],"approved_version":"1.0.0","approved_digest":"{DIGEST}"}}}}}}"#
                ),
            ),
            (
                "unsorted grants",
                format!(
                    r#"{{"record_version":1,"plugins":{{"local/probe":{{"origin":"{ORIGIN}","granted":["log","a"],"approved_version":"1.0.0","approved_digest":"{DIGEST}"}}}}}}"#
                ),
            ),
            (
                "repeated grants",
                format!(
                    r#"{{"record_version":1,"plugins":{{"local/probe":{{"origin":"{ORIGIN}","granted":["log","log"],"approved_version":"1.0.0","approved_digest":"{DIGEST}"}}}}}}"#
                ),
            ),
        ];
        for (case, text) in damaged {
            let (_dir, store, path) = store_with_text(&text);
            let error = store.entry(&id("local/probe")).expect_err(case);
            assert_eq!(error.kind(), ErrorKind::NotGranted, "{case}: {error}");
            assert!(
                error.detail().contains("grants.json"),
                "{case}: the detail `{}` names no path",
                error.detail()
            );

            let probe = manifest("local/probe", "1.0.0", Some(true));
            let optional = manifest("local/probe", "1.0.0", Some(false));
            let refusals = [
                store.approve(&probe, ORIGIN, DIGEST).map(drop),
                store.set_optional(&optional, ORIGIN, "log", true),
                store.remove(&id("local/probe")).map(drop),
                store.prune(&probe, ORIGIN).map(drop),
            ];
            for refusal in refusals {
                assert_eq!(
                    refusal.map_err(|error| error.kind()),
                    Err(ErrorKind::NotGranted),
                    "{case}"
                );
            }
            assert_eq!(
                fs::read_to_string(&path).expect("still there"),
                text,
                "{case}"
            );
            assert_eq!(store.generation(), 0, "{case}");
        }
    }

    #[test]
    fn a_record_that_cannot_be_read_grants_nothing() {
        let dir = TempDir::new().expect("a temporary directory");
        // A directory where the file should be
        let store = GrantStore::open(dir.path());
        let error = store.entry(&id("local/probe")).expect_err("unreadable");
        assert_eq!(error.kind(), ErrorKind::NotGranted);
    }

    #[test]
    fn approve_grants_every_required_permission() {
        for_each_store(|store| {
            let approval = store
                .approve(
                    &manifest("local/probe", "1.0.0", Some(true)),
                    ORIGIN,
                    DIGEST,
                )
                .expect("approved");
            assert_eq!(
                approval,
                Approval {
                    required: vec!["log".to_owned()],
                    replaced: None
                }
            );
            assert_eq!(
                entry(store, "local/probe"),
                Some(Entry {
                    origin: ORIGIN.to_owned(),
                    granted: vec!["log".to_owned()],
                    approved_version: "1.0.0".to_owned(),
                    approved_digest: DIGEST.to_owned(),
                })
            );
        });
    }

    #[test]
    fn approve_leaves_optional_permissions_off() {
        for_each_store(|store| {
            let approval = store
                .approve(
                    &manifest("local/probe", "1.0.0", Some(false)),
                    ORIGIN,
                    DIGEST,
                )
                .expect("approved");
            assert!(approval.required.is_empty());
            assert_eq!(granted(store, "local/probe"), Vec::<String>::new());
            assert!(
                entry(store, "local/probe").is_some(),
                "approved with nothing granted"
            );
        });
    }

    #[test]
    fn approve_from_the_same_origin_keeps_what_is_still_declared() {
        for_each_store(|store| {
            let optional = manifest("local/probe", "1.0.0", Some(false));
            store.approve(&optional, ORIGIN, DIGEST).expect("approved");
            store
                .set_optional(&optional, ORIGIN, "log", true)
                .expect("turned on");

            // Still declared, still optional: stays on
            store
                .approve(
                    &manifest("local/probe", "1.1.0", Some(false)),
                    ORIGIN,
                    "sha256:1",
                )
                .expect("approved again");
            assert_eq!(granted(store, "local/probe"), ["log"]);
            let entry = entry(store, "local/probe").expect("approved");
            assert_eq!(entry.approved_version, "1.1.0");
            assert_eq!(entry.approved_digest, "sha256:1");

            // No longer declared: dropped
            store
                .approve(&manifest("local/probe", "1.2.0", None), ORIGIN, DIGEST)
                .expect("approved again");
            assert_eq!(granted(store, "local/probe"), Vec::<String>::new());
        });
    }

    #[test]
    fn approve_drops_undeclared_names_the_host_does_not_know() {
        let (_dir, store, _path) = store_with_text(&format!(
            r#"{{"record_version":1,"plugins":{{"local/probe":{{"origin":"{ORIGIN}","granted":["log","telepathy"],"approved_version":"1.0.0","approved_digest":"{DIGEST}"}}}}}}"#
        ));
        store
            .approve(
                &manifest("local/probe", "1.1.0", Some(true)),
                ORIGIN,
                DIGEST,
            )
            .expect("approved");
        assert_eq!(granted(&store, "local/probe"), ["log"]);
    }

    #[test]
    fn approve_from_another_origin_replaces_the_approval() {
        for_each_store(|store| {
            let optional = manifest("local/probe", "1.0.0", Some(false));
            store.approve(&optional, ORIGIN, DIGEST).expect("approved");
            store
                .set_optional(&optional, ORIGIN, "log", true)
                .expect("turned on");

            let approval = store
                .approve(&optional, OTHER_ORIGIN, DIGEST)
                .expect("approved from the other origin");
            assert_eq!(approval.replaced.as_deref(), Some(ORIGIN));
            let entry = entry(store, "local/probe").expect("approved");
            assert_eq!(entry.origin, OTHER_ORIGIN);
            assert!(
                entry.granted.is_empty(),
                "optional permissions start off again"
            );

            let approval = store
                .approve(&optional, OTHER_ORIGIN, DIGEST)
                .expect("approved again");
            assert_eq!(approval.replaced, None, "the same origin replaces nothing");
        });
    }

    #[test]
    fn set_optional_turns_one_permission_on_and_off() {
        for_each_store(|store| {
            let optional = manifest("local/probe", "1.0.0", Some(false));
            store.approve(&optional, ORIGIN, DIGEST).expect("approved");
            store
                .set_optional(&optional, ORIGIN, "log", true)
                .expect("on");
            assert_eq!(granted(store, "local/probe"), ["log"]);
            store
                .set_optional(&optional, ORIGIN, "log", true)
                .expect("on again");
            assert_eq!(granted(store, "local/probe"), ["log"]);
            store
                .set_optional(&optional, ORIGIN, "log", false)
                .expect("off");
            assert_eq!(granted(store, "local/probe"), Vec::<String>::new());
        });
    }

    #[test]
    fn set_optional_refuses_what_is_not_an_optional_permission_of_this_approval() {
        for_each_store(|store| {
            let required = manifest("local/probe", "1.0.0", Some(true));
            let optional = manifest("local/probe", "1.0.0", Some(false));
            let undeclared = manifest("local/probe", "1.0.0", None);
            let kind = |result: Result<(), RuntimeError>| result.map_err(|error| error.kind());

            assert_eq!(
                kind(store.set_optional(&optional, ORIGIN, "log", true)),
                Err(ErrorKind::NotGranted),
                "no approval"
            );

            store.approve(&required, ORIGIN, DIGEST).expect("approved");
            assert_eq!(
                kind(store.set_optional(&required, ORIGIN, "log", false)),
                Err(ErrorKind::RequiredTogether)
            );
            assert_eq!(
                kind(store.set_optional(&undeclared, ORIGIN, "log", true)),
                Err(ErrorKind::NotDeclared)
            );
            assert_eq!(
                kind(store.set_optional(&optional, OTHER_ORIGIN, "log", true)),
                Err(ErrorKind::NotGranted),
                "another origin"
            );
            assert_eq!(granted(store, "local/probe"), ["log"], "nothing changed");
        });
    }

    #[test]
    fn remove_deletes_the_approval() {
        for_each_store(|store| {
            store
                .approve(
                    &manifest("local/probe", "1.0.0", Some(true)),
                    ORIGIN,
                    DIGEST,
                )
                .expect("approved");
            store
                .approve(
                    &manifest("local/other", "1.0.0", Some(true)),
                    ORIGIN,
                    DIGEST,
                )
                .expect("approved");
            assert_eq!(store.remove(&id("local/probe")), Ok(true));
            assert_eq!(entry(store, "local/probe"), None);
            assert!(
                entry(store, "local/other").is_some(),
                "other plugins keep theirs"
            );
            assert_eq!(store.remove(&id("local/probe")), Ok(false));
        });
    }

    #[test]
    fn prune_removes_what_the_manifest_no_longer_declares() {
        for_each_store(|store| {
            store
                .approve(
                    &manifest("local/probe", "1.0.0", Some(true)),
                    ORIGIN,
                    DIGEST,
                )
                .expect("approved");
            let before = store.generation();
            assert_eq!(
                store.prune(&manifest("local/probe", "1.0.0", Some(true)), ORIGIN),
                Ok(Vec::new())
            );
            assert_eq!(
                store.generation(),
                before,
                "nothing to prune, nothing written"
            );

            // From another origin the approval is not this package's
            let quiet = manifest("local/probe", "1.1.0", None);
            assert_eq!(store.prune(&quiet, OTHER_ORIGIN), Ok(Vec::new()));
            assert_eq!(granted(store, "local/probe"), ["log"]);

            assert_eq!(store.prune(&quiet, ORIGIN), Ok(vec!["log".to_owned()]));
            assert_eq!(granted(store, "local/probe"), Vec::<String>::new());
            assert!(entry(store, "local/probe").is_some(), "still approved");
        });
    }

    #[test]
    fn every_change_increments_the_generation() {
        for_each_store(|store| {
            let optional = manifest("local/probe", "1.0.0", Some(false));
            let mut last = store.generation();
            let mut changed = |store: &GrantStore| {
                let now = store.generation();
                assert!(now > last, "{now} after {last}");
                last = now;
            };
            store.approve(&optional, ORIGIN, DIGEST).expect("approved");
            changed(store);
            store
                .set_optional(&optional, ORIGIN, "log", true)
                .expect("on");
            changed(store);
            store
                .set_optional(&optional, ORIGIN, "log", false)
                .expect("off");
            changed(store);
            store
                .prune(&manifest("local/probe", "1.0.0", None), ORIGIN)
                .expect("pruned");
            store
                .set_optional(&optional, ORIGIN, "log", true)
                .expect("on");
            store
                .prune(&manifest("local/probe", "1.1.0", None), ORIGIN)
                .expect("pruned");
            changed(store);
            store.remove(&id("local/probe")).expect("removed");
            changed(store);

            let before = store.generation();
            entry(store, "local/probe");
            let _ = store.set_optional(&optional, ORIGIN, "log", true);
            assert_eq!(
                store.generation(),
                before,
                "reads and refusals change nothing"
            );
        });
    }

    #[test]
    fn two_threads_changing_one_file_lose_no_update() {
        let dir = TempDir::new().expect("a temporary directory");
        let path = Arc::new(dir.path().join("grants.json"));
        let threads: Vec<_> = (0..2)
            .map(|thread| {
                let path = Arc::clone(&path);
                thread::spawn(move || {
                    // Each thread has its own store, as two processes would
                    let store = GrantStore::open(path.as_path());
                    for n in 0..25 {
                        let plugin = manifest(&format!("local/t{thread}-{n}"), "1.0.0", Some(true));
                        store.approve(&plugin, ORIGIN, DIGEST).expect("approved");
                    }
                })
            })
            .collect();
        for thread in threads {
            thread.join().expect("the thread finished");
        }
        let store = GrantStore::open(path.as_path());
        for thread in 0..2 {
            for n in 0..25 {
                assert!(
                    entry(&store, &format!("local/t{thread}-{n}")).is_some(),
                    "local/t{thread}-{n} was lost"
                );
            }
        }
    }
}
