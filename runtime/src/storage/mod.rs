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
use std::time::Instant;

use crate::calls::Phase;
use crate::error::RuntimeError;
use crate::grants::GrantStore;
use crate::limits::Limits;

pub use connection::engine_check;

/// The directory of the stores, inside the data directory.
pub(crate) const STORAGE_DIR: &str = "storage";

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
        let _ = (storage_dir, id, origin, quota_bytes, approved_quota_bytes);
        StoreBinding {
            id: String::new(),
            origin: String::new(),
            file: PathBuf::new(),
            quota_bytes: 0,
            approved_quota_bytes: 0,
        }
    }

    /// `factor × approved quota + extra`; the connection rounds it up to whole pages and never
    /// sets it below the file's current size (research R10).
    pub(crate) fn file_cap_bytes(&self, limits: &Limits) -> u64 {
        let _ = limits;
        0
    }
}

/// The file name of the store of `id` from `origin`.
pub(crate) fn store_file_name(id: &str, origin: &str) -> String {
    let _ = (id, origin);
    String::new()
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

/// One run's storage: the real `storage` interface of a plugin granted storage
/// (`data-model.md`, Run state).
pub(crate) struct StorageRun {
    _private: (),
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
        let _ = (store, phase, grants, limits, deadline);
        StorageRun { _private: () }
    }

    pub(crate) fn get(&mut self, name: &str) -> Result<Option<Vec<u8>>, CallError> {
        let _ = name;
        Err(CallError::NotGranted)
    }

    pub(crate) fn set(&mut self, name: &str, value: &[u8]) -> Result<(), CallError> {
        let _ = (name, value);
        Err(CallError::NotGranted)
    }

    pub(crate) fn delete(&mut self, name: &str) -> Result<(), CallError> {
        let _ = name;
        Err(CallError::NotGranted)
    }

    /// One page of the names that start with `prefix` and come after `after`, and the name to
    /// pass as `after` for the next page when more remain.
    pub(crate) fn list(
        &mut self,
        prefix: &str,
        after: Option<&str>,
    ) -> Result<(Vec<String>, Option<String>), CallError> {
        let _ = (prefix, after);
        Err(CallError::NotGranted)
    }

    /// The store's usage and the declared quota.
    pub(crate) fn usage(&mut self) -> Result<(u64, u64), CallError> {
        Err(CallError::NotGranted)
    }

    /// Ends the run. An update's writes are committed only when `succeeded` (the update
    /// returned `ok` with valid output), the run was not aborted and storage is still granted,
    /// checked under the grant record's lock until the commit ends; otherwise they are rolled
    /// back. An aborted run or a failed commit is `storage-failed`, and storage taken away
    /// `not-granted`, both naming why.
    pub(crate) fn finish(self, succeeded: bool) -> Result<StorageCounts, RuntimeError> {
        let _ = succeeded;
        Ok(StorageCounts::default())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use serde_json::json;
    use sha2::{Digest, Sha256};
    use tempfile::TempDir;

    use super::*;
    use crate::error::ErrorKind;
    use crate::grants;
    use crate::manifest::{Manifest, PluginId};

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
        dir: TempDir,
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
                dir,
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
