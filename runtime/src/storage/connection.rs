//! The hardened connection to one store and the host's fixed statements
//! (`specs/004-plugin-storage/research.md`, R4, R5, R8). This is the only file that runs SQL.
//!
//! Every connection, in every mode, is set up the same way, in the order of R4: [`configure`]
//! (an absolute path, the open flags, no attached databases, defensive mode with triggers, views
//! and untrusted schemas off, the length limit, the pragmas), then the transaction and the
//! schema check, then [`authorize`], which sets the statement length limit and from then on
//! allows only the statements below. Every
//! name and value is bound as a parameter, never written into SQL, and no statement calls a
//! function, so lengths and usage are counted in Rust.

use std::path::Path;
use std::sync::OnceLock;
use std::time::Duration;

use rusqlite::config::DbConfig;
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use rusqlite::limits::Limit;
use rusqlite::{Connection, ErrorCode, OpenFlags, OptionalExtension, ffi, params};

/// The SQLite this host is built and reviewed with: `libsqlite3-sys` 0.39.0, pinned in
/// `runtime/Cargo.toml`, bundles it (research R1).
const SQLITE_VERSION: &str = "3.54.0";

/// The host's schema, created with the file and checked at every open (research R5).
const CREATE_STORE: &str =
    "CREATE TABLE store (key TEXT PRIMARY KEY NOT NULL, value ANY NOT NULL) STRICT, WITHOUT ROWID";
const CREATE_ENTRIES: &str = "CREATE TABLE entries (name BLOB PRIMARY KEY NOT NULL, value BLOB NOT NULL) STRICT, WITHOUT ROWID";
/// The store's own rows: `format`, `id`, `origin` and `usage`.
const STORE_FORMAT: i64 = 1;
const INSERT_IDENTITY: &str =
    "INSERT INTO store (key, value) VALUES ('format', 1), ('id', ?1), ('origin', ?2), ('usage', 0)";
const READ_SCHEMA: &str = "SELECT type, name, tbl_name, sql FROM sqlite_schema ORDER BY name";
const READ_STORE: &str = "SELECT key, value FROM store ORDER BY key";

/// The statements a plugin's calls and the CLI run, after the authorizer is installed.
const GET: &str = "SELECT value FROM entries WHERE name = ?1";
const PUT: &str = "INSERT OR REPLACE INTO entries (name, value) VALUES (?1, ?2)";
const REMOVE: &str = "DELETE FROM entries WHERE name = ?1";
const REMOVE_ALL: &str = "DELETE FROM entries";
/// Listing is a range on the name's bytes, never `LIKE` or `GLOB`, whose `%` and `_` would turn
/// a name into a pattern; the extra row says whether another page follows.
const LIST_BOUNDED: &str =
    "SELECT name FROM entries WHERE name >= ?1 AND name < ?2 AND name > ?3 ORDER BY name LIMIT ?4";
const LIST_OPEN: &str =
    "SELECT name FROM entries WHERE name >= ?1 AND name > ?2 ORDER BY name LIMIT ?3";
const ENTRIES: &str = "SELECT name, value FROM entries ORDER BY name";
const READ_USAGE: &str = "SELECT value FROM store WHERE key = 'usage'";
const WRITE_USAGE: &str = "UPDATE store SET value = ?1 WHERE key = 'usage'";

/// Every statement the host prepares on a store connection, for `SQLITE_LIMIT_SQL_LENGTH`. The
/// pragmas of [`configure`] are shorter than the longest of them.
const STATEMENTS: &[&str] = &[
    CREATE_STORE,
    CREATE_ENTRIES,
    INSERT_IDENTITY,
    READ_SCHEMA,
    READ_STORE,
    GET,
    PUT,
    REMOVE,
    REMOVE_ALL,
    LIST_BOUNDED,
    LIST_OPEN,
    ENTRIES,
    READ_USAGE,
    WRITE_USAGE,
    "BEGIN DEFERRED",
    "BEGIN IMMEDIATE",
    "BEGIN EXCLUSIVE",
    "COMMIT",
    "ROLLBACK",
];

/// The page cache of one connection, in KiB: a store never holds much host memory, and an
/// update of 1 MiB spills pages to the file (research R4, R8).
const CACHE_KIB: i64 = 256;
/// Room in `SQLITE_LIMIT_LENGTH` for a row's own header above the largest name and value.
const ROW_HEADER_BYTES: usize = 4096;

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

    fn begin(self) -> &'static str {
        match self {
            Mode::Render | Mode::Show | Mode::List => "BEGIN DEFERRED",
            Mode::Update => "BEGIN IMMEDIATE",
            Mode::Clear => "BEGIN EXCLUSIVE",
        }
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
    file_cap_bytes: u64,
}

/// Why one attempt at opening failed: a store error, or a hot journal that a read-only
/// connection cannot roll back (`SQLITE_READONLY_ROLLBACK`, research R8).
pub(crate) enum Failure {
    Store(StoreError),
    HotJournal,
}

impl From<StoreError> for Failure {
    fn from(error: StoreError) -> Self {
        Failure::Store(error)
    }
}

/// Opens the store at `file` for `mode`, hardened as research R4 sets out, and checks that it
/// holds the host's schema and, when `identity` is given, that identity. `Ok(None)` when there
/// is no store yet: the file does not exist, or has no tables, in a mode that does not create
/// it.
///
/// A read-only open that finds the hot journal of an interrupted update opens the store
/// read-write once, which rolls the journal back, then opens it read-only again; if that does
/// not work, the store is `Busy`, never `Damaged`: it only needs recovering (research R8).
pub(crate) fn open(
    file: &Path,
    mode: Mode,
    identity: Option<&Identity>,
    settings: &Settings,
) -> Result<Option<StoreConnection>, StoreError> {
    engine_check().map_err(StoreError::Unavailable)?;
    if !file.is_absolute() {
        // A relative name could be read as a URI (SQLITE_USE_URI is compiled in) or move
        // with the working directory; the binding always gives an absolute one
        return Err(StoreError::Unavailable(format!(
            "the store path {} is not absolute",
            file.display()
        )));
    }
    match open_once(file, mode, identity, settings) {
        Ok(opened) => Ok(opened),
        Err(Failure::Store(error)) => Err(error),
        Err(Failure::HotJournal) => {
            let busy = |detail: &str| {
                StoreError::Busy(format!(
                    "an interrupted update left a journal that could not be rolled back: {detail}"
                ))
            };
            match open_once(file, Mode::Clear, None, settings) {
                Ok(recovered) => drop(recovered),
                // The schema check may still refuse the file; the read-only open says so
                Err(Failure::Store(StoreError::Damaged(_))) => {}
                Err(Failure::Store(error)) => return Err(busy(&describe(&error))),
                Err(Failure::HotJournal) => return Err(busy("the file cannot be written")),
            }
            match open_once(file, mode, identity, settings) {
                Ok(opened) => Ok(opened),
                Err(Failure::Store(error)) => Err(error),
                Err(Failure::HotJournal) => Err(busy("it is still there")),
            }
        }
    }
}

fn describe(error: &StoreError) -> String {
    match error {
        StoreError::Damaged(detail)
        | StoreError::Busy(detail)
        | StoreError::Unavailable(detail)
        | StoreError::FileCap(detail) => detail.clone(),
    }
}

fn open_once(
    file: &Path,
    mode: Mode,
    identity: Option<&Identity>,
    settings: &Settings,
) -> Result<Option<StoreConnection>, Failure> {
    if mode != Mode::Update && !file.exists() {
        return Ok(None);
    }
    let mut connection = configure(file, mode, settings)?;
    if !check_schema(&connection, mode, identity)? {
        return Ok(None);
    }
    authorize(&mut connection)?;
    Ok(Some(StoreConnection {
        connection,
        file_cap_bytes: settings.file_cap_bytes,
    }))
}

/// Steps 1 to 6 of research R4: everything a connection is set up with before its first
/// statement on the store's tables.
pub(crate) fn configure(
    file: &Path,
    mode: Mode,
    settings: &Settings,
) -> Result<Connection, Failure> {
    // SQLITE_OPEN_URI is left out: the path is a file name, never a URI
    let mut flags = OpenFlags::SQLITE_OPEN_NO_MUTEX | OpenFlags::SQLITE_OPEN_EXRESCODE;
    flags |= match mode {
        Mode::Update => OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        Mode::Clear => OpenFlags::SQLITE_OPEN_READ_WRITE,
        Mode::Render | Mode::Show | Mode::List => OpenFlags::SQLITE_OPEN_READ_ONLY,
    };
    let connection =
        Connection::open_with_flags(file, flags).map_err(|error| failure(error, "opening"))?;
    let setup = |error| failure(error, "setting up the connection");
    connection
        .set_limit(Limit::SQLITE_LIMIT_ATTACHED, 0)
        .map_err(setup)?;
    for (config, on) in [
        (DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true),
        (DbConfig::SQLITE_DBCONFIG_ENABLE_TRIGGER, false),
        (DbConfig::SQLITE_DBCONFIG_ENABLE_VIEW, false),
        (DbConfig::SQLITE_DBCONFIG_TRUSTED_SCHEMA, false),
    ] {
        if connection.set_db_config(config, on).map_err(setup)? != on {
            return Err(
                StoreError::Unavailable(format!("SQLite refused to set {config:?}")).into(),
            );
        }
    }
    let length = settings.max_name_bytes + settings.max_value_bytes + ROW_HEADER_BYTES;
    set_limit(&connection, Limit::SQLITE_LIMIT_LENGTH, length)?;
    let page_size: i64 = pragma(&connection, "PRAGMA page_size")?;
    let page_count: i64 = pragma(&connection, "PRAGMA page_count")?;
    let page_size = page_size.max(1) as u64;
    let cap_pages = settings
        .file_cap_bytes
        .div_ceil(page_size)
        .max(page_count.max(1) as u64);
    let set: i64 = pragma(&connection, &format!("PRAGMA max_page_count = {cap_pages}"))?;
    if (set as u64) < cap_pages.min(page_count as u64) {
        return Err(StoreError::Unavailable(format!(
            "SQLite kept max_page_count at {set} for a cap of {cap_pages} pages"
        ))
        .into());
    }
    let _: i64 = pragma(&connection, "PRAGMA secure_delete = ON")?;
    if mode.writes() {
        let journal: String = pragma(&connection, "PRAGMA journal_mode = DELETE")?;
        if !journal.eq_ignore_ascii_case("delete") {
            return Err(StoreError::Damaged(format!(
                "the store keeps a {journal} journal, which the host never sets"
            ))
            .into());
        }
    }
    connection
        .execute_batch(&format!(
            "PRAGMA synchronous = FULL; PRAGMA cache_size = -{CACHE_KIB}"
        ))
        .map_err(setup)?;
    connection
        .busy_timeout(settings.busy_timeout)
        .map_err(setup)?;
    Ok(connection)
}

fn set_limit(connection: &Connection, limit: Limit, value: usize) -> Result<(), Failure> {
    let value = i32::try_from(value).map_err(|_| {
        StoreError::Unavailable(format!("{limit:?} of {value} does not fit SQLite"))
    })?;
    connection
        .set_limit(limit, value)
        .map(drop)
        .map_err(|error| failure(error, "setting up the connection"))
}

/// The first row's first column of a pragma, which some pragmas return and others do not.
fn pragma<T: rusqlite::types::FromSql + Default>(
    connection: &Connection,
    sql: &str,
) -> Result<T, Failure> {
    let mut statement = connection
        .prepare(sql)
        .map_err(|error| failure(error, "setting up the connection"))?;
    let mut rows = statement
        .query([])
        .map_err(|error| failure(error, "setting up the connection"))?;
    match rows
        .next()
        .map_err(|error| failure(error, "setting up the connection"))?
    {
        Some(row) => row
            .get(0)
            .map_err(|error| failure(error, "setting up the connection")),
        None => Ok(T::default()),
    }
}

/// Steps 7 and 8 of research R4: starts the mode's transaction, creates the schema in an
/// update of a file without tables, and checks it holds exactly the host's schema and rows.
/// `false` when there is no store yet (a file without tables, outside an update).
fn check_schema(
    connection: &Connection,
    mode: Mode,
    identity: Option<&Identity>,
) -> Result<bool, Failure> {
    connection
        .execute_batch(mode.begin())
        .map_err(|error| failure(error, "starting a transaction"))?;
    let read = |error| failure(error, "reading the schema");
    let mut schema: Vec<(String, String, String, Option<String>)> = {
        let mut statement = connection.prepare(READ_SCHEMA).map_err(read)?;
        statement
            .query_map([], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
            })
            .map_err(read)?
            .collect::<Result<_, _>>()
            .map_err(read)?
    };
    if schema.is_empty() {
        if mode != Mode::Update {
            return Ok(false);
        }
        let Some(identity) = identity else {
            return Err(
                StoreError::Unavailable("an update needs the store's identity".into()).into(),
            );
        };
        let create = |error| failure(error, "creating the store");
        for sql in [CREATE_STORE, CREATE_ENTRIES] {
            connection.execute_batch(sql).map_err(create)?;
        }
        connection
            .execute(INSERT_IDENTITY, params![identity.id, identity.origin])
            .map_err(create)?;
        schema = vec![
            table("entries", CREATE_ENTRIES),
            table("store", CREATE_STORE),
        ];
    }
    let expected = [
        table("entries", CREATE_ENTRIES),
        table("store", CREATE_STORE),
    ];
    if schema != expected {
        let names: Vec<String> = schema
            .iter()
            .map(|(kind, name, ..)| format!("{kind} {name}"))
            .collect();
        return Err(StoreError::Damaged(format!(
            "the file holds {}, not the host's two tables as it writes them",
            names.join(", ")
        ))
        .into());
    }
    let stored = read_identity(connection)?;
    if let Some(identity) = identity
        && stored != *identity
    {
        return Err(StoreError::Damaged(format!(
            "the file is the store of {} from {}",
            stored.id, stored.origin
        ))
        .into());
    }
    Ok(true)
}

fn table(name: &str, sql: &str) -> (String, String, String, Option<String>) {
    (
        "table".to_owned(),
        name.to_owned(),
        name.to_owned(),
        Some(sql.to_owned()),
    )
}

/// The `store` table: exactly `format` = 1, `id` and `origin` as text, and `usage` a whole
/// number, or the file is damaged.
fn read_identity(connection: &Connection) -> Result<Identity, StoreError> {
    use rusqlite::types::Value;
    let read = |error| store_error(error, "reading the store's identity");
    let mut statement = connection.prepare(READ_STORE).map_err(read)?;
    let rows: Vec<(String, Value)> = statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(read)?
        .collect::<Result<_, _>>()
        .map_err(read)?;
    let damaged =
        || StoreError::Damaged("the store's own rows are not the ones the host writes".to_owned());
    let [
        (format_key, format),
        (id_key, id),
        (origin_key, origin),
        (usage_key, usage),
    ] = <[(String, Value); 4]>::try_from(rows).map_err(|_| damaged())?;
    match (
        (format_key.as_str(), format),
        (id_key.as_str(), id),
        (origin_key.as_str(), origin),
        (usage_key.as_str(), usage),
    ) {
        (
            ("format", Value::Integer(STORE_FORMAT)),
            ("id", Value::Text(id)),
            ("origin", Value::Text(origin)),
            ("usage", Value::Integer(usage)),
        ) if usage >= 0 => Ok(Identity { id, origin }),
        _ => Err(damaged()),
    }
}

/// Step 9 of research R4: from here on the connection runs only the host's statements on its
/// two tables. Everything else is denied: every function, `ATTACH`, `DETACH`, `PRAGMA`,
/// `CREATE`, `DROP`, `ALTER`, savepoints, reads of `sqlite_schema` and any action SQLite adds.
pub(crate) fn authorize(connection: &mut Connection) -> Result<(), Failure> {
    // Set here rather than with the length limit: creating the tables runs SQLite's own,
    // longer statements
    let sql_length = STATEMENTS.iter().map(|sql| sql.len()).max().unwrap_or(0);
    set_limit(connection, Limit::SQLITE_LIMIT_SQL_LENGTH, sql_length)?;
    connection
        .authorizer(Some(|context: AuthContext<'_>| {
            let main = context.database_name == Some("main");
            let host_table = |table: &str| main && matches!(table, "store" | "entries");
            match context.action {
                AuthAction::Select | AuthAction::Transaction { .. } => Authorization::Allow,
                AuthAction::Read { table_name, .. }
                | AuthAction::Insert { table_name }
                | AuthAction::Update { table_name, .. }
                | AuthAction::Delete { table_name }
                    if host_table(table_name) =>
                {
                    Authorization::Allow
                }
                _ => Authorization::Deny,
            }
        }))
        .map_err(|error| failure(error, "installing the authorizer"))
}

/// The bundled SQLite is the one this host was built and reviewed with, and its defensive mode
/// takes effect (research R1). Checked once per process, before any store is opened.
pub fn engine_check() -> Result<(), String> {
    static CHECKED: OnceLock<Result<(), String>> = OnceLock::new();
    CHECKED
        .get_or_init(|| {
            let version = rusqlite::version();
            if version != SQLITE_VERSION {
                return Err(format!(
                    "the linked SQLite is {version}, not {SQLITE_VERSION}, the one this host is built and reviewed with"
                ));
            }
            let connection = Connection::open_in_memory_with_flags(
                OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
            )
            .map_err(|error| format!("SQLite cannot open a connection: {error}"))?;
            let defensive = connection
                .set_db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE, true)
                .and_then(|_| connection.db_config(DbConfig::SQLITE_DBCONFIG_DEFENSIVE))
                .map_err(|error| format!("SQLite has no defensive mode: {error}"))?;
            if defensive {
                Ok(())
            } else {
                Err("SQLite does not turn its defensive mode on".to_owned())
            }
        })
        .clone()
}

/// The rule behind a SQLite error, with what the host was doing.
fn store_error(error: rusqlite::Error, doing: &str) -> StoreError {
    let detail = format!("{doing}: {error}");
    match error.sqlite_error_code() {
        Some(ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => StoreError::Busy(detail),
        Some(ErrorCode::DatabaseCorrupt | ErrorCode::NotADatabase) => StoreError::Damaged(detail),
        Some(ErrorCode::DiskFull) => StoreError::FileCap(detail),
        _ => StoreError::Unavailable(detail),
    }
}

fn failure(error: rusqlite::Error, doing: &str) -> Failure {
    match error.sqlite_error() {
        Some(code) if code.extended_code == ffi::SQLITE_READONLY_ROLLBACK => Failure::HotJournal,
        _ => Failure::Store(store_error(error, doing)),
    }
}

impl StoreConnection {
    /// The error of a write: a full file names the cap, since SQLite cannot tell it from a
    /// full disk.
    fn write_error(&self, error: rusqlite::Error, doing: &str) -> StoreError {
        match store_error(error, doing) {
            StoreError::FileCap(_) => StoreError::FileCap(format!(
                "the store's file reached its size limit of {} bytes, or the disk is full",
                self.file_cap_bytes
            )),
            other => other,
        }
    }

    pub(crate) fn get(&self, name: &[u8]) -> Result<Option<Vec<u8>>, StoreError> {
        self.connection
            .query_row(GET, params![name], |row| row.get(0))
            .optional()
            .map_err(|error| store_error(error, "reading a value"))
    }

    /// Stores `value` under `name`, replacing any value there.
    pub(crate) fn put(&self, name: &[u8], value: &[u8]) -> Result<(), StoreError> {
        self.connection
            .execute(PUT, params![name, value])
            .map(drop)
            .map_err(|error| self.write_error(error, "writing a value"))
    }

    pub(crate) fn remove(&self, name: &[u8]) -> Result<(), StoreError> {
        self.connection
            .execute(REMOVE, params![name])
            .map(drop)
            .map_err(|error| self.write_error(error, "deleting a value"))
    }

    /// At most `page` names that start with `prefix` and come after `after`, in byte order, and
    /// whether more remain.
    pub(crate) fn list(
        &self,
        prefix: &[u8],
        after: Option<&[u8]>,
        page: usize,
    ) -> Result<(Vec<Vec<u8>>, bool), StoreError> {
        let read = |error| store_error(error, "listing names");
        let after = after.unwrap_or_default();
        let limit = i64::try_from(page.saturating_add(1)).unwrap_or(i64::MAX);
        let mut names: Vec<Vec<u8>> = match successor(prefix) {
            Some(end) => {
                let mut statement = self.connection.prepare(LIST_BOUNDED).map_err(read)?;
                statement
                    .query_map(params![prefix, end, after, limit], |row| row.get(0))
                    .map_err(read)?
                    .collect::<Result<_, _>>()
                    .map_err(read)?
            }
            None => {
                let mut statement = self.connection.prepare(LIST_OPEN).map_err(read)?;
                statement
                    .query_map(params![prefix, after, limit], |row| row.get(0))
                    .map_err(read)?
                    .collect::<Result<_, _>>()
                    .map_err(read)?
            }
        };
        let more = names.len() > page;
        names.truncate(page);
        Ok((names, more))
    }

    /// Every entry, in byte order of names.
    pub(crate) fn entries(&self) -> Result<Vec<Entry>, StoreError> {
        let read = |error| store_error(error, "reading the entries");
        let mut statement = self.connection.prepare(ENTRIES).map_err(read)?;
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .map_err(read)?
            .collect::<Result<_, _>>()
            .map_err(read)
    }

    /// Deletes every entry; `secure_delete` overwrites their content.
    pub(crate) fn remove_all(&self) -> Result<(), StoreError> {
        self.connection
            .execute(REMOVE_ALL, [])
            .map(drop)
            .map_err(|error| self.write_error(error, "deleting every entry"))
    }

    pub(crate) fn usage(&self) -> Result<u64, StoreError> {
        let usage: i64 = self
            .connection
            .query_row(READ_USAGE, [], |row| row.get(0))
            .map_err(|error| store_error(error, "reading the usage"))?;
        u64::try_from(usage).map_err(|_| StoreError::Damaged(format!("the usage is {usage}")))
    }

    pub(crate) fn set_usage(&self, usage: u64) -> Result<(), StoreError> {
        let usage = i64::try_from(usage)
            .map_err(|_| StoreError::Unavailable(format!("a usage of {usage} bytes")))?;
        self.connection
            .execute(WRITE_USAGE, params![usage])
            .map(drop)
            .map_err(|error| self.write_error(error, "writing the usage"))
    }

    /// The id and origin the store records.
    pub(crate) fn identity(&self) -> Result<Identity, StoreError> {
        read_identity(&self.connection)
    }

    pub(crate) fn commit(&self) -> Result<(), StoreError> {
        self.connection
            .execute_batch("COMMIT")
            .map_err(|error| self.write_error(error, "committing"))
    }

    pub(crate) fn rollback(&self) -> Result<(), StoreError> {
        self.connection
            .execute_batch("ROLLBACK")
            .map_err(|error| store_error(error, "rolling back"))
    }

    /// Whether SQLite ended the transaction this connection opened, as it does on a full file
    /// (research R7): every later statement would then commit on its own.
    pub(crate) fn transaction_ended(&self) -> bool {
        self.connection.is_autocommit()
    }
}

/// What [`plant_foreign_store`] writes into a store.
#[cfg(feature = "test-network")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Foreign {
    /// A table the host never creates.
    ExtraTable,
    /// A trigger on the entries.
    Trigger,
    /// A view of the entries.
    View,
    /// Another plugin's id in the store's identity.
    OtherId,
}

/// Changes the store at `file` as someone outside the host could, for the hostile case
/// `foreign-store`, so no test opens a connection itself and `clippy.toml` keeps every SQLite
/// connection in this module (research R15). `test-network` is the runtime's test-only feature,
/// refused in release builds by its `compile_error!`; it gates every test hook, not only the
/// network's.
#[cfg(feature = "test-network")]
pub fn plant_foreign_store(file: &Path, kind: Foreign) -> Result<(), String> {
    let connection = Connection::open_with_flags(
        file,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|error| error.to_string())?;
    let sql = match kind {
        Foreign::ExtraTable => "CREATE TABLE extra (x)",
        Foreign::Trigger => {
            "CREATE TRIGGER spy AFTER INSERT ON entries BEGIN DELETE FROM store; END"
        }
        Foreign::View => "CREATE VIEW peek AS SELECT * FROM entries",
        Foreign::OtherId => "UPDATE store SET value = 'local/someone-else' WHERE key = 'id'",
    };
    connection
        .execute_batch(sql)
        .map_err(|error| error.to_string())
}

/// The shortest byte string greater than every string that starts with `prefix`, or `None` when
/// there is none (an empty prefix, or one of only `0xFF` bytes).
fn successor(prefix: &[u8]) -> Option<Vec<u8>> {
    let mut end = prefix.to_vec();
    while end.last() == Some(&0xff) {
        end.pop();
    }
    let last = end.last_mut()?;
    *last += 1;
    Some(end)
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
