//! What the development CLI does with stores besides a plugin's own calls: list, show, clear
//! (`specs/004-plugin-storage/research.md`, R14). Only the host's own controls reach a store
//! this way (spec FR-014); every connection is hardened as a plugin's is.

use std::path::{Path, PathBuf};

use super::connection::{Entry, Identity, StoreError};
use crate::limits::Limits;

/// One store found in the storage directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreInfo {
    pub file: PathBuf,
    /// The id and origin the store records, when it can be read.
    pub identity: Option<(String, String)>,
    /// Its usage, or why it is damaged.
    pub state: StoreState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StoreState {
    Used(u64),
    Damaged(String),
}

/// What a store holds, for `runtime storage show`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreView {
    pub used_bytes: u64,
    /// Every entry, in byte order of names.
    pub entries: Vec<Entry>,
}

/// Every `*.sqlite` file of `storage_dir`, with what it records, sorted by id then origin; a
/// file whose name is not the hash of the id and origin inside it is damaged.
pub(crate) fn list(storage_dir: &Path, limits: &Limits) -> Vec<StoreInfo> {
    let _ = (storage_dir, limits);
    Vec::new()
}

/// What the store at `file` holds, read-only; `None` when there is no store yet.
pub(crate) fn show(
    file: &Path,
    identity: &Identity,
    limits: &Limits,
) -> Result<Option<StoreView>, StoreError> {
    let _ = (file, identity, limits);
    Err(StoreError::Unavailable("not implemented".to_owned()))
}

/// Empties the store at `file` in one exclusive transaction and returns the usage it had; `None`
/// when there is no store. The file keeps its size: later writes reuse its pages.
pub(crate) fn clear(
    file: &Path,
    identity: &Identity,
    limits: &Limits,
) -> Result<Option<u64>, StoreError> {
    let _ = (file, identity, limits);
    Err(StoreError::Unavailable("not implemented".to_owned()))
}

/// Deletes the store file at `file`, damaged or orphaned, once no other process holds it.
pub(crate) fn remove(file: &Path, limits: &Limits) -> Result<(), StoreError> {
    let _ = (file, limits);
    Err(StoreError::Unavailable("not implemented".to_owned()))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::Duration;

    use tempfile::TempDir;

    use super::*;
    use crate::storage::connection::{self, Mode, Settings};
    use crate::storage::store_file_name;

    fn identity(id: &str, origin: &str) -> Identity {
        Identity {
            id: id.to_owned(),
            origin: origin.to_owned(),
        }
    }

    fn settings() -> Settings {
        Settings {
            max_name_bytes: 256,
            max_value_bytes: 256 * 1024,
            file_cap_bytes: 1024 * 1024,
            busy_timeout: Duration::from_millis(200),
        }
    }

    /// A store of `id` from `origin` in `dir`, holding `entries`.
    fn store(dir: &Path, id: &str, origin: &str, entries: &[(&[u8], &[u8])]) -> PathBuf {
        let file = dir.join(store_file_name(id, origin));
        let connection = connection::open(
            &file,
            Mode::Update,
            Some(&identity(id, origin)),
            &settings(),
        )
        .unwrap()
        .unwrap();
        let mut usage = 0;
        for (name, value) in entries {
            connection.put(name, value).unwrap();
            usage += (name.len() + value.len() + 64) as u64;
        }
        connection.set_usage(usage).unwrap();
        connection.commit().unwrap();
        file
    }

    fn storage_dir() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let storage = fs::canonicalize(dir.path()).unwrap();
        (dir, storage)
    }

    #[test]
    fn list_shows_every_store_sorted_with_its_identity_and_usage() {
        let (_dir, storage) = storage_dir();
        let limits = Limits::default();
        let b = store(&storage, "local/b", "/b.wasm", &[(b"n", b"v")]);
        let a2 = store(&storage, "local/a", "/z.wasm", &[]);
        let a1 = store(&storage, "local/a", "/a.wasm", &[(b"x", b"yy")]);
        fs::write(storage.join("notes.txt"), b"not a store").unwrap();
        let info = |file: PathBuf, id: &str, origin: &str, used| StoreInfo {
            file,
            identity: Some((id.to_owned(), origin.to_owned())),
            state: StoreState::Used(used),
        };
        assert_eq!(
            list(&storage, &limits),
            [
                info(a1, "local/a", "/a.wasm", 1 + 2 + 64),
                info(a2, "local/a", "/z.wasm", 0),
                info(b, "local/b", "/b.wasm", 1 + 1 + 64),
            ]
        );
    }

    #[test]
    fn list_shows_a_renamed_or_foreign_file_as_damaged() {
        let (_dir, storage) = storage_dir();
        let limits = Limits::default();
        let a = store(&storage, "local/a", "/a.wasm", &[]);
        // A's store under B's file name passes for nobody
        let renamed = storage.join(store_file_name("local/b", "/b.wasm"));
        fs::copy(&a, &renamed).unwrap();
        let garbage = storage.join("0000.sqlite");
        fs::write(&garbage, vec![7u8; 4096]).unwrap();
        let listed = list(&storage, &limits);
        let damaged: Vec<&PathBuf> = listed
            .iter()
            .filter(|info| matches!(info.state, StoreState::Damaged(_)))
            .map(|info| &info.file)
            .collect();
        assert_eq!(listed.len(), 3);
        assert!(damaged.contains(&&renamed), "{listed:?}");
        assert!(damaged.contains(&&garbage), "{listed:?}");
        assert!(!damaged.contains(&&a));
    }

    #[test]
    fn show_returns_every_entry_in_byte_order() {
        let (_dir, storage) = storage_dir();
        let limits = Limits::default();
        let file = store(
            &storage,
            "local/a",
            "/a.wasm",
            &[(b"b", b"2"), (b"a", b"1")],
        );
        let view = show(&file, &identity("local/a", "/a.wasm"), &limits)
            .unwrap()
            .unwrap();
        assert_eq!(view.used_bytes, 2 * (1 + 1 + 64));
        assert_eq!(
            view.entries,
            [
                (b"a".to_vec(), b"1".to_vec()),
                (b"b".to_vec(), b"2".to_vec())
            ]
        );
        let missing = storage.join(store_file_name("local/none", "/n.wasm"));
        assert_eq!(
            show(&missing, &identity("local/none", "/n.wasm"), &limits),
            Ok(None)
        );
        assert!(!missing.exists(), "show created a store");
        // Another plugin's identity: damaged
        let error = show(&file, &identity("local/b", "/b.wasm"), &limits).unwrap_err();
        assert!(matches!(error, StoreError::Damaged(_)), "{error:?}");
    }

    #[test]
    fn clear_empties_the_store_and_overwrites_what_it_held() {
        let (_dir, storage) = storage_dir();
        let limits = Limits::default();
        let marker = b"SECRET-MARKER-0123456789";
        let file = store(
            &storage,
            "local/a",
            "/a.wasm",
            &[(b"k", marker), (b"j", b"x")],
        );
        let other = store(&storage, "local/b", "/b.wasm", &[(b"k", b"keep")]);
        let other_before = fs::read(&other).unwrap();
        let size = fs::metadata(&file).unwrap().len();
        let cleared = clear(&file, &identity("local/a", "/a.wasm"), &limits).unwrap();
        assert_eq!(cleared, Some(2 * 64 + 1 + 24 + 1 + 1));
        let view = show(&file, &identity("local/a", "/a.wasm"), &limits)
            .unwrap()
            .unwrap();
        assert_eq!(view.used_bytes, 0);
        assert!(view.entries.is_empty());
        assert_eq!(fs::metadata(&file).unwrap().len(), size, "no VACUUM");
        let bytes = fs::read(&file).unwrap();
        assert!(
            !bytes.windows(marker.len()).any(|window| window == marker),
            "secure_delete overwrote the deleted value"
        );
        assert_eq!(
            fs::read(&other).unwrap(),
            other_before,
            "another store changed"
        );
        let missing = storage.join(store_file_name("local/none", "/n.wasm"));
        assert_eq!(
            clear(&missing, &identity("local/none", "/n.wasm"), &limits),
            Ok(None)
        );
    }

    #[test]
    fn a_store_in_use_is_busy_and_nothing_changes() {
        let (_dir, storage) = storage_dir();
        let limits = Limits {
            max_storage_busy_wait: Duration::from_millis(200),
            ..Limits::default()
        };
        let file = store(&storage, "local/a", "/a.wasm", &[(b"k", b"v")]);
        let before = fs::read(&file).unwrap();
        let holder = connection::open(
            &file,
            Mode::Update,
            Some(&identity("local/a", "/a.wasm")),
            &settings(),
        )
        .unwrap()
        .unwrap();
        holder.put(b"k", b"held").unwrap();
        let error = clear(&file, &identity("local/a", "/a.wasm"), &limits).unwrap_err();
        assert!(matches!(error, StoreError::Busy(_)), "{error:?}");
        let error = remove(&file, &limits).unwrap_err();
        assert!(matches!(error, StoreError::Busy(_)), "{error:?}");
        holder.rollback().unwrap();
        drop(holder);
        assert_eq!(fs::read(&file).unwrap(), before);
    }

    #[test]
    fn remove_deletes_a_damaged_store_and_nothing_else() {
        let (_dir, storage) = storage_dir();
        let limits = Limits::default();
        let damaged = storage.join("0000.sqlite");
        fs::write(&damaged, vec![7u8; 4096]).unwrap();
        let other = store(&storage, "local/b", "/b.wasm", &[]);
        remove(&damaged, &limits).unwrap();
        assert!(!damaged.exists());
        assert!(other.is_file());
        let orphan = store(&storage, "local/c", "/gone.wasm", &[(b"k", b"v")]);
        remove(&orphan, &limits).unwrap();
        assert!(!orphan.exists());
    }
}
