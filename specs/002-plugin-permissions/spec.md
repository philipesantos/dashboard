# Feature Specification: Plugin Permissions

**Feature Branch**: `002-plugin-permissions`

**Created**: 2026-10-22

**Status**: Draft

**Input**: User description: "Plugin permissions (Phase 2 of the development plan, first of five
features). Goal: give plugins powers only through a manifest the plugin declares and grants the
user approves, so that every later capability (network through the proxy, storage, secrets,
OAuth, features 003–005) plugs into one permission engine. A plugin is born with no power
(constitution Principle II)."

## User Scenarios & Testing *(mandatory)*

The people using this feature are the maintainer, who runs plugins and decides what they may
do, and plugin authors, who declare what their plugin needs. End users make the same decisions
later, in the host dialogs of Phase 3; this feature provides the rules those dialogs apply. In
this phase the maintainer makes every decision through the command-line tool.

### User Story 1 - A plugin gets exactly the capabilities declared and granted (Priority: P1)

A plugin package states, in its manifest, every capability it needs, each marked required or
optional. The maintainer sees what the plugin requests and approves its required permissions
all together, or the plugin does not run. The plugin then runs with exactly the capabilities
that are both declared and granted, and nothing else.

**Why this priority**: this is Principle II made concrete. Every capability of features
003–005 (network, storage, secrets, accounts) depends on it, and without it the only safe
plugin is one with no capability at all.

**Independent Test**: give a plugin a manifest, approve its required permissions through the
tool, run it and check it works; then load a plugin whose required permissions were never
approved and check it is refused, naming the permission.

**Acceptance Scenarios**:

1. **Given** a plugin whose manifest declares the capabilities it uses, **When** the maintainer
   asks what it requests, **Then** the tool lists every requested permission, marked required
   or optional, and every login provider, and which of them are granted.
2. **Given** the maintainer approved the plugin's required permissions, **When** the plugin
   runs, **Then** it works with those capabilities.
3. **Given** a plugin that uses a capability its manifest does not declare, **When** it is
   loaded, **Then** it is refused before running, and the error names that capability and says
   it was not declared.
4. **Given** a plugin that declares a required capability the maintainer has not approved,
   **When** it is loaded, **Then** it is refused before running, and the error names that
   capability and says it was not granted.
5. **Given** a package without a manifest, or whose manifest does not describe that exact
   plugin, **When** it is loaded, **Then** it is refused, and the error says why.
6. **Given** a plugin with several required permissions, **When** the maintainer tries to
   approve only some of them, **Then** the tool refuses: required permissions are approved all
   together or not at all.

---

### User Story 2 - Hostile permission cases are contained (Priority: P1)

The maintainer runs hostile packages that each try to get a capability without the right to
it: one that uses a capability it never declared, one that declares a capability nobody
granted, one that uses an optional capability that is off, one whose manifest belongs to
another plugin, one whose manifest breaks the schema, and one whose new version quietly asks
for more. Each is refused, the error names the rule, and the host keeps working; the one whose
optional capability is off runs, and gets nothing from it. The cases run automatically on
Windows, macOS and Linux and break the build if any one is not contained.

**Why this priority**: every security rule needs a test that breaks the build (Principle VIII).
The phase gate (exfiltration blocked) builds on these rules.

**Independent Test**: run the hostile suite; every new case is refused with its rule, or gets
nothing from an optional capability that is off, and a well-behaved plugin with a valid
manifest and grants runs normally right afterwards.

**Acceptance Scenarios**:

1. **Given** each hostile package, **When** it is loaded, **Then** it is refused with an error
   naming its rule (scenario 4 covers the optional capability that is off).
2. **Given** any hostile package was just refused, **When** a well-behaved plugin runs next,
   **Then** it produces its normal result.
3. **Given** the cases of feature 001, **When** the suite runs, **Then** they still pass: the
   suite only grows.
4. **Given** a hostile package that uses an optional capability that is off, **When** it runs,
   **Then** every use answers "not granted" and nothing reaches that capability.

---

### User Story 3 - Updates never widen access silently (Priority: P2)

The maintainer installs a new version of a plugin that is already approved. If it asks for
nothing new, it runs with the grants it had. If it requires a new permission, it does not run
until the maintainer approves it. Required permissions are never taken back one by one: the
maintainer can remove the plugin's approval as a whole, and the plugin then does not run until
it is approved again.

**Why this priority**: an update that gains a permission silently is the classic way a trusted
plugin turns hostile (threat model: "Update adds a permission without approval").

**Independent Test**: approve a plugin, load a new version with the same permissions (it
runs), load one with an extra required permission (refused, naming it), approve it (runs),
remove the plugin's approval (refused at the next call).

**Acceptance Scenarios**:

1. **Given** an approved plugin, **When** a new version requests the same permissions,
   **Then** it runs without any new approval.
2. **Given** an approved plugin, **When** a new version requires a permission it was not
   granted, **Then** it is refused before running, the error names the new permission, and the
   grants already given stay as they were.
3. **Given** that refused version, **When** the maintainer approves the new permission,
   **Then** it runs.
4. **Given** a new version that no longer requests a permission, **When** a later version asks
   for it again, **Then** it needs approval again.
5. **Given** an approved plugin, **When** the maintainer tries to take back one of its required
   permissions, **Then** the tool refuses and says that required permissions change only with
   a new version or by removing the plugin's approval.
6. **Given** an approved plugin, **When** the maintainer removes its approval, **Then** it is
   refused at its next call, naming the permissions that are not granted, and it needs the
   whole approval again to run.

---

### User Story 4 - Optional permissions let a plugin run with less (Priority: P2)

An author marks a permission optional when the plugin works without it, for example a widget
that shows public data and, with an account connected, private data too. Optional permissions
start off. The maintainer turns each one on or off with the host's controls at any time, and
the plugin runs either way: while an optional permission is off, every use of that capability
answers "not granted" and gives nothing.

**Why this priority**: the user decides how much each plugin may do without losing the plugin,
and contract v1 must carry the "not granted" answer before features 003–005 define their
interfaces on it.

**Independent Test**: approve a plugin's required permissions and leave an optional one off:
the plugin runs, and its use of that capability answers "not granted" and has no effect; turn
it on, and from the next call the capability works; turn it off, and from the next call it
answers "not granted" again.

**Acceptance Scenarios**:

1. **Given** a plugin whose optional permission is off, **When** it runs and uses that
   capability, **Then** it keeps running, every use answers "not granted", and nothing reaches
   the capability.
2. **Given** that plugin, **When** the maintainer turns the optional permission on, **Then**
   the capability works from the plugin's next call.
3. **Given** an optional permission that is on, **When** the maintainer turns it off, **Then**
   every use answers "not granted" from the plugin's next call, even if the plugin stayed
   loaded.
4. **Given** an approved plugin, **When** a new version adds an optional permission, **Then**
   it runs without any new approval, with that permission off.
5. **Given** an optional permission that is off, **When** a new version marks it required,
   **Then** that version is refused until the maintainer approves it.

---

### User Story 5 - Authors declare what their plugin needs (Priority: P2)

A plugin author writes a manifest for their plugin using the published schema and
documentation, checks it before shipping, and builds against contract v1, which later features
extend with one interface per capability.

**Why this priority**: permissions only work if authors can declare them correctly, and the
example plugin is the starting point for every author.

**Independent Test**: validate the example plugin's manifest against the published schema,
build and run the example with it, and check that an invalid manifest is reported with the
exact problem.

**Acceptance Scenarios**:

1. **Given** the published schema and the author documentation, **When** an author writes a
   manifest for the example plugin, **Then** it validates and the example runs.
2. **Given** a manifest with a mistake (a missing field, a malformed version, an unknown
   permission), **When** it is checked, **Then** the error names the field and the problem.
3. **Given** a plugin built against contract v0 whose manifest declares `log` as required,
   **When** `log` is granted, **Then** it runs as it did before this feature.
4. **Given** a plugin built against contract v0 whose manifest marks a permission optional,
   **When** it is loaded, **Then** it is refused, naming the permission: contract v0 cannot
   answer "not granted".

### Edge Cases

- A manifest requests a permission this host does not know (from a newer host version): the
  plugin is refused, naming the unknown permission; nothing unknown can be granted.
- A manifest requests the same permission twice, or none at all: duplicates count once; a
  plugin with no permissions is valid. A manifest that marks the same permission both required
  and optional is invalid.
- A new version marks as required an optional permission that is on: it runs, since the
  permission is already granted. A new version marks as optional a required permission: it
  stays granted, and the maintainer can then turn it off.
- A plugin learns that an optional permission is off only from the "not granted" answer: it
  cannot read the grant record, ask the host to turn the permission on, or open a host dialog
  (Principle VII). Anything it says about it appears only inside its own widget.
- A manifest declares a login provider before account login exists (feature 005): the field is
  accepted and shown, and gives no capability.
- The grant record is missing, unreadable or corrupted: the host treats it as granting nothing
  and says so; it never treats damage as consent.
- Two packages claim the same plugin id: a plugin's id says where it comes from, so the
  package's origin decides, not what the package claims. An installed plugin's id is its
  repository, and the installation keeps the repository's permanent identifier: an account that
  takes over a renamed or deleted repository's name gets a different plugin, with no grants
  (checked by the installation feature, Phase 3).
- A package loaded from a local file during development claims the id of an installed plugin:
  it never receives that plugin's grants.
- A new package has the same version number as the installed one but different contents: it is
  treated like any other new version.
- A manifest that is not valid JSON, is far larger than any manifest needs, or nests without
  end is refused before any other check.
- A plugin is removed: its grants are removed with it, so a later package with the same id
  starts with none.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Every plugin package MUST carry a manifest; a package without one MUST be
  refused. Writing log messages is a permission like any other: a plugin that logs MUST declare
  it, and its messages reach the host only when the user granted it.
- **FR-002**: The manifest MUST state the plugin id, version, author, the permissions it
  requests, each marked required or optional, and the login providers it would use. The id
  MUST be either the plugin's repository, `<host>/<owner>/<repository>` (one plugin per
  repository), or a local id, `local/<name>`, for a plugin that comes from no repository.
- **FR-003**: The project MUST publish the manifest schema. The host MUST refuse a manifest
  that does not conform to it, naming the field and the problem.
- **FR-004**: The manifest MUST be bound to the exact plugin it describes. A package whose
  manifest does not match its plugin MUST be refused.
- **FR-005**: The host MUST make available to a plugin only the capabilities that its manifest
  declares and the user granted. Every other capability MUST be unavailable to it.
- **FR-006**: A plugin that uses a capability its manifest does not declare MUST be refused
  before it runs, with an error naming the capability and saying it was not declared.
- **FR-007**: A plugin that declares a required capability the user has not granted, and uses
  it, MUST be refused before it runs, with an error naming the capability and saying it was not
  granted.
- **FR-008**: A manifest that requests a permission the host does not know MUST be refused,
  naming that permission.
- **FR-009**: The host MUST record the user's grants and keep them across runs. Only the host's
  own controls, in this phase the command-line tool, MAY read or change them. No plugin MAY
  read, change, request at run time or pre-approve a grant; it learns that an optional
  permission is off only from the "not granted" answer (FR-019).
- **FR-010**: Neither plugin output nor AI output MAY create, widen or pre-approve a grant
  (Principle VII): every grant comes from the user, through the host.
- **FR-011**: A new version of an approved plugin that requires a permission it was not
  granted MUST be refused until the user grants that permission, with an error naming it. The
  grants already given MUST stay unchanged. A permission a new version adds as optional MUST
  start off, and the plugin MUST run without a new approval.
- **FR-012**: A permission a new version no longer requests MUST stop being granted to the
  plugin, so requesting it again needs a new approval.
- **FR-013**: The plugin contract MUST move to version 1, structured so that features 003–005
  add each capability as its own interface tied to one permission, and every capability
  interface MUST be able to answer "not granted". The host MUST keep running contract v0
  plugins (only `log`) whose manifest declares `log` as required and to which it is granted.
  Contract v0 cannot answer "not granted", so a v0 plugin whose manifest marks a permission
  optional MUST be refused, naming the permission.
- **FR-014**: The user MUST be able to list, for a plugin, what it requests, required and
  optional, and what is granted; to turn each optional permission on or off; and to remove the
  plugin's approval as a whole. Required permissions MUST NOT be taken back one by one: they
  change only with a new version, approved again, or by removing the plugin's approval. Every
  change MUST take effect from the plugin's next call, even while the plugin stays loaded: the
  host MUST NOT keep using what it prepared under the old grants. A call already running when
  access is taken away MAY finish, within its time limit.
- **FR-015**: Every refusal MUST name, in one message, the permission or the manifest problem
  and the rule that applied: no manifest, not declared, not granted, new permission needs
  approval, manifest does not match the plugin, invalid manifest, unknown permission, required
  permissions change only together, optional permission needs contract v1.
- **FR-016**: The hostile-plugin suite MUST gain one case per rule in FR-001, FR-003, FR-004,
  FR-006, FR-007, FR-008, FR-011 and FR-019, run on Windows, macOS and Linux on every change
  that is not documentation-only, and fail the build if any case is not refused or, for
  FR-019, if the capability gives the plugin anything (Principle VIII).
- **FR-017**: The example plugin MUST ship a manifest that validates against the published
  schema, and the author documentation MUST explain how to write one. Every plugin in this
  repository, examples and test fixtures alike, uses a local id: none is installed from a
  repository.
- **FR-018**: The user MUST approve a plugin's required permissions together: all of them or
  none, and without that approval the plugin MUST NOT run. Optional permissions MUST start off,
  and the user turns each one on or off separately, only through the host's own controls
  (Principle VII).
- **FR-019**: An optional capability that is off MUST be linked only to a stand-in that gives
  no capability and answers every use with "not granted" (Principle II); the plugin keeps
  running.

### Key Entities

- **Plugin id**: where a plugin comes from: its repository (`<host>/<owner>/<repository>`),
  or a local id (`local/<name>`) for examples, test fixtures, development builds and, in
  Phase 5, modules generated on the user's machine.
- **Plugin package**: what an author ships: the plugin component and its manifest, bound so
  that neither can be swapped.
- **Manifest**: the plugin's declaration of who it is (id, version, author) and what it needs
  (permissions, login providers).
- **Permission**: the right to one capability, marked required or optional, possibly narrowed
  by a scope (for example, one network domain, defined by feature 003).
- **Grant**: the user's approval, kept by the host, of a plugin's required permissions, all
  together, or of one of its optional permissions.
- **Stand-in**: what the host links in place of an optional capability that is off: it gives
  nothing and answers every use with "not granted".
- **Grant record**: every grant the host keeps; host data that no plugin can reach.
- **Login provider declaration**: an account the plugin would ask the user to connect, shown
  now and used by feature 005.
- **Contract v1**: the agreement between host and plugin that capability interfaces extend.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the new hostile permission cases are refused on all three operating
  systems, alongside the cases of feature 001.
- **SC-002**: For every combination of declared, granted and used capability, required or
  optional, a plugin runs only when each required capability it uses is both declared and
  granted, and an optional capability that is off gives it nothing, verified by tests.
- **SC-003**: Every refusal tells the maintainer, in one message, the permission or manifest
  problem and the rule, without reading source code or host logs.
- **SC-004**: An update that requests no new permission runs with no new approval; one that
  requires a new permission is refused every time until it is approved; one that adds an
  optional permission runs with it off.
- **SC-005**: Removing a plugin's approval, or turning an optional permission on or off, takes
  effect from the plugin's next call, every time, even if the plugin stayed loaded.
- **SC-006**: The example plugin's manifest validates against the published schema in CI.
- **SC-007**: Checking the manifest and the grants adds less than 10 ms to loading a plugin on
  every CI system.

## Assumptions

- One person uses the host on a machine; grants belong to that user's profile.
- In this phase the maintainer approves plugins, turns optional permissions on and off, and
  removes approvals with the command-line tool. The host dialogs that replace it for end users
  arrive in Phase 3 and apply the same rules.
- Disabling a plugin (it stops loading, and its grants and data are kept) and uninstalling it
  (everything is removed) arrive with installation in Phase 3. In this phase, removing a
  plugin's approval is how the maintainer stops it.
- The warning for dangerous combinations of permissions arrives with the host dialogs in
  Phase 3. It runs on every grant change, not only at installation: approving a plugin,
  approving an update and turning an optional permission on, each judged on the permissions the
  plugin will hold afterwards, so a plugin cannot ask for little and gain the rest one
  permission at a time.
- The manifest and the plugin travel together as one package; the plan decides how they are
  bound.
- This feature defines how permissions work; each later feature defines its own permissions
  and their scopes (for example, the network domains of feature 003).
- Login providers are declared and shown now; connecting accounts arrives in feature 005.
- Plugins are installed only from their authors' repositories, one plugin per repository, at
  a pinned release; never from a file. Installation arrives in Phase 3: it checks that the
  manifest id is the repository the package came from and keeps the repository's permanent
  identifier, so updates must come from that same repository. In this phase the command-line
  tool loads local files in development mode only.
- Package signatures, which prove who published a release, arrive in Phase 6.
- Out of scope: the network proxy (003), storage (004), the secrets vault and OAuth (005), the
  TypeScript example (006), any graphical interface, disabling and uninstalling (Phase 3),
  signatures and the plugin store (Phase 6).
