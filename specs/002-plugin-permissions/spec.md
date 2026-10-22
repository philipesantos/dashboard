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

A plugin package states, in its manifest, every capability it needs. The maintainer sees what
the plugin requests, grants some or all of it, and the plugin then runs with exactly the
capabilities that are both declared and granted, and nothing else.

**Why this priority**: this is Principle II made concrete. Every capability of features
003–005 (network, storage, secrets, accounts) depends on it, and without it the only safe
plugin is one with no capability at all.

**Independent Test**: give a plugin a manifest, grant its permissions through the tool, run
it and check it works; then remove a grant and check it is refused, naming the permission.

**Acceptance Scenarios**:

1. **Given** a plugin whose manifest declares the capabilities it uses, **When** the maintainer
   asks what it requests, **Then** the tool lists every requested permission and every login
   provider, and which of them are granted.
2. **Given** every requested permission is granted, **When** the plugin runs, **Then** it works
   with those capabilities.
3. **Given** a plugin that uses a capability its manifest does not declare, **When** it is
   loaded, **Then** it is refused before running, and the error names that capability and says
   it was not declared.
4. **Given** a plugin that declares a capability the maintainer has not granted, **When** it is
   loaded, **Then** it is refused before running, and the error names that capability and says
   it was not granted.
5. **Given** a package without a manifest, or whose manifest does not describe that exact
   plugin, **When** it is loaded, **Then** it is refused, and the error says why.

---

### User Story 2 - Hostile permission cases are contained (Priority: P1)

The maintainer runs hostile packages that each try to get a capability without the right to
it: one that uses a capability it never declared, one that declares a capability nobody
granted, one whose manifest belongs to another plugin, one whose manifest breaks the schema,
and one whose new version quietly asks for more. Each is refused, the error names the rule,
and the host keeps working. The cases run automatically on Windows, macOS and Linux and break
the build if any one is not refused.

**Why this priority**: every security rule needs a test that breaks the build (Principle VIII).
The phase gate (exfiltration blocked) builds on these rules.

**Independent Test**: run the hostile suite; every new case is refused with its rule, and a
well-behaved plugin with a valid manifest and grants runs normally right afterwards.

**Acceptance Scenarios**:

1. **Given** each hostile package, **When** it is loaded, **Then** it is refused with an error
   naming its rule.
2. **Given** any hostile package was just refused, **When** a well-behaved plugin runs next,
   **Then** it produces its normal result.
3. **Given** the cases of feature 001, **When** the suite runs, **Then** they still pass: the
   suite only grows.

---

### User Story 3 - Updates and revocation never widen access silently (Priority: P2)

The maintainer installs a new version of a plugin that is already granted. If it asks for
nothing new, it runs with the grants it had. If it asks for a new permission, it does not get
it until the maintainer approves it. The maintainer can also take back any grant at any time.

**Why this priority**: an update that gains a permission silently is the classic way a trusted
plugin turns hostile (threat model: "Update adds a permission without approval").

**Independent Test**: grant a plugin, load a new version with the same permissions (it runs),
load one with an extra permission (refused, naming it), approve it (runs), revoke it (refused
again at the next load).

**Acceptance Scenarios**:

1. **Given** a granted plugin, **When** a new version requests the same permissions, **Then**
   it runs without any new approval.
2. **Given** a granted plugin, **When** a new version requests a permission it was not
   granted, **Then** it is refused before running, the error names the new permission, and the
   grants already given stay as they were.
3. **Given** that refused version, **When** the maintainer grants the new permission, **Then**
   it runs.
4. **Given** a new version that no longer requests a permission, **When** a later version asks
   for it again, **Then** it needs approval again.
5. **Given** a granted permission, **When** the maintainer revokes it, **Then** the plugin is
   refused at its next load if it still uses that capability.

---

### User Story 4 - Authors declare what their plugin needs (Priority: P2)

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
3. **Given** a plugin built against contract v0 whose manifest declares `log`, **When** `log`
   is granted, **Then** it runs as it did before this feature.

### Edge Cases

- A manifest requests a permission this host does not know (from a newer host version): the
  plugin is refused, naming the unknown permission; nothing unknown can be granted.
- A manifest requests the same permission twice, or none at all: duplicates count once; a
  plugin with no permissions is valid.
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
  it, and the user MUST grant it.
- **FR-002**: The manifest MUST state the plugin id, version, author, the permissions it
  requests and the login providers it would use. The id MUST be either the plugin's repository,
  `<host>/<owner>/<repository>` (one plugin per repository), or a local id, `local/<name>`, for
  a plugin that comes from no repository.
- **FR-003**: The project MUST publish the manifest schema. The host MUST refuse a manifest
  that does not conform to it, naming the field and the problem.
- **FR-004**: The manifest MUST be bound to the exact plugin it describes. A package whose
  manifest does not match its plugin MUST be refused.
- **FR-005**: The host MUST make available to a plugin only the capabilities that its manifest
  declares and the user granted. Every other capability MUST be unavailable to it.
- **FR-006**: A plugin that uses a capability its manifest does not declare MUST be refused
  before it runs, with an error naming the capability and saying it was not declared.
- **FR-007**: A plugin that declares a capability the user has not granted, and uses it, MUST
  be refused before it runs, with an error naming the capability and saying it was not granted.
- **FR-008**: A manifest that requests a permission the host does not know MUST be refused,
  naming that permission.
- **FR-009**: The host MUST record the user's grants and keep them across runs. Only the host's
  own controls, in this phase the command-line tool, MAY read or change them. No plugin MAY
  read, change, request at run time or pre-approve a grant.
- **FR-010**: Neither plugin output nor AI output MAY create, widen or pre-approve a grant
  (Principle VII): every grant comes from the user, through the host.
- **FR-011**: A new version of a granted plugin that requests a permission it was not granted
  MUST be refused until the user grants that permission, with an error naming it. The grants
  already given MUST stay unchanged.
- **FR-012**: A permission a new version no longer requests MUST stop being granted to the
  plugin, so requesting it again needs a new approval.
- **FR-013**: The plugin contract MUST move to version 1, structured so that features 003–005
  add each capability as its own interface tied to one permission. The host MUST keep running
  contract v0 plugins (only `log`) whose manifest declares `log` and to which it is granted.
- **FR-014**: The user MUST be able to list, for a plugin, what it requests and what is
  granted, and to revoke any grant. A revocation MUST take effect from the plugin's next load.
- **FR-015**: Every refusal MUST name, in one message, the permission or the manifest problem
  and the rule that applied: no manifest, not declared, not granted, new permission needs
  approval, manifest does not match the plugin, invalid manifest, unknown permission.
- **FR-016**: The hostile-plugin suite MUST gain one case per rule in FR-001, FR-003, FR-004,
  FR-006, FR-007, FR-008 and FR-011, run on Windows, macOS and Linux on every change that is not
  documentation-only, and fail the build if any case is not refused (Principle VIII).
- **FR-017**: The example plugin MUST ship a manifest that validates against the published
  schema, and the author documentation MUST explain how to write one. Every plugin in this
  repository, examples and test fixtures alike, uses a local id: none is installed from a
  repository.

### Key Entities

- **Plugin id**: where a plugin comes from: its repository (`<host>/<owner>/<repository>`),
  or a local id (`local/<name>`) for examples, test fixtures, development builds and, in
  Phase 5, modules generated on the user's machine.
- **Plugin package**: what an author ships: the plugin component and its manifest, bound so
  that neither can be swapped.
- **Manifest**: the plugin's declaration of who it is (id, version, author) and what it needs
  (permissions, login providers).
- **Permission**: the right to one capability, possibly narrowed by a scope (for example, one
  network domain, defined by feature 003).
- **Grant**: the user's approval of one permission for one plugin, kept by the host.
- **Grant record**: every grant the host keeps; host data that no plugin can reach.
- **Login provider declaration**: an account the plugin would ask the user to connect, shown
  now and used by feature 005.
- **Contract v1**: the agreement between host and plugin that capability interfaces extend.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the new hostile permission cases are refused on all three operating
  systems, alongside the cases of feature 001.
- **SC-002**: For every combination of declared, granted and used capability, a plugin runs
  only when each capability it uses is both declared and granted, verified by tests.
- **SC-003**: Every refusal tells the maintainer, in one message, the permission or manifest
  problem and the rule, without reading source code or host logs.
- **SC-004**: An update that requests no new permission runs with no new approval; one that
  requests a new permission is refused every time until it is approved.
- **SC-005**: A revoked grant has no effect from the plugin's next load, every time.
- **SC-006**: The example plugin's manifest validates against the published schema in CI.
- **SC-007**: Checking the manifest and the grants adds less than 10 ms to loading a plugin on
  every CI system.

## Assumptions

- One person uses the host on a machine; grants belong to that user's profile.
- In this phase the maintainer grants and revokes with the command-line tool. The host dialogs
  that replace it for end users arrive in Phase 3 and apply the same rules.
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
  TypeScript example (006), any graphical interface (Phase 3), signatures and the plugin store
  (Phase 6).
