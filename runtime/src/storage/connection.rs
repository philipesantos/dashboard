//! The hardened connection to one store and the host's fixed statements
//! (`specs/004-plugin-storage/research.md`, R4, R5, R8). This is the only file that runs SQL.

use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;

/// The host's schema, created with the file and checked at every open (research R5).
const CREATE_STORE: &str =
    "CREATE TABLE store (key TEXT PRIMARY KEY NOT NULL, value ANY NOT NULL) STRICT, WITHOUT ROWID";
const CREATE_ENTRIES: &str = "CREATE TABLE entries (name BLOB PRIMARY KEY NOT NULL, value BLOB NOT NULL) STRICT, WITHOUT ROWID";

/// What a connection is opened for, which sets its open flags and its transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    /// Read-only, one read transaction held until the run ends.
    Render,
    /// Read-write in `BEGIN IMMEDIATE`, creating the file and its schema when needed.
    Update,
    /// Read-only, for `runtime storage show`.
    Show,
    /// Read-only, for `runtime storage list`, which has no binding to compare with.
    List,
    /// Read-write in `BEGIN EXCLUSIVE`, for `runtime storage clear`; never creates the file.
    Clear,
}

impl Mode {
    fn writes(self) -> bool {
        matches!(self, Mode::Update | Mode::Clear)
    }
}

/// The plugin a store belongs to, as its binding says and its `store` table records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Identity {
    pub(crate) id: String,
    pub(crate) origin: String,
}

/// What the host sets on every connection, from its limits and the store binding.
#[derive(Debug, Clone)]
pub(crate) struct Settings {
    /// The largest name and value the host accepts; the connection's length limit sits 4 KiB
    /// above their sum, for the row's own header.
    pub(crate) max_name_bytes: usize,
    pub(crate) max_value_bytes: usize,
    /// The file cap in bytes, rounded up to whole pages for `max_page_count`, never below the
    /// file's current size.
    pub(crate) file_cap_bytes: u64,
    /// How long a statement waits for another process holding the store.
    pub(crate) busy_timeout: Duration,
}

/// Why the store could not do what was asked; each maps to a rule of the contract.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StoreError {
    /// The file is not one the host wrote for this binding; it was left as it is.
    Damaged(String),
    /// Another process holds the store past the wait.
    Busy(String),
    /// The store could not be read or written.
    Unavailable(String),
    /// The file reached its cap (or the disk is full): SQLite ended the transaction.
    FileCap(String),
}

/// A stored name and its value.
pub(crate) type Entry = (Vec<u8>, Vec<u8>);

/// One open, hardened connection to a store, inside the transaction its [`Mode`] sets.
pub(crate) struct StoreConnection {
    connection: Connection,
}

/// Opens the store at `file` for `mode`, hardened as research R4 sets out, and checks that it
/// holds the host's schema and, when `identity` is given, that identity. `Ok(None)` when there
/// is no store yet: the file does not exist, or has no tables, in a mode that does not create
/// it.
pub(crate) fn open(
    file: &Path,
    mode: Mode,
    identity: Option<&Identity>,
    settings: &Settings,
) -> Result<Option<StoreConnection>, StoreError> {
    let _ = (file, mode, identity, settings);
    Err(StoreError::Unavailable("not implemented".to_owned()))
}

/// The bundled SQLite is the one this host was built and reviewed with, and its defensive mode
/// takes effect (research R1). Checked once per process, before any store is opened.
pub fn engine_check() -> Result<(), String> {
    Err("not implemented".to_owned())
}

impl StoreConnection {
    pub(crate) fn get(&self, name: &[u8]) -> Result<Option<Vec<u8>>, StoreError> {
        let _ = name;
        Err(StoreError::Unavailable("not implemented".to_owned()))
    }

    /// Stores `value` under `name`, replacing any value there.
    pub(crate) fn put(&self, name: &[u8], value: &[u8]) -> Result<(), StoreError> {
        let _ = (name, value);
        Err(StoreError::Unavailable("not implemented".to_owned()))
    }

    pub(crate) fn remove(&self, name: &[u8]) -> Result<(), StoreError> {
        let _ = name;
        Err(StoreError::Unavailable("not implemented".to_owned()))
    }

    /// At most `page` names that start with `prefix` and come after `after`, in byte order, and
    /// whether more remain.
    pub(crate) fn list(
        &self,
        prefix: &[u8],
        after: Option<&[u8]>,
        page: usize,
    ) -> Result<(Vec<Vec<u8>>, bool), StoreError> {
        let _ = (prefix, after, page);
        Err(StoreError::Unavailable("not implemented".to_owned()))
    }

    /// Every entry, in byte order of names.
    pub(crate) fn entries(&self) -> Result<Vec<Entry>, StoreError> {
        Err(StoreError::Unavailable("not implemented".to_owned()))
    }

    /// Deletes every entry; `secure_delete` overwrites their content.
    pub(crate) fn remove_all(&self) -> Result<(), StoreError> {
        Err(StoreError::Unavailable("not implemented".to_owned()))
    }

    pub(crate) fn usage(&self) -> Result<u64, StoreError> {
        Err(StoreError::Unavailable("not implemented".to_owned()))
    }

    pub(crate) fn set_usage(&self, usage: u64) -> Result<(), StoreError> {
        let _ = usage;
        Err(StoreError::Unavailable("not implemented".to_owned()))
    }

    /// The id and origin the store records.
    pub(crate) fn identity(&self) -> Result<Identity, StoreError> {
        Err(StoreError::Unavailable("not implemented".to_owned()))
    }

    pub(crate) fn commit(&self) -> Result<(), StoreError> {
        Err(StoreError::Unavailable("not implemented".to_owned()))
    }

    pub(crate) fn rollback(&self) -> Result<(), StoreError> {
        Err(StoreError::Unavailable("not implemented".to_owned()))
    }

    /// Whether SQLite ended the transaction this connection opened, as it does on a full file
    /// (research R7): every later statement would then commit on its own.
    pub(crate) fn transaction_ended(&self) -> bool {
        self.connection.is_autocommit()
    }
}

/// The shortest byte string greater than every string that starts with `prefix`, or `None` when
/// there is none (an empty prefix, or one of only `0xFF` bytes).
fn successor(prefix: &[u8]) -> Option<Vec<u8>> {
    let _ = prefix;
    None
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use rusqlite::OpenFlags;
    use sha2::{Digest, Sha256};
    use tempfile::TempDir;

    use super::*;

    const KIB: usize = 1024;

    fn settings() -> Settings {
        Settings {
            max_name_bytes: 256,
            max_value_bytes: 256 * KIB,
            file_cap_bytes: 8 * 1024 * KIB as u64,
            busy_timeout: Duration::from_millis(100),
        }
    }

    fn identity() -> Identity {
        Identity {
            id: "local/probe".to_owned(),
            origin: "/plugins/probe.wasm".to_owned(),
        }
    }

    fn store_in(dir: &TempDir) -> PathBuf {
        dir.path().join("store.sqlite")
    }

    fn open_update(file: &Path) -> StoreConnection {
        open(file, Mode::Update, Some(&identity()), &settings())
            .expect("the store opens for update")
            .expect("update creates the store")
    }

    /// A store holding `entries`, committed, with its usage as the host counts it.
    fn store_with(file: &Path, entries: &[(&[u8], &[u8])]) {
        let store = open_update(file);
        let mut usage = 0;
        for (name, value) in entries {
            store.put(name, value).unwrap();
            usage += (name.len() + value.len() + 64) as u64;
        }
        store.set_usage(usage).unwrap();
        store.commit().unwrap();
    }

    fn digest(file: &Path) -> Vec<u8> {
        Sha256::digest(fs::read(file).unwrap()).to_vec()
    }

    /// A plain connection, with none of the host's layers, to plant what a test needs.
    fn raw(file: &Path) -> Connection {
        Connection::open_with_flags(
            file,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .unwrap()
    }

    #[test]
    fn the_bundled_engine_is_the_reviewed_one() {
        engine_check().expect("SQLite 3.54.0 with defensive mode");
    }

    #[test]
    fn a_file_named_like_a_uri_is_a_plain_file() {
        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("file:x");
        fs::create_dir(&nested).unwrap();
        let file = nested.join("store.sqlite");
        assert!(file.is_absolute());
        let store = open_update(&file);
        store.commit().unwrap();
        drop(store);
        assert!(file.is_file(), "the store is the file at its path");
        let names: Vec<_> = fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, ["file:x"], "nothing else appears beside it");
    }

    #[test]
    fn a_relative_path_is_refused() {
        let error = open(
            Path::new("storage/store.sqlite"),
            Mode::Update,
            Some(&identity()),
            &settings(),
        )
        .err()
        .expect("refused");
        assert!(matches!(error, StoreError::Unavailable(_)), "{error:?}");
    }

    #[test]
    fn read_modes_never_create_the_file() {
        let dir = TempDir::new().unwrap();
        let file = store_in(&dir);
        for mode in [Mode::Render, Mode::Show, Mode::List, Mode::Clear] {
            let opened = open(&file, mode, Some(&identity()), &settings()).unwrap();
            assert!(opened.is_none(), "{mode:?}: no store yet");
            assert!(!file.exists(), "{mode:?} created the file");
        }
    }

    #[test]
    fn a_file_without_tables_is_an_empty_store() {
        let dir = TempDir::new().unwrap();
        let file = store_in(&dir);
        fs::write(&file, b"").unwrap();
        let opened = open(&file, Mode::Render, Some(&identity()), &settings()).unwrap();
        assert!(opened.is_none());
        // Update creates the schema in it
        let store = open_update(&file);
        assert_eq!(store.usage().unwrap(), 0);
    }

    #[test]
    fn update_creates_exactly_the_host_schema_and_identity() {
        let dir = TempDir::new().unwrap();
        let file = store_in(&dir);
        store_with(&file, &[]);
        let raw = raw(&file);
        let mut statement = raw
            .prepare("SELECT type, name, tbl_name, sql FROM sqlite_schema ORDER BY name")
            .unwrap();
        let rows: Vec<(String, String, String, String)> = statement
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(
            rows,
            [
                (
                    "table".to_owned(),
                    "entries".to_owned(),
                    "entries".to_owned(),
                    CREATE_ENTRIES.to_owned()
                ),
                (
                    "table".to_owned(),
                    "store".to_owned(),
                    "store".to_owned(),
                    CREATE_STORE.to_owned()
                ),
            ]
        );
        let format: i64 = raw
            .query_row("SELECT value FROM store WHERE key = 'format'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(format, 1);
        let store = open(&file, Mode::Show, Some(&identity()), &settings())
            .unwrap()
            .unwrap();
        assert_eq!(store.identity().unwrap(), identity());
        assert_eq!(store.usage().unwrap(), 0);
    }

    /// Every way a file can differ from what the host writes; each must read as damaged, with
    /// the file left byte for byte as it was.
    #[test]
    fn a_file_the_host_did_not_write_is_damaged_and_left_alone() {
        let plants: [(&str, &str); 10] = [
            ("an extra table", "CREATE TABLE extra (x)"),
            ("an index", "CREATE INDEX values_index ON entries (value)"),
            (
                "a trigger",
                "CREATE TRIGGER spy AFTER INSERT ON entries BEGIN DELETE FROM store; END",
            ),
            ("a view", "CREATE VIEW peek AS SELECT * FROM entries"),
            (
                "another format",
                "UPDATE store SET value = 2 WHERE key = 'format'",
            ),
            (
                "another id",
                "UPDATE store SET value = 'local/other' WHERE key = 'id'",
            ),
            (
                "another origin",
                "UPDATE store SET value = '/elsewhere.wasm' WHERE key = 'origin'",
            ),
            (
                "a usage that is not a number",
                "UPDATE store SET value = 'many' WHERE key = 'usage'",
            ),
            (
                "an extra identity row",
                "INSERT INTO store (key, value) VALUES ('owner', 'x')",
            ),
            (
                "other SQL for a table",
                "DROP TABLE entries; CREATE TABLE entries (name BLOB PRIMARY KEY NOT NULL, value BLOB NOT NULL)",
            ),
        ];
        for (what, sql) in plants {
            let dir = TempDir::new().unwrap();
            let file = store_in(&dir);
            store_with(&file, &[(b"name", b"value")]);
            raw(&file).execute_batch(sql).unwrap();
            let before = digest(&file);
            for mode in [Mode::Render, Mode::Update, Mode::Show, Mode::Clear] {
                let error = open(&file, mode, Some(&identity()), &settings())
                    .err()
                    .unwrap_or_else(|| panic!("{what}: {mode:?} opened the store"));
                assert!(
                    matches!(error, StoreError::Damaged(_)),
                    "{what}: {mode:?} gave {error:?}"
                );
                assert_eq!(digest(&file), before, "{what}: {mode:?} changed the file");
            }
        }
    }

    #[test]
    fn a_file_that_is_not_a_database_is_damaged() {
        let dir = TempDir::new().unwrap();
        let file = store_in(&dir);
        fs::write(&file, vec![0x42; 8192]).unwrap();
        let error = open(&file, Mode::Render, Some(&identity()), &settings())
            .err()
            .expect("refused");
        assert!(matches!(error, StoreError::Damaged(_)), "{error:?}");
        assert_eq!(fs::read(&file).unwrap(), vec![0x42; 8192]);
    }

    #[test]
    fn list_mode_reads_the_identity_without_a_binding() {
        let dir = TempDir::new().unwrap();
        let file = store_in(&dir);
        store_with(&file, &[]);
        let store = open(&file, Mode::List, None, &settings()).unwrap().unwrap();
        assert_eq!(store.identity().unwrap(), identity());
    }

    #[test]
    fn names_round_trip_byte_for_byte() {
        let dir = TempDir::new().unwrap();
        let file = store_in(&dir);
        let store = open_update(&file);
        let names: [&[u8]; 6] = [
            b"a\0b",
            b"../../etc/passwd",
            b"it's \"quoted\"",
            b"'; DROP TABLE entries; --",
            b"ATTACH DATABASE '/tmp/x' AS y",
            b"%_*?",
        ];
        for (index, name) in names.iter().enumerate() {
            store.put(name, &[index as u8, 0, 255]).unwrap();
        }
        for (index, name) in names.iter().enumerate() {
            assert_eq!(store.get(name).unwrap(), Some(vec![index as u8, 0, 255]));
        }
        assert_eq!(store.get(b"a").unwrap(), None, "NUL is part of the name");
        store.put(names[0], b"replaced").unwrap();
        assert_eq!(store.get(names[0]).unwrap(), Some(b"replaced".to_vec()));
        store.remove(names[1]).unwrap();
        store.remove(b"never written").unwrap();
        assert_eq!(store.get(names[1]).unwrap(), None);
        store.commit().unwrap();
        let entries = open(&file, Mode::Show, Some(&identity()), &settings())
            .unwrap()
            .unwrap()
            .entries()
            .unwrap();
        assert_eq!(entries.len(), 5, "the tables are still there");
    }

    #[test]
    fn list_is_a_byte_range_never_a_pattern() {
        let dir = TempDir::new().unwrap();
        let file = store_in(&dir);
        let names: [&[u8]; 8] = [
            b"a",
            b"a%",
            b"ab",
            b"a\xff",
            b"a\xff\xff",
            b"b",
            b"\xff",
            b"\xff\xff",
        ];
        let store = open_update(&file);
        for name in names {
            store.put(name, b"").unwrap();
        }
        let list =
            |prefix: &[u8], after: Option<&[u8]>, page| store.list(prefix, after, page).unwrap();
        let all: Vec<Vec<u8>> = {
            let mut sorted: Vec<Vec<u8>> = names.iter().map(|name| name.to_vec()).collect();
            sorted.sort();
            sorted
        };
        assert_eq!(list(b"", None, 100), (all.clone(), false));
        assert_eq!(list(b"a%", None, 100), (vec![b"a%".to_vec()], false));
        assert_eq!(
            list(b"a", None, 100).0,
            [&b"a"[..], b"a%", b"ab", b"a\xff", b"a\xff\xff"]
        );
        assert_eq!(list(b"a\xff", None, 100).0, [&b"a\xff"[..], b"a\xff\xff"]);
        assert_eq!(list(b"\xff", None, 100).0, [&b"\xff"[..], b"\xff\xff"]);
        assert_eq!(
            list(b"a", None, 2),
            (vec![b"a".to_vec(), b"a%".to_vec()], true)
        );
        assert_eq!(
            list(b"a", Some(b"a%"), 2),
            (vec![b"ab".to_vec(), b"a\xff".to_vec()], true)
        );
        assert_eq!(
            list(b"a", Some(b"a\xff"), 2),
            (vec![b"a\xff\xff".to_vec()], false)
        );
        assert_eq!(list(b"c", None, 100), (Vec::new(), false));
    }

    #[test]
    fn the_successor_of_a_prefix_bounds_the_range() {
        assert_eq!(successor(b""), None);
        assert_eq!(successor(b"a"), Some(b"b".to_vec()));
        assert_eq!(successor(b"a\xff\xff"), Some(b"b".to_vec()));
        assert_eq!(successor(b"ab\xff"), Some(b"ac".to_vec()));
        assert_eq!(successor(b"\xff\xff"), None);
    }

    #[test]
    fn the_largest_entry_fits_under_the_length_limit() {
        let dir = TempDir::new().unwrap();
        let file = store_in(&dir);
        let store = open_update(&file);
        store.put(&[b'n'; 256], &vec![b'v'; 256 * KIB]).unwrap();
        // The host refuses anything larger first; the connection's own limit is the layer
        // beneath, 4 KiB above the largest name and value
        assert!(
            store
                .put(b"n", &vec![b'v'; 256 * KIB + 256 + 4 * KIB + 1])
                .is_err()
        );
    }

    #[test]
    fn usage_is_kept_in_the_store() {
        let dir = TempDir::new().unwrap();
        let file = store_in(&dir);
        let store = open_update(&file);
        store.set_usage(12_345).unwrap();
        store.commit().unwrap();
        drop(store);
        let store = open(&file, Mode::Render, Some(&identity()), &settings())
            .unwrap()
            .unwrap();
        assert_eq!(store.usage().unwrap(), 12_345);
    }

    #[test]
    fn a_full_file_ends_the_transaction() {
        let dir = TempDir::new().unwrap();
        let file = store_in(&dir);
        store_with(&file, &[(b"kept", b"before")]);
        let small = Settings {
            file_cap_bytes: 64 * KIB as u64,
            ..settings()
        };
        let store = open(&file, Mode::Update, Some(&identity()), &small)
            .unwrap()
            .unwrap();
        let mut error = None;
        for index in 0..64u32 {
            if let Err(failed) = store.put(&index.to_be_bytes(), &[7; 16 * KIB]) {
                error = Some(failed);
                break;
            }
        }
        let error = error.expect("the file cap stops the writes");
        assert!(matches!(error, StoreError::FileCap(_)), "{error:?}");
        assert!(
            store.transaction_ended(),
            "SQLite rolled the transaction back"
        );
        drop(store);
        let store = open(&file, Mode::Show, Some(&identity()), &settings())
            .unwrap()
            .unwrap();
        assert_eq!(
            store.entries().unwrap(),
            [(b"kept".to_vec(), b"before".to_vec())]
        );
        let size = fs::metadata(&file).unwrap().len();
        assert!(size <= 64 * KIB as u64 + 4096, "{size} bytes");
    }

    #[test]
    fn the_file_cap_never_goes_below_the_file() {
        let dir = TempDir::new().unwrap();
        let file = store_in(&dir);
        let store = open_update(&file);
        for index in 0..32u32 {
            store.put(&index.to_be_bytes(), &[1; 8 * KIB]).unwrap();
        }
        store.commit().unwrap();
        drop(store);
        let size = fs::metadata(&file).unwrap().len();
        // A cap below the file: reading and deleting still work
        let tiny = Settings {
            file_cap_bytes: 4096,
            ..settings()
        };
        let store = open(&file, Mode::Update, Some(&identity()), &tiny)
            .unwrap()
            .unwrap();
        store.remove(&0u32.to_be_bytes()).unwrap();
        store.commit().unwrap();
        assert!(fs::metadata(&file).unwrap().len() <= size);
    }

    #[test]
    fn statements_outside_the_host_s_own_are_refused() {
        let dir = TempDir::new().unwrap();
        let file = store_in(&dir);
        let store = open_update(&file);
        let attempts = [
            "ATTACH DATABASE ':memory:' AS other",
            "SELECT load_extension('x')",
            "SELECT length(x'00')",
            "PRAGMA writable_schema = ON",
            "PRAGMA journal_mode = WAL",
            "CREATE TABLE extra (x)",
            "DROP TABLE entries",
            "SELECT sql FROM sqlite_schema",
            "SAVEPOINT s",
        ];
        for sql in attempts {
            assert!(
                store.connection.execute_batch(sql).is_err(),
                "{sql} was allowed"
            );
        }
        // The host's own statements still run
        store.put(b"a", b"b").unwrap();
        assert_eq!(store.get(b"a").unwrap(), Some(b"b".to_vec()));
    }

    #[test]
    fn render_reads_one_view_while_another_connection_writes() {
        let dir = TempDir::new().unwrap();
        let file = store_in(&dir);
        store_with(&file, &[(b"a", b"1")]);
        let render = open(&file, Mode::Render, Some(&identity()), &settings())
            .unwrap()
            .unwrap();
        assert_eq!(render.get(b"a").unwrap(), Some(b"1".to_vec()));
        let update = open_update(&file);
        update.put(b"a", b"2").unwrap();
        // The update waits for the render to finish: its commit is busy while render reads
        let error = update.commit().expect_err("busy while render reads");
        assert!(matches!(error, StoreError::Busy(_)), "{error:?}");
        assert_eq!(render.get(b"a").unwrap(), Some(b"1".to_vec()));
        drop(render);
        update.commit().unwrap();
        let render = open(&file, Mode::Render, Some(&identity()), &settings())
            .unwrap()
            .unwrap();
        assert_eq!(render.get(b"a").unwrap(), Some(b"2".to_vec()));
    }

    #[test]
    fn render_cannot_write_even_if_the_host_tried() {
        let dir = TempDir::new().unwrap();
        let file = store_in(&dir);
        store_with(&file, &[]);
        let render = open(&file, Mode::Render, Some(&identity()), &settings())
            .unwrap()
            .unwrap();
        assert!(
            render.put(b"a", b"b").is_err(),
            "the connection is read-only"
        );
    }

    /// The state a process killed during an update leaves: the store and its hot journal,
    /// copied while the update holds its transaction past the page cache.
    fn killed_update(dir: &TempDir) -> PathBuf {
        let file = store_in(dir);
        store_with(&file, &[(b"kept", b"before")]);
        let update = open_update(&file);
        for index in 0..256u32 {
            update.put(&index.to_be_bytes(), &[9; 4 * KIB]).unwrap();
        }
        let journal = file.with_extension("sqlite-journal");
        assert!(journal.is_file(), "the update spilled pages past the cache");
        let crashed = dir.path().join("crashed");
        fs::create_dir(&crashed).unwrap();
        fs::copy(&file, crashed.join("store.sqlite")).unwrap();
        fs::copy(&journal, crashed.join("store.sqlite-journal")).unwrap();
        update.rollback().unwrap();
        crashed.join("store.sqlite")
    }

    #[test]
    fn a_hot_journal_is_rolled_back_before_reading() {
        let dir = TempDir::new().unwrap();
        let file = killed_update(&dir);
        for mode in [Mode::Render, Mode::Show, Mode::List] {
            let identity = (mode != Mode::List).then(identity);
            let store = open(&file, mode, identity.as_ref(), &settings())
                .unwrap_or_else(|error| panic!("{mode:?}: {error:?}"))
                .unwrap();
            assert_eq!(
                store.entries().unwrap(),
                [(b"kept".to_vec(), b"before".to_vec())],
                "{mode:?} reads the state from before the update"
            );
        }
        assert!(!file.with_extension("sqlite-journal").exists());
    }

    #[test]
    fn a_hot_journal_that_cannot_be_rolled_back_is_busy_not_damaged() {
        let dir = TempDir::new().unwrap();
        let file = killed_update(&dir);
        let mut permissions = fs::metadata(&file).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&file, permissions).unwrap();
        let error = open(&file, Mode::Render, Some(&identity()), &settings())
            .err()
            .expect("cannot recover a read-only file");
        assert!(matches!(error, StoreError::Busy(_)), "{error:?}");
        let mut permissions = fs::metadata(&file).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        permissions.set_readonly(false);
        fs::set_permissions(&file, permissions).unwrap();
    }
}
