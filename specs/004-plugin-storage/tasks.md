---

description: "Task list for plugin storage (feature 004)"
---

# Tasks: Plugin Storage

**Input**: Design documents from `specs/004-plugin-storage/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md

**Tests**: Required. The hostile suite is the security gate (Principle VIII) and gains the 23
cases of `data-model.md` (Hostile cases), plus the 8 tests of its "Other tests named" table.
Every test task comes before the implementation it checks and fails until that implementation
lands. Test helpers make a test **fail, never skip**, when a fixture is missing. No test reaches
the internet, and no test reads `specs/`.

**Organization**: Tasks are grouped by user story. Contract 1.2.0, the manifest field, the
storage module (connection, calls, commit), the engine, the grant record v3 and loading block
every story, so they are foundational and built test first; the stories then add the example,
the hostile cases, the quota and all-or-nothing checks, the CLI's storage commands and the
author tooling.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to (US1–US6)

## Path Conventions

Paths follow plan.md: `runtime/` and `wit/` (security core), `plugins/` (guest crates), `bench/`,
`docs/`, `.github/`. Built plugins go to `target/plugins/<crate-name>.wasm`. Field rules are in
`data-model.md`; decisions and numbers in `research.md` (R1–R17); CLI output in
`contracts/cli.md`; the contract draft in `contracts/widget-v1.2.wit`; the schema draft in
`contracts/manifest.schema.json`.

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: contract 1.2.0 and its frozen predecessor, the schema, dependencies, lint rules and
the new plugin crates. The runtime still behaves as in feature 003 at the end of this phase, so
every existing test keeps passing.

- [X] T001 Freeze contract 1.1.0: copy `wit/v1/widget.wit` unchanged to `wit/history/1.1.0/widget.wit` (still `dashboard:widget@1.1.0`); point `wit_bindgen::generate!` in `plugins/fixtures/net-probe/src/lib.rs` and `plugins/examples/github-releases/src/lib.rs` to `"../../../wit/history/1.1.0"` (research R13, SC-007); confirm `cargo xtask plugins` and `cargo test --workspace` still pass
- [X] T002 Rewrite `wit/v1/widget.wit` as `dashboard:widget@1.2.0` from `specs/004-plugin-storage/contracts/widget-v1.2.wit`: `log`, `connectors`, `renderer` and `updater` unchanged from 1.1.0; interface `storage` with `enum rule` of exactly the eight cases `render-only-reads`, `over-quota`, `over-limit`, `invalid-name`, `damaged`, `busy`, `unavailable`, `run-aborted`, `record problem { rule, detail: string }`, `variant storage-error { not-granted, refused(problem), failed(problem) }` (`not-granted` first), `record page { names: list<string>, next: option<string> }`, `record store-usage { used-bytes: u64, quota-bytes: u64 }`, and `get: func(name: string) -> result<option<list<u8>>, storage-error>`, `set: func(name: string, value: list<u8>) -> result<_, storage-error>`, `delete: func(name: string) -> result<_, storage-error>`, `%list: func(prefix: string, after: option<string>) -> result<page, storage-error>`, `usage: func() -> result<store-usage, storage-error>`; worlds `widget` and `connected-widget` unchanged, plus `stored-widget { import log; import storage; export renderer; export updater; }` and `connected-stored-widget { import log; import connectors; import storage; export renderer; export updater; }`; keep every doc comment and drop the draft wording; the runtime's `bindgen!` stays on world `connected-widget` for now, so the build still passes and the 1.0.0 and 1.1.0 plugins still load (Wasmtime matches their imports against the 1.2.0 interfaces); `runtime/tests/contract_v1.rs` keeps passing, since `storage-error` starts with `not-granted`
- [X] T003 Run the spike of research R13 in the scratchpad, outside the repository: a guest crate built with `wit_bindgen::generate!` on `wit/v1`, world `stored-widget`, whose `render` and `update` never call a `storage` function; record with `wasm-tools component wit` whether the component still imports `dashboard:widget/storage@1.2.0`, and write the result into research R13 (`specs/004-plugin-storage/research.md`): if it keeps the import, the manifest of such a guest must declare storage, which the author guide (T054) says; if not, the guide says to declare what the plugin calls
- [X] T004 [P] Add `quota_bytes` to `wit/manifest.schema.json` exactly as in `specs/004-plugin-storage/contracts/manifest.schema.json`: an integer from 64 to 8388608, required by an `if`/`then` when `name` is `storage` and refused by the `else` on any other permission, and `permissions` holding at most one `storage` entry (`contains` with `minContains` 0 and `maxContains` 1), keeping the file's existing formatting for the other fields
- [X] T005 Update `runtime/Cargo.toml` (research R1, R15): `rusqlite = { version = "=0.41.0", default-features = false, features = ["bundled", "hooks", "limits"] }` and `libsqlite3-sys = { version = "=0.39.0", default-features = false }`, each with a one-line comment saying why (the first is the only SQLite binding, without the statement `cache` feature; the second pins the SQLite that `bundled` compiles, 3.54.0, which rusqlite asks for only as a caret requirement); confirm `cargo tree -p runtime -e normal` contains `libsqlite3-sys v0.39.0` and no `hashlink`, `cargo tree -d -p runtime` adds no avoidable duplicate, and `cargo deny check` passes with no new license
- [X] T006 [P] Extend `clippy.toml` (research R15): `disallowed-types` gains `rusqlite::Connection`, and `disallowed-methods` gains `rusqlite::Connection::open`, `open_with_flags`, `open_with_flags_and_vfs`, `open_in_memory`, `open_in_memory_with_flags` and `open_in_memory_with_flags_and_vfs`, each with `reason = "only runtime/src/storage/ opens a store (Principle IX, research R15)"` (with `allow-invalid = true` if Clippy warns in crates without `rusqlite`); the storage module will allow them with `#![allow(clippy::disallowed_types, clippy::disallowed_methods)]` as the first line of `runtime/src/storage/mod.rs`, which also covers its submodules; confirm `cargo clippy --workspace --all-targets --locked -- -D warnings` and Clippy on `plugins/` for `wasm32-unknown-unknown` still pass
- [X] T007 Create `plugins/fixtures/store-probe/` (research R17): contract 1.2.0, `wit_bindgen::generate!({ path: "../../../wit/v1", world: "stored-widget" })`; `update(config)` reads `{"steps": [...], "end": "ok" | "error" | "panic" | "fuel" | "memory" | "loop", "output": "not-json" | n}` and `render(config)` reads the same steps from `config.config.render_steps` (render receives the document of feature 003, since the probe exports `updater`); each step is `{"op": "get" | "set" | "delete" | "list" | "usage", "name", "value" | "value_hex" | "value_size", "prefix", "after", "repeat": n}` or `{"op": "spin", "iterations": n}`, where `{i}` in a name, prefix or value is replaced by the repetition's index, `value_size` is a value of that many `x` bytes, and `repeat` defaults to 1; it returns `{"answers": [...], "more": n, "render_config": …}` (the last only in render), each answer `{"ok": …}` (a `get` value as `{"hex": …}` or `null`, a page as `{"names", "next"}`, usage as `{"used", "quota"}`) or `{"error": "not-granted" | "refused:<rule>" | "failed:<rule>", "detail"}`, keeping the first 32 answers and counting the rest in `"more"`; `end` then returns `ok` with the answers, returns an error naming the first error answer, panics, burns fuel without end, grows memory until refused, or loops until the time limit; `"output": "not-json"` returns `{` and `"output": n` a valid JSON document of n bytes instead of the answers; `[package.metadata.dashboard] manifest = false`; add it to `members` in `plugins/Cargo.toml` and a line to `plugins/fixtures/README.md`
- [X] T008 Create `plugins/examples/update-counter/` (research R17, FR-018): contract 1.2.0, world `stored-widget`; `update(config)` reads `{"note": <string, optional>}`, increments the stored `count` (decimal text), stores `first_run_ms` the first time (the time from the configuration's `now_ms` when given, else 0), and keeps the last five notes under `notes/0` to `notes/4`, returning `{"count": n}`; a storage error makes `update` return an error naming it, so nothing is kept; `render(config)` reads `count`, `first_run_ms` and the notes through `list("notes/", none)` and returns `{"title": "Update counter", "text": "Updated <n> times", "items": [notes]}`, or `{"title": …, "text": "Not updated yet"}` when nothing is stored; `[package.metadata.dashboard] manifest = false` for now, because the runtime's manifest parser learns `quota_bytes` in T013, which adds the manifest; add it to `plugins/Cargo.toml` members and to `plugins/examples/README.md`
- [X] T009 [P] Create `plugins/hostile/storage-file/` (research R17): `[package.metadata.dashboard] target = "wasm32-wasip2"`, as `read-file`, built with `wit_bindgen::generate!` on `"../../../wit/v1"`, world `stored-widget`, exporting both `renderer` and `updater`, so that its contract and its declared storage pass load step 5 and only step 6 refuses it, for its `wasi:filesystem` imports; its `render` and `update` open with `std::fs` the path its configuration gives (a store file of the data directory) and returns what it read; `manifest.json` with id `local/storage-file` and `"permissions": [{"name": "storage", "required": true, "quota_bytes": 65536}]` is added in T013 (until then `manifest = false`); add it to `plugins/Cargo.toml` members; run `cargo xtask plugins` (the runtime does not load any new plugin yet)

**Checkpoint**: 1.2.0 and the frozen 1.1.0 in `wit/`; schema, dependencies and lint rules in
place; the new crates build; every existing test still passes

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: the error kind and limits, the manifest field, quoting, the storage module with its
hardened connection, the storage interface on the host, contract 1.2.0 in the engine, the grant
record v3, loading and the commit; the existing tests moved to the new API

**⚠️ CRITICAL**: No user story work can begin until this phase is complete

- [X] T010 [P] Add `StorageFailed` to `ErrorKind` in `runtime/src/error.rs`, `as_str` `storage-failed` (an update's storage could not be committed, or required storage is unavailable at load, `data-model.md` Error kinds); extend the kind-name unit test; create the empty modules `runtime/src/storage/mod.rs` (with the Clippy allow of T006 as its first line), `runtime/src/storage/connection.rs` and `runtime/src/storage/admin.rs`, and declare `pub mod storage` in `runtime/src/lib.rs`, so the tasks below each fill files of their own
- [X] T011 [P] Add the storage limits of research R10 to `Limits` in `runtime/src/limits.rs`, with their defaults and a doc comment each: `max_storage_quota_bytes` 8 MiB (the host cap); `storage_entry_cost_bytes` 64; `max_storage_name_bytes` 256 (also `list`'s `prefix` and `after`); `max_storage_value_bytes` 256 KiB; `max_storage_calls_per_run` 1,000 (every call counted, refused ones included); `max_storage_written_bytes_per_update` 1 MiB (names and values of every `set`, replaced ones included); `storage_list_page` 100; `storage_file_cap_factor` 5 and `storage_file_cap_extra_bytes` 64 KiB (the file cap is `factor × approved quota + extra`, rounded up to whole pages, never below the file's current size); `max_storage_busy_wait` 2 s (the `busy_timeout`, cut to what is left of the run's time limit); unit test of the defaults
- [X] T012 Write the manifest tests for `quota_bytes` first, in `runtime/src/manifest.rs`'s test module (`data-model.md`, Manifest): `quota_bytes` required on `storage` (`invalid-manifest` naming `permissions[<i>].quota_bytes`), refused on any other permission, naming the same path; an integer from 64 to 8388608 (0, 63, 8388609, a negative, a fraction, a string refused, naming both bounds); `storage` at most once, a second entry `invalid-manifest` whatever its `required`; `Manifest::storage()` returns `None` without it and `Some(StorageRequest { required, quota_bytes })` with it
- [X] T013 Implement `quota_bytes` in `runtime/src/manifest.rs` so T012 passes; then add `plugins/examples/update-counter/manifest.json` (`"manifest_version": 1`, `"id": "local/update-counter"`, `"version": "0.1.0"`, `"author": "Dashboard contributors"`, `"permissions": [{"name": "storage", "required": true, "quota_bytes": 65536}]`, `"login_providers": []`) and `plugins/hostile/storage-file/manifest.json` (T009), remove both crates' `manifest = false`, and confirm `cargo xtask plugins` packs them and `runtime/tests/manifest_schema.rs` accepts them
- [X] T014 [P] Write the quoting tests first, in `runtime/src/text.rs`'s test module, for a new `quote_entry(bytes: &[u8], max_bytes: usize) -> String` with a stub (research R14, `contracts/cli.md` `storage show`): UTF-8 is printed in double quotes, escaped as `escape` does, and inside the quotes `"` as `\"` and `\` as `\\`, so `count" = "999` stays one quoted field; line breaks and terminal escapes never reach the output raw; bytes that are not UTF-8 print as `bytes(<size>) <hex>`; anything longer than `max_bytes` (1 KiB in the CLI) is cut and ends with ` [truncated, <size> bytes]`; the empty value prints as `""`
- [X] T015 [P] Implement `quote_entry` in `runtime/src/text.rs` so T014 passes
- [X] T016 Write the connection tests first, in `runtime/src/storage/connection.rs`'s test module, with stubs for `open(file, mode: Mode { Render, Update, Show, List, Clear }, binding_identity: Option<(&str, &str)>, max_page_count) -> Result<StoreConnection, StoreError>` and the statements (research R4, R5, R8): a store path is always absolute (a relative data directory and one named `file:x` both give a plain absolute file, never a URI); render, show and list open read-only and never create the file (a missing store reads as empty); only update creates the file, with exactly the `CREATE TABLE store (key TEXT PRIMARY KEY NOT NULL, value ANY NOT NULL) STRICT, WITHOUT ROWID` and `CREATE TABLE entries (name BLOB PRIMARY KEY NOT NULL, value BLOB NOT NULL) STRICT, WITHOUT ROWID` of research R5 and the rows `format` = 1, `id`, `origin`, `usage` = 0; the schema check refuses as `damaged`, leaving the file byte for byte as it was, a store with another table, an index, a trigger, a view, other SQL for a table, `format` other than 1, or another `id` or `origin` than the binding; get, insert or replace, delete, read usage and write usage round-trip names holding NUL, `../`, quotes and `'; DROP TABLE entries; --` byte for byte; `list(prefix, after, page)` is a byte range with no `LIKE` or `GLOB` (`a%` lists no `ab`; the successor of a prefix ending in `0xFF` bytes drops them and increments the byte before; a prefix of only `0xFF` bytes has no upper bound; an empty prefix lists every name), at most `page` names in byte order, and says whether more remain; a name of 256 B with a value of 256 KiB is kept under the `SQLITE_LIMIT_LENGTH` of R4 (name plus value limits plus 4 KiB); with a small `max_page_count`, a write that fills the file leaves `is_autocommit()` true, and the connection reports the transaction ended; a hot journal left by a child process killed after spilling pages makes a read-only open fail with extended code 776, and `open` recovers it with one read-write open and then reads the state from before (research R8); a read-only open that still fails after recovery is `busy`, never `damaged`
- [X] T017 Implement `runtime/src/storage/connection.rs` so T016 passes, the only code that uses `rusqlite` (research R1, R4, R5, R8): one setup for every mode, in two production functions always called in sequence by `open`, `configure` (steps 1–8 of research R4) and `authorize` (step 9), in the order of research R4: the absolute path; `OpenFlags` (read-only for render, show and list; read-write for update and clear, creating only in update; `SQLITE_OPEN_EXRESCODE`); `set_limit(SQLITE_LIMIT_ATTACHED, 0)`; `set_db_config` `DEFENSIVE` on, `ENABLE_TRIGGER` off, `ENABLE_VIEW` off, `TRUSTED_SCHEMA` off; `SQLITE_LIMIT_LENGTH` to `max_storage_name_bytes + max_storage_value_bytes + 4096` and `SQLITE_LIMIT_SQL_LENGTH` to the length of the longest host statement; the pragmas `max_page_count`, `secure_delete = ON`, `journal_mode = DELETE`, `synchronous = FULL`, `busy_timeout` and `cache_size = -256`; the schema created (update only, on a file with no tables) and then checked; then the authorizer, allowing only reads and writes of `store` and `entries`, `SELECT` and transactions, and denying every function, `ATTACH`, `DETACH`, `PRAGMA`, `CREATE`, `DROP`, `ALTER`, reads of `sqlite_schema` and any action it does not name; statements prepared once per connection with every name and value bound as a parameter; `begin_read()` (`BEGIN DEFERRED` and a first read, held until the run ends), `begin_immediate()`, `begin_exclusive()`, `commit()`, `rollback()` and `transaction_ended()` (`is_autocommit()` while the host believes a transaction is open); and `engine_check() -> Result<(), String>`, run once per process before any store is opened: `rusqlite::version()` is exactly `3.54.0` and `DEFENSIVE` reads back as on, on an in-memory connection
- [X] T018 Write the grant record tests first, in `runtime/src/grants.rs`'s test module, for `data-model.md` (Grant record, version 3; Approval digest) verbatim: `record_version` `3`; records of versions 1 and 2 read as version 3 without `storage_quota_bytes` and are written back as 3, a newer one is refused; `storage_quota_bytes` is present only once an approved manifest declared storage, set by `approve` to the quota it showed, smaller or larger than before, and never changed by a load; the approval digest of a manifest without storage is byte for byte the one feature 003 computed (a literal pinned in the test, computed before this change), and with storage gains `"storage": {"required": …, "quota_bytes": …}`, changing when either changes; approving a plugin that declares storage, required or optional, with no expected digest or another one is `approval-changed` and records nothing; the storage check at load: storage required, or optional and on, with a declared quota above `storage_quota_bytes`, or none recorded, is `needs-new-approval` naming both quotas, the entry unchanged; a lower declared quota needs nothing and changes nothing; `set_optional(storage, on)` above the approved quota is `needs-new-approval`; `while_granted(plugin, origin, permission, f)` runs `f` only while the record, read under its shared lock, grants that permission from that origin, and a writer waiting for the exclusive lock waits until `f` returns
- [X] T019 Implement grant record v3, the digest and `while_granted` in `runtime/src/grants.rs` so T018 passes (research R12), keeping feature 002's locking and atomic writes; `approve` and `approve_connected` take the expected digest whenever connectors or storage are declared; move `runtime/src/bin/runtime.rs` and `bench/src/bin/compile_cost.rs` to the new signatures, so the workspace still builds
- [X] T020 Write the storage call tests first, in `runtime/src/storage/mod.rs`'s test module with stubs, following steps 1–9 of `data-model.md` (A storage call) in order (step 0, the stand-in, is tested with T024 and `optional-off`), on temporary stores and a grant record file in a temporary directory: the store binding's file is `<data dir>/storage/<hex SHA-256 of "<id>\n<origin>">.sqlite`; the file cap is `storage_file_cap_factor × approved quota + storage_file_cap_extra_bytes` rounded up to whole pages, and never below the file's current size; the call counter goes up for every call, refused ones included, and the call past `max_storage_calls_per_run` is `refused(over-limit)` before anything else; storage no longer granted from this origin in the grant record (read again on every call) answers `not-granted`, and every later call `not-granted` without reading the record; after the transaction ended, every call `failed(run-aborted)`; `set` or `delete` in render `refused(render-only-reads)` with no connection opened; an empty name, a name, `prefix` or `after` over 256 bytes `refused(invalid-name)`, a value over 256 KiB and a `set` past 1 MiB written in the update `refused(over-limit)`; `set` whose usage after it (old entry gone, new one counted as name + value + 64) exceeds the quota `refused(over-quota)`, naming the quota and the usage, nothing kept; with usage over a lowered quota, deletes pass and a write passes only if usage after it is within the quota; a write that reaches the file cap answers `failed(over-limit)` and aborts the run; a damaged store answers `failed(damaged)` and is left as it was; render reads one view for the whole run; update reads its own writes; `usage()` returns the usage and the declared quota; `list` pages of `storage_list_page` names with `next`; the run's counts (calls, writes, bytes written) are kept for the report
- [X] T021 Implement `runtime/src/storage/mod.rs` so T020 passes (research R3, R6, R7, R9, R10, R11): `StoreBinding { id, origin, file, quota_bytes, approved_quota_bytes }` built from the canonical data directory; `StorageRun` holding the binding, the phase (the `Phase` of `runtime/src/calls.rs`), the connection opened on the first call (read-only with `begin_read` in render, read-write with `begin_immediate` in update), `calls`, `written_bytes`, `taken_away` and `aborted`, the grant store, the plugin's id and origin, and the run's deadline for the busy wait; usage kept in the `store` table, changed in the transaction of each write; `StorageRun::finish(commit: bool) -> Result<StorageCounts, RuntimeError>`, which commits only when `commit` is true, the run was not aborted and storage is still granted, checked by `GrantStore::while_granted` (T019) holding the grant record's shared lock until the commit ends, and rolls back otherwise; an aborted run, or a commit that fails (busy, disk full), is `storage-failed` naming the first failure, and storage taken away is `not-granted` naming storage
- [X] T022 Write the contract tests first, as unit tests in `runtime/src/permissions.rs` with WAT components: a v1 component whose contract names all carry `1.2.0` is v1 with minor 2; one carrying `1.3.0`, or mixing `1.1.0` and `1.2.0` names, is `contract-mismatch` naming both versions; a manifest that declares storage on a 1.0.0 or 1.1.0 component is `contract-too-old` naming 1.2.0; one that declares storage and exports no `updater` is `missing-updater`, saying storage is written only in an update; an import of `dashboard:widget/storage@1.2.0` without the manifest declaring storage is `not-declared`; the registry maps `storage` to `dashboard:widget/storage` since 1.2.0, with no v0 interface
- [X] T023 Implement contract 1.2.0 in `runtime/src/permissions.rs` so T022 passes (research R9, R13): `V1_VERSION` 1.2.0; the registry gains the `storage` row with `since` 1.2.0; storage's `contract-too-old` and `missing-updater` in the decision; the CLI prints `v1 (1.2.0)`
- [X] T024 Bind contract 1.2.0 and link storage in `runtime/src/lib.rs` and `runtime/src/host.rs` (research R9, R13): `bindgen!` for `mod v1` on `"../wit/v1"`, world `connected-stored-widget`; implement the generated `storage` host trait for `HostState`, whose new field `storage: Option<StorageRun>` is the real interface when storage is granted and `None` for the stand-in, which answers every call `not-granted`, counts nothing and opens nothing (a unit test in `runtime/src/host.rs` checks step 0 of `data-model.md`, A storage call); add to `runtime/tests/contract.rs`: `hello-widget` (1.0.0), `github-releases` and `net-probe` (1.1.0) load, link and run exactly as before on the 1.2.0 host
- [X] T025 Load and run storage in `runtime/src/lib.rs` (`data-model.md`, Load decision; End of a run): the data directory made absolute when the runtime starts and canonical once `storage/` exists (research R4); step 3 with `quota_bytes`; step 3a, a declared quota above `max_storage_quota_bytes` is `invalid-manifest` naming both bounds; step 5 with storage's `contract-too-old` and `missing-updater`; step 7 the storage quota check of T019; step 8a, when storage is granted, `<data dir>/storage/` exists or is created as a directory the host can write, and `engine_check` passed: otherwise required storage is `storage-failed` naming the cause, and optional storage that is on is linked to the real interface, whose calls answer `failed(unavailable)`; step 9 links `storage` real with the store binding when granted and the stand-in when optional and off; no step deletes a store; `Plugin::render` and `Plugin::update` give each run its `StorageRun` (`max_page_count` from the approved quota, the busy wait cut to what is left of the run's time limit); `Plugin::update` finishes the storage run before it records kept data: it commits only when `update` returned `ok` with valid output within its size, and otherwise rolls back; a commit failure makes the update fail as `storage-failed` (previous kept data stays) and storage taken away as `not-granted`; `UpdateReport` and `RenderReport` gain `storage: Option<StorageCounts>` (calls, writes kept, usage, quota)
- [X] T026 Move the existing tests to the new API: update `runtime/tests/common/mod.rs` (`stored_manifest(id, version, required, quota_bytes)`, a `store_probe(manifest)` packing `store-probe` as `probe` does, `steps(&[…])` building the probe's configuration, `approved` passing the approval digest when the manifest declares storage), `runtime/tests/contract.rs`, `runtime/tests/permissions.rs`, `runtime/tests/hostile.rs`, `runtime/tests/updates.rs` and `runtime/tests/cli.rs` where the API changed; every test of features 001 to 003 passes with unchanged expectations, as 1.0.0 and 1.1.0 plugins on the 1.2.0 host (SC-007)

**Checkpoint**: storage loads, links and runs with every rule; the suites of features 001 to
003 are green

---

## Phase 3: User Story 1 - A widget remembers its own data (Priority: P1) 🎯 MVP

**Goal**: the maintainer sees a widget's storage and quota, approves exactly what was shown, and
the widget writes in update and reads in render, across runs of the host and new versions

**Independent Test**: quickstart step 2

### Tests for User Story 1

> Write these first; they fail until the implementation tasks below land

- [X] T027 [P] [US1] Create `runtime/tests/storage.rs` with User Story 1's acceptance scenarios at the API level, on a temporary data directory and grant record: `update-counter` approved updates twice and renders a count of 2 with both notes; a new `Runtime` on the same data directory renders the same values; a new version of the example from the same origin (repacked with its version bumped) reads the values the earlier one wrote, with its kept data discarded as in feature 003; with `store-probe`: `get` of a name never written is `none`; an empty value is kept and counts 64; two `set`s of one name in an update keep the last; `delete` of an absent name succeeds; `list` in an update reflects the update's own writes; 250 names under a prefix come back in byte order in pages of 100 with `next`, and names outside the prefix never; `usage` matches the sum of names, values and 64 per entry; add the tests `storage_unavailable_at_load` (a data directory where `storage/` is a file: required storage refused as `storage-failed`, optional storage's calls `failed(unavailable)`), `data_dir_is_absolute` (a relative data directory and one named `file:x` give store files at absolute paths) and `bundled_sqlite` (`runtime::storage::engine_check()` passes, so the linked SQLite is 3.54.0 with defensive mode)
- [X] T028 [P] [US1] Add CLI tests to `runtime/tests/cli.rs`, every command with `--grants` and `--data` in a temporary directory: `permissions` of `update-counter` prints exactly the format of `contracts/cli.md` (`contract: v1 (1.2.0)`, `permissions: storage (required, …)`, the `storage:` line with the quota, `not approved` or `approved up to <n> bytes`, and `used <n> bytes`, then `approval digest:`) and changes nothing; a declared quota above the approved one adds `(above the approved quota: approve again)`; `approve` without `--expect` is a usage error (exit 2) saying to run `permissions`, granting nothing; with the printed digest it prints `approved …`, `granted: storage` and `storage: quota 65536 bytes`; with a stale digest `approval-changed`; `update --config '{"note":"first"}'` prints `updated …` and `storage: <c> calls, <w> writes kept, used <u> of 65536 bytes`; `run` renders the count and notes; the CLI half of `data_dir_is_absolute`: `--data` given as a relative path and as `file:x` puts the store at an absolute path under that directory; every refusal names its field, limit or quota and its rule (SC-003)

### Implementation for User Story 1

- [ ] T029 [US1] Add to `runtime/src/bin/runtime.rs` exactly as in `contracts/cli.md`: the `storage:` line of `permissions` and the approval digest for widgets that declare storage; `approve --expect` required whenever storage is declared, required or optional, and the `storage: quota …` line; the `storage:` line of `update`; update the usage text and the module comment
- [ ] T030 [US1] Write `plugins/examples/update-counter/README.md` (`permissions`, `approve --expect`, `update --config`, `run`, and what render shows) and update `plugins/examples/README.md`: `update-counter` is the storage example; `github-releases` and `net-probe` are built from the frozen 1.1.0 contract to show that older minors keep running

**Checkpoint**: quickstart step 2 works; User Story 1 is complete

---

## Phase 4: User Story 2 - No plugin reaches another plugin's storage (Priority: P1)

**Goal**: the isolation cases of `data-model.md` (Hostile cases) contained on three systems,
every other store and host record unchanged byte for byte: the gate case of Phase 2

**Independent Test**: `cargo test -p runtime --test hostile storage`; every case passes and
`update-counter` updates and renders right after each one

### Tests for User Story 2

- [ ] T031 [US2] Create `runtime/tests/hostile/storage.rs`, declared in `runtime/tests/hostile.rs` as `#[path = "hostile/storage.rs"] mod storage;` with its module comment naming `specs/004-plugin-storage/data-model.md` (Hostile cases); write its helpers: a `Snapshot` of the grant record, every file under `widget-data/`, the request log and every file under `storage/`, taken after a case's setup and compared byte for byte after it runs, except what the case names as changed on purpose (its own store on a successful update, its grants when it changes them) — the `host-records-untouched` case, applied by every storage case; and `well_behaved(&runtime)`, an update and a render of `update-counter` producing its normal result, run after every case
- [ ] T032 [US2] Add the isolation cases to `runtime/tests/hostile/storage.rs` with `store-probe`: `other-plugin` (A stores values; B, approved from another file, reads, lists and writes with A's names and sees only its own store, which holds none of A's values; A's store unchanged), `same-id-other-origin` (a copy of an approved plugin at another path is `not-granted`; once approved, its own empty store, the first one's unchanged), `name-injection` (names with `../`, `/`, `\`, NUL, quotes, line breaks, `'; DROP TABLE entries; --`, `ATTACH DATABASE '/tmp/x' AS y`, `%` and `_` round-trip exactly; a `list` with prefix `a%` returns no `ab`; nothing appears in the data directory but the plugin's own store file); and in `same-id-other-origin`, the reverse: the file at an approved plugin's path replaced by a plugin with another id gets, once approved, its own empty store, the first one's unchanged (spec edge case)
- [ ] T033 [US2] Add `storage-file` (the `storage-file` crate, given the path of another plugin's store, is refused at load as `capability-refused`, listing `wasi:filesystem/*`, before running) and `foreign-store`, whose files come from `#[cfg(feature = "test-network")] pub fn plant_foreign_store(file, kind: Foreign { ExtraTable, Trigger, View, OtherId })` added to `runtime/src/storage/connection.rs` by this task, so the test opens no `rusqlite::Connection` and the Clippy rule of T006 needs no exception (`a store file replaced by another SQLite database with an extra table, one with a trigger, one with a view, and one holding another id; and a store file renamed to another plugin's file name: each answers `failed(damaged)` with nothing run, changed or deleted, the file byte for byte as it was, and `runtime storage list` later shows it `damaged`; other stores keep working) to `runtime/tests/hostile/storage.rs`
- [ ] T034 [US2] Add the white-box cases (Principle VIII; no plugin can observe them): in `runtime/src/storage/connection.rs`, `#[cfg(feature = "test-network")] pub fn hardening_report(file, mode) -> HardeningReport`, which opens the store through the production setup function of T017, with no switch production does not use, reads each layer back (`SQLITE_LIMIT_ATTACHED`, `LENGTH` and `SQL_LENGTH` with `-1`; the `db_config` getters of `DEFENSIVE`, `ENABLE_TRIGGER`, `ENABLE_VIEW`, `TRUSTED_SCHEMA`; `max_page_count`, `secure_delete` and `cache_size` read after `configure` and before `authorize`, the two functions `open` calls) and makes one attempt per layer on the finished connection (`ATTACH`, `load_extension('x')`, `length(x'00')`, a `PRAGMA`, a `CREATE TABLE`, a statement longer than the SQL limit, a value past the length limit, and a write on the render connection), recording each outcome; in `runtime/src/lib.rs`, `#[cfg(feature = "test-network")] pub fn Plugin::store_binding(&self) -> Option<PathBuf>`, the linked binding's file, `None` for the stand-in; both hooks say in their doc comment that `test-network` is the runtime's test-only feature, refused in release builds by its `compile_error!`, and gates every test hook, not only the network's; then the cases `store-hardened` (every layer as R4 sets it and every attempt refused) and `store-binding` (the binding is the file of the plugin's id and origin; the stand-in has none) in `runtime/tests/hostile/storage.rs`
- [ ] T035 [US2] Update `plugins/hostile/README.md` and `plugins/fixtures/README.md`: the storage cases live in `runtime/tests/hostile/storage.rs` on `store-probe`, `update-counter` and `storage-file`, with a table of case, rule and expected outcome taken from `data-model.md`; ask the maintainer to push, then confirm the suite is green on Windows, macOS and Linux in CI, fixing the host (never the test) if a case is not contained

**Checkpoint**: the gate's storage case passes on three systems

---

## Phase 5: User Story 3 - The quota bounds what a plugin keeps (Priority: P2)

**Goal**: writes past the quota refused with nothing kept; a larger quota needs approval; limits
on names, values, calls, bytes and the file keep every widget from filling the disk

**Independent Test**: User Story 3's independent test of spec.md

### Tests for User Story 3

- [ ] T036 [US3] Add the approval and quota cases to `runtime/tests/hostile/storage.rs`: `quota-raised` (a version declaring a larger quota is `needs-new-approval` naming both quotas, the approval unchanged; after a re-approval at a smaller quota, a version back at the earlier larger one is refused again), `approval-digest` (approving with no digest or a stale one is `approval-changed`, nothing granted), `quota-fill` (writes up to the quota, then one that would pass it is `refused(over-quota)` naming the quota and the usage, not kept; replacing a value counts the old entry as gone), `quota-lowered` (a version with a quota below the stored usage: data kept, deletes succeed, a write leaving usage over the new quota refused, one within it kept; the file cap stays that of the approved quota)
- [ ] T037 [US3] Add the limit cases to `runtime/tests/hostile/storage.rs`: `many-entries` (thousands of empty values, then values of 32 B, 500 B, 1,000 B, 1,500 B and 4 KiB up to the 8 MiB cap, over several updates within the bytes per update: each counts the entry cost, `over-quota` comes before the file cap, and the file stays under its cap), `churn` (rewrites and deletes of the same names with the same value sizes over 200 updates: the file never passes its cap), `file-cap-hit` (with `storage_file_cap_factor` 1 and `storage_file_cap_extra_bytes` 0 in the test's `Limits`, a write reaches the file cap: `failed(over-limit)`, later calls `failed(run-aborted)`, and the update (a) returning `ok` and (b) trapping both leave the store byte for byte as before and fail as `storage-failed`), `over-limits` (a name of exactly 256 B with a value of exactly 256 KiB is kept; one byte more of either, a `prefix` or `after` over 256 B, call 1,001 of a run, and a `set` past 1 MiB in one update: each `refused(invalid-name)` or `refused(over-limit)` naming its limit), and `unwritable-store` beside `file-cap-hit` (on Unix, `storage/` and the store file made read-only after the store exists; on Windows, the file marked read-only: the next update's first write answers `failed(unavailable)`, the update fails as `storage-failed`, and the store is as before; spec edge case: the disk full or the file not writable)
- [ ] T038 [P] [US3] Add to `runtime/tests/permissions.rs` one test per row of `data-model.md` (Combinations), named after the row, cross-referencing the hostile cases that already cover a row, and the other tests named there: `storage_needs_its_declaration`, `required_storage_needs_approval`, `turning_storage_on_checks_the_quota`, `approval_records_the_quota_shown`; and, with a `Limits` whose `max_storage_quota_bytes` is lower than an approved quota, the plugin refused as `invalid-manifest` naming both bounds, its data kept (spec edge case)

### Implementation for User Story 3

- [ ] T039 [US3] Make the refusal details match `data-model.md` and `contracts/cli.md` exactly in `runtime/src/storage/mod.rs`, `runtime/src/grants.rs` and `runtime/src/lib.rs`: `over-quota` names the quota and the usage after the write; `needs-new-approval` names the approved and the declared quota; `invalid-manifest` names the field path and both bounds; each limit names itself and its value (SC-003)

**Checkpoint**: User Story 3 is complete

---

## Phase 6: User Story 4 - Writes are all or nothing, and render only reads (Priority: P2)

**Goal**: an update's writes are kept only when it succeeds; render never changes the store;
turning storage off or removing the approval applies from the next call, and at the commit

**Independent Test**: User Story 4's independent test of spec.md

### Tests for User Story 4

- [ ] T040 [US4] Add to `runtime/tests/hostile/storage.rs`: `write-in-render` (`set` and `delete` in render `refused(render-only-reads)`, store unchanged), `crash-after-writes` (writes, then a panic, out of fuel, out of memory, the time limit, an error, output that is not JSON, and output over 256 KiB: the store byte for byte as before, the previous kept data kept), `optional-off` (storage optional and off: every call `not-granted`, no store file created or changed; turned on, the earlier data is there)
- [ ] T041 [US4] Add `taken-away-mid-update` to `runtime/tests/hostile/storage.rs` (research R17), with the change made through a second `GrantStore` on the same record file, from another thread, as another process would: the probe writes, then spins with no further call; the test waits for the store's `-journal` file, then turns storage off, and in a second run removes the approval; (a) the update returns `ok` and fails as `not-granted` naming storage, nothing kept; (b) a variant with one more call after the change answers `not-granted`; (c) a render with a 10 s render time limit reads, spins about 3 s and reads again while the grants change 1 s after it started: the second read answers `not-granted`
- [ ] T042 [US4] Add `killed-mid-update` to `runtime/tests/hostile/storage.rs` (research R8, R17): run `env!("CARGO_BIN_EXE_runtime") update` as a child process on the probe packed to a temporary file and approved in a temporary record, with a manifest quota of 2 MiB and steps of 225 `set`s of `k{i}` with `value_size` 4096 (about 900 KiB, under the 1 MiB per update and well past the 256 KiB page cache), then a `spin` of 10⁹ iterations; wait for the journal, kill the child, then check that a render through the library and `runtime storage show` read the state from before the update, that neither reports `damaged`, and that the journal is gone afterwards
- [ ] T043 [P] [US4] Add User Story 4's other scenarios to `runtime/tests/storage.rs`: a widget that declares storage and no connector runs its update and writes its store; an update that returns `ok` keeps all its writes together; a render while an update of the same plugin writes from another thread (a second `Runtime` on the same data directory) sees the values from before or after that update, never part of it; an update that cannot take the store within its wait (another `Runtime` holds the write) fails as `storage-failed` naming `busy`, its writes not kept, the previous kept data kept; a widget declaring storage without `updater` is `missing-updater`
- [ ] T044 [P] [US4] Add CLI tests to `runtime/tests/cli.rs`: `update` of a probe that writes and then runs out of fuel exits 1 with `error: cpu-budget-exceeded: … (storage writes not kept, previous data kept)`; one that reaches the file cap prints `error: storage-failed: … nothing this update wrote was kept (previous data kept)`; `run` afterwards renders the previous values

### Implementation for User Story 4

- [ ] T045 [US4] Make the failure messages of `update` match `contracts/cli.md` exactly, including `(storage writes not kept, previous data kept)` for an update that wrote and failed, in `runtime/src/lib.rs` and `runtime/src/bin/runtime.rs`

**Checkpoint**: User Story 4 is complete

---

## Phase 7: User Story 5 - The maintainer sees and clears a plugin's storage (Priority: P3)

**Goal**: the CLI lists every store, shows one escaped, and clears one by package or by id and
origin; turning storage off or removing the approval keeps the data

**Independent Test**: quickstart steps 4 and 5

### Tests for User Story 5

- [ ] T046 [US5] Write the admin tests first, in `runtime/src/storage/admin.rs`'s test module with stubs (research R14): `list(data dir)` returns every `*.sqlite` under `storage/` with its id, origin and usage read from the file, sorted by id then origin, marking as `damaged` a file whose schema fails the check or whose name is not the hash of the id and origin inside it; `show(binding)` returns the usage and every entry in byte order of names, opened read-only (a hot journal recovered first), `None` when there is no store yet; `clear(binding)` empties the store in one `BEGIN EXCLUSIVE` transaction (every entry deleted, usage 0, no `VACUUM`), the deleted values absent from the file's bytes afterwards (`secure_delete = ON`), the file's size unchanged; `remove(binding)` deletes the file of a damaged or orphaned store under an exclusive lock; a store held by an update in another connection is `busy`, nothing changed; no function touches any other store
- [ ] T047 [US5] Implement `runtime/src/storage/admin.rs` so T046 passes, and the public `Runtime::list_stores() -> Vec<StoreInfo>`, `Runtime::show_store(manifest, origin)` and `Runtime::clear_store(id, origin)` in `runtime/src/lib.rs`, the last removing the store when it is damaged or its origin's file is gone
- [ ] T048 [P] [US5] Add CLI tests to `runtime/tests/cli.rs`: `storage list` prints one line per store as in `contracts/cli.md` (`used <u> of <q> bytes` with the grant record's approved quota, `not approved`, `no file`, `damaged: <detail>`), sorted, changing nothing; `storage show` prints the header, `used <u> of <q> bytes, <n> entries`, and each entry quoted by `quote_entry`, values cut at 1 KiB, `no store yet` before the first update, and a damaged store as `error: storage-failed: damaged: …` with exit 1; `storage clear <plugin>` prints `cleared <id> from <origin> (<n> bytes)` and leaves other stores unchanged; `storage clear --id <id> --origin <path>` clears a store whose file is gone; `storage clear` of a damaged store deletes its file; a store busy in another process is `error: storage-failed: busy: …`, exit 1; `optional <plugin> storage off` prints `storage off (data kept)` and the data is there when turned on; `optional … storage on` above the approved quota is `needs-new-approval`; `remove` prints `storage kept: run runtime storage clear to delete it`, and approving again from the same origin finds the earlier data (User Story 5 scenarios 2 and 3)
- [ ] T049 [US5] Add `show-spoof` to `runtime/tests/hostile/storage.rs`: names and values with line breaks, terminal escapes, `"`, `\` and `count" = "999` printed by `runtime storage show` escaped, one entry per line, no entry passing for two fields or for the tool's own lines

### Implementation for User Story 5

- [ ] T050 [US5] Add `storage list|show|clear` to `runtime/src/bin/runtime.rs` exactly as in `contracts/cli.md`, and extend `optional` (`storage on|off`) and `remove` with their storage lines; update the usage text

**Checkpoint**: User Story 5 is complete

---

## Phase 8: User Story 6 - Authors build on contract 1.2.0 (Priority: P3)

**Goal**: authors declare storage from the schema and docs; plugins built for 1.0.0 and 1.1.0
keep running; a newer minor is refused

**Independent Test**: User Story 6's independent test of spec.md; the schema and the parser
agree

### Tests for User Story 6

- [ ] T051 [P] [US6] Extend `runtime/tests/contract_v1.rs`: the capability rule covers `storage` (its error's first case is `not-granted`; `renderer` and `updater` stay excluded); 1.2.0 contains every interface, function and type of 1.1.0 unchanged, and 1.1.0 every one of 1.0.0, comparing the resolved definitions and ignoring doc comments; add `frozen_contracts`: `wit/history/1.0.0/widget.wit` and `wit/history/1.1.0/widget.wit` match SHA-256 values pinned in the test
- [ ] T052 [P] [US6] Add to `runtime/tests/contract.rs`: a component claiming contract 1.3.0 is `contract-mismatch` naming both versions; a 1.1.0 component (`net-probe` repacked) whose manifest declares storage is `contract-too-old` naming 1.2.0; every plugin built for 1.0.0 and 1.1.0 in `plugins/` renders as before (SC-007)
- [ ] T053 [P] [US6] Extend `runtime/tests/manifest_schema.rs`: for a corpus of manifests covering every rule of T012 (`quota_bytes` missing, on another permission, 63, 64, 8388608, 8388609, a fraction, a string, two `storage` entries), the schema of `wit/manifest.schema.json` and `Manifest::parse` agree, with the host-only rule listed apart (a quota above a lower host cap); `plugins/examples/update-counter/manifest.json` validates and parses (SC-008)

### Implementation for User Story 6

- [ ] T054 [P] [US6] Update `docs/plugin-authors.md` (FR-018): contract 1.2.0 and the worlds `stored-widget` and `connected-stored-widget`; declaring storage and `quota_bytes`; how usage is counted (name + value + 64 per entry) and what the limits are; read-only render and all-or-nothing updates; handling `not-granted`, `refused` and `failed`, `run-aborted` included; that storage is kept across new versions from the same origin while kept data is not; that storage is not encrypted and not for secrets (Principle IV); what T003 found about declaring storage for a world that imports it
- [ ] T055 [P] [US6] Update `wit/README.md`: contract 1.2.0, `history/1.1.0` frozen and pinned by hash, the `storage` interface and the two new worlds, and `quota_bytes` in the manifest schema

**Checkpoint**: an author can declare and use storage from the docs alone

---

## Phase 9: Polish & Cross-Cutting Concerns

**Purpose**: measurements, documents, checks and review

- [ ] T056 Create `bench/src/bin/storage_cost.rs` in the `dashboard-bench` crate (research R16, `contracts/cli.md`): pack `target/plugins/store-probe.wasm` with a manifest declaring storage, approve it in a grant record file in a temporary directory, write 100 values in a warm-up update, and print the medians of 100 runs each as `storage_open_us`, `storage_get_us`, `storage_set_us`, `storage_commit_us` and `render_reading_100_us`, computed as differences of medians between runs that differ only in their storage calls, then `file_bytes_at_quota` for a store filled to its quota with values of 32 B, 1,000 B and 4 KiB; `--iterations <n>` (default 100); add `bench/tests/storage_cost.rs`, which runs the binary with `--iterations 3` and checks that every line is printed
- [ ] T057 Update `.github/workflows/bench.yml`: a step running `cargo run -p dashboard-bench --release --locked --bin storage_cost | tee storage.txt`, added to the summary, which points to `specs/004-plugin-storage/performance.md`
- [ ] T058 Once the maintainer has pushed, run the bench workflow on the three systems and write `specs/004-plugin-storage/performance.md`: each storage call against SC-006 (under 1 ms), `render_reading_100_us` against render's limits, the file sizes against the file cap of research R10, and the limits of R10 confirmed or changed, with the commit measured
- [ ] T059 [P] Update `docs/threat-model.md` (FR-019): rows for reading or changing another plugin's store, a file copying an approved id, names as paths or commands, the storage file through the file system, a store file changed outside the host, filling the disk, half-written updates, writes in render and storage taken away mid-run; each with its asset, attacker, boundary, principle, test and Phase 2
- [ ] T060 [P] Update `SECURITY.md` (scope names plugin storage), `CONTRIBUTING.md` (the security core list gains the storage module) and `.github/pull_request_template.md` (its `Cargo.lock` line gains `rusqlite` and `libsqlite3-sys`; its security tests line gains `plugins/fixtures/store-probe`) (FR-019, research R15)
- [ ] T061 Run every local check: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets --locked -- -D warnings`, the same for `plugins/` on `wasm32-unknown-unknown`, `cargo xtask plugins --locked`, `cargo test --workspace --locked`, `cargo deny check`, `cargo audit` and `cargo audit --file plugins/Cargo.lock`
- [ ] T062 Walk through `specs/004-plugin-storage/quickstart.md` on a clean temporary directory and fix any drift between it, the CLI and the contracts
- [ ] T063 Human review (Principle IX): the maintainer reviews every commit under `runtime/`, `wit/` and `plugins/`, and the `Cargo.lock` change that adds `rusqlite` and `libsqlite3-sys`, line by line, and ticks the pull request's review box with their handle; CI green on the three systems; merge with `gh pr merge --merge --match-head-commit <sha> --author-email 8193616+philipesantos@users.noreply.github.com`
- [ ] T064 After the merge, mark in the development guide the Phase 2 item for per-plugin storage as done and record in the Phase 2 gate line that the storage case passes; leave "Contrato WIT v1 com essas interfaces" open until features 005 and 006 add theirs

---

## Dependencies & Execution Order

### Phase Dependencies

- **Setup (Phase 1)**: no dependencies
- **Foundational (Phase 2)**: depends on Setup; blocks every user story
- **User Stories (Phases 3–8)**: depend on Foundational; US1 first (MVP), then US2 (the gate);
  US3, US4, US5 and US6 can follow in any order
- **Polish (Phase 9)**: depends on every user story

### User Story Dependencies

- **US1 (P1)**: after Foundational
- **US2 (P1)**: after Foundational; uses `update-counter` as the well-behaved widget, and
  `foreign-store` checks `runtime storage list`, which lands in T050 (that assertion is added
  with T050 if US2 runs first)
- **US3 (P2)**: after Foundational; uses the helpers of T031
- **US4 (P2)**: after Foundational; uses the helpers of T031; `killed-mid-update` uses
  `runtime storage show` (T050) and T044 builds on `update` (T029)
- **US5 (P3)**: after Foundational; T049 uses the helpers of T031
- **US6 (P3)**: after Foundational

### Within Each Phase

- Setup: T001 before T002 (the 1.1.0 plugins move to the frozen copy before `wit/v1` changes);
  T002 before T003, T007, T008 and T009; T005 before T006; T007 → T008 → T009 (all edit
  `plugins/Cargo.toml`)
- Foundational: T010 and T011 first (T010 creates the empty modules the others fill); T012 →
  T013; T014 → T015; T016 → T017; T018 → T019; T020 (after T017 and T019) → T021; T022 → T023
  → T024 (after T021); T025 after T013, T019 and T024; T026 last
- `runtime/src/bin/runtime.rs` tasks run in order (T029 → T045 → T050)
- `runtime/tests/hostile/storage.rs` tasks run in order (T031 → T032 → T033 → T034 → T036 →
  T037 → T040 → T041 → T042 → T049); so do the tasks of different stories on
  `runtime/tests/cli.rs`, `runtime/tests/storage.rs` and `runtime/tests/permissions.rs`
- Tests before their implementation; they must fail first

### Parallel Opportunities

- Setup: T004 with T005–T006; T009 with T004
- Foundational: T010 with T011; T014–T015 with the manifest and connection tasks; T018–T019
  and T022–T023 with T016–T017
- User Story 1: T027 and T028
- User Story 3: T038 with T036–T037
- User Story 4: T043 and T044 with the hostile cases
- User Story 5: T048 with T046–T047
- User Story 6: T051, T052 and T053; T054 and T055
- Polish: T059 and T060

---

## Parallel Example: User Story 6

```bash
# Tests in three different files:
Task: "runtime/tests/contract_v1.rs: 1.2.0 ⊇ 1.1.0 ⊇ 1.0.0; frozen contracts pinned by hash"
Task: "runtime/tests/contract.rs: 1.3.0 refused; storage on 1.1.0 is contract-too-old"
Task: "runtime/tests/manifest_schema.rs: quota_bytes, schema and parser agree"

# Then the documentation:
Task: "docs/plugin-authors.md: storage, quota, read-only render, contract 1.2.0"
Task: "wit/README.md: contract 1.2.0, history/1.1.0, storage"
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1: Setup
2. Phase 2: Foundational
3. Phase 3: User Story 1
4. **STOP and VALIDATE**: quickstart step 2. The host already applies every rule, but the gate
   is User Story 2: nothing may build on this until its cases pass on three systems.

### Incremental Delivery

1. Setup + Foundational → contract 1.2.0, the storage module and grant record v3, with the
   suites of features 001 to 003 green
2. User Story 1 → permissions, approve with the digest, update and run through the CLI
3. User Story 2 → the isolation cases green on three systems: **the gate's storage case**
4. User Story 3 → quota, approvals and limits
5. User Story 4 → all-or-nothing updates, read-only render, storage taken away mid-run
6. User Story 5 → `storage list|show|clear`, optional and remove keep the data
7. User Story 6 → schema, contract checks, author documentation
8. Polish → measurements, threat model, documents, local checks, quickstart, human review

---

## Notes

- [P] tasks touch different files and depend on no incomplete task
- All work happens on the `004-plugin-storage` branch, in one pull request opened as a draft
  when the maintainer asks to push. Never commit this feature directly to `master`
- One commit per task or small group of tasks, without any co-author or tool attribution. The
  maintainer reviews each commit line by line (Principle IX); a fix after review is a new commit,
  never a rewrite of a reviewed one
- Push only when the maintainer asks; starting a bench run is the maintainer's call too
- Everything under `runtime/` and `wit/` is security core, as is the `Cargo.lock` change that
  adds `rusqlite` and `libsqlite3-sys`; `plugins/hostile/`, `plugins/fixtures/` and
  `runtime/tests/hostile/` hold the security tests: line-by-line human review (T063)
- Never commit built `.wasm` files; CI builds every plugin from source
- No test reaches the internet or reads `specs/`
