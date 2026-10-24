# Research: Plugin Storage

Decisions behind [plan.md](plan.md), each with what it rests on. Spike results come from
throwaway projects outside the repository, built on Linux on 2026-10-24 (rusqlite 0.41.0 with
its bundled SQLite 3.54.0, release build); the plan's first tasks repeat the parts that matter on
all three systems.

## R1. The storage engine

**Decision**: `rusqlite` `=0.41.0` with the features `bundled` (SQLite 3.54.0 compiled into the
host, the same on the three systems, never the system's library), `hooks` (the authorizer, R4)
and `limits` (connection limits, R4). Nothing else: no `load_extension`, `functions`, `vtab`,
`backup` or `blob` feature. One database file per plugin store (constitution, Fixed Technical
Decisions: SQLite, one database per plugin).

Only `runtime/src/storage/` uses `rusqlite`; Clippy's `disallowed-types` and
`disallowed-methods` refuse `rusqlite::Connection` and its constructors anywhere else (R15), so
every statement the host runs is in that module and its line-by-line review covers them all.

**Spike**: the dependency tree adds `libsqlite3-sys` 0.39.0 (MIT, builds SQLite with `cc`),
`hashlink`, `hashbrown`, `foldhash` (Zlib), `fallible-iterator`, `fallible-streaming-iterator`,
`smallvec`, `bitflags`, `pkg-config`, `vcpkg`, `cc`, `shlex`, `find-msvc-tools`: MIT, Apache-2.0
or Zlib, all already allowed in `deny.toml`. rusqlite's MSRV is 1.89, below the workspace's
1.97.

**Rationale**: the fixed decision names SQLite; the bundled build pins one SQLite version for
every user, so hardening behaves the same everywhere, and no system library a user installed
changes it. rusqlite is the maintained binding with the authorizer and limits APIs.

**Alternatives considered**: the system SQLite (version differs per system, may lack
`SQLITE_DBCONFIG_DEFENSIVE`); `sqlx` (async, pulls a runtime); a key-value crate such as `redb`
or `sled` (would need a constitution amendment of the fixed decision); compiling SQLite with
`SQLITE_OMIT_LOAD_EXTENSION` through `LIBSQLITE3_FLAGS` in `.cargo/config.toml` (an environment
setting is easy to lose in another build of the host, such as the Tauri app; R4 closes extensions
on the connection instead, and the white-box case checks it).

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
function before any statement runs:

1. `OpenFlags`: read-write and create for `update` and for the CLI's `clear`; **read-only** for
   `render`, `show` and `list` (SQLite itself then refuses writes, under the host's own refusal,
   R7); never URI file names; `SQLITE_OPEN_EXRESCODE`.
2. `set_limit(SQLITE_LIMIT_ATTACHED, 0)`: no other database can be attached at all.
3. `set_db_config`: `DEFENSIVE` on (no `writable_schema`, no direct shadow-table writes),
   `ENABLE_TRIGGER` off, `ENABLE_VIEW` off, `TRUSTED_SCHEMA` off; extension loading stays off
   (its default, and rusqlite's `load_extension` feature is not compiled in).
4. `set_limit(SQLITE_LIMIT_LENGTH, …)` to the largest name plus value the host accepts, and
   `SQLITE_LIMIT_SQL_LENGTH` to the length of the host's longest statement.
5. Pragmas, set by the host before the authorizer is installed: `max_page_count` (the file cap,
   R10), `secure_delete = FAST` (deleted values are overwritten on their pages),
   `journal_mode = DELETE` and `synchronous = FULL` (R8), `busy_timeout` (R8).
6. An authorizer that allows only what the host's statements need, and denies everything else:
   reading and writing the two tables of R5, `SELECT`, and transactions. It denies every
   function, `ATTACH`, `DETACH`, `PRAGMA`, `CREATE`, `DROP`, `ALTER` and anything new.

The host's statements are a fixed set of prepared statements (R5), with every name and value
bound as a parameter, never formatted into SQL. They use no SQL function, so lengths and usage are
computed in Rust.

**Spike** (each layer alone, then together): with only the limit, `ATTACH` fails ("too many
attached databases - max 0"); with nothing set, `ATTACH` succeeds, as the control;
`load_extension('x')` fails with extension loading at its default; with `DEFENSIVE` on,
`PRAGMA writable_schema=ON` and a write to `sqlite_schema` fail; with the authorizer, `ATTACH`,
`PRAGMA`, `CREATE`, `length()` and `load_extension()` all fail as "not authorized". A name holding
NUL, `../`, quotes and `'; DROP TABLE entries; --` round-trips byte for byte.

**Rationale**: plugins never send SQL, so these layers protect against a mistake in the host's
own statements and against a store file someone changed outside the host (triggers and views in
a planted schema never run). Each layer is checked on its own by the white-box case
`store-hardened`, since no plugin can observe them (Principle VIII).

**Alternatives considered**: the authorizer alone (one mistake in its match would open
everything); `SQLITE_OMIT_LOAD_EXTENSION` at compile time (R1).

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
  says whether there is a next page.

**Rationale**: the schema check makes the file's content what the host wrote or nothing; storing
the identity in the file makes the binding checkable without a separate index; byte ranges keep
names opaque (spec FR-003).

## R6. Usage and the quota

**Decision**: an entry's usage is `len(name in UTF-8) + len(value) + storage_entry_cost_bytes`;
a store's usage is the sum over its entries, kept in the `store` table and changed in the same
transaction as every write. A write checks the usage it would leave: `usage - old entry + new
entry`, where the old entry is the one it replaces (spec edge case). A write is refused as
`over-quota` when that is above the quota, unless it does not raise usage while usage is already
above the quota after a lowered quota (spec FR-006): then it succeeds only if usage after it is
within the quota; deletes always succeed. The quota is the one the manifest declares now (R12);
the approval records the largest the user approved.

`storage_entry_cost_bytes` is provisional at 64. Spike: 2,000 entries of 32 bytes (16 + 16)
make a 94,208-byte file, about 11 bytes of SQLite overhead per entry beyond the two header pages;
64 per entry leaves room for B-tree pages left partly empty by deletes. The bench measures it on
the three systems (R13).

`usage` computed this way never depends on page sizes or the file system, so it is the same on
every system (spec FR-005); the file cap (R10) bounds what SQLite actually uses on disk.

**Alternatives considered**: the file's size as usage (differs per system and after deletes);
computing usage by scanning all entries at each write (cost grows with the store).

## R7. Read-only render, all-or-nothing update

**Decision**:

- A run opens its store connection on its first storage call, not before, so a run that never
  calls storage costs nothing.
- `render`: the connection is opened read-only (R4) and starts a read transaction (`BEGIN
  DEFERRED` and a first read), held until render ends, so every read sees the same state. `set`
  and `delete` are refused as `refused(render-only-reads)` by the host before SQLite is reached;
  the read-only connection is the layer beneath. A store that does not exist yet reads as empty.
- `update`: the connection is opened read-write and starts `BEGIN IMMEDIATE`: no other writer
  can start until it ends, and its own reads see its own writes. The transaction commits only
  after the plugin's `update` returned `ok` with valid output and the grant check of R9 passes;
  in every other case (an error, a trap, a limit, invalid or oversized output, storage taken away
  during the run) it rolls back. The store is committed before the host records the kept data;
  a commit that fails fails the update as `storage-failed`, naming the cause, and the previous
  kept data stays.
- A widget that declares storage must export `updater` (`missing-updater`, spec clarification 2);
  one that declares storage and no connector has its update run as any other.

**Rationale**: the transaction gives all-or-nothing for free, and the commit happens only in host
code after the guest is gone, so no guest behaviour can commit half an update. Read-only render
keeps render a pure view (spec FR-009).

**Alternatives considered**: committing each write (half-written data after a trap); a snapshot
copy of the store for render (costs a copy per render).

## R8. Concurrency between processes

**Decision**: `journal_mode = DELETE` (SQLite's rollback journal) with `synchronous = FULL`.
`busy_timeout` is what is left of the run's time limit, at most 2 s. A render holds a shared lock
while it reads; an update holds the reserved lock from its first storage call, and takes the
exclusive lock only to commit, waiting for renders in progress (each at most 1 s). An update that
cannot get its lock in time fails as `failed(busy)`, its writes not kept.

**Rationale**: the CLI runs one process per command, and two may run at once; the rollback
journal keeps one file per store at rest (no `-wal` and `-shm` files), and a crash during a
commit is rolled back by SQLite on the next open. Render and update never see part of each other
(spec FR-010, US4 scenario 6).

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
  every later call answers `not-granted`. The commit of R7 checks once more.
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
| `max_storage_quota_bytes` (the host cap) | 8 MiB | Room for settings, notes and caches of a widget; keeps 50 plugins under 1 GiB of files |
| `storage_entry_cost_bytes` | 64 | R6 |
| `max_storage_name_bytes` | 256 | Names are keys, not data |
| `max_storage_value_bytes` | 256 KiB | The size of an update's output; one value never dwarfs render's memory |
| `max_storage_calls_per_run` | 1,000 | Every call counted, refused ones included; bounds host time per run (R9) |
| `max_storage_written_bytes_per_update` | 1 MiB | Names and values of every `set` in one update, replaced ones included; bounds the journal |
| `storage_list_page` | 100 names | One page of `list` |
| File cap | `2 × quota + 64 KiB`, rounded up to pages | `max_page_count`; above it SQLite answers "full", reported as `over-limit` |

A call past `max_storage_calls_per_run` is `refused(over-limit)`; so is a `set` past the bytes per
update, a name or value past its size (`invalid-name` for names), and a write that hits the file
cap, which is checked by SQLite itself, so no sequence of writes and deletes passes it. The
`churn` and `many-entries` cases check that real use never reaches the cap with the entry cost
above.

## R11. Failures

The `storage` interface answers, besides `not-granted`, `refused(problem)` (the host refused
before changing anything: `render-only-reads`, `over-quota`, `over-limit`, `invalid-name`) and
`failed(problem)` (the store could not do it: `damaged`, `busy`, `unavailable`), each with a
detail for the author ([contract](contracts/widget-v1.2.wit)). A store that is `damaged` is never
deleted or repaired by the host; `runtime storage clear` empties it on the maintainer's word. A
full disk or an unwritable data directory is `unavailable`. Load-time refusals reuse existing
kinds: `invalid-manifest`, `contract-too-old`, `missing-updater`, `needs-new-approval`,
`approval-changed`; one new kind, `storage-failed`, names an update whose commit failed.

## R12. Approvals and the grant record, version 3

**Decision**:

- The grant record moves to version 3: an entry gains `storage_quota_bytes`, the largest quota the
  user approved for it, present only when an approved manifest declared storage. Versions 1 and 2
  are read as version 3 without it, and written back as 3.
- The approval digest (feature 003, `data-model.md`) gains `"storage": {"required": …,
  "quota_bytes": …}` only for a manifest that declares storage, so the digests of widgets without
  storage, and their kept data, do not change.
- `approve` needs `--expect <digest>` for any plugin that declares storage, required or optional
  (spec clarification 1), as for connectors; the approval records the declared quota.
- Load step 7 for storage: when storage is required, or optional and on, a declared quota above
  `storage_quota_bytes` (or one never approved) is `needs-new-approval`, naming both quotas, the
  approval unchanged. A lower quota needs nothing, and the record keeps what was approved, so a
  later version may go back up to it.
- `optional <plugin> storage on` is refused as `needs-new-approval` when the declared quota is
  above the approved one, or none was approved.
- Removing an approval, or turning storage off, deletes no store (spec FR-011); only `runtime
  storage clear` does (R14).

**Rationale**: the same pattern as connectors: what was shown is what is approved, and only an
increase needs the user again.

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
  now; every capability of 1.2.0 answers `not-granted` first.
- `V1_VERSION` becomes 1.2.0; a manifest that declares storage on a contract older than 1.2.0 is
  `contract-too-old`, naming 1.2.0. Every plugin of the repository built for 1.0.0 and 1.1.0 keeps
  building from `wit/history/` and runs unchanged (spec SC-007).

**Spike needed at the start of implementation**: a guest built against `stored-widget` that never
calls a `storage` function: whether wit-component keeps the import (then its manifest must
declare storage). The author guide says to declare what the world imports either way.

## R14. CLI

**Decision** (full contract in [contracts/cli.md](contracts/cli.md)):

- `permissions` shows a `storage:` line (required or optional, the quota, granted or not, the
  approved quota) and, when storage is declared, the approval digest.
- `approve` needs `--expect` when storage is declared.
- `runtime storage list` lists every store in the data directory: id, origin, usage, the approved
  quota or `not approved`, and `no file` when the origin's file is gone (spec clarification 3).
- `runtime storage show <plugin>` prints a store's usage and quota and every entry, one per line:
  names and text values escaped and quoted, values that are not UTF-8 as hexadecimal, each cut at
  1 KiB with the full size shown.
- `runtime storage clear <plugin>` and `runtime storage clear --id <id> --origin <path>` empty a
  store under an exclusive lock and `VACUUM` it; when the origin's file is gone, the store's file
  is removed.
- `runtime bench` gains storage numbers (R16).

## R15. Keeping the review on what matters

`clippy.toml` refuses `rusqlite::Connection` and its `open*` constructors outside
`runtime/src/storage/`, the module that holds every SQL statement (R1). `deny.toml` needs no new
license; `rusqlite` is pinned exactly, as `ureq` is, since a minor version could change the
authorizer's action list. The pull request template already lists `runtime/` as security core;
it gains `plugins/fixtures/store-probe` beside the other fixtures.

## R16. Measuring

`runtime bench` adds, for a plugin that declares storage, `storage_open_us` (first call of a
run), `storage_get_us`, `storage_set_us` (median per call, grant check included),
`storage_commit_us`, and `render_reading_100_us` (a render that reads 100 values). `bench.yml`
benches the example on the three systems; `performance.md` records the numbers, the entry cost
and file sizes measured, and confirms or changes R10 (spec SC-006).

## R17. The example and the fixture

- `plugins/examples/update-counter`: contract 1.2.0, `stored-widget`, storage required with a
  64 KiB quota. Its update increments a counter and keeps the last five notes the configuration
  gives it; its render reads them and shows them (spec FR-018, SC-008).
- `plugins/fixtures/store-probe`: contract 1.2.0, `stored-widget`; makes the storage calls its
  configuration lists, in `update` and in `render`, returns what each answered, and can end with a
  trap (panic, out of fuel, out of memory, an endless loop) or an error after them. The hostile
  cases pack it with each case's manifest, as feature 003 packs `net-probe`.
- `plugins/hostile/storage-file`: `wasm32-wasip2`, opens a path its configuration gives with
  `std::fs`; refused at load as `capability-refused`, listing `wasi:filesystem/*`.
