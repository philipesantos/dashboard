# Feature Specification: Plugin Storage

**Feature Branch**: `004-plugin-storage`

**Created**: 2026-10-24

**Status**: Draft

**Input**: User description: "Feature 004 — per-plugin isolated storage with a quota (Phase 2 of
the guide; gate case: reading another plugin's storage must be blocked; constitution v2.3.1,
Principles II, VIII, IX; fixed decision: SQLite, one database per plugin). A new `storage`
permission, required or optional, declared in the manifest and approved like any other; the host
links the import only when granted, and an optional one left off gets a stand-in that answers
not-granted, writes nothing and keeps the existing data. The plugin declares its quota in the
manifest, up to a host cap; the quota is shown at approval and is part of what is approved, so an
update that raises it requires a new approval (lowering it does not; data already over a lowered
quota is kept, and only deletes and writes that bring usage back under it succeed). The plugin
never names a database: the host binds it at link time from the plugin's id and origin (in
development, the canonical path of the file, as for grants), so a file that copies an id gets
nothing. Plugins see a key-value interface (get, set, delete, list keys by prefix, paginated)
that the host runs with its own prepared statements; plugins never send SQL, keys and values are
opaque data, never paths, and the connection refuses ATTACH and extension loading and runs in
defensive mode. Usage counts the bytes of keys and values plus a fixed cost per entry, so it is
the same on every system; there are also caps on file size, key length, value size and writes per
run; a write past the quota fails as over-quota and leaves no partial write. Storage is read-only
in render: render reads a consistent snapshot, and a write from render is refused (like a
connector call outside update), so render stays a pure view the host may run as often as it
likes. Writes happen only in update; each update is one transaction, committed only when the call
returns ok and rolled back on a trap, limit, error, or an approval taken away or turned off
during the run. A widget may export updater only to write storage, without connectors. Data is
kept across updates from the same origin; another origin starts empty. Contract v1 moves to
1.2.0: freeze 1.1.0 under wit/history/1.1.0 first, then add the storage interface, whose error
enum starts with not-granted, and extend runtime/tests/contract_v1.rs to the 1.1.0→1.2.0 pair; a
manifest declaring storage with a contract older than 1.2.0 is contract-too-old. The development
CLI shows a plugin's storage and usage (every text escaped) and clears it; development storage
lives in the development data directory, which installed plugins never use, apart from host
records (grants, kept data, request log), which no plugin can reach. Deliverables: a small
example plugin that remembers something across loads (writes in update, reads in render), the
hostile cases below, docs (SECURITY.md, docs/threat-model.md, wit/README.md,
docs/plugin-authors.md, plugins/hostile/README.md), and bench numbers (call latency, the cap, the
per-entry cost) in the feature's performance.md. Hostile cases: other-plugin (B lists and reads
with A's key names: sees only its own empty store; A's data unchanged); same-id-other-origin
(not-granted; once approved, its own empty store); key-injection (keys with ../, /, NUL, quotes,
SQL and ATTACH text: stored opaque or invalid-key, nothing outside its own store touched);
file-access (opening the storage file via wasi:filesystem: capability-refused);
quota-escalating-update (raising the quota: needs-new-approval); quota-fill, many-keys and churn
(over-quota or over-limit; usage and file size stay under their caps; other plugins unaffected);
write-in-render (refused, nothing changed); crash-mid-write (writes then traps on fuel, memory or
panic: rolled back); optional-off and turned off mid-run (not-granted, nothing written, data
intact when turned back on); host-records-untouched (grants, kept data and request log unchanged
by every case); show-spoof (keys and values with line breaks and terminal escapes printed escaped
by the CLI); connection-hardened (white box: ATTACH and extension loading refused on the
connection itself). Out of scope: SQL exposed to plugins; storage per widget instance or shared
between plugins; sync, backup, export or encryption at rest (storage is not for secrets, which
plugins never receive, Principle IV); a quota the user changes and storage of installed plugins
(Phase 3, with installation); uninstall deleting data and disable keeping it (Phase 3; the CLI
clears data explicitly until then); a file or blob API; the vault, OAuth and the token case
(feature 005); the TypeScript example (separate Phase 2 item)."

## Clarifications

### Session 2026-10-24

- Q: When the maintainer approves a widget that declares storage, should the tool require the
  approval digest that the permissions listing printed, as it does for widgets with connectors?
  → A: Yes, whenever storage is declared, required or optional; the quota is part of the approval
  digest (FR-008).
- Q: Must a widget that declares storage also export the update entry point? → A: Yes; one that
  does not is refused at load as missing updater, naming that storage is written only in an
  update (FR-012).
- Q: How should the maintainer find and clear a store whose package file was moved or deleted?
  → A: The tool lists every store (id, origin, usage, quota, and whether the origin's file still
  exists) and clears one by id and origin, whether or not the file exists (FR-014).

## User Scenarios & Testing *(mandatory)*

The people using this feature are the maintainer, who runs plugins and approves them through the
command-line tool, and plugin authors. End users make the same decisions later, in the host
dialogs of Phase 3.

A plugin's storage is a small set of named values that the host keeps for that plugin alone,
across runs of the host and across new versions of the plugin. The plugin asks the host to read
and write values by name; it never sees where or how the host keeps them, and has no way to name
anything outside its own store.

### User Story 1 - A widget remembers its own data (Priority: P1)

An author writes a widget that remembers something across loads, such as how many times it has
updated and when it first ran. Its manifest declares the storage permission and the quota it
needs. The maintainer asks what the widget requests and sees storage with that quota, and
approves it. When the widget updates, it writes its values; when it renders, it reads them and
shows them. After the host is closed and run again, and after a new version of the widget built
to the same file, the widget still finds its values.

**Why this priority**: widgets need state of their own (settings, notes, what they last saw)
without any access to the user's files, and this is the only sanctioned way to keep it.

**Independent Test**: approve the example widget, run an update and a render, and check that the
render shows the stored values; run the host again, update and render: the values carried over
and changed as the widget wrote them.

**Acceptance Scenarios**:

1. **Given** a widget that declares storage, **When** the maintainer asks what it requests,
   **Then** the tool lists storage as required or optional, with the quota the widget declared,
   and the approval digest of what it showed.
2. **Given** the package changed after the maintainer asked, such as a larger quota, **When** the
   maintainer approves with the digest shown earlier, **Then** the approval is refused and
   nothing is granted; approving without a digest is refused too.
3. **Given** the widget is approved, **When** an update writes values and returns successfully,
   **Then** the values are kept, and the next render reads them.
4. **Given** values a widget wrote, **When** the host runs again and the widget renders, **Then**
   it reads the same values.
5. **Given** values a widget wrote, **When** a new version of the widget from the same origin is
   loaded and approved where needed, **Then** it reads the values the earlier version wrote.
6. **Given** a widget that stores values, **When** it lists the names it stored that start with a
   prefix, **Then** it receives them in a fixed order, a page at a time, with a way to ask for
   the next page.

---

### User Story 2 - No plugin reaches another plugin's storage (Priority: P1)

The maintainer runs hostile plugins that each try to read or change data that is not theirs: one
uses the names another plugin stored under; one is a file that copies an approved plugin's id;
one uses names that look like paths or database commands; one tries to open the storage file
directly; one tries to reach the host's own records through storage. Each finds only its own
store, or is refused, nothing outside its store is read or changed, and the host keeps working.
The cases run automatically on Windows, macOS and Linux and break the build if any one is not
contained.

**Why this priority**: this is the Phase 2 gate case "reading another plugin's storage is
blocked", and every security rule needs a test that breaks the build (Principle VIII).

**Independent Test**: run the hostile suite; every new case is contained, every other plugin's
store and the host's records are unchanged byte for byte, and a well-behaved widget updates and
renders normally right afterwards.

**Acceptance Scenarios**:

1. **Given** plugin A has stored values, **When** plugin B reads, lists and writes using A's
   names, **Then** B sees only its own store, which holds none of A's values, and A's store is
   unchanged.
2. **Given** an approved plugin, **When** a file from another origin claims the same id, **Then**
   it is refused as not granted; **when** that file is approved in its own right, **Then** it gets
   a store of its own, empty, never the first plugin's.
3. **Given** names containing `../`, `/`, `\`, a NUL character, quotes, line breaks or database
   commands, **When** a plugin writes and reads them, **Then** each is kept and returned exactly
   as given, as an ordinary name inside the plugin's own store, and nothing outside that store is
   touched.
4. **Given** a plugin whose component asks for file system access to open its storage file or
   any other file, **When** it is loaded, **Then** it is refused before running.
5. **Given** every hostile case, **When** it runs, **Then** the host's records (grants, kept data,
   request log) and every other plugin's store are unchanged.
6. **Given** any hostile case was just contained, **When** a well-behaved widget updates and
   renders next, **Then** it produces its normal result; the cases of features 001, 002 and 003
   still pass.

---

### User Story 3 - The quota bounds what a plugin keeps (Priority: P2)

A widget declares the quota it needs, up to a cap the host sets. Writes that would take its usage
past the quota are refused and change nothing. A new version that asks for a larger quota does
not run until approved; one that asks for less runs, keeps what is already stored, and accepts
only deletes and writes that leave usage within the new quota. Limits on the size of each name
and value, on how many storage calls a run makes and on the size of the file on disk keep one
widget from filling the disk or slowing the host, however it writes.

**Why this priority**: without a quota a plugin can fill the user's disk; an update that silently
asks for more space is an escalation, under feature 002's rule.

**Independent Test**: approve a widget with a small quota; write until a write is refused as over
quota, and check nothing of that write was kept; load a version with a larger quota (refused
until approved) and one with a smaller quota (runs, deletes succeed, growing writes refused).

**Acceptance Scenarios**:

1. **Given** a widget whose usage is near its quota, **When** a write would take usage past it,
   **Then** the write is refused as over quota, naming the quota and the usage, and nothing of
   that write is kept.
2. **Given** a manifest that declares a quota over the host's cap, or storage without a quota,
   **When** it is loaded, **Then** it is refused, naming the field and the cap.
3. **Given** an approved widget, **When** a new version declares a larger quota, **Then** it is
   refused until approved, naming the old and the new quota, and the grants already given stay as
   they were.
4. **Given** a new version that declares a smaller quota than the usage already stored, **When**
   it runs, **Then** its data is kept, deletes succeed, and a write succeeds only if usage after
   it is within the new quota.
5. **Given** a widget that writes many tiny values, rewrites and deletes the same values without
   end, or makes thousands of storage calls in one run, **When** it runs, **Then** the calls past
   a limit are refused naming the limit, usage stays within the quota, the file on disk stays
   within its cap, and other plugins are not affected.
6. **Given** a name or a value longer than its limit, **When** it is written, **Then** the write
   is refused, naming the limit.

---

### User Story 4 - Writes are all or nothing, and render only reads (Priority: P2)

A widget writes only during an update; render reads a consistent view and can never change what
is stored, so the host may render as often as it needs. The writes of an update are kept only when
the update finishes successfully: when it fails, runs out of a limit, crashes, or its approval is
taken away or its storage turned off while it runs, nothing it wrote is kept.

**Why this priority**: a widget killed for exceeding a limit must not leave half-written data,
and rendering must stay free of side effects, as it is free of network calls (feature 003).

**Independent Test**: make a widget write and then crash, exceed its CPU, memory or time, or
return an error: its store is as it was before the update; make a widget write in render: the
write is refused and nothing changes; turn storage off during an update: nothing it wrote is kept.

**Acceptance Scenarios**:

1. **Given** an update that writes values and then fails (returns an error, crashes, exceeds its
   CPU, memory or time limit, or returns invalid output), **When** it ends, **Then** none of its
   writes is kept and the store is exactly as before the update.
2. **Given** an update that writes values and returns successfully, **When** it ends, **Then** all
   of its writes are kept together.
3. **Given** a widget during render, **When** it writes or deletes a value, **Then** the call is
   refused, naming that render only reads, and the store is unchanged.
4. **Given** an update in progress, **When** the maintainer turns the widget's optional storage
   off or removes its approval, from any process, **Then** from the next call storage answers
   "not granted" or the widget is refused, and nothing the update wrote is kept, even if it then
   returns successfully.
5. **Given** a widget that exports the update entry point and declares storage but no connector,
   **When** the maintainer runs an update, **Then** it runs and may write its store.
6. **Given** a render while an update of the same widget writes in another process, **When**
   render reads, **Then** it sees the values as they were either before or after that update,
   never part of it.

---

### User Story 5 - The maintainer sees and clears a plugin's storage (Priority: P3)

The maintainer asks the tool what a plugin keeps: its usage against its quota and its stored
names and values, every text shown escaped so nothing a plugin wrote can disguise the output.
The maintainer can clear a plugin's store. Turning optional storage off, or removing the
plugin's approval, keeps the data; only clearing removes it. Disabling and uninstalling arrive in
Phase 3.

**Why this priority**: the maintainer needs to see what a plugin keeps and to reset it, and data
must not disappear because a permission was turned off for a while.

**Independent Test**: store values with names and contents that hold line breaks and terminal
escapes; show the store and check that every text is escaped and each entry is on its own line;
turn storage off and on and check the data is still there; clear the store and check it is empty.

**Acceptance Scenarios**:

1. **Given** a plugin with stored values, **When** the maintainer shows its storage, **Then** the
   tool lists the usage, the quota and each name with its value, escaping every text and showing
   values that are not text in a form that cannot be mistaken for the tool's own output.
2. **Given** optional storage that is off, **When** the widget calls it, **Then** every call
   answers "not granted", nothing is read or written, and the widget keeps running; **when**
   storage is turned on again, **Then** the earlier data is there.
3. **Given** a plugin's approval is removed, **When** it is approved again from the same origin,
   **Then** its earlier data is there.
4. **Given** a plugin's store, **When** the maintainer clears it, **Then** it is empty and its
   usage is zero; other plugins' stores are unchanged.
5. **Given** a store whose package file was moved or deleted, **When** the maintainer lists the
   stores, **Then** it appears with its id, origin, usage and quota, marked as having no file;
   **when** the maintainer clears it by id and origin, **Then** it is gone and no other store
   changes.

---

### User Story 6 - Authors build on contract 1.2.0 (Priority: P3)

Authors declare storage with the published manifest schema and documentation and build against
contract 1.2.0. Plugins built for 1.0.0 and 1.1.0 keep running; one built for a newer minor
version is refused.

**Why this priority**: storage is useful only if authors can use it correctly, and existing
plugins must not break when the contract grows.

**Independent Test**: validate the example widget's manifest against the schema and run it; run
the existing 1.0.0 and 1.1.0 plugins of the repository on the new host; check that a plugin
declaring storage while built for 1.1.0 is refused naming 1.2.0.

**Acceptance Scenarios**:

1. **Given** the published schema and documentation, **When** an author declares storage and a
   quota like the example, **Then** the manifest validates and the example runs.
2. **Given** plugins built for contract 1.0.0 and 1.1.0, **When** they run on this host, **Then**
   they work as before; **given** a plugin built for a newer 1.x than the host's, **When** it is
   loaded, **Then** it is refused as a contract mismatch, naming both versions.
3. **Given** a plugin built for contract 1.1.0 or older whose manifest declares storage, **When**
   it is loaded, **Then** it is refused, naming 1.2.0 as the version storage needs.

### Edge Cases

- A widget that declares storage but exports no update entry point: refused at load as missing
  updater, since nothing could ever write its store.
- A plugin that declares storage but never calls it: allowed; required storage still needs
  approval, as every required permission does (feature 002).
- A plugin that does not declare storage calls it: refused as not declared at load, as for any
  capability (feature 002).
- A plugin reads a name that was never written: it is told the name is absent, which is a normal
  answer, not an error.
- A plugin writes the empty name: refused as an invalid name. Any other text within the length
  limit is a valid name, kept and compared exactly as given, with no normalization of case,
  Unicode or separators.
- A value is empty: allowed; it is stored and counts the fixed cost of an entry.
- A write replaces a value with a larger one near the quota: usage is checked as it would be
  after the write, counting the old value as gone.
- Two values are written to the same name in one update: the last one is kept.
- A delete of a name that is absent: succeeds and changes nothing.
- Listing while the same update writes: the list reflects the update's own writes so far.
- The host's cap changes in a later version of the host to below a quota already approved: the
  plugin is refused until a new version declares a quota within the cap; its data is kept.
- A storage file the host finds damaged, of an unknown format, or changed outside the host:
  the plugin's storage calls fail naming that the store is damaged, nothing is deleted, and the
  maintainer may clear it; other plugins are not affected.
- The disk is full or the storage file cannot be written: the update fails naming the cause, and
  the store is as before.
- Two runs of the host use the same plugin at once: writes of one update are never interleaved
  with another's; an update that cannot start writing within its time limit fails, naming that
  the store is busy, and nothing is kept.
- The host's data directory is unavailable: plugins that declare required storage are refused
  naming the cause; optional storage answers as unavailable, not as granted.
- A plugin keeps a secret in storage: the store is not encrypted; documentation says storage is
  not for secrets, which plugins never receive (Principle IV).
- A new version of a widget is loaded: its kept data from feature 003 is discarded, as before,
  but its storage is kept. Kept data is the host's copy of the last update; storage is the
  plugin's own state.
- A package file is moved or deleted: its store stays, unreachable by any plugin, since another
  path is another origin; the maintainer finds it in the list of stores and clears it.
- A file at the same path is replaced by another plugin with a different id: it gets its own
  store; data belongs to the id and the origin together.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Storage MUST be a permission, named `storage`, that a plugin declares in its
  manifest as required or optional, together with its quota, and that the user approves under the
  rules of feature 002. The host MUST link the storage capability only when it is granted; an
  optional storage permission MUST start off and, while off, MUST be linked only to a stand-in
  that answers every call with "not granted", reads nothing, writes nothing and leaves the stored
  data as it is (Principle II).
- **FR-002**: Each plugin MUST have one store of its own, chosen by the host when it links the
  plugin from the plugin's id and origin together: in this phase, the origin is the canonical path
  of the package file, as for grants. No call MAY let a plugin name, choose or reach a store, a
  file or a location: the capability acts only on the store the host bound. A package from
  another origin that claims the same id MUST inherit neither grants nor data.
- **FR-003**: Plugins MUST see storage only as named values: read a value by name, write a value
  under a name, delete a name, and list the names that start with a prefix, in a fixed order,
  one page at a time with a bounded page size. Names are non-empty text and values are bytes; the
  host MUST treat both as opaque data, never as a path, a command or a pattern, and keep and
  return them exactly as given.
- **FR-004**: The host MUST run storage with its own fixed operations only: no plugin MAY send
  the host any query, command or expression. The store MUST refuse attaching other databases and
  loading extensions, and run in its most restrictive mode, so that even a fault in the host's own
  operations reaches nothing outside the plugin's store.
- **FR-005**: A plugin's usage MUST be the bytes of its names and values plus a fixed cost per
  stored entry, computed the same way on every system. The quota a plugin declares MUST be a
  number of such bytes, at least the cost of one entry and at most the host's cap; the host MUST
  refuse a manifest that declares storage without a quota, or with a quota outside those bounds,
  naming the field and the bounds.
- **FR-006**: A write that would take usage past the quota MUST be refused as over quota, naming
  the quota and the usage, and nothing of it MAY be kept. While usage is over the quota (after a
  new version lowered it), deletes MUST succeed and a write MUST succeed only if usage after it is
  within the quota.
- **FR-007**: The host MUST limit the length of a name, the size of a value, the number of
  storage calls in each run of `update` or `render` (every call counted, refused ones included),
  the bytes written in each update, the page size of a list, and the size of each store's file on
  disk. A call over a limit MUST be refused, naming the limit; the file MUST never grow past its
  cap, whatever sequence of writes and deletes a plugin makes.
- **FR-008**: Before approval, the host MUST show that the plugin declares storage, required or
  optional, and its quota; the quota is part of what is approved. In this phase, the tool MUST
  approve a plugin that declares storage, required or optional, only when given the digest of
  what it showed, quota included, and MUST refuse when anything changed since, granting
  nothing, as for connectors (feature 003). A new version that declares a
  larger quota MUST be refused until approved, naming the old and the new quota, with the grants
  already given unchanged. A new version that declares the same or a smaller quota MUST NOT need a
  new approval for it.
- **FR-009**: Storage MUST be read-only during render: render MAY read and list, and MUST read one
  consistent view of the store for the whole run; a write or delete during render MUST be refused,
  naming that render only reads, and change nothing.
- **FR-010**: A plugin MAY write only during an update. Each update's writes MUST be kept all
  together when the update returns successfully, and none of them MAY be kept when it returns an
  error, crashes, exceeds a limit, returns invalid output, or when its storage is turned off or its
  approval removed while it runs. A render or another update never sees part of an update's
  writes.
- **FR-011**: Turning a plugin's optional storage off, or removing its approval, from any process,
  MUST take effect from the plugin's next storage call, even within a run already in progress, and
  an update running when it happens MUST NOT keep any of its writes. Neither action deletes the
  stored data.
- **FR-012**: A widget that declares storage MUST export the update entry point, where it writes
  its store, or be refused at load as missing updater, naming that storage is written only in an
  update. A widget that declares storage and no connector MAY export the update entry point only
  to write its store, and the host MUST run its updates as for a widget with connectors. Feature
  003's rules for kept data are unchanged; storage is kept across new versions of the plugin from
  the same origin, while kept data is not.
- **FR-013**: Every plugin's store MUST live apart from the host's own records (grants, kept data,
  request log), which no storage call can read or change, and from every other plugin's store. In
  this phase stores live in the development data directory, which installed plugins never use;
  storage for installed plugins arrives with installation in Phase 3.
- **FR-014**: The development command-line tool MUST show a plugin's storage (usage, quota, and
  each name and value) escaping every text a plugin wrote, and MUST clear a plugin's store. It
  MUST list every store in the development data directory, with its id, origin, usage, quota and
  whether the origin's file still exists, and MUST clear a store by id and origin whether or not
  that file exists. Only
  the host's own controls MAY read or clear a store other than through the plugin's own calls.
- **FR-015**: Contract v1 MUST move to version 1.2.0, adding the storage capability, whose every
  failure can answer "not granted"; version 1.1.0 MUST first be kept unchanged as a record, and a
  test MUST fail the build if 1.2.0 does more than add to 1.1.0. The host MUST run plugins built
  for any v1 minor version up to its own as before, and refuse one built for a newer minor version
  as a contract mismatch, naming both versions. A plugin that declares storage MUST be built for
  1.2.0 or later, or be refused, naming the version needed.
- **FR-016**: Every refusal and failure MUST name, in one message, the name, limit or field
  concerned and the rule that applied: not granted, render only reads, over quota, over a limit,
  invalid name, store damaged, store busy, store unavailable; and, when a plugin loads or is
  approved: invalid manifest (quota missing or outside its bounds), contract too old, missing
  updater, needs a new approval (larger quota), approval changed (or no digest given).
- **FR-017**: The hostile suite MUST gain one case per rule: another plugin's names
  (FR-002, FR-003), a file that copies an approved id (FR-002), names that look like paths or
  commands (FR-003, FR-004), opening the storage file through the file system (FR-002), a version
  that raises its quota (FR-008), filling the quota (FR-006), many tiny values and endless
  rewrites (FR-005, FR-007), too many calls in one run (FR-007), a write in render (FR-009), a
  crash, CPU, memory or time limit after writes (FR-010), optional storage off and turned off
  during an update (FR-001, FR-011), a spoofed display of stored names and values (FR-014), and
  a white-box check that the store refuses attaching and extensions (FR-004). Every case MUST also
  leave the host's records and every other store unchanged (FR-013). The suite MUST run on
  Windows, macOS and Linux on every change that is not documentation-only and fail the build if
  any case is not contained (Principle VIII).
- **FR-018**: The project MUST ship an example widget that writes its store in an update and reads
  it in render, the manifest schema's storage declaration, and documentation for plugin authors
  (declaring storage and a quota, the read-only render, all-or-nothing updates, and that storage is
  not for secrets).
- **FR-019**: The security documents MUST describe plugin storage and its rules: `SECURITY.md`,
  the threat model, the contracts' `README`, the plugin author guide and the hostile suite's
  `README`.

### Key Entities

- **Store**: the named values the host keeps for one plugin, bound to its id and origin together;
  it has a usage and a quota.
- **Entry**: one name and its value; a name is non-empty text, a value is bytes.
- **Usage**: the bytes of a store's names and values plus a fixed cost per entry.
- **Quota**: the most usage a plugin declares it needs, approved with its permissions and never
  above the host's cap.
- **Host cap**: the largest quota the host accepts from any plugin.
- **Storage declaration**: what a manifest says about storage: required or optional, and the
  quota.
- **Contract 1.2.0**: contract v1 with the storage capability; the host still runs plugins built
  for 1.0.0 and 1.1.0.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the new hostile storage cases are contained on all three operating systems,
  alongside the cases of features 001, 002 and 003, and after each case every other store and
  every host record is unchanged byte for byte.
- **SC-002**: For every combination of declared, approved, turned-on and called storage, a value
  is read or written only when the plugin declared storage, the user approved it, it is on, and
  the store is the plugin's own, verified by tests.
- **SC-003**: Every refusal tells the maintainer, in one message, the name, limit or field and
  the rule, without reading source code or host logs.
- **SC-004**: An update that fails in any of the ways of FR-010 leaves the store exactly as it was,
  every time.
- **SC-005**: Usage never exceeds the approved quota, and no store's file grows past its cap, in
  any test, whatever the sequence of writes and deletes.
- **SC-006**: A render that reads 100 values finishes within render's limits, and storage calls
  add less than 1 ms each on every CI system, as measured in the feature's performance record.
- **SC-007**: Every plugin built for contract 1.0.0 or 1.1.0 in the repository passes unchanged on
  the new host, and a plugin built for a newer minor version is refused every time.
- **SC-008**: The example widget validates against the published schema in CI and keeps its values
  across two runs of the host and across a new version from the same origin.

## Assumptions

- In this phase the maintainer approves plugins and inspects and clears storage with the
  command-line tool. The host dialogs of Phase 3 apply the same rules: they show the quota before
  approval, and disabling a plugin keeps its data while uninstalling deletes it.
- The host's cap, the fixed cost per entry, the limits of FR-007 and the file size cap are set in
  the plan and measured on the three systems, as features 001 and 003 did.
- The fixed technical decision applies: one database per plugin, using SQLite (constitution,
  Fixed Technical Decisions). Plugins never see that choice; the contract speaks only of named
  values.
- The storage engine is part of the plugin runtime, so every change to it gets line-by-line review
  (Principle IX).
- Storage is per plugin, not per widget instance: until the host app (Phase 3) can show several
  widgets of one plugin, there is one instance per plugin.
- Another process on the user's machine with access to the user's files can read or change the
  stores; that is outside the threat model, as for the host's other records. The host only makes
  sure such changes cannot reach other stores or the host's records.
- One person uses the host on a machine, as in features 002 and 003.
- Out of scope: queries or commands sent by plugins; storage per widget instance or shared between
  plugins; sync, backup, export or encryption at rest; a quota the user changes, storage for
  installed plugins, disabling and uninstalling (Phase 3, with installation); a file or blob
  capability; the secrets vault, OAuth and the token case (feature 005); the TypeScript example
  (its own Phase 2 item).
