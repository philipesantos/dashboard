# Research: Plugin Storage

Decisions behind [plan.md](plan.md), each with what it rests on. Spike results come from
throwaway projects outside the repository, built on Linux on 2026-10-24 (rusqlite 0.41.0 with
its bundled SQLite 3.54.0, release build); the plan's first tasks repeat the parts that matter on
all three systems. An independent review of the plan against constitution v2.3.1 changed R1, R4,
R6 to R12, R14, R15 and R17; its findings, each reproduced before acting, are listed in
[checklists/requirements.md](checklists/requirements.md).

## R1. The storage engine

**Decision**: `rusqlite` `=0.41.0` with `default-features = false` and the features `bundled`
(SQLite 3.54.0 compiled into the host, the same on the three systems), `hooks` (the authorizer,
R4) and `limits` (connection limits, R4). Nothing else: not the default `cache` (statement cache,
`hashlink`), nor `load_extension`, `functions`, `vtab`, `backup` or `blob`. The runtime also
depends directly on `libsqlite3-sys` `=0.39.0`, which rusqlite only requires as `0.39.0` (a caret
requirement, rusqlite's `Cargo.toml`), so the SQLite the host compiles changes only through
`runtime/Cargo.toml`, never through a `Cargo.lock` update alone. One database file per plugin
store (constitution, Fixed Technical Decisions: SQLite, one database per plugin).

The build can still be switched to the system's SQLite by the environment
(`LIBSQLITE3_SYS_USE_PKG_CONFIG`) or given extra compile flags (`LIBSQLITE3_FLAGS`,
libsqlite3-sys' `build.rs`). So the storage module checks, before it opens any store, that the
linked SQLite is exactly 3.54.0 and that `SQLITE_DBCONFIG_DEFENSIVE` takes effect; otherwise
every store is `unavailable` and a plugin with required storage is refused at load (R11), and a
test fails the build on any other SQLite. Extra compile flags cannot be detected this way; the
white-box case `store-hardened` checks each layer on the SQLite that was actually built.

Only `runtime/src/storage/` uses `rusqlite`; Clippy's `disallowed-types` and
`disallowed-methods` refuse `rusqlite::Connection` and its constructors anywhere else (R15), so
every statement the host runs is in that module and its line-by-line review covers them all.

**Spike**: the dependency tree (with the default `cache`, which the decision leaves out) adds
`libsqlite3-sys` 0.39.0 (MIT, builds SQLite with `cc`), `hashlink`, `hashbrown`, `foldhash`
(Zlib), `fallible-iterator`, `fallible-streaming-iterator`, `smallvec`, `bitflags`, `pkg-config`,
`vcpkg`, `cc`, `shlex`, `find-msvc-tools`: MIT, Apache-2.0 or Zlib, all already allowed in
`deny.toml`. rusqlite's MSRV is 1.89, below the workspace's 1.97.

**Rationale**: the fixed decision names SQLite; the bundled build pins one SQLite version for
every user, so hardening behaves the same everywhere, and no system library a user installed
changes it. rusqlite is the maintained binding with the authorizer and limits APIs.

**Alternatives considered**: the system SQLite (version differs per system, may lack
`SQLITE_DBCONFIG_DEFENSIVE`); `sqlx` (async, pulls a runtime); a key-value crate such as `redb`
or `sled` (would need a constitution amendment of the fixed decision); compiling SQLite with
`SQLITE_OMIT_LOAD_EXTENSION` through `LIBSQLITE3_FLAGS` in `.cargo/config.toml` (an environment
setting is easy to lose in another build of the host, such as the Tauri app; R4 closes extensions
on the connection instead; the version check above catches the system library, and the white-box
case checks every layer on whatever was built).

## R2. Declaring storage in the manifest

**Decision**: storage is an entry of the manifest's `permissions` list, with one more field that
only it has:

```json
{ "name": "storage", "required": true, "quota_bytes": 262144 }
```

- `quota_bytes` is required when `name` is `storage` and refused on any other permission,
  naming the field (`permissions[1].quota_bytes`).
- `storage` appears at most once; a second entry is `invalid-manifest`, whatever its
  `required`.
- The quota is a whole number of usage bytes (R6), at least `storage_entry_cost_bytes` (one empty
  entry) and at most the host's cap (`max_storage_quota_bytes`, R10). The schema states the
  default cap as its maximum; the host checks its own cap at load, so a host whose cap is lower
  refuses the manifest as `invalid-manifest`, naming the field and both bounds (spec edge case:
  data kept, the plugin refused until a version within the cap).
- `wit/manifest.schema.json` gains the field with an `if`/`then` on `name`; the schema test checks
  the parser and the schema agree on every case above ([draft](contracts/manifest.schema.json)).

**Rationale**: being a permission, storage goes through feature 002's engine unchanged: required
ones approved together, optional ones off at first and toggled with `optional`, the registry and
the link plan. Putting the quota on the same entry keeps one declaration in one place.

**Alternatives considered**: a top-level `storage` object beside `permissions` (two places to
declare one capability, and the engine would need a second path, as connectors did because they
are data); a quota chosen by the host alone (the maintainer chose a declared quota).

## R3. Binding a store to a plugin

**Decision**: a store is the file `<data dir>/storage/<hex SHA-256 of "<id>\n<origin>">.sqlite`,
like kept data's file names (feature 003, R14). The host computes it when it links the plugin
(load step 9) and puts it in the linked plugin, beside the connector table: the store binding.
No function of the `storage` interface takes a store, path or location; every call acts on the
binding of the linked plugin. The file also holds its own identity (R5: `id`, `origin`), and
the host refuses a file whose identity differs from the binding as `damaged`, so a renamed or
copied file is never another plugin's store.

The origin is the package file's canonical path in this phase (feature 002, FR-020): a file that
copies an approved id from elsewhere gets no approval, and once approved in its own right, another
store. A new build at the same path keeps its store (spec FR-012). Phase 3's installed plugins
use their repository's permanent id as origin, in the installed data directory, which this
feature does not create.

**Rationale**: the plugin can only name entries, never stores, so the gate case ("read another
plugin's storage") has no input to work with; the host's own choice of file is a pure function of
what the approval already binds to.

**Alternatives considered**: one database for all plugins with a column per plugin (the fixed
decision says one per plugin, and a query bug would cross plugins); a file name from the id alone
(another origin would share it).

## R4. Hardening the connection

**Decision**: every connection the host opens on a store, in every mode, is set up by one
function, in this order, before any statement of a plugin's call runs:

1. The path: the data directory is made absolute and canonical when the runtime starts (the CLI's
   `--data` included), so a store's path always starts with `/` or a drive letter. The bundled
   build defines `SQLITE_USE_URI` (libsqlite3-sys' `build.rs`), so SQLite would read a name that
   starts with `file:` as a URI whatever the open flags; an absolute path never does.
2. `OpenFlags`: read-write for `update` and the CLI's `clear`, creating the file only in
   `update`; **read-only** for `render`, `show` and `list` (SQLite itself then refuses writes,
   under the host's own refusal, R7); `SQLITE_OPEN_EXRESCODE`.
3. `set_limit(SQLITE_LIMIT_ATTACHED, 0)`: no other database can be attached at all.
4. `set_db_config`: `DEFENSIVE` on (no `writable_schema`, no direct shadow-table writes),
   `ENABLE_TRIGGER` off, `ENABLE_VIEW` off, `TRUSTED_SCHEMA` off; extension loading stays off
   (its default, and rusqlite's `load_extension` feature is not compiled in).
5. `set_limit(SQLITE_LIMIT_LENGTH, …)` to the largest name plus value the host accepts plus 4 KiB
   (SQLite counts a row's record header against it: the spike refused a 256 B name with a 256 KiB
   value at exactly their sum). The host's own checks (R10) are the exact limits; this is the
   layer beneath. (`SQLITE_LIMIT_SQL_LENGTH` is set in step 9, see there.)
6. Pragmas: `max_page_count` (the file cap, R10), `secure_delete = ON` (deleted names and values
   are overwritten with zeros, on B-tree, overflow and free pages; `FAST` would leave free and
   overflow pages as they were), `journal_mode = DELETE` and `synchronous = FULL` (R8),
   `busy_timeout` (R8), `cache_size = -256` (at most 256 KiB of page cache per connection, so a
   store never holds much host memory, and an update of 1 MiB spills pages to the file, which
   the `killed-mid-update` case needs to leave a hot journal, R8).
7. In `update` and `clear` only, on a file with no tables yet: the host's `CREATE TABLE`
   statements and its identity rows (R5).
8. The schema check (R5): it reads `sqlite_schema`, so it runs before the authorizer.
9. `SQLITE_LIMIT_SQL_LENGTH` to the length of the host's longest statement (set here, not in
   step 5: creating the tables in step 7 runs SQLite's own, longer statements, which the
   implementation found), then an authorizer that allows only what the host's call statements
   need, and denies everything
   else: reading and writing the two tables of R5, `SELECT`, and transactions. It denies every
   function, `ATTACH`, `DETACH`, `PRAGMA`, `CREATE`, `DROP`, `ALTER`, reads of `sqlite_schema`,
   and anything new.

Steps 7 and 8 are host statements with no plugin input; from step 9 on, only the fixed prepared
statements of R5 run, with every name and value bound as a parameter, never formatted into SQL.
They use no SQL function, so lengths and usage are computed in Rust.

**Spike** (each layer alone, then together): with only the limit, `ATTACH` fails ("too many
attached databases - max 0"); with nothing set, `ATTACH` succeeds, as the control;
`load_extension('x')` fails with extension loading at its default; with `DEFENSIVE` on,
`PRAGMA writable_schema=ON` and a write to `sqlite_schema` fail; with the authorizer, `ATTACH`,
`PRAGMA`, `CREATE`, `length()` and `load_extension()` all fail as "not authorized". A name holding
NUL, `../`, quotes and `'; DROP TABLE entries; --` round-trips byte for byte.

**Testing each layer** (the white-box case `store-hardened`): the setup is two production
functions that every open calls in sequence, `configure` (steps 1–8) and `authorize` (step 9). The
test calls them as `open` does, with no switch that production code does not use, and reads each
layer's state back
(`SQLITE_LIMIT_ATTACHED`, `LENGTH` and `SQL_LENGTH` queried with `-1`; the `db_config` getters for
`DEFENSIVE`, `ENABLE_TRIGGER`, `ENABLE_VIEW`, `TRUSTED_SCHEMA`; `PRAGMA max_page_count`,
`secure_delete` and `cache_size` read between `configure` and `authorize`) and makes one attempt per layer on the
finished connection: `ATTACH`, `load_extension()`, any other function, `PRAGMA`, `CREATE`, a
statement longer than the SQL limit, a value past the length limit, a write on the render
connection. A store file with a trigger or a view planted in it is the case `foreign-store`, whose files a
test-only function of the storage module writes, so no test opens a connection itself (R15).

**Rationale**: plugins never send SQL, so these layers protect against a mistake in the host's
own statements and against a store file someone changed outside the host (triggers and views in
a planted schema never run, and the schema check refuses the file anyway). Each layer is checked
by itself, since no plugin can observe them (Principle VIII).

**Alternatives considered**: the authorizer alone (one mistake in its match would open
everything); `SQLITE_OMIT_LOAD_EXTENSION` at compile time (R1); test-only switches to turn layers
off one by one (they would test a setup production never runs).

## R5. The store's format and statements

**Decision**: two tables, created by the host when it creates the file, and checked every time
it opens one:

```sql
CREATE TABLE store (key TEXT PRIMARY KEY NOT NULL, value ANY NOT NULL) STRICT, WITHOUT ROWID;
-- format = 1, id, origin, usage (the store's usage in bytes, R6)
CREATE TABLE entries (name BLOB PRIMARY KEY NOT NULL, value BLOB NOT NULL) STRICT, WITHOUT ROWID;
```

- Names are stored as the bytes of their UTF-8, so they compare and sort as bytes, whatever their
  content, and a NUL is an ordinary byte.
- On open, the host checks that `sqlite_schema` holds exactly these two tables and their indexes,
  with the SQL it writes, that `format` is 1 and that `id` and `origin` match the binding;
  anything else is `damaged` (R11), and nothing is changed or deleted.
- Statements: get one value; insert or replace one entry; delete one entry; read one value's
  length (for usage); list names in a range (below); read and write `usage`.
- Listing by prefix is a range on the name bytes: `name >= prefix AND name < successor(prefix)
  AND name > after ORDER BY name LIMIT page + 1`, where `successor` is the shortest byte string
  greater than every string that starts with the prefix (no upper bound when there is none). No
  `LIKE` or `GLOB`, whose `%` and `_` would turn a name into a pattern. One more row than the page
  says whether there is a next page. `prefix` and `after` follow the name's length limit
  (`invalid-name` past it); an empty `prefix` lists every name.
- `runtime storage list` has no binding to compare with, so it checks that each file's name is the
  hash of the `id` and `origin` stored in it, and shows any other file as `damaged`: a file renamed
  or copied in the directory never shows another plugin's identity.

**Rationale**: the schema check makes the file's content what the host wrote or nothing; storing
the identity in the file makes the binding checkable without a separate index; byte ranges keep
names opaque (spec FR-003).

## R6. Usage and the quota

**Decision**: an entry's usage is `len(name in UTF-8) + len(value) + storage_entry_cost_bytes`;
a store's usage is the sum over its entries, kept in the `store` table and changed in the same
transaction as every write. A write is refused as `over-quota` when the usage it would leave,
`usage - old entry + new entry` (the old entry being the one it replaces, spec edge case), is
above the quota; deletes always succeed. The same rule covers a store already over a lowered
quota (spec FR-006): only writes that leave usage within the quota pass. The quota is the one the
manifest declares now; the approval records the quota the user last approved (R12).

`storage_entry_cost_bytes` is provisional at 64. Spike: 2,000 entries of 32 bytes (16 + 16)
make a 94,208-byte file, about 11 bytes of SQLite overhead per entry beyond the two header pages;
64 per entry leaves room for B-tree pages left partly empty by deletes. The bench measures it on
the three systems (R16).

Usage is not disk space. A value just too large to fit in its B-tree page takes a whole 4 KiB
overflow page: in the spike, 1 MiB of usage made files of 0.54 MB (32-byte values), 1.09 MB
(500 B), 4.58 MB (1,000 B), 4.19 MB (1,100 B), 3.14 MB (1,500 B), 2.37 MB (2,000 B) and 1.21 MB
(4,000 B): at most 4.4 times the usage. The file cap (R10) is set above that, so the quota, not
the cap, is what a widget meets in normal use.

`usage` computed this way never depends on page sizes or the file system, so it is the same on
every system (spec FR-005); the file cap (R10) bounds what SQLite actually uses on disk.

**Alternatives considered**: the file's size as usage (differs per system and after deletes);
counting usage in pages, or rounding values up to the overflow pages they take (closer to disk,
but authors could no longer predict usage from their data, and a SQLite upgrade could change it;
the maintainer chose logical usage with a higher file cap); computing usage by scanning all
entries at each write (cost grows with the store).

## R7. Read-only render, all-or-nothing update

**Decision**:

- A run opens its store connection on its first storage call, not before, so a run that never
  calls storage costs nothing.
- `render`: the connection is opened read-only (R4) and starts a read transaction (`BEGIN
  DEFERRED` and a first read), held until render ends, so every read sees the same state. `set`
  and `delete` are refused as `refused(render-only-reads)` by the host before SQLite is reached;
  the read-only connection is the layer beneath. A store that does not exist yet reads as empty.
- `update`: the connection is opened read-write and starts `BEGIN IMMEDIATE`: no other writer
  can start until it ends, and its own reads see its own writes.
- **A transaction SQLite ended is the end of the run's storage.** SQLite rolls the whole
  transaction back by itself on some errors, `SQLITE_FULL` (the file cap) and `SQLITE_IOERR`
  among them, and then runs every later statement in autocommit mode, each committed on its own
  (spike: after "database or disk is full" in `BEGIN IMMEDIATE`, `is_autocommit()` is true, the
  next insert succeeds and stays, and `ROLLBACK` answers "no transaction is active"). So after
  every statement that fails in an update, the host checks `is_autocommit()`; when the transaction
  is gone, or a write failed at all (a read-only file fails a write without ending the
  transaction, which the implementation found), the run is aborted: the failing call answers its
  rule (`over-limit` for the file cap,
  `over-limit` also for a full disk, `unavailable` for an I/O error, R11), every later storage call answers
  `failed(run-aborted)` without running anything, and the update fails as `storage-failed`, with
  nothing of it kept. The host never runs a plugin's statement outside its own transaction.
- **Commit.** The transaction commits only after the plugin's `update` returned `ok` with valid
  output and the run was not aborted, and only while the grant record still grants storage from
  this origin: the host takes the grant record's shared lock, reads it, commits, and only then
  releases the lock, so a removal or a turn-off cannot fall between the check and the commit (a
  grant change waits for the lock; grant changes never take a store's lock, so the order cannot
  deadlock). In every other case (an error, a trap, a limit, invalid or oversized output, an
  aborted run, storage taken away during the run) it rolls back. The store is committed before the
  host records the kept data; a commit that fails fails the update as `storage-failed`, naming the
  cause, and the previous kept data stays.
- A widget that declares storage must export `updater` (`missing-updater`, spec clarification 2);
  one that declares storage and no connector has its update run as any other.

**Rationale**: the transaction gives all-or-nothing, and the commit happens only in host code
after the guest is gone, so no guest behaviour can commit part of an update: SQLite's own
rollback is caught and turned into the end of the run, never into autocommit. Read-only render
keeps render a pure view (spec FR-009).

**Alternatives considered**: committing each write (half-written data after a trap); a snapshot
copy of the store for render (costs a copy per render); savepoints around each write so a full
file undoes only that write (SQLite may still roll back the whole transaction on `SQLITE_FULL`, as
the spike showed).

## R8. Concurrency between processes

**Decision**: `journal_mode = DELETE` (SQLite's rollback journal) with `synchronous = FULL`.
`busy_timeout` is what is left of the run's time limit, at most 2 s. A render holds a shared lock
while it reads; an update holds the reserved lock from its first storage call, and takes the
exclusive lock only to commit, waiting for renders in progress (each at most 1 s). An update that
cannot get its lock in time fails as `failed(busy)`, its writes not kept.

**A process killed during an update** (Ctrl-C, a crash, a power cut) leaves a hot journal beside
the store. A read-write open rolls it back; a read-only open cannot, and fails with
`SQLITE_READONLY_ROLLBACK` (extended code 776; spike: a child process aborted after writing past
its page cache left the journal, the read-only open failed with 776, and a read-write open then
read the state from before the update and removed the journal). So when a read-only open
(`render`, `show`, `list`) meets 776, the host opens the store read-write once, hardened as in R4,
reads one row so SQLite rolls the journal back, closes it, and opens read-only again; if that fails,
the call answers `failed(busy)`, never `damaged`, so the maintainer is never told to clear a store
that only needs recovering. The case `killed-mid-update` checks it.

**Rationale**: the CLI runs one process per command, and two may run at once; the rollback journal
keeps one file per store at rest (no `-wal` and `-shm` files), and SQLite's own recovery restores
the state from before an interrupted update. Render and update never see part of each other (spec
FR-010, US4 scenario 6).

**Alternatives considered**: WAL mode (readers never wait, but two more files per store, and a
reader held open stops checkpoints; renders are short, so waiting is cheap).

## R9. Linking the storage capability and taking it away

**Decision**:

- The permission registry gains `storage`: interface `dashboard:widget/storage`, since 1.2.0, no
  v0 interface. Load steps 6–9 then treat it as they treat `log`: an import without a declaration
  is `not-declared`; a granted import gets the real interface, an optional one that is off gets
  the stand-in, which holds no binding and answers `not-granted` to every call.
- The real interface holds the store binding (R3), the run's phase, its counters (R10) and the
  plugin's id and origin. Each storage call re-reads the grant record, under its lock, as a
  connector call does before it sends (feature 003, R12); when storage is no longer granted from
  this origin, the call answers `not-granted`, the run's transaction is marked to roll back, and
  every later call answers `not-granted`. The commit of R7 checks once more, under the grant
  record's lock, so a removal after the plugin's last call is caught too.
- The stand-in answers every call `not-granted` and counts nothing: it holds no store and reaches
  nothing, so it needs no limit, and Principle II leaves it no other answer.
- An update whose storage was taken away fails as `not-granted`, naming storage, and its writes
  are not kept.

**Spike**: `GrantStore::entry` on a record of 20 plugins takes 64 µs per call (Linux, release);
at the call limit of R10 that is under 70 ms of a run. The bench measures it on the three systems
(R13).

**Rationale**: Principle II: the binding exists only when storage was granted at link time, and
call-time checks only take it away. The same check connectors use keeps one rule for "from the
next call, from any process" (spec FR-011).

**Alternatives considered**: checking only at commit (reads in render would continue after the
storage is turned off); checking the record's modification time to skip the read (time
resolution differs per file system).

## R10. Limits

Provisional, in `Limits`, measured before the feature closes (R13):

| Limit | Value | Why |
| --- | --- | --- |
| `max_storage_quota_bytes` (the host cap) | 8 MiB | Room for settings, notes and caches of a widget |
| `storage_entry_cost_bytes` | 64 | R6 |
| `max_storage_name_bytes` | 256 | Names are keys, not data; also the limit of `list`'s `prefix` and `after` |
| `max_storage_value_bytes` | 256 KiB | The size of an update's output; one value never dwarfs render's memory |
| `max_storage_calls_per_run` | 1,000 | Every call counted, refused ones included; bounds host time per run (R9) |
| `max_storage_written_bytes_per_update` | 1 MiB | Names and values of every `set` in one update, replaced ones included; bounds the journal |
| `storage_list_page` | 100 names | One page of `list`; fixed by the host, so no call can ask for more |
| File cap | `5 × approved quota + 64 KiB`, rounded up to pages, never below the file's current size | `max_page_count`; above the worst measured file (4.4 × usage, R6); at the 8 MiB cap, about 40 MiB |

A call past `max_storage_calls_per_run` is `refused(over-limit)`; so is a `set` past the bytes per
update, and a value past its size; a name, prefix or `after` past its size is `invalid-name`. The
file cap is checked by SQLite itself, so no sequence of writes and deletes passes it; a write that
reaches it ends the run's transaction (R7). The cap follows the approved quota, not the declared
one: `max_page_count` cannot be set below a file's current size (spike: set to 3 on a 1,119-page
file, it stays 1,119), so a cap computed from a lowered quota would sit below the file for good.
After a re-approval at a smaller quota, the cap is that quota's, or the file's current size when
that is larger. The `churn` and `many-entries` cases fill the quota with values from 32 B to 4 KiB,
1,000 B among them, and check that the quota is met first and the file stays under its cap.

## R11. Failures

The `storage` interface answers, besides `not-granted`, `refused(problem)` (the host refused
before changing anything: `render-only-reads`, `over-quota`, `over-limit`, `invalid-name`) and
`failed(problem)` (the store could not do it: `damaged`, `busy`, `unavailable`, and
`run-aborted` after the run's transaction ended, R7), each with a detail for the author
([contract](contracts/widget-v1.2.wit)). The file cap answers `failed(over-limit)`: unlike the
refusals, it ends the run. A store that is `damaged` is never changed, deleted or repaired by the
host on its own; `runtime storage clear` removes it on the maintainer's word (R14). An
unwritable data directory or store file is `unavailable`. A full disk answers as the file cap
does, `failed(over-limit)`, with a detail naming both causes: SQLite reports both as
`SQLITE_FULL`, and the host cannot tell them apart without asking the system for free space,
which the standard library does not offer (found in implementation).

The data directory itself (spec edge case): at load step 8a, before linking, the host checks that
`<data dir>/storage/` exists or can be created as a directory it can write, and that the SQLite
check of R1 passed. If not, a plugin whose storage is required is refused as `storage-failed`,
naming the cause; one whose storage is optional and on is linked to the real interface, whose
calls answer `failed(unavailable)`, never as if they had worked.

Load-time refusals otherwise reuse existing kinds: `invalid-manifest`, `contract-too-old`,
`missing-updater`, `needs-new-approval`, `approval-changed`. One new kind, `storage-failed`, names
an update whose storage could not be committed (busy, damaged, unavailable, the file cap, an
aborted run) and a plugin refused at load because storage is unavailable.

## R12. Approvals and the grant record, version 3

**Decision**:

- The grant record moves to version 3: an entry gains `storage_quota_bytes`, the quota of the
  user's last approval, present only when an approved manifest declared storage. Versions 1 and 2
  are read as version 3 without it, and written back as 3.
- The approval digest (feature 003, `data-model.md`) gains `"storage": {"required": …,
  "quota_bytes": …}` only for a manifest that declares storage, so the digests of widgets without
  storage, and their kept data, do not change.
- `approve` needs `--expect <digest>` for any plugin that declares storage, required or optional
  (spec clarification 1), as for connectors; the approval sets `storage_quota_bytes` to the quota
  it showed, smaller or larger than before (spec clarification 4).
- Load step 7 for storage: when storage is required, or optional and on, a declared quota above
  `storage_quota_bytes` (or one never approved) is `needs-new-approval`, naming both quotas, the
  approval unchanged. A lower quota needs nothing and changes nothing in the record, so a later
  version may go back up to the approved quota; after the user approves a smaller quota, going
  above it needs the user again.
- `optional <plugin> storage on` is refused as `needs-new-approval` when the declared quota is
  above the approved one, or none was approved.
- Removing an approval, or turning storage off, deletes no store (spec FR-011); only `runtime
  storage clear` does (R14).

**Rationale**: the same pattern as connectors: what was shown is what is approved, and only an
increase over the user's last decision needs the user again.

**Alternatives considered**: keeping the largest quota ever approved (fewer approvals, but a quota
the user last saw smaller would run without asking; the maintainer chose the last one shown).

## R13. Contract 1.2.0

**Decision**:

- Copy `wit/v1/widget.wit` (1.1.0) to `wit/history/1.1.0/widget.wit`, unchanged, before changing
  `wit/v1`.
- `wit/v1/widget.wit` becomes `dashboard:widget@1.2.0`: every interface of 1.1.0 unchanged; a new
  `storage` interface ([draft](contracts/widget-v1.2.wit)); two new worlds, `stored-widget`
  (imports `log` and `storage`, exports `renderer` and `updater`) and `connected-stored-widget`
  (imports `log`, `connectors` and `storage`, exports both). The host binds the widest world,
  `connected-stored-widget`, and keeps looking up exports by their exact names (feature 003, R11).
- `runtime/tests/contract_v1.rs` checks 1.2.0 against 1.1.0, and 1.1.0 against 1.0.0, as it does
  now; every capability of 1.2.0 answers `not-granted` first. The comparison alone would follow a
  frozen file that someone edited, so the test also pins `wit/history/1.0.0/widget.wit` and
  `wit/history/1.1.0/widget.wit` by their SHA-256.
- `V1_VERSION` becomes 1.2.0; a manifest that declares storage on a contract older than 1.2.0 is
  `contract-too-old`, naming 1.2.0. Every plugin of the repository built for 1.0.0 and 1.1.0 keeps
  building from `wit/history/` and runs unchanged (spec SC-007).

**Spike** (T003, Linux, wit-bindgen 0.63 and wit-component 0.262, as `cargo xtask plugins`
uses): a guest built with `generate!` on `wit/v1`, world `stored-widget`, whose `render` and
`update` never call a `storage` function, encodes to a component that imports nothing, in a
release and in a debug build; the same guest calling `storage::get` once imports
`dashboard:widget/storage@1.2.0`. So the import follows the calls, not the world: a plugin that
never calls storage need not declare it, and one that declares it without importing it loads, as
any declared permission it does not use (spec edge case). The author guide says to declare
storage when the plugin calls it.

## R14. CLI

**Decision** (full contract in [contracts/cli.md](contracts/cli.md)):

- `permissions` shows a `storage:` line (required or optional, the quota, granted or not, the
  approved quota) and, when storage is declared, the approval digest.
- `approve` needs `--expect` when storage is declared.
- `runtime storage list` lists every store in the data directory: id, origin, usage, the approved
  quota or `not approved`, and `no file` when the origin's file is gone (spec clarification 3).
- `runtime storage show <plugin>` prints a store's usage and quota and every entry, one per line:
  names and text values in double quotes, escaped as log messages are and, inside the quotes, `"`
  as `\"` and `\` as `\\` (feature 001's escaping leaves both as they are, `text.rs`, so a name
  such as `count" = "999` could otherwise pass for two fields); values that are not UTF-8 as
  `bytes(<size>)` and hexadecimal; each cut at 1 KiB with the full size shown. A new function
  beside `text::escape` does the quoting, tested on its own.
- `runtime storage clear <plugin>` and `runtime storage clear --id <id> --origin <path>` empty a
  store in one `BEGIN EXCLUSIVE` transaction on a connection hardened as in R4: every entry
  deleted, usage set to 0, committed; `secure_delete = ON` overwrites the deleted content. No
  `VACUUM`: it attaches a database internally, which the attach limit and the authorizer refuse
  (spike: "too many attached databases - max 0"), and it cannot run inside a transaction; the
  freed pages are reused by later writes, and the file never passes its cap. A store that is
  `damaged`, or whose origin's file is gone, is removed instead: under an exclusive lock taken by a
  read-write open (which first rolls back any hot journal, R8), the host closes it and deletes the
  database file, the journal being already gone; a store busy in another process is `busy`, and
  nothing changes.
- `runtime bench` gains storage numbers (R16).

## R15. Keeping the review on what matters

`clippy.toml` refuses `rusqlite::Connection` and its `open*` constructors outside
`runtime/src/storage/`, the module that holds every SQL statement (R1). `deny.toml` needs no new
license. `rusqlite` and `libsqlite3-sys` are pinned exactly, as `ureq` is (R1): a minor version
could change the authorizer's action list, and a new `libsqlite3-sys` bundles another SQLite,
which is C code in the security core. The pull request template already lists `runtime/` as
security core; its `Cargo.lock` line, which names the network stack's crates, gains `rusqlite`
and `libsqlite3-sys`, and its fixtures line gains `plugins/fixtures/store-probe`.

## R16. Measuring

The numbers come from a bench binary, `bench/src/bin/storage_cost.rs` in the `dashboard-bench`
crate, as `compile_cost` does for loading: it packs the `store-probe` fixture with a manifest that
declares storage, approves it in a grant record file of its own (so every call re-reads a real
record, R9), writes 100 values in a warm-up update, and runs each shape 100 times. Per-call costs
are differences of medians between runs that differ only in their storage calls, so no timing
code enters the host: `storage_open_us` (a render with one `get` minus one with none),
`storage_get_us` and `storage_set_us` (100 calls minus one, divided by 99; grant check included),
`storage_update_100_us` (an update of 100 sets that commits, minus the same update with no
storage calls: the open, the writes and the commit; a commit cannot be measured against a
rollback, which costs about the same, as the first runs showed), and `render_reading_100_us` (a whole render that reads 100 values). It also prints
the file size of a store filled to its quota with values of 32 B, 1,000 B and 4 KiB. `bench.yml`
runs it on the three systems; `performance.md` records the numbers, the entry cost and the file
sizes measured, and confirms or changes R10 (spec SC-006).

## R17. The example and the fixture

- `plugins/examples/update-counter`: contract 1.2.0, `stored-widget`, storage required with a
  64 KiB quota. Its update increments a counter and keeps the last five notes the configuration
  gives it; its render reads them and shows them (spec FR-018, SC-008).
- `plugins/fixtures/store-probe`: contract 1.2.0, `stored-widget`; makes the storage calls its
  configuration lists, in `update` and in `render`, returns what each answered, and can end with a
  trap (panic, out of fuel, out of memory, an endless loop) or an error after them. A `spin` step
  burns a given number of loop iterations between calls, so a test can change the grants while the
  plugin runs. The hostile cases pack it with each case's manifest, as feature 003 packs
  `net-probe`.
- Changing grants during a run without a race: an update's first write creates the store's
  `-journal` file, so the test waits for that file, then turns storage off or removes the approval
  from another grant store, while the probe spins with no further storage call and then returns
  `ok`; only the commit check of R7 can then refuse the writes. A variant makes one more call after
  the change (`not-granted`). For render, which leaves no file to watch, the test runs with a
  render time limit of 10 s, starts a render that reads, spins for about 3 s and reads again, and
  changes the grants 1 s after the start; the second read must answer `not-granted`. A run of the
  host taking longer than 1 s to reach its first read would fail the test, never pass it wrongly.
- `killed-mid-update`: the test starts a child process, the test binary itself running an
  ignored half of the case (not `runtime update`, whose CPU budget would end the update before
  the test could kill it), with the probe writing more than SQLite's page cache holds and then
  spinning; it waits until the store file has grown by those pages, kills the process, and checks
  that `render` and `storage show` read the state from before the update (R8).
- `plugins/hostile/storage-file`: `wasm32-wasip2`, opens a path its configuration gives with
  `std::fs`; refused at load as `capability-refused`, listing `wasi:filesystem/*`.
