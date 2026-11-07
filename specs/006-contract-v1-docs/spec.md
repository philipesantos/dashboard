# Feature Specification: Contract v1 Closed and Docs for Plugin Authors

**Feature Branch**: `006-contract-v1-docs`

**Created**: 2026-10-25

**Status**: Draft

**Input**: User description: "Feature 006 — contract v1 closed at 1.3.0, and the first
documentation for plugin authors (Phase 2 of the guide: "Contrato WIT v1 com essas interfaces,
todas capazes de responder 'não concedida', e primeira documentação para autores", the last open
Phase 2 item now that the TypeScript example moves to Phase 5 with a JS engine run by the host;
constitution v2.4.1, Principles II, VIII, IX). No new capability and no new minor: wit/v1 stays at
1.3.0, which is frozen under wit/history/1.3.0 and pinned by its SHA-256 in
runtime/tests/contract_v1.rs like the earlier minors, so what authors build against no longer
moves; from here on v1 changes only by a new minor that only adds, frozen the same way, or by a
v2. Before freezing, a fresh-eyes review of the contract against the host and the docs checks that
every function, type and error case is reachable, behaves as its doc comment says, and is
documented, and that no world or name promises what the host does not do; a finding is fixed in
the docs or the host, never by editing a published version, and anything only a breaking change
could fix is recorded for v2. The documentation becomes a guide an author can follow without
reading the specs: a getting-started walkthrough from an empty folder to a widget built, packed,
approved and running with the development CLI (in Rust, from hello-widget), walked on a clean
directory on Windows, macOS and Linux; guides for render and update, connectors, accounts and
storage, each starting from its example; and a reference with every interface, function, type and
error case of 1.3.0 (when it happens and what a widget should do with it, not-granted first),
every host error a plugin can cause (capability-refused, contract-mismatch, contract-too-old,
invalid-package and the limit errors), every manifest field, every limit, and the CLI commands an
author uses, with every user-visible text escaped as the CLI shows it. The docs stay true by
tests: a check fails the build when an interface, function, type, error case, manifest field or
limit of the contract is missing from the reference, or the reference names one the contract does
not have; every command and file shown in the getting-started walkthrough and the guides runs in
CI as written. What render returns stays "a JSON document of at most 64 KiB" until Phase 3 defines
the widget content the host frame draws (with DESIGN.md and the host frame spec), in a later
minor; the docs say so. Housekeeping: the examples and fixtures drop the obsolete login_providers
field from their manifests (the schema still accepts it and it still grants nothing), and every
place that promises TypeScript in feature 006 (plugins/examples/README.md, docs) points to Phase 5
instead. Out of scope: the widget content format and user actions (Phase 3), installing from
repositories (Phase 3), TypeScript and the host's JS engine (Phase 5), a docs site (Phase 6).
Deliverables: wit/history/1.3.0 and its pin, the review's findings and what was done with each,
docs/plugin-authors.md restructured (or split under docs/authors/), wit/README.md,
docs/connectors.md and the example READMEs aligned with it, CONTRIBUTING.md saying how to keep the
reference in step with the contract, and the drift and walkthrough checks in CI. No new hostile
case: the feature adds no security rule, and the existing suite must pass unchanged."

## Clarifications

### Session 2026-10-25

- Q: If the review finds a flaw that only a breaking change can fix, is it fixed now or recorded
  for a v2? → A: Fixed now, if the maintainer chooses it. No version has been released and the
  only plugins are this repository's, so a break costs nothing outside it, while a later v2 would
  leave the host with three contracts before its first user. Before the first public release,
  such a finding comes to the maintainer with its cost; if they choose the fix, it ships in this
  feature as contract 2.0.0, v1 is retired in the same feature (the host stops running v1
  plugins, and the examples and fixtures move to 2.0.0), and 2.0.0 is the version frozen. v0
  keeps running. After the first public release, such a finding is recorded for the next major
  instead. (Raised by a side note on the spec.)
- Q: When CI runs the commands of the walkthrough and the guides, how closely must their output
  match what the docs show? → A: Exactly. The docs show each step's whole output and CI compares
  all of it, with the values that change from run to run (digests, paths, times) written as
  placeholders such as `<digest>`; the examples of CLI output in the reference are checked the
  same way.
- Q: How is SC-001 (someone new to the project reaches a running widget from the docs alone in
  under 30 minutes) verified? → A: Three ways together: CI walks the walkthrough on Windows, macOS
  and Linux; an independent agent with no context, in a clean Linux container, follows only the
  docs (it may clone the repository as the docs say, but reads no code or spec) and records every
  point where it got stuck; and the maintainer walks it once by hand on Linux, timed.
- Q: Where do the new docs checks (walkthrough, guides and reference examples, on three systems)
  run in CI? → A: In a job of their own per system, in parallel with the current jobs and
  required by the single "CI result" check, so the run's wall time barely grows and the timed
  hostile cases keep their machine to themselves.

## User Scenarios & Testing *(mandatory)*

The people in these stories are a **plugin author**, who writes a widget against the contract
and has never read this project's specs, and the **maintainer**, who owns the contract and the
host. "The docs" are the documentation for plugin authors in the repository; "the reference" is
the part of it that lists every item of the contract.

### User Story 1 - An author goes from an empty folder to a running widget (Priority: P1)

An author who has never used the project opens the docs, follows the getting-started walkthrough
on their own system and, starting from an empty folder outside this repository, ends with a
widget of their own that is built, packed with its manifest, approved and running in the
development CLI. They need nothing but the docs, the contract files the repository publishes, the
development CLI and tools the walkthrough tells them to install, with the versions it was walked
with.

**Why this priority**: the guide asks for a first documentation for authors; an author who cannot
get a widget running in one sitting goes elsewhere, and every later phase (the app, AI-generated
modules) builds on authors being able to write plugins.

**Independent Test**: on a clean directory of each system, follow the walkthrough exactly as
written, command by command, and check that the last step shows the widget's content and that no
step needed a file or a tool the walkthrough did not mention; then have an agent with no context
follow it in a clean Linux container, and the maintainer walk it once by hand, timed.

**Acceptance Scenarios**:

1. **Given** an empty folder outside the repository and the tools the walkthrough lists,
   **When** the author follows every step as written, **Then** the widget builds, is packed with
   its manifest, is approved, and running it prints the content it returns.
2. **Given** the walkthrough, **When** it is followed on Windows, macOS or Linux, **Then** every
   command shown for that system works as written, and each step says what it should print.
3. **Given** the author makes one of the mistakes the walkthrough warns about (building for a
   target whose standard library imports WASI, forgetting the manifest, running before
   approving), **When** they run the step, **Then** the refusal they see is the one the
   walkthrough shows, and the walkthrough says how to fix it.
4. **Given** the walkthrough, **When** an author reads it, **Then** it never sends them to a
   feature spec to finish a step; links to specs, where they remain, are background only.

---

### User Story 2 - What authors build against no longer moves (Priority: P1)

The maintainer closes contract v1 at 1.3.0: it gets an independent review against the host and
the docs, every finding is resolved, and then 1.3.0 is frozen as published, so an author who
builds against it today builds against exactly what the host will keep running. From then on v1
changes only by a new minor that only adds, frozen the same way, or by a v2. If the review finds
a flaw that only a breaking change fixes and the maintainer chooses to fix it, the closed
contract is 2.0.0 instead, and v1 is retired before anyone outside the repository depends on it.

**Why this priority**: an author's work is only as stable as the contract under it; freezing
before the review would publish mistakes the docs then have to explain forever.

**Independent Test**: check that the frozen 1.3.0 is byte-identical to the contract the host
binds, that changing one byte of any frozen version fails the build, and that every finding of
the review is recorded with an outcome.

**Acceptance Scenarios**:

1. **Given** contract 1.3.0, **When** it is frozen, **Then** the frozen copy is identical to the
   contract the host and the examples build against, and the build pins it by its digest like
   1.0.0, 1.1.0 and 1.2.0.
2. **Given** a frozen version, **When** anyone edits a byte of it, **Then** the build fails,
   naming the version.
3. **Given** the review of the contract, **When** it finds that a function, type or error case
   cannot be reached, behaves otherwise than its documentation says, or is undocumented, or that
   a world or name promises what the host does not do, **Then** the finding is recorded with its
   outcome: fixed in the docs, fixed in the host with a test, fixed by a breaking change, or kept
   with a stated reason; the contract is frozen only when no finding is left without an outcome.
4. **Given** a finding that only a breaking change could fix, **When** the review reports it,
   **Then** it goes to the maintainer with its cost (what changes for the examples, the fixtures
   and the host); if they choose the fix, the closed contract is 2.0.0, v1 is retired in this
   feature and no published version is edited; if not, it is kept with the reason.
5. **Given** the contract is frozen, **When** a later feature needs to add to v1, **Then** the
   docs for contributors say how: freeze the current version first, add a new minor that only
   adds, and extend the test that checks it only adds.

---

### User Story 3 - An author looks up any part of the contract (Priority: P2)

While writing a widget, an author wants to know what a function does, what each error case
means, when it happens and what their widget should do about it; what a manifest field accepts;
what a limit is and what happens past it; what an error from the host means; and which command
of the development CLI does what. They find each in the reference, in one place, without reading
the WIT file, the host's code or the specs.

**Why this priority**: the walkthrough gets an author started; the reference is what they keep
open while they work, and it is where not-granted and the other answers either get handled or get
ignored.

**Independent Test**: pick items at random from the contract, the manifest schema, the connector
schema, the limits, the host's errors and the CLI, and find each in the reference with its
meaning, when it happens and what to do.

**Acceptance Scenarios**:

1. **Given** any interface, function, type or error case of 1.3.0, **When** the author looks it
   up, **Then** the reference says what it is, the minor it arrived in, when each answer happens
   and what a widget should do with it, with not-granted first in every error type.
2. **Given** any error the host reports for a plugin (such as capability-refused,
   contract-mismatch, contract-too-old, invalid-package, needs-new-approval and each limit's
   error), **When** the author looks it up, **Then** the reference shows it as the CLI prints it,
   what causes it and how to fix it.
3. **Given** any field of the manifest or of the connector format, **When** the author looks it
   up, **Then** the reference says what it accepts, whether it is required, and what it changes
   at approval.
4. **Given** any limit, **When** the author looks it up, **Then** the reference gives its
   default and the error past it.
5. **Given** any command of the development CLI an author uses, **When** the author looks it up,
   **Then** the reference gives its options and an example of what it prints, with every text
   the plugin controls shown escaped exactly as the CLI shows it.
6. **Given** what render returns, **When** the author looks it up, **Then** the reference says
   it is a JSON document of at most 64 KiB, that the content format the host frame draws arrives
   in Phase 3 in a later minor, and that widgets will need to adopt it then.

---

### User Story 4 - The docs cannot drift from the contract or the host (Priority: P2)

A contributor changes the contract, the manifest or connector format, a limit, an error the host
reports, or a CLI command, and forgets the docs. The build fails and names what the reference is
missing or names wrongly. A contributor who edits the walkthrough or a guide finds out in the
same pull request if a command no longer works as written.

**Why this priority**: docs that are right today and wrong in a month mislead authors in exactly
the places where safety depends on them, such as not-granted and the approval digest.

**Independent Test**: remove one item from the reference, add one that does not exist, change
one command in the walkthrough to something that fails; each time the build fails and names it.

**Acceptance Scenarios**:

1. **Given** an item of the contract, a manifest or connector field, a limit or a host error
   missing from the reference, **When** the build runs, **Then** it fails, naming the item.
2. **Given** the reference names an item that does not exist, or gives a limit's default other
   than the host's, **When** the build runs, **Then** it fails, naming it.
3. **Given** a command or file shown in the walkthrough or a guide, **When** the build runs on
   Windows, macOS and Linux, **Then** it is run as written (against test services instead of an
   external one, through a test build of the same CLI, with the command's text unchanged) and
   the build fails if it does not work or if any line of its
   output differs from the docs, apart from placeholders for values that change between runs.
4. **Given** a pull request that changes only the docs these checks cover, **When** CI decides
   what to run, **Then** it runs everything, as for code, because those docs are used as code.

---

### User Story 5 - Each capability has a guide that starts from its example (Priority: P3)

An author who wants their widget to fetch data, act as the user's account or keep data of its
own reads the guide for that capability. Each guide starts from the example that uses it,
explains the flow (what update and render do, what the host keeps, what the maintainer approves),
and ends with the widget running.

**Why this priority**: the reference says what each item is; a guide says how they fit together.
It matters less than the walkthrough and the reference, and builds on both.

**Independent Test**: follow each guide from its example to a running widget; check that each
command runs as written.

**Acceptance Scenarios**:

1. **Given** the guide for render and update, **When** the author follows it, **Then** they know
   what each call receives and returns, that each call runs in a fresh instance, and that render
   never waits on the network or writes.
2. **Given** the guides for connectors, accounts and storage, **When** the author follows one,
   **Then** it starts from github-releases, private-notes or update-counter, shows how to handle
   not-granted and the other answers, and ends with the widget running in the development CLI.

---

### User Story 6 - Leftovers from earlier features are gone (Priority: P3)

An author copying an example finds no obsolete field to copy, and no page promises a TypeScript
example that is not coming in this phase.

**Why this priority**: small, but every example is a template someone will copy.

**Independent Test**: search the examples, fixtures and docs for login_providers and for promises
of TypeScript in feature 006.

**Acceptance Scenarios**:

1. **Given** the examples and fixtures, **When** their manifests are read, **Then** none carries
   login_providers, apart from a fixture that tests the obsolete field itself.
2. **Given** a manifest that still carries login_providers, **When** the host reads it, **Then**
   it is still valid and the field still grants nothing.
3. **Given** any page that mentioned TypeScript in feature 006, **When** it is read, **Then** it
   says TypeScript plugins arrive in Phase 5, and promises no toolchain for them.

---

### Edge Cases

- A review finding shows the host behaving otherwise than the contract's doc comment says: the
  host is fixed to match, with a test, if the comment describes the safe behavior; if the host's
  behavior is the safe one, the comment of 1.3.0 is corrected before the freeze (a comment changes
  no definition), and a frozen older version's comment stays as it is, with the docs saying what
  holds.
- A finding would make the host stricter for plugins that run today, such as refusing a value it
  now accepts: it is a security fix if it closes a hole, with its own test and review; otherwise
  it is a breaking-change finding, and goes to the maintainer (FR-006).
- The maintainer chooses a breaking fix: every example and fixture built for v1 moves to 2.0.0 in
  the same feature, the hostile suite keeps every case (rebuilt for 2.0.0 where it was built for
  v1), and a v1 plugin is refused as a contract the host does not run.
- The walkthrough's commands differ per system (paths, shell syntax, how an environment variable
  is set): the walkthrough shows each system's form where they differ, and CI runs the form shown
  for that system.
- A guide reaches GitHub or another service: CI runs it against the test services, through a
  test build of the same CLI whose network leads to them, never the real service; the command's
  text does not change.
- A tool the walkthrough uses releases a version that breaks a step: the walkthrough names the
  version it was walked with, and the CI check, pinned to it, keeps passing until the pin moves
  in a reviewed change.
- An item exists only in an older minor or in v0: the reference covers v1 at 1.3.0, marks the
  minor each item arrived in, and says in one place what v0 still runs and that new plugins use
  v1.
- Text a plugin controls appears in an example of CLI output: the reference shows it escaped as
  the CLI prints it, never raw.
- The drift check meets an item it cannot classify, such as a new kind of WIT definition: it
  fails and names it, rather than pass silently.
- A pull request changes only Markdown the drift or walkthrough checks read: CI runs in full, as
  for code, since the constitution does not count Markdown the build uses as documentation; a
  change to any other Markdown still skips the build.

## Requirements *(mandatory)*

### Functional Requirements

**The contract**

- **FR-001**: Contract v1 MUST stay at 1.3.0: this feature MUST NOT add, remove or change any
  interface, function, type, case or world, and the host MUST run every plugin that runs today,
  except where a review finding closes a security hole (FR-005) or the maintainer chooses a
  breaking fix (FR-006). The doc comments of 1.3.0 MAY be corrected before it is frozen.
- **FR-002**: The closed contract (1.3.0, or 2.0.0 under FR-006) MUST be frozen under the
  repository's history of published versions, identical to the contract the host binds, and
  pinned by its digest in the test that pins 1.0.0, 1.1.0 and 1.2.0; editing any byte of a frozen
  version MUST fail the build, naming the version, and while the current contract declares the
  closed version it MUST equal the frozen copy.
- **FR-003**: The contributor docs MUST state how v1 changes from now on: a new minor that only
  adds, freezing the current version first and extending the test that checks a minor only adds;
  anything else is a v2.
- **FR-004**: Before the contract is frozen, the contract MUST get a review by someone who did not write
  it, which checks, for every interface, function, type and error case, that a plugin can reach
  it, that the host behaves as its documentation says, and that it is documented; and, for every
  world and name, that it promises nothing the host does not do.
- **FR-005**: Every review finding MUST be recorded in the feature's directory with its outcome:
  fixed in the docs; fixed in the host, with a test that fails without the fix; fixed by a
  breaking change (FR-006); or kept, with the reason. A fix in the host that refuses something
  accepted today MUST be a security fix with its own test and line-by-line review (Principle IX);
  any other such change is a breaking-change finding (FR-006). The contract MUST NOT be frozen
  while a finding has no outcome.
- **FR-006**: A finding that only a breaking change could fix MUST go to the maintainer with its
  cost (what changes for the contract, the host, the examples, the fixtures and the docs). Before
  the first public release, if the maintainer chooses the fix, it MUST ship in this feature as
  contract 2.0.0, which becomes the closed contract: v1 is retired in the same feature (the host
  refuses v1 plugins, naming the versions it runs; the examples and fixtures move to 2.0.0; the
  hostile suite keeps every case), v0 keeps running, and no published version is edited. A
  finding the maintainer does not choose to fix is kept, with the reason. After the first public
  release, such findings MUST instead be recorded for the next major.

**The docs**

- **FR-007**: The docs MUST include a getting-started walkthrough that takes an author from an
  empty folder outside the repository to a widget of their own, in Rust, built, packed with its
  manifest, approved and running in the development CLI, using only the contract files the
  repository publishes, the development CLI, and tools the walkthrough names with the versions it
  was walked with; it MUST NOT need the repository's internal build tooling for the author's own
  plugin.
- **FR-008**: The walkthrough MUST show, for each step, what it should print, and MUST show the
  refusal and the fix for the mistakes authors are likely to make: building for a target that
  imports WASI, a missing or invalid manifest, running before approving, building against a newer
  contract than the host runs.
- **FR-009**: The docs MUST include a guide for each of render and update, connectors, accounts
  and storage, each starting from its example (hello-widget, github-releases, private-notes,
  update-counter) and ending with the widget running, including how a widget handles not-granted
  and each other answer of the capability.
- **FR-010**: The reference MUST list every interface, function, type, case and world of 1.3.0,
  each with what it is, the minor it arrived in, when each answer happens and what a widget should
  do with it, with not-granted first in every error type.
- **FR-011**: The reference MUST list every error the host reports for a plugin, by the name the
  CLI prints (including capability-refused, contract-mismatch, contract-too-old, invalid-package,
  no-manifest, manifest-mismatch, needs-new-approval and each limit's error), with its cause and
  its fix.
- **FR-012**: The reference MUST list every field of the manifest and of the connector format,
  with what it accepts, whether it is required, and what it changes at approval; and every limit,
  with its default and the error past it.
- **FR-013**: The reference MUST list the commands and options of the development CLI that an
  author uses, with an example of what each prints; every text a plugin, a manifest or a
  connector controls MUST appear escaped exactly as the CLI prints it, and each example MUST be
  checked against what the CLI prints, as in FR-018.
- **FR-014**: The docs MUST say that render returns a JSON document of at most 64 KiB, that the
  content format the host frame draws is defined in Phase 3 in a later minor, and that widgets
  will adopt it then.
- **FR-015**: Every fact an author needs to finish a task MUST be in the docs; a link to a feature
  spec MAY remain only as background.
- **FR-016**: The docs MUST say what contract v0 still runs and that new plugins use v1, without
  documenting v0 item by item.

**Keeping the docs true**

- **FR-017**: A check MUST fail the build when an interface, function, type, case or world of the
  current contract, a field of the manifest or connector format, a limit, an error the host
  reports for a plugin, or a command or option of the development CLI an author uses is missing
  from the reference, or when the reference names one that does
  not exist or gives a limit's default other than the host's; it MUST name each item, and MUST
  fail on an item it cannot classify rather than skip it.
- **FR-018**: Every command and file shown in the walkthrough and the guides MUST run in CI on
  Windows, macOS and Linux as written, in the form shown for that system. The docs MUST show each
  command's whole output, and the build MUST fail if a command fails or its output differs from
  the docs in any line; only values that change from run to run (digests, paths, times) MAY be
  written as named placeholders, such as `<digest>`, and each placeholder MUST match only its kind
  of value. A command that reaches an external service MUST run against the test services,
  through a test build of the same CLI whose network leads to them; the command's text MUST NOT
  change.
- **FR-019**: The docs that the checks of FR-017 and FR-018 read are used as code, so they are
  not documentation for the constitution's rule (Development Workflow): a change to them, even to
  Markdown alone, MUST run CI as any code change does, the security suite included. The checks of FR-018 MUST run in a job of their own on each of the three systems, in
  parallel with the existing jobs, never on the machine that runs the hostile suite, and the
  required "CI result" check MUST fail when one of them fails.
- **FR-020**: The contributor docs MUST say how to keep the reference in step: a change to the
  contract, a schema, a limit, a host error or an author-facing CLI command updates the reference
  in the same pull request, and the check names what is missing.

**Housekeeping and scope**

- **FR-021**: The manifests of the examples and fixtures MUST NOT carry login_providers, apart from
  fixtures that test the obsolete field; a manifest that carries it MUST stay valid and the field
  MUST grant nothing.
- **FR-022**: Every page that promises TypeScript in feature 006 MUST instead say that TypeScript
  plugins arrive in Phase 5, without naming a toolchain or an engine: the constitution's fixed
  decision for JS/TS plugins still names jco and componentize-js, and Phase 5 amends it first.
- **FR-023**: The repository's other docs that describe the contract or the author's workflow
  (the contract directory's overview, the connector format guide, the examples' overview and each
  example's own page) MUST agree with the docs and point to them instead of repeating them.
- **FR-024**: The feature MUST NOT add or remove a security rule or a hostile case: the existing
  security suite MUST pass unchanged on Windows, macOS and Linux; a host fix from the review adds
  a test for that fix (FR-005), not a new rule.
- **FR-025**: The widget content format and user actions, installing from repositories,
  TypeScript plugins, and a docs site MUST NOT be part of this feature.

### Key Entities

- **Published contract version**: a version of the contract, frozen once published and pinned
  by its digest; 1.0.0 to 1.3.0 after this feature, or 2.0.0 too if a breaking fix was chosen.
- **Review finding**: something the review found in the contract, with the item it concerns, what
  was wrong, and its outcome (docs fix, host fix with test, breaking fix, kept with reason).
- **Breaking-change finding**: a finding only a new major could fix, with its cost and the
  maintainer's choice.
- **Reference entry**: one item an author can look up (contract item, manifest or connector
  field, limit, host error, CLI command), with its meaning, when it happens and what to do.
- **Walkthrough**: the getting-started steps, each with its commands per system and what they
  print, and the tool versions it was walked with.
- **Guide**: the steps for one capability, from its example to a running widget.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The walkthrough reaches a running widget from the docs alone, checked three ways:
  CI walks it on Windows, macOS and Linux; an independent agent with no context, in a clean Linux
  container, follows only the docs (cloning the repository where they say to, reading no code or
  spec) and every point where it got stuck is fixed in the docs, until a walk has none; and the
  maintainer walks it once by hand on Linux in under 30 minutes, not counting the download of the
  tools.
- **SC-002**: 100% of the items of contract 1.3.0, the manifest and connector fields, the limits
  and the host's plugin errors appear in the reference, and the drift check fails, naming the
  item, both when one is removed from the reference and when one that does not exist is added.
- **SC-003**: 100% of the commands and files shown in the walkthrough and the guides run in CI on
  the three systems, and changing one to a failing command fails the build.
- **SC-004**: Every review finding has an outcome before the freeze: 0 findings open.
- **SC-005**: Changing one byte of any frozen version fails the build, naming the
  version.
- **SC-006**: The security suite passes unchanged on the three systems, with no hostile case
  removed.
- **SC-007**: An author completes the walkthrough and each guide without opening a feature spec:
  0 required links into the specs.

## Assumptions

- Until releases exist (Phase 6), the author builds the development CLI from a clone of the
  repository; the walkthrough says so, and the author's own plugin still lives in a folder of its
  own outside the clone.
- The walkthrough and the guides are in Rust, the only plugin language until Phase 5; Go and
  Python remain in the fixed decisions but get no docs in this feature.
- The review is done by a reviewer who did not write the contract (an independent agent review,
  as for the plans of 003 to 005) and every finding is checked at the source before it is acted
  on; the maintainer decides only the findings with a real trade-off.
- The docs are in English, like the rest of the repository's docs.
- The docs may stay one page or be split into several under a directory for authors; the plan
  chooses, and the links from the rest of the repository follow.
- The commands of the walkthrough and guides are checked against the CI machines of the three
  systems; "a clean directory" means a fresh folder and a fresh development data directory, not
  a freshly installed system.
- No new limit, permission or capability is needed: everything the docs describe exists on
  master after feature 005.
