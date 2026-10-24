#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
//! Plugin storage: the only code that opens a plugin's store and the only user of `rusqlite`
//! (`specs/004-plugin-storage/research.md`, R1, R15). `clippy.toml` forbids `rusqlite`
//! everywhere else, so reviewing this module reviews every SQL statement the host runs and every
//! hardening layer of the connection.
//!
//! A plugin's calls act only on the store its binding names, chosen when the plugin is linked
//! from its id and origin (R3); no call names a store. Each run of `render` or `update` gets a
//! [`StorageRun`], which answers the calls in the order of `data-model.md` (A storage call) and,
//! for an update, commits its writes only when the update succeeded and storage is still
//! granted (R7).

mod admin;
// Until the storage calls use it (tasks T021 and T047)
#[allow(dead_code)]
mod connection;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::calls::Phase;
use crate::error::{ErrorKind, RuntimeError};
use crate::grants::GrantStore;
use crate::limits::Limits;
use crate::manifest::PluginId;
use connection::{Identity, Mode, Settings, StoreConnection, StoreError};

pub use connection::engine_check;
#[cfg(feature = "test-network")]
pub use connection::{Foreign, HardeningReport, hardening_report, plant_foreign_store};

/// The directory of the stores, inside the data directory.
pub(crate) const STORAGE_DIR: &str = "storage";

/// Load step 8a (`data-model.md`, Load decision): `<data dir>/storage/`, created when missing,
/// canonical, and a directory the host can write, on the SQLite this host was reviewed with.
/// Otherwise why not, for `storage-failed` or `failed(unavailable)`.
pub(crate) fn storage_dir(data_dir: &Path) -> Result<PathBuf, String> {
    engine_check()?;
    let dir = data_dir.join(STORAGE_DIR);
    std::fs::create_dir_all(&dir)
        .map_err(|error| format!("{} cannot be created: {error}", dir.display()))?;
    let dir = std::fs::canonicalize(&dir)
        .map_err(|error| format!("{} has no canonical path: {error}", dir.display()))?;
    if !dir.is_dir() {
        return Err(format!("{} is not a directory", dir.display()));
    }
    // A file created and removed at once: the directory can hold a store
    tempfile::NamedTempFile::new_in(&dir)
        .map(drop)
        .map_err(|error| format!("{} cannot be written: {error}", dir.display()))?;
    Ok(dir)
}

/// The usage of the store of `id` from `origin` in `data_dir`, read without creating anything:
/// 0 when it has none yet. For `runtime permissions`.
pub(crate) fn used_bytes(
    data_dir: &Path,
    id: &str,
    origin: &str,
    limits: &Limits,
) -> Result<u64, String> {
    let file = data_dir.join(STORAGE_DIR).join(store_file_name(id, origin));
    if !file.exists() {
        return Ok(0);
    }
    let identity = Identity {
        id: id.to_owned(),
        origin: origin.to_owned(),
    };
    match connection::open(&file, Mode::Show, Some(&identity), &read_settings(limits)) {
        Ok(None) => Ok(0),
        Ok(Some(connection)) => connection.usage().map_err(|error| describe(&error)),
        Err(error) => Err(describe(&error)),
    }
}

/// The settings of a connection that only reads, outside a run: no file cap to speak of, and the
/// longest wait for another process.
fn read_settings(limits: &Limits) -> Settings {
    Settings {
        max_name_bytes: limits.max_storage_name_bytes,
        max_value_bytes: limits.max_storage_value_bytes,
        file_cap_bytes: 0,
        busy_timeout: limits.max_storage_busy_wait,
    }
}

/// Which store a linked plugin reaches, computed at link time and never from a call
/// (`data-model.md`, Store binding).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoreBinding {
    pub(crate) id: String,
    pub(crate) origin: String,
    /// `<data dir>/storage/<hex SHA-256 of "<id>\n<origin>">.sqlite`.
    pub(crate) file: PathBuf,
    /// The quota the manifest declares, which writes are checked against.
    pub(crate) quota_bytes: u64,
    /// The quota of the last approval, which sets the file cap.
    pub(crate) approved_quota_bytes: u64,
}

impl StoreBinding {
    /// The binding of `id` from `origin`, its file under `storage_dir`.
    pub(crate) fn new(
        storage_dir: &Path,
        id: &str,
        origin: &str,
        quota_bytes: u64,
        approved_quota_bytes: u64,
    ) -> StoreBinding {
        StoreBinding {
            id: id.to_owned(),
            origin: origin.to_owned(),
            file: storage_dir.join(store_file_name(id, origin)),
            quota_bytes,
            approved_quota_bytes,
        }
    }

    /// `factor × approved quota + extra`; the connection rounds it up to whole pages and never
    /// sets it below the file's current size (research R10).
    pub(crate) fn file_cap_bytes(&self, limits: &Limits) -> u64 {
        limits
            .storage_file_cap_factor
            .saturating_mul(self.approved_quota_bytes)
            .saturating_add(limits.storage_file_cap_extra_bytes)
    }

    fn identity(&self) -> Identity {
        Identity {
            id: self.id.clone(),
            origin: self.origin.clone(),
        }
    }
}

/// The file name of the store of `id` from `origin`.
pub(crate) fn store_file_name(id: &str, origin: &str) -> String {
    let hash: String = Sha256::digest(format!("{id}\n{origin}").as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    format!("{hash}.sqlite")
}

/// The rule behind a call that was refused or failed, as the contract names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Rule {
    RenderOnlyReads,
    OverQuota,
    OverLimit,
    InvalidName,
    Damaged,
    Busy,
    Unavailable,
    RunAborted,
}

/// What a storage call answers when it does not succeed, as the contract's `storage-error`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum CallError {
    NotGranted,
    Refused(Rule, String),
    Failed(Rule, String),
}

/// What a run did with storage, for its report.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StorageCounts {
    /// Every storage call of the run, refused ones included.
    pub calls: usize,
    /// Writes and deletes kept: those of an update that committed.
    pub writes_kept: usize,
    /// The store's usage after the run, in bytes.
    pub used_bytes: u64,
    /// The quota the manifest declares.
    pub quota_bytes: u64,
}

/// The limits a run applies, from [`Limits`].
struct RunLimits {
    entry_cost: u64,
    max_name: usize,
    max_value: usize,
    max_calls: usize,
    max_written: u64,
    page: usize,
    busy_wait: Duration,
    settings_file_cap: u64,
}

/// The run's store, opened on its first call that needs it.
enum Opened {
    NotYet,
    /// No store yet, in a run that does not create one: every read finds nothing.
    Empty,
    Store {
        connection: StoreConnection,
        /// The usage when the run opened the store, and now.
        initial_usage: u64,
        usage: u64,
    },
}

/// One run's storage: the real `storage` interface of a plugin granted storage
/// (`data-model.md`, Run state).
pub(crate) struct StorageRun {
    store: Result<Arc<StoreBinding>, String>,
    /// The plugin, for the grant check of every call; `None` when the store is unavailable.
    plugin: Option<PluginId>,
    phase: Phase,
    grants: Arc<GrantStore>,
    limits: RunLimits,
    deadline: Instant,
    opened: Opened,
    calls: usize,
    written_bytes: u64,
    writes: usize,
    /// Set when a call found storage no longer granted: every later call answers
    /// `not-granted`, and nothing the run wrote is kept.
    taken_away: bool,
    /// The failure that ended the run's transaction, when SQLite ended it (research R7).
    aborted: Option<String>,
}

impl StorageRun {
    /// The storage of one run of `phase` on `store`, or on no store when the data directory is
    /// unavailable (every call then fails as `unavailable`, naming why). `grants` is read again
    /// on every call; `deadline` bounds how long a call waits for another process.
    pub(crate) fn new(
        store: Result<Arc<StoreBinding>, String>,
        phase: Phase,
        grants: Arc<GrantStore>,
        limits: &Limits,
        deadline: Instant,
    ) -> StorageRun {
        let plugin = store
            .as_ref()
            .ok()
            .and_then(|binding| PluginId::parse(&binding.id).ok());
        let settings_file_cap = store
            .as_ref()
            .map_or(0, |binding| binding.file_cap_bytes(limits));
        StorageRun {
            store,
            plugin,
            phase,
            grants,
            limits: RunLimits {
                entry_cost: limits.storage_entry_cost_bytes,
                max_name: limits.max_storage_name_bytes,
                max_value: limits.max_storage_value_bytes,
                max_calls: limits.max_storage_calls_per_run,
                max_written: limits.max_storage_written_bytes_per_update,
                page: limits.storage_list_page,
                busy_wait: limits.max_storage_busy_wait,
                settings_file_cap,
            },
            deadline,
            opened: Opened::NotYet,
            calls: 0,
            written_bytes: 0,
            writes: 0,
            taken_away: false,
            aborted: None,
        }
    }

    pub(crate) fn get(&mut self, name: &str) -> Result<Option<Vec<u8>>, CallError> {
        self.begin(false)?;
        self.check_name(name, "the name")?;
        match self.open()? {
            None => Ok(None),
            Some(connection) => connection
                .get(name.as_bytes())
                .map_err(|error| self.statement_failed(error)),
        }
    }

    pub(crate) fn set(&mut self, name: &str, value: &[u8]) -> Result<(), CallError> {
        self.begin(true)?;
        self.check_name(name, "the name")?;
        if value.len() > self.limits.max_value {
            return Err(CallError::Refused(
                Rule::OverLimit,
                format!(
                    "the value is {} bytes, over the limit of {}",
                    value.len(),
                    self.limits.max_value
                ),
            ));
        }
        let size = (name.len() + value.len()) as u64;
        if self.written_bytes + size > self.limits.max_written {
            return Err(CallError::Refused(
                Rule::OverLimit,
                format!(
                    "this update would write {} bytes of names and values, over the limit of {}",
                    self.written_bytes + size,
                    self.limits.max_written
                ),
            ));
        }
        let quota = self.quota();
        let entry_cost = self.limits.entry_cost;
        let Some(connection) = self.open()? else {
            unreachable!("an update always has a store")
        };
        let old = connection
            .get(name.as_bytes())
            .map_err(|error| self.statement_failed(error))?;
        let usage = self.usage_now();
        let old_entry = old.map_or(0, |old| (name.len() + old.len()) as u64 + entry_cost);
        let after = usage - old_entry + size + entry_cost;
        if after > quota {
            return Err(CallError::Refused(
                Rule::OverQuota,
                format!(
                    "the store's usage would be {after} bytes, over its quota of {quota} (now {usage})"
                ),
            ));
        }
        self.write(|connection| {
            connection.put(name.as_bytes(), value)?;
            connection.set_usage(after)
        })?;
        self.set_usage_now(after);
        self.written_bytes += size;
        Ok(())
    }

    pub(crate) fn delete(&mut self, name: &str) -> Result<(), CallError> {
        self.begin(true)?;
        self.check_name(name, "the name")?;
        let entry_cost = self.limits.entry_cost;
        let Some(connection) = self.open()? else {
            unreachable!("an update always has a store")
        };
        let old = connection
            .get(name.as_bytes())
            .map_err(|error| self.statement_failed(error))?;
        let Some(old) = old else {
            // Removing a name that is not there succeeds and changes nothing
            self.writes += 1;
            return Ok(());
        };
        let after = self.usage_now() - ((name.len() + old.len()) as u64 + entry_cost);
        self.write(|connection| {
            connection.remove(name.as_bytes())?;
            connection.set_usage(after)
        })?;
        self.set_usage_now(after);
        Ok(())
    }

    /// One page of the names that start with `prefix` and come after `after`, and the name to
    /// pass as `after` for the next page when more remain.
    pub(crate) fn list(
        &mut self,
        prefix: &str,
        after: Option<&str>,
    ) -> Result<(Vec<String>, Option<String>), CallError> {
        self.begin(false)?;
        self.check_length(prefix, "the prefix")?;
        if let Some(after) = after {
            self.check_length(after, "after")?;
        }
        let page = self.limits.page;
        let Some(connection) = self.open()? else {
            return Ok((Vec::new(), None));
        };
        let (names, more) = connection
            .list(prefix.as_bytes(), after.map(str::as_bytes), page)
            .map_err(|error| self.statement_failed(error))?;
        let names = names
            .into_iter()
            .map(String::from_utf8)
            .collect::<Result<Vec<String>, _>>()
            .map_err(|_| {
                CallError::Failed(
                    Rule::Damaged,
                    "the store holds a name that is not UTF-8, which the host never writes"
                        .to_owned(),
                )
            })?;
        let next = if more { names.last().cloned() } else { None };
        Ok((names, next))
    }

    /// The store's usage and the declared quota.
    pub(crate) fn usage(&mut self) -> Result<(u64, u64), CallError> {
        self.begin(false)?;
        let quota = self.quota();
        Ok(match self.open()? {
            None => (0, quota),
            Some(_) => (self.usage_now(), quota),
        })
    }

    /// Ends the run. An update's writes are committed only when `succeeded` (the update
    /// returned `ok` with valid output), the run was not aborted and storage is still granted,
    /// checked under the grant record's lock until the commit ends; otherwise they are rolled
    /// back. An aborted run or a failed commit is `storage-failed`, and storage taken away
    /// `not-granted`, both naming why.
    pub(crate) fn finish(self, succeeded: bool) -> Result<StorageCounts, RuntimeError> {
        let counts = |writes_kept, used_bytes| StorageCounts {
            calls: self.calls,
            writes_kept,
            used_bytes,
            quota_bytes: self.quota(),
        };
        let Opened::Store {
            connection,
            initial_usage,
            usage,
        } = &self.opened
        else {
            return Ok(counts(0, self.usage_without_opening()));
        };
        if self.phase == Phase::Render {
            // Dropping the connection ends the read transaction
            return Ok(counts(0, *usage));
        }
        let not_kept = "nothing this update wrote was kept";
        // An aborted run fails as such even when the update then failed too: its first failure
        // is what ended its writes
        if let Some(first) = &self.aborted {
            let _ = connection.rollback();
            return Err(RuntimeError::new(
                ErrorKind::StorageFailed,
                format!("{first}; {not_kept}"),
            ));
        }
        if !succeeded {
            let _ = connection.rollback();
            return Ok(counts(0, *initial_usage));
        }
        let taken_away = || {
            RuntimeError::new(
                ErrorKind::NotGranted,
                format!(
                    "storage was turned off or its approval removed during the update; {not_kept}"
                ),
            )
        };
        if self.taken_away {
            let _ = connection.rollback();
            return Err(taken_away());
        }
        let (Ok(binding), Some(plugin)) = (&self.store, &self.plugin) else {
            unreachable!("a store was opened, so the binding is there")
        };
        // The grant record's shared lock is held through the commit: a removal or a turn-off
        // waits for it, and one made before is seen here (research R7)
        match self
            .grants
            .while_granted(plugin, &binding.origin, STORAGE, || connection.commit())
        {
            Ok(Some(Ok(()))) => Ok(counts(self.writes, *usage)),
            Ok(Some(Err(error))) => {
                let _ = connection.rollback();
                Err(RuntimeError::new(
                    ErrorKind::StorageFailed,
                    format!(
                        "the update's writes could not be committed, {}; {not_kept}",
                        describe(&error)
                    ),
                ))
            }
            Ok(None) | Err(_) => {
                let _ = connection.rollback();
                Err(taken_away())
            }
        }
    }

    /// Steps 1 to 4 of a call (`data-model.md`, A storage call): counted first, then storage
    /// still granted, then the run not aborted, then writes only in update.
    fn begin(&mut self, writes: bool) -> Result<(), CallError> {
        self.calls += 1;
        if self.calls > self.limits.max_calls {
            return Err(CallError::Refused(
                Rule::OverLimit,
                format!(
                    "more than {} storage calls in one run",
                    self.limits.max_calls
                ),
            ));
        }
        if self.taken_away || !self.still_granted() {
            self.taken_away = true;
            return Err(CallError::NotGranted);
        }
        if let Some(first) = &self.aborted {
            return Err(CallError::Failed(
                Rule::RunAborted,
                format!(
                    "an earlier call of this update ended its writes ({first}); nothing it wrote will be kept"
                ),
            ));
        }
        if writes && self.phase == Phase::Render {
            return Err(CallError::Refused(
                Rule::RenderOnlyReads,
                "render only reads storage; write it in update".to_owned(),
            ));
        }
        Ok(())
    }

    /// The grant record as it is now: a change by any process applies from the next call.
    fn still_granted(&self) -> bool {
        match (&self.store, &self.plugin) {
            (Ok(binding), Some(plugin)) => self
                .grants
                .permission_granted(plugin, &binding.origin, STORAGE)
                .unwrap_or(false),
            // No store to reach: every call fails as unavailable anyway
            _ => true,
        }
    }

    fn check_name(&self, name: &str, what: &str) -> Result<(), CallError> {
        if name.is_empty() {
            return Err(CallError::Refused(
                Rule::InvalidName,
                format!("{what} is empty"),
            ));
        }
        self.check_length(name, what)
    }

    fn check_length(&self, text: &str, what: &str) -> Result<(), CallError> {
        if text.len() > self.limits.max_name {
            return Err(CallError::Refused(
                Rule::InvalidName,
                format!(
                    "{what} is {} bytes in UTF-8, over the limit of {}",
                    text.len(),
                    self.limits.max_name
                ),
            ));
        }
        Ok(())
    }

    fn quota(&self) -> u64 {
        self.store.as_ref().map_or(0, |binding| binding.quota_bytes)
    }

    fn usage_now(&self) -> u64 {
        match &self.opened {
            Opened::Store { usage, .. } => *usage,
            Opened::NotYet | Opened::Empty => 0,
        }
    }

    fn set_usage_now(&mut self, now: u64) {
        if let Opened::Store { usage, .. } = &mut self.opened {
            *usage = now;
        }
    }

    /// The usage of a store the run never opened, for its report: read with a read-only
    /// connection, or 0.
    fn usage_without_opening(&self) -> u64 {
        let Ok(binding) = &self.store else {
            return 0;
        };
        connection::open(
            &binding.file,
            Mode::Show,
            Some(&binding.identity()),
            &self.settings(),
        )
        .ok()
        .flatten()
        .and_then(|connection| connection.usage().ok())
        .unwrap_or(0)
    }

    fn settings(&self) -> Settings {
        Settings {
            max_name_bytes: self.limits.max_name,
            max_value_bytes: self.limits.max_value,
            file_cap_bytes: self.limits.settings_file_cap,
            busy_timeout: self
                .limits
                .busy_wait
                .min(self.deadline.saturating_duration_since(Instant::now())),
        }
    }

    /// Step 6: the store, opened on the first call that needs it; `None` when there is none
    /// yet in a run that does not create one.
    fn open(&mut self) -> Result<Option<&StoreConnection>, CallError> {
        if let Opened::NotYet = self.opened {
            let binding = match &self.store {
                Ok(binding) => binding.clone(),
                Err(reason) => {
                    return Err(CallError::Failed(
                        Rule::Unavailable,
                        format!("storage is unavailable: {reason}"),
                    ));
                }
            };
            let mode = match self.phase {
                Phase::Render => Mode::Render,
                Phase::Update => Mode::Update,
            };
            let opened = connection::open(
                &binding.file,
                mode,
                Some(&binding.identity()),
                &self.settings(),
            )
            .map_err(call_error)?;
            self.opened = match opened {
                None => Opened::Empty,
                Some(connection) => {
                    let usage = connection.usage().map_err(call_error)?;
                    Opened::Store {
                        connection,
                        initial_usage: usage,
                        usage,
                    }
                }
            };
        }
        Ok(match &self.opened {
            Opened::Store { connection, .. } => Some(connection),
            Opened::NotYet | Opened::Empty => None,
        })
    }

    /// Step 8: runs a write; when it fails and SQLite ended the transaction, the run is aborted.
    fn write(
        &mut self,
        statements: impl FnOnce(&StoreConnection) -> Result<(), StoreError>,
    ) -> Result<(), CallError> {
        let Opened::Store { connection, .. } = &self.opened else {
            unreachable!("a write runs on an open store")
        };
        match statements(connection) {
            Ok(()) => {
                self.writes += 1;
                Ok(())
            }
            Err(error) => Err(self.statement_failed(error)),
        }
    }

    /// The answer to a statement that failed. In an update whose transaction SQLite ended (a
    /// full file, an I/O error), every later call answers `run-aborted` and nothing is kept.
    fn statement_failed(&mut self, error: StoreError) -> CallError {
        let ended = matches!(
            &self.opened,
            Opened::Store { connection, .. } if connection.transaction_ended()
        );
        if self.phase == Phase::Update && ended && self.aborted.is_none() {
            self.aborted = Some(describe(&error));
        }
        call_error(error)
    }
}

/// The permission that carries a store.
const STORAGE: &str = "storage";

fn describe(error: &StoreError) -> String {
    match error {
        StoreError::Damaged(detail) => format!("the store is damaged: {detail}"),
        StoreError::Busy(detail) => format!("the store is busy: {detail}"),
        StoreError::Unavailable(detail) => format!("the store is unavailable: {detail}"),
        StoreError::FileCap(detail) => detail.clone(),
    }
}

fn call_error(error: StoreError) -> CallError {
    let rule = match &error {
        StoreError::Damaged(_) => Rule::Damaged,
        StoreError::Busy(_) => Rule::Busy,
        StoreError::Unavailable(_) => Rule::Unavailable,
        StoreError::FileCap(_) => Rule::OverLimit,
    };
    CallError::Failed(rule, describe(&error))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use serde_json::json;
    use tempfile::TempDir;

    use super::*;
    use crate::grants;
    use crate::manifest::Manifest;

    const ID: &str = "local/probe";
    const ORIGIN: &str = "/plugins/probe.wasm";
    const DIGEST: &str = "sha256:00";
    const QUOTA: u64 = 64 * 1024;

    fn manifest(required: bool, quota_bytes: u64) -> Manifest {
        let manifest = json!({
            "manifest_version": 1, "id": ID, "version": "0.1.0", "author": "A",
            "permissions": [{ "name": "storage", "required": required, "quota_bytes": quota_bytes }],
            "login_providers": []
        });
        Manifest::parse(&serde_json::to_vec(&manifest).unwrap()).unwrap()
    }

    /// A data directory, a grant record file approving storage, and the plugin's binding.
    struct Setup {
        _dir: TempDir,
        grants: Arc<GrantStore>,
        binding: Arc<StoreBinding>,
        limits: Limits,
    }

    impl Setup {
        fn new(quota: u64) -> Setup {
            Setup::with_limits(quota, Limits::default())
        }

        fn with_limits(quota: u64, limits: Limits) -> Setup {
            let dir = TempDir::new().unwrap();
            let grants = Arc::new(GrantStore::open(dir.path().join("grants.json")));
            let manifest = manifest(true, quota);
            let expected = grants::approval_digest(DIGEST, &manifest, &[]);
            grants
                .approve_connected(&manifest, ORIGIN, DIGEST, &[], &expected)
                .unwrap();
            let storage = dir.path().join(STORAGE_DIR);
            fs::create_dir(&storage).unwrap();
            let binding = Arc::new(StoreBinding::new(&storage, ID, ORIGIN, quota, quota));
            Setup {
                _dir: dir,
                grants,
                binding,
                limits,
            }
        }

        fn run(&self, phase: Phase) -> StorageRun {
            StorageRun::new(
                Ok(self.binding.clone()),
                phase,
                self.grants.clone(),
                &self.limits,
                Instant::now() + Duration::from_secs(10),
            )
        }

        /// Runs an update that applies `writes` and commits.
        fn committed(&self, writes: impl FnOnce(&mut StorageRun)) -> StorageCounts {
            let mut run = self.run(Phase::Update);
            writes(&mut run);
            run.finish(true).expect("committed")
        }

        fn read(&self, name: &str) -> Option<Vec<u8>> {
            self.run(Phase::Render).get(name).unwrap()
        }

        fn file_digest(&self) -> Option<Vec<u8>> {
            fs::read(&self.binding.file)
                .ok()
                .map(|bytes| Sha256::digest(bytes).to_vec())
        }
    }

    fn refused(rule: Rule) -> impl Fn(&CallError) -> bool {
        move |error| matches!(error, CallError::Refused(found, _) if *found == rule)
    }

    fn failed(rule: Rule) -> impl Fn(&CallError) -> bool {
        move |error| matches!(error, CallError::Failed(found, _) if *found == rule)
    }

    #[test]
    fn the_binding_is_the_file_of_the_id_and_origin() {
        let storage = Path::new("/data/storage");
        let binding = StoreBinding::new(storage, ID, ORIGIN, QUOTA, QUOTA);
        let hash: String = Sha256::digest(format!("{ID}\n{ORIGIN}").as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        assert_eq!(binding.file, storage.join(format!("{hash}.sqlite")));
        assert_eq!(store_file_name(ID, ORIGIN), format!("{hash}.sqlite"));
        assert_eq!(binding.id, ID);
        assert_eq!(binding.origin, ORIGIN);
        // Another origin, or another id, is another file
        assert_ne!(
            StoreBinding::new(storage, ID, "/elsewhere.wasm", QUOTA, QUOTA).file,
            binding.file
        );
        assert_ne!(
            StoreBinding::new(storage, "local/other", ORIGIN, QUOTA, QUOTA).file,
            binding.file
        );
    }

    #[test]
    fn the_file_cap_follows_the_approved_quota() {
        let limits = Limits::default();
        let binding = StoreBinding::new(Path::new("/data/storage"), ID, ORIGIN, 4096, 65536);
        assert_eq!(binding.file_cap_bytes(&limits), 5 * 65536 + 64 * 1024);
    }

    #[test]
    fn values_written_in_an_update_are_read_back() {
        let setup = Setup::new(QUOTA);
        let counts = setup.committed(|run| {
            run.set("count", b"1").unwrap();
            run.set("empty", b"").unwrap();
            assert_eq!(
                run.get("count").unwrap(),
                Some(b"1".to_vec()),
                "reads its own writes"
            );
            run.set("count", b"2").unwrap();
            run.delete("never written").unwrap();
        });
        assert_eq!(counts.writes_kept, 4);
        assert_eq!(counts.calls, 5);
        assert_eq!(
            setup.read("count"),
            Some(b"2".to_vec()),
            "the last write is kept"
        );
        assert_eq!(setup.read("empty"), Some(Vec::new()));
        assert_eq!(setup.read("missing"), None);
        // Usage: names, values and 64 per entry
        let expected = (5 + 1 + 64) + (5 + 64);
        assert_eq!(counts.used_bytes, expected);
        assert_eq!(setup.run(Phase::Render).usage().unwrap(), (expected, QUOTA));
    }

    #[test]
    fn a_render_without_a_store_reads_it_as_empty_and_creates_nothing() {
        let setup = Setup::new(QUOTA);
        let mut run = setup.run(Phase::Render);
        assert_eq!(run.get("x").unwrap(), None);
        assert_eq!(run.list("", None).unwrap(), (Vec::new(), None));
        assert_eq!(run.usage().unwrap(), (0, QUOTA));
        run.finish(true).unwrap();
        assert!(!setup.binding.file.exists());
    }

    #[test]
    fn render_only_reads() {
        let setup = Setup::new(QUOTA);
        setup.committed(|run| run.set("a", b"1").unwrap());
        let before = setup.file_digest();
        let mut run = setup.run(Phase::Render);
        assert!(refused(Rule::RenderOnlyReads)(
            &run.set("a", b"2").unwrap_err()
        ));
        assert!(refused(Rule::RenderOnlyReads)(
            &run.delete("a").unwrap_err()
        ));
        assert_eq!(run.get("a").unwrap(), Some(b"1".to_vec()));
        run.finish(true).unwrap();
        assert_eq!(setup.file_digest(), before);
    }

    #[test]
    fn render_reads_one_view_for_the_whole_run() {
        let setup = Setup::new(QUOTA);
        setup.committed(|run| run.set("a", b"1").unwrap());
        let mut render = setup.run(Phase::Render);
        assert_eq!(render.get("a").unwrap(), Some(b"1".to_vec()));
        // An update elsewhere cannot commit while render reads; render still sees its view
        let mut update = StorageRun::new(
            Ok(setup.binding.clone()),
            Phase::Update,
            setup.grants.clone(),
            &setup.limits,
            Instant::now() + Duration::from_millis(300),
        );
        update.set("a", b"2").unwrap();
        let error = update.finish(true).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::StorageFailed, "{error}");
        assert!(error.to_string().contains("busy"), "{error}");
        assert_eq!(render.get("a").unwrap(), Some(b"1".to_vec()));
    }

    #[test]
    fn nothing_is_kept_when_the_update_fails() {
        let setup = Setup::new(QUOTA);
        setup.committed(|run| run.set("a", b"1").unwrap());
        let before = setup.file_digest();
        let mut run = setup.run(Phase::Update);
        run.set("a", b"2").unwrap();
        run.set("b", b"3").unwrap();
        let counts = run.finish(false).unwrap();
        assert_eq!(counts.writes_kept, 0);
        assert_eq!(setup.read("a"), Some(b"1".to_vec()));
        assert_eq!(setup.read("b"), None);
        assert_eq!(setup.file_digest(), before);
    }

    #[test]
    fn every_call_counts_and_the_limit_comes_first() {
        let limits = Limits {
            max_storage_calls_per_run: 3,
            ..Limits::default()
        };
        let setup = Setup::with_limits(QUOTA, limits);
        let mut run = setup.run(Phase::Render);
        assert!(refused(Rule::RenderOnlyReads)(
            &run.set("a", b"").unwrap_err()
        ));
        run.get("a").unwrap();
        assert!(refused(Rule::InvalidName)(&run.get("").unwrap_err()));
        // Call 4: over the limit, whatever else is wrong with it
        let error = run.set("", b"").unwrap_err();
        assert!(refused(Rule::OverLimit)(&error), "{error:?}");
        assert!(refused(Rule::OverLimit)(&run.usage().unwrap_err()));
        assert_eq!(run.finish(true).unwrap().calls, 5);
    }

    #[test]
    fn storage_taken_away_answers_not_granted_from_the_next_call() {
        let setup = Setup::new(QUOTA);
        let mut run = setup.run(Phase::Update);
        run.set("a", b"1").unwrap();
        // Another process removes the approval
        GrantStore::open(setup.grants.path().unwrap())
            .remove(&PluginId::parse(ID).unwrap())
            .unwrap();
        assert_eq!(run.get("a").unwrap_err(), CallError::NotGranted);
        // Approved again: still not granted for this run
        let manifest = manifest(true, QUOTA);
        let expected = grants::approval_digest(DIGEST, &manifest, &[]);
        setup
            .grants
            .approve_connected(&manifest, ORIGIN, DIGEST, &[], &expected)
            .unwrap();
        assert_eq!(run.set("b", b"2").unwrap_err(), CallError::NotGranted);
        let error = run.finish(true).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::NotGranted, "{error}");
        assert!(error.to_string().contains("storage"), "{error}");
        assert_eq!(setup.read("a"), None, "nothing of the update is kept");
    }

    #[test]
    fn storage_taken_away_after_the_last_call_is_caught_at_the_commit() {
        let setup = Setup::new(QUOTA);
        let mut run = setup.run(Phase::Update);
        run.set("a", b"1").unwrap();
        GrantStore::open(setup.grants.path().unwrap())
            .remove(&PluginId::parse(ID).unwrap())
            .unwrap();
        let error = run.finish(true).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::NotGranted, "{error}");
        let manifest = manifest(true, QUOTA);
        let expected = grants::approval_digest(DIGEST, &manifest, &[]);
        setup
            .grants
            .approve_connected(&manifest, ORIGIN, DIGEST, &[], &expected)
            .unwrap();
        assert_eq!(setup.read("a"), None);
    }

    #[test]
    fn names_and_values_have_limits() {
        let setup = Setup::new(8 * 1024 * 1024);
        let mut run = setup.run(Phase::Update);
        let name = "n".repeat(256);
        run.set(&name, &vec![b'v'; 256 * 1024]).unwrap();
        assert!(refused(Rule::InvalidName)(&run.set("", b"").unwrap_err()));
        assert!(refused(Rule::InvalidName)(
            &run.set(&"n".repeat(257), b"").unwrap_err()
        ));
        // 256 bytes of UTF-8, not 256 characters
        assert!(refused(Rule::InvalidName)(
            &run.get(&"é".repeat(129)).unwrap_err()
        ));
        assert!(refused(Rule::OverLimit)(
            &run.set("v", &vec![0; 256 * 1024 + 1]).unwrap_err()
        ));
        assert!(refused(Rule::InvalidName)(
            &run.list(&"p".repeat(257), None).unwrap_err()
        ));
        assert!(refused(Rule::InvalidName)(
            &run.list("", Some(&"a".repeat(257))).unwrap_err()
        ));
        run.list("", Some("")).unwrap();
        run.finish(true).unwrap();
    }

    #[test]
    fn the_bytes_written_in_one_update_are_bounded() {
        let setup = Setup::new(8 * 1024 * 1024);
        let mut run = setup.run(Phase::Update);
        let value = vec![0; 256 * 1024];
        for index in 0..3 {
            run.set(&format!("v{index}"), &value).unwrap();
        }
        // The fourth value of 256 KiB would pass 1 MiB with the names
        let error = run.set("v3", &value).unwrap_err();
        assert!(refused(Rule::OverLimit)(&error), "{error:?}");
        // Replacing counts too
        let error = run.set("v0", &value).unwrap_err();
        assert!(refused(Rule::OverLimit)(&error), "{error:?}");
        run.set("small", b"x").unwrap();
        run.finish(true).unwrap();
    }

    #[test]
    fn a_write_past_the_quota_is_refused_and_not_kept() {
        let setup = Setup::new(1000);
        let mut run = setup.run(Phase::Update);
        run.set("a", &[0; 400]).unwrap(); // 465
        run.set("b", &[0; 400]).unwrap(); // 930
        let error = run.set("c", &[0; 10]).unwrap_err();
        let CallError::Refused(Rule::OverQuota, detail) = &error else {
            panic!("{error:?}");
        };
        assert!(
            detail.contains("1000") && detail.contains("1005"),
            "{detail}"
        );
        assert_eq!(run.get("c").unwrap(), None);
        // Replacing a value counts the old entry as gone
        run.set("a", &[0; 470]).unwrap(); // 930 - 465 + 535 = 1000
        assert!(refused(Rule::OverQuota)(
            &run.set("a", &[0; 471]).unwrap_err()
        ));
        let counts = run.finish(true).unwrap();
        assert_eq!(counts.used_bytes, 1000);
    }

    #[test]
    fn over_a_lowered_quota_deletes_pass_and_writes_must_fit() {
        let setup = Setup::new(1000);
        setup.committed(|run| {
            run.set("a", &[0; 400]).unwrap();
            run.set("b", &[0; 400]).unwrap();
        });
        // A new version lowers its quota to 500; the approved quota stays 1000
        let lowered = Arc::new(StoreBinding::new(
            setup.binding.file.parent().unwrap(),
            ID,
            ORIGIN,
            500,
            1000,
        ));
        let mut run = StorageRun::new(
            Ok(lowered),
            Phase::Update,
            setup.grants.clone(),
            &setup.limits,
            Instant::now() + Duration::from_secs(10),
        );
        assert_eq!(run.usage().unwrap(), (930, 500));
        assert!(refused(Rule::OverQuota)(&run.set("c", b"").unwrap_err()));
        // Still over after the write: refused, even though it shrinks the store
        assert!(refused(Rule::OverQuota)(
            &run.set("a", &[0; 300]).unwrap_err()
        ));
        run.delete("b").unwrap();
        run.set("a", &[0; 300]).unwrap();
        assert_eq!(run.finish(true).unwrap().used_bytes, 365);
    }

    #[test]
    fn a_write_that_reaches_the_file_cap_ends_the_run() {
        let limits = Limits {
            storage_file_cap_factor: 1,
            storage_file_cap_extra_bytes: 0,
            ..Limits::default()
        };
        let setup = Setup::with_limits(64 * 1024, limits);
        setup.committed(|run| run.set("kept", b"before").unwrap());
        let before = setup.file_digest();
        for succeeded in [true, false] {
            let mut run = setup.run(Phase::Update);
            let mut answer = Ok(());
            for index in 0..64 {
                answer = run.set(&format!("v{index}"), &[1; 900]);
                if answer.is_err() {
                    break;
                }
            }
            let error = answer.expect_err("the file cap stops the writes");
            assert!(failed(Rule::OverLimit)(&error), "{error:?}");
            assert!(failed(Rule::RunAborted)(&run.get("kept").unwrap_err()));
            assert!(failed(Rule::RunAborted)(&run.set("x", b"").unwrap_err()));
            let error = run.finish(succeeded).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::StorageFailed, "{error}");
            assert_eq!(
                setup.file_digest(),
                before,
                "nothing of the update was kept"
            );
        }
        assert_eq!(setup.read("kept"), Some(b"before".to_vec()));
    }

    #[test]
    fn a_damaged_store_fails_its_calls_and_is_left_alone() {
        let setup = Setup::new(QUOTA);
        fs::write(&setup.binding.file, b"not a database, but somebody's file").unwrap();
        let before = setup.file_digest();
        for phase in [Phase::Render, Phase::Update] {
            let mut run = setup.run(phase);
            assert!(failed(Rule::Damaged)(&run.get("a").unwrap_err()));
            let _ = run.finish(true);
        }
        assert_eq!(setup.file_digest(), before);
    }

    #[test]
    fn an_unavailable_data_directory_fails_every_call() {
        let setup = Setup::new(QUOTA);
        let mut run = StorageRun::new(
            Err("the data directory cannot be written".to_owned()),
            Phase::Update,
            setup.grants.clone(),
            &setup.limits,
            Instant::now() + Duration::from_secs(10),
        );
        let error = run.get("a").unwrap_err();
        let CallError::Failed(Rule::Unavailable, detail) = &error else {
            panic!("{error:?}");
        };
        assert!(detail.contains("cannot be written"), "{detail}");
        assert!(failed(Rule::Unavailable)(&run.set("a", b"").unwrap_err()));
        run.finish(true).unwrap();
    }

    #[test]
    fn list_pages_through_names_in_byte_order() {
        let setup = Setup::new(1024 * 1024);
        setup.committed(|run| {
            for index in 0..250 {
                run.set(&format!("notes/{index:03}"), b"").unwrap();
            }
            run.set("other", b"").unwrap();
        });
        let mut run = setup.run(Phase::Render);
        let mut names = Vec::new();
        let mut after = None;
        let mut pages = 0;
        loop {
            let (page, next) = run.list("notes/", after.as_deref()).unwrap();
            assert!(page.len() <= 100);
            names.extend(page);
            pages += 1;
            match next {
                Some(next) => after = Some(next),
                None => break,
            }
        }
        assert_eq!(pages, 3);
        let expected: Vec<String> = (0..250).map(|index| format!("notes/{index:03}")).collect();
        assert_eq!(names, expected);
    }
}
