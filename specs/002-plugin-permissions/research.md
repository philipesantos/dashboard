# Research: Plugin Permissions

Spikes ran on 2026-10-22 on the maintainer's Linux machine (x86_64, rustc 1.99.1, Wasmtime
50.0.0, wasmparser 0.261.0, release builds), against the plugins of feature 001 built with
`cargo xtask plugins`. Numbers are indicative; `performance.md` re-measures them on all three
CI systems.

## R1. Binding the manifest to the component

**Decision**: a plugin package is one WebAssembly component file. Its manifest is the payload
of exactly one top-level custom section named `dashboard-manifest`, holding the manifest's
UTF-8 JSON. The runtime reads that section before compiling. A package with no such section is
refused as `no-manifest`; one with two or more, or with the section inside a nested core module
or component, is refused as `manifest-mismatch`. The package digest is the SHA-256 of the
whole file, manifest included. The format is in [contracts/package.md](contracts/package.md).

**Rationale**:

- One file cannot be paired with the wrong manifest: changing either part changes the file and
  its digest, which the grant record keeps (R4) and which Phase 3 pins at installation.
- Wasmtime 50 compiled a component with the extra section unchanged, and `wasmparser` found it
  in 8 µs, walking the top level and skipping nested modules and components.
- Two manifests in one package are the real risk: a tool that shows the first and a host that
  reads the last would disagree about what the plugin asks for. Refusing duplicates and nested
  copies leaves exactly one manifest that both read.
- Phase 3 installs one release asset, pinned by tag and hash. Phase 6 can sign the whole file,
  either as a detached signature or as a later custom section that covers everything before it,
  as `wasmsign2` does for modules.

**Alternatives considered**:

- A separate `manifest.json` that carries the component's hash: binds just as well, but every
  package becomes two files that travel, download and get lost separately, and every build has
  to rewrite the hash.
- An archive (zip or tar) holding both: one file, but it adds an archive parser, with its
  path-traversal and decompression-bomb risks, to the security core.
- Manifest data in WIT or in the component's exports: it would have to be instantiated, that
  is run, before its manifest is known.

## R2. Manifest format, parsing and the published schema

**Decision**: the manifest is JSON, described by a JSON Schema (draft 2020-12) published as
`wit/manifest.schema.json` ([draft](contracts/manifest.schema.json)). The runtime does not run
a schema validator: it parses with `serde` into typed structs with `deny_unknown_fields`,
wrapped by `serde_path_to_error` so every error names the field path, then checks the rules a
type cannot express (id grammar, SemVer, lengths, duplicates). A test (`manifest_schema.rs`)
runs a corpus of valid and invalid manifests through both the schema, with the `boon` crate,
and the parser, and fails if they ever disagree; it also validates every `manifest.json` in the
repository (SC-006). Before parsing, the section must be at most 16 KiB.

**Rationale**:

- The parser is security core and small. A general JSON Schema engine, with regular
  expressions, remote references and formats, would multiply the code that reads untrusted
  input; keeping it in tests still proves the schema is the truth.
- `serde_json` rejects duplicate struct fields, so a manifest cannot say `"id"` twice and have
  two readers pick different values. Unknown fields are refused: a manifest written for a newer
  host fails clearly on an older one instead of being half understood.
- With typed structs, nesting is bounded by the format itself (three levels), and `serde_json`
  stops at depth 128 anyway; the 16 KiB cap bounds the work before parsing (spec edge case).
- `serde_path_to_error` turns "invalid type: integer, expected a string" into the same message
  at `permissions[0].name`, which FR-003 requires.

**Alternatives considered**:

- Validating at run time with the `jsonschema` or `boon` crate: precise errors for free, but far
  more untrusted-input code in the security core.
- TOML: friendlier to write by hand, but the spec and the authors' tools (editors, JSON Schema
  support) point to JSON.

## R3. Plugin ids

**Decision**: an id is lowercase ASCII and has one of two forms (FR-002):

- `github.com/<owner>/<repository>`: owner 1–39 characters of `a-z`, `0-9` and inner hyphens;
  repository 1–100 characters of `a-z`, `0-9`, `-`, `_` and `.`, not starting with a dot (so
  never `.` or `..`). Only `github.com` is accepted for now; Phase 3 can add hosts.
- `local/<name>`: name 1–64 characters of `a-z`, `0-9` and inner hyphens.

**Rationale**: GitHub compares names without case, so one lowercase spelling avoids two ids for
one repository. Accepting only the host the project installs from keeps every id meaningful;
adding a host later is a schema change, not a reinterpretation. In this phase the CLI accepts
both forms, so authors can test their real manifest, but grants always come from the
development record (R4), so an id never brings an installed plugin's grants.

**Alternatives considered**: any host name (no installer for it yet, and look-alike hosts become
possible); case-insensitive comparison (every reader must then normalize the same way).

## R4. The grant record

**Decision**:

- **Where**: `<data dir>/dev.philsantos.dashboard/development-grants.json`, where `<data dir>`
  comes from `dirs::data_dir()` (`~/.local/share` on Linux, `~/Library/Application Support` on
  macOS, the roaming `AppData` on Windows): the directory Tauri gives the app in Phase 3. The
  CLI takes `--grants <path>` to use another file; tests always do. The app's identifier is
  still provisional and moves with the rename before the first release.
- **Namespaces**: this feature has only the development record, which the CLI uses for local
  files. Phase 3 adds a separate record for installed plugins, keyed by the repository's
  permanent identifier. The runtime takes the record to use as an argument, so a load can
  never consult both, and a development package that claims an installed plugin's id only ever
  sees development grants (spec edge case).
- **Format**: JSON, `record_version: 1`, one entry per plugin id with the origin it was
  approved from, the set of granted permissions and, for display, the version and digest last
  approved ([data-model.md](data-model.md#grant-record)).
- **Origin**: an entry grants only to packages from its own origin (FR-020). In the CLI, a
  package's origin is the canonical path of its file (`std::fs::canonicalize`), which must be
  UTF-8; tests use labels. A path that canonicalization does not unify with the approved one,
  such as another spelling on a case-insensitive file system or a hard link, counts as another
  origin: the mistake is a refusal, never a grant. Phase 3's installed record uses the
  repository's permanent identifier as the origin.
- **Writes**: every change takes an exclusive lock on `development-grants.json.lock`
  (`std::fs::File::lock`, stable since Rust 1.89), re-reads the record, applies the change,
  writes a temporary file in the same directory, syncs it and renames it over the record
  (`tempfile::NamedTempFile::persist`, which replaces the file on Windows too), then unlocks.
  Reads take a shared lock.
- **Damage**: a record that is missing counts as empty. One that cannot be read, is not valid
  JSON, fails its own checks or has a newer `record_version` counts as granting nothing: loads
  are refused as `not-granted` with the reason and the path, and changes are refused until the
  maintainer fixes or deletes the file. Damage is never treated as consent, and the host never
  overwrites a record it could not read.

**Rationale**: a lost update could bring a removed approval back (one process removes it while
another writes an older copy), so read-modify-write must hold the lock. Writing beside the
record and renaming means a crash leaves either the old record or the new one, never half of
each. Plugins have no file access at all, so the record is out of their reach (FR-009). The
origin keeps a file from elsewhere that copies an approved id from inheriting its grants: to
use the approved path, it would have to overwrite a file in the user's own directories.

**Alternatives considered**: SQLite (the plan's choice for plugin storage, feature 004, but
heavy for a few entries); one file per plugin (more files to lock); the OS keychain (built for
secrets, not for a list the user reads).

## R5. The permission engine

**Decision**:

- **Registry**: a static table maps each permission the host knows to the interface that
  carries it in each contract version. Today: `log` → `dashboard:widget/log@0.1.0` (v0) and
  `dashboard:widget/log@1.0.0` (v1). Features 003–005 add one row each.
- **Load, in order** (each step refuses with its own kind and stops; refusals within a step list
  every offending name):
  1. Size and component preamble (feature 001).
  2. Manifest section (R1): `no-manifest`, `manifest-mismatch`.
  3. Manifest (R2, R3): `invalid-manifest`, then `unknown-permission` (FR-008).
  4. Compile (feature 001): `invalid-package`.
  5. Contract: the version of the exported `renderer` sets the plugin's contract; an import
     from another `dashboard:widget` version, including a newer minor, is `contract-mismatch`;
     a v0 plugin whose manifest marks a permission optional is `optional-needs-v1` (FR-013).
  6. Imports: one outside every registered interface is `capability-refused` (feature 001's
     rule: the host offers no such capability); one whose permission the manifest does not
     declare is `not-declared` (FR-006).
  7. Grants: the plugin must have an approval entry from this package's origin, and every
     required permission must be granted, whether the component imports it or not, because
     required permissions are approved together (FR-018). Missing ones are `not-granted` when
     the plugin has no entry or its entry belongs to another origin (FR-020), and
     `needs-new-approval` when an earlier version was approved (FR-007, FR-011).
  8. Pruning: granted permissions this manifest no longer declares are removed from the record
     (FR-012).
  9. Link: a linker built for this plugin gets the real interface for each granted permission
     the component imports, and the stand-in for each optional one that is off (FR-019).
- **Stand-in**: a separate host type per capability interface, implementing the same bindgen
  trait, whose every function returns the interface's `not-granted` error and touches nothing
  else. The real implementation is never linked for a permission that is off, so the stand-in
  cannot leak by mistake.

**Rationale**:

- Cheap checks run before the compile, so a bad manifest costs microseconds, not the tens of
  milliseconds a compile takes.
- The spike linked the stand-in for a v1 `log` import, and building a fresh linker plus
  `instantiate_pre` for hello-widget took 5–17 µs; a linker per plugin costs nothing that
  matters.
- Wasmtime's linker matched an import of `log@1.1.0` against a definition of `log@1.0.0`: it
  resolves by semver-compatible major, in both directions. The runtime's own name check (step
  5) therefore refuses a contract newer than the host's, so a plugin built for a later minor
  is a clear `contract-mismatch`, not a link that might fail later.
- Checking the grants after the imports gives the most specific rule: an undeclared import is
  the author's mistake, an ungranted one is the user's decision.

**Alternatives considered**:

- One shared linker that checks grants inside each host function at call time: the capability
  would be linked into every plugin and guarded by a runtime `if`, against the letter and the
  intent of Principle II.
- Pre-building one linker per subset of permissions: no measurable gain at these costs.

## R6. Grant changes from the next call

**Decision**: the grant store keeps a generation counter that every change increments. A
`Plugin` keeps the compiled component, its manifest and the generation its linking was built
for. At the start of every call, if the generation changed, it re-runs steps 7–9 of R5 against
the current grants and links again; if the plugin is no longer approved, the call is refused
with that error and nothing is instantiated. A call already running keeps its own instance and
finishes within its time limit (FR-014).

**Rationale**: in the spike, re-linking cost 5–17 µs and the compile, the expensive part, is
kept. Instantiating afresh for every call (feature 001, R6) leaves no live instance holding the
old capability. Reading an atomic counter costs nothing measurable per call.

**Alternatives considered**: interrupting the running call when access is removed (a 1 s
window does not justify reaching into a live store now; feature 003 can revisit it for network
requests in flight); reloading the whole plugin, compile included, on every change (tens of
milliseconds for nothing).

In the CLI every command is its own process, so the change shows on the next command. In the
Phase 3 app, host dialogs change grants through the same in-process store, so a loaded widget
sees the change on its next render. The app is the only writer of its record; watching the file
for other writers is not needed.

## R7. Contract v1 and v0 compatibility

**Decision**:

- Contracts move into one directory per version, because `bindgen!` and `wit-bindgen` read one
  package per directory: `wit/v0/widget.wit` (unchanged, `dashboard:widget@0.1.0`) and
  `wit/v1/widget.wit` (`dashboard:widget@1.0.0`, [draft](contracts/widget-v1.wit)).
- v1 keeps `renderer` as in v0. Every capability function returns `result<T, E>`, where `E` is
  an enum or variant of its interface whose first case is `not-granted`; v1 `log.write` returns
  `result<_, log-error>` with `enum log-error { not-granted }`. A test (`contract_v1.rs`) parses
  `wit/v1` with `wit-parser` and fails if any function of any interface other than `renderer`
  breaks that rule.
- Features 003–005 add one interface each to `wit/v1`, bump the package to the next minor
  (1.1.0, 1.2.0, …) and add one row to the registry; the world imports every capability, and a
  plugin's component imports only what it uses (R10).
- The host binds both packages, `bindgen!` once per directory in its own module. A v0 plugin
  links v0 `log`; since v0 `log` has no error, a v0 plugin cannot have optional permissions
  (`optional-needs-v1`).

**Rationale**: a shared error type across interfaces would force every capability into one
error shape; each interface keeping its own errors, with `not-granted` first by rule and by test,
lets feature 003 return `timeout` or `domain-not-allowed` while the stand-in stays uniform. The
spike compiled both bindings side by side and linked the v1 stand-in.

**Alternatives considered**: a shared `types` interface with one error variant (couples every
capability's errors); keeping v0's signatures in v1 (no way to answer "not granted"); a major
version per feature (every plugin rebuilt for each new capability).

## R8. Updates

**Decision**: the grant entry is keyed by plugin id and belongs to the origin it was approved
from (R4), so a package from that origin with a different digest is a new version, whatever its
version number says (spec edge case). Its required permissions are checked against the entry:
all granted, it runs (US3 scenario 1); any missing, `needs-new-approval`, naming them, and the
entry is left untouched (FR-011). A permission it adds as optional starts off (FR-011).
Permissions it no longer declares leave the entry at its first load (R5, step 8; FR-012). A
package from another origin that claims the id gets none of the entry's grants: it is
`not-granted`, naming the approved origin, until approved, and that approval replaces the entry
with the required permissions only (FR-020). The entry records the version and digest last
approved, for `runtime permissions` to show.

**Rationale**: approving the set, not the component, is what lets an unchanged update run
without asking again, which the spec requires. Identity across versions is the id, read from the
origin approved; in Phase 3 the installation makes that origin the repository.

**Alternatives considered**: grants per id alone (any file that claims the id inherits them);
grants per digest (every rebuild asks again, contrary to US3 scenario 1, and approving on every
build turns approval into a reflex); a separate `--grants` for other people's packages (safe
only when remembered); trusting the version number (the spec treats same-number packages as new
versions).

## R9. CLI commands

**Decision**: the CLI is the host's control in this phase, as Principle VII allows until the
host dialogs exist (constitution v1.3.0), and works in development mode only, on local files,
with the development record ([contracts/cli.md](contracts/cli.md)):

- `run` and `bench` now read the manifest and the grants.
- `permissions <plugin>`: lists id, version, author, contract, each permission with required or
  optional and granted or not, and the login providers.
- `approve <plugin>`: grants every required permission the manifest declares, all at once, and
  prints them; there is no way to approve only some (FR-018). An approval from another origin
  is replaced, and the tool says so (FR-020).
- `optional <plugin> <permission> on|off`: turns one optional permission on or off; refused for
  a required one (`required-together`) or one the manifest does not declare.
- `remove <plugin-id>`: removes the plugin's approval and every grant with it.
- `pack <component> --manifest <file> --output <file>`: validates a manifest and adds it as the
  manifest section, for authors outside this repository's xtask (FR-017).

Every command takes `--grants <path>`. Every refusal prints `error: <kind>: <detail>` and exits
with status 1, as in feature 001.

**Rationale**: one verb per decision the spec names keeps each decision explicit, and none can
approve part of the required set. Taking the package file means the CLI always reads the
manifest that will actually run.

**Alternatives considered**: a single `grant <plugin> <permission>` (allows partial approval of
required permissions); flags on `run` that grant on the fly (a decision hidden in a run).

## R10. Fixtures, packing and the hostile cases

**Decision**:

- Every crate in `plugins/` has a `manifest.json` beside its `Cargo.toml`, which the xtask adds
  as the manifest section after componentizing (`wasm-encoder`). A crate can opt out with
  `[package.metadata.dashboard] manifest = false`; its component is written bare.
- The feature 001 hostile crates get manifests (`local/<name>`, `log` required where they log),
  so their own rules still decide them. `hello-widget` moves to contract v1. A new
  `plugins/fixtures/hello-v0` keeps a well-behaved v0 plugin for FR-013.
- Two bare fixtures serve the permission cases: `probe` (v1, calls `log` and puts in its output
  what `log` answered) and `quiet` (v1, imports nothing). Each hostile case packs them with its
  own manifest in the test, through the same `pack` function as the CLI, so the manifests sit
  next to the assertions.
- Duplicate and nested manifest sections are built in the test (a second section appended to
  the bytes; a WAT component with the section inside a nested module).
- FR-009 and FR-010 are checked by running every case, and the probe, against a file-backed
  record: its bytes and the store's generation must not change across any render.
- FR-020 is checked by loading the approved `probe`, byte for byte, from another origin.

**Rationale**: the rules under test are about the package and the grants, not about plugin
code; two small plugins cover every combination. `wit-component` dropped every unused import in
the spike (`deep-recursion` imports nothing although its world imports `log`), so `quiet`
imports nothing and the probe imports exactly `log`.

**Alternatives considered**: one crate per case (a dozen near-identical crates); manifests
written by hand into binary fixtures (unreviewable).

## R11. Measuring SC-007

**Decision**: `runtime bench` prints two more values: `checks_us`, the time to find and parse
the manifest, read the grant record and run the import and grant checks, without the compile;
and `relink_us`, the time to link again after a grant change. `bench.yml` records them on the
three CI systems into `specs/002-plugin-permissions/performance.md`, against the 10 ms target.

**Rationale**: the spike's parts add up to well under a millisecond (8 µs for the section,
5–17 µs to link); the record's file read is the only unknown, and only CI can show the slowest
system. A unit test with a timing assertion would be flaky on shared runners.

## R12. Dependencies

| Crate | Where | Why | Already in `Cargo.lock` |
| --- | --- | --- | --- |
| `serde` (derive) | runtime | Typed manifest and record | Yes |
| `serde_path_to_error` | runtime | Field path in manifest errors | No (dtolnay, MIT OR Apache-2.0) |
| `semver` | runtime | Plugin versions | Yes |
| `sha2` 0.10 | runtime | Package digest | Yes |
| `wasmparser` 0.261 | runtime | Finding the manifest section; the version Wasmtime 50 uses | Yes |
| `dirs` 7 | runtime | Default record path | Yes |
| `tempfile` | runtime | Atomic writes | Yes |
| `boon` | runtime, dev | Checking the schema against the parser | No (MIT OR Apache-2.0) |
| `wit-parser` 0.261 | runtime, dev | Checking the v1 rule | Yes |
| `wasm-encoder` 0.262 | xtask | Adding the manifest section | Yes |

Each must pass `cargo deny` (licenses, sources) and `cargo audit` in CI.
