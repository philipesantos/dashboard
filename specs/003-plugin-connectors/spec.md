# Feature Specification: Plugin Connectors

**Feature Branch**: `003-plugin-connectors`

**Created**: 2026-10-23

**Status**: Draft

**Input**: User description: "Feature 003 — plugin connectors (replaces generic plugin network;
constitution v2.0.0, Principles III, IV, VII, IX). Plugins have no generic network access. A
widget reaches an external service only by calling the operations of a connector its manifest
declares. A connector is a declaration (data, never code) that the host runs: each operation has
a fixed destination, typed parameters the host validates, and operations that change data are
declared apart from those that only read. A widget depends on a specific connector (its id and
major version), never on a 'type' of connector. A connector's destination is the project's
official connector catalog, whose domains are reserved to it (a protocol connector such as RSS
takes its destination from the user), or a domain whose owner authorizes the connector's
repository in a file under .well-known, verified at installation, at every update and
periodically. A connector may live in the widget's own repository (an author's own service,
including paid ones) or in a repository of its own. All connector traffic goes through the host
proxy: HTTPS only, exact hosts, never a loopback or private address (DNS answers and redirects
included), size and rate limits, request log; before approval the host shows every destination.
Network calls are separated from rendering: a widget calls connectors only in a new update entry
point with its own wall-clock budget; the host keeps the bounded JSON data it returns and hands
it to render, which keeps its 1 s limit; the host updates on load, on the widget's interval and
on request, and without network the widget shows the last data and when it was fetched. Contract
v1 moves to 1.1.0 and the host keeps accepting older v1 minors while refusing newer ones. The
development CLI may run local connectors with unverified or loopback destinations, in the
development record only. Phase 2 gate case: a request outside the connector's declared
operations or to an unverified destination is blocked. Out of scope: credentials and OAuth
(005), installation dialogs and the cross-account warning (Phase 3), user actions (Phase 3),
storage (004), signatures and the store (Phase 6). Docs that still describe the domain allowlist
must be updated. First catalog connectors need no authentication."

## User Scenarios & Testing *(mandatory)*

The people using this feature are the maintainer, who runs widgets and approves them and the
connectors they use; widget authors; connector authors, often the same people, publishing a
connector for their own service; owners of a web service, who decide which repositories may
publish a connector for it; and the project, which keeps the official connector catalog. End
users make the same decisions later, in the host dialogs of Phase 3; in this phase the
maintainer makes them through the command-line tool.

A connector describes, as data, the operations a widget may call on one external service: for
each, where the request goes and which values the widget may fill in. The host makes every
request itself; a widget never touches the network.

### User Story 1 - A widget gets external data only through declared operations (Priority: P1)

An author writes a widget that shows the latest releases of a public GitHub repository. Its
manifest declares the catalog's GitHub connector and the one operation it calls. The maintainer
asks what the widget requests and sees the connector, its destination and that operation, marked
as a read, and approves it. When the widget updates, it calls the operation with a repository
name; the host checks the value, makes the request and hands back the response; the widget
returns its data, and its render shows the releases. Anything outside the declaration is refused
before a byte leaves the machine.

**Why this priority**: this is Principle III made concrete. It replaces generic network access,
and accounts (feature 005) build on it.

**Independent Test**: approve the example widget, run an update and a render, and check that the
releases appear and the request log shows the one call; then make the widget call an operation
it did not declare, and pass a value outside its parameter's type, and check that both are
refused with nothing sent.

**Acceptance Scenarios**:

1. **Given** a widget that declares a connector, **When** the maintainer asks what it requests,
   **Then** the tool lists each connector with its version, where its destinations come from
   (official catalog, verified for its repository, chosen by the user, or local development),
   every destination host, and each operation the widget calls, marked as a read or a change.
2. **Given** the widget is approved, **When** an update runs and the widget calls a declared
   operation with valid values, **Then** the host sends the request to the operation's
   destination, gives the widget the response, and records the call in the request log.
3. **Given** the widget calls an operation it did not declare, or one the connector does not
   have, **When** the call is made, **Then** it is refused before anything is sent, naming the
   operation and the rule.
4. **Given** a value outside its parameter's type (too long, wrong format, out of range, not
   among the allowed values, or a parameter that is unknown or missing), **When** the call is
   made, **Then** it is refused before anything is sent, naming the parameter and the rule.
5. **Given** a widget whose component asks for any network access other than connectors, such as
   sockets or generic HTTP, **When** it is loaded, **Then** it is refused before running.
6. **Given** an approved widget, **When** it calls a connector during render, **Then** the call
   is refused: connectors are called only during an update.

---

### User Story 2 - Hostile network cases are contained (Priority: P1)

The maintainer runs hostile widgets and connectors that each try to send data where the user did
not approve: one calls an operation it did not declare; one passes a value outside its type; one
uses a connector that claims a catalog domain; one uses a connector for a domain that never
authorized its repository; one whose destination leads to the user's machine or local network,
directly, through its name or through a redirect; one that uses plain HTTP; one that calls a
connector during render; one that calls without end. Each is refused or contained, nothing
reaches a destination it was not approved for, and the host keeps working. The cases run
automatically on Windows, macOS and Linux, without depending on the internet, and break the
build if any one is not contained.

**Why this priority**: every security rule needs a test that breaks the build (Principle VIII).
This feature delivers the Phase 2 gate case "a request outside the connector's operations or to
an unverified destination is blocked".

**Independent Test**: run the hostile suite; every new case is refused with its rule, the test
services receive no request from a refused case, and a well-behaved widget updates and renders
normally right afterwards.

**Acceptance Scenarios**:

1. **Given** each hostile case, **When** it runs, **Then** it is refused with an error naming
   its rule, and no request reaches a destination it was not approved for.
2. **Given** any hostile case was just refused, **When** a well-behaved widget updates and
   renders next, **Then** it produces its normal result.
3. **Given** the cases of features 001 and 002, **When** the suite runs, **Then** they still
   pass: the suite only grows.
4. **Given** a hostile widget that calls connectors past a limit (calls per update, rate,
   response size, update time), **When** it runs, **Then** the calls past the limit are
   refused or stopped, its previous data is kept, and other widgets are not affected.

---

### User Story 3 - The network stays out of rendering (Priority: P2)

A widget's data is refreshed in an update, separately from rendering. Render shows the last data
the host kept and when it was fetched, without waiting on the network. When the network is down
or the service fails, render still shows the previous data, and that the last update failed. The
host decides when to update: when the widget is loaded, at the widget's interval (never below the
host's minimum), and when the user asks.

**Why this priority**: render keeps its 1 s limit and the dashboard stays responsive; a
service's rate limits are not spent on every render; widgets keep showing data offline.

**Independent Test**: update the example widget, take its test service down, and render: the
previous data appears with its fetch time and the failure; bring the service back, update, and
the new data appears.

**Acceptance Scenarios**:

1. **Given** a widget with kept data, **When** it renders, **Then** render receives the data,
   the time it was fetched and the outcome of the last update, and finishes within render's
   limits without any network request.
2. **Given** an update fails (service unreachable, error, update over its time limit, invalid
   output, a refused call), **When** the widget renders next, **Then** it shows the previous data
   and the failure.
3. **Given** a service that takes longer than render's 1 s limit to answer, but less than the
   update's own limit, **When** the widget updates, **Then** the update completes and its data
   is kept.
4. **Given** an update that exceeds its own time limit, **When** the limit is reached, **Then**
   the update is stopped, its calls in flight are abandoned, and the previous data is kept.
5. **Given** a widget whose manifest declares an update interval below the host's minimum,
   **When** it is loaded, **Then** it is refused, naming the minimum.
6. **Given** a widget updated successfully once, **When** it renders in a later run of the host,
   **Then** it shows the data of that update.

---

### User Story 4 - An author's own service through a verified domain (Priority: P2)

An author runs their own service, perhaps a paid one, and publishes a connector for it in their
repository. On each host the connector reaches, the service's owner, usually the author,
publishes an authorization that lists that repository. The host checks it before approval, at
every update of the connector and periodically; when the domain stops authorizing the
repository, the connector stops working. Nobody else can publish a connector for that domain,
and nobody can use the catalog's domains.

**Why this priority**: authors can build widgets on their own services, including paid ones,
without opening the door to impostor or duplicate connectors.

**Independent Test**: a test service publishes an authorization listing a connector's
repository: approval and update work; remove the repository from the authorization and the next
verification stops the connector; a connector that claims a catalog domain is refused.

**Acceptance Scenarios**:

1. **Given** a connector from repository R whose every destination host authorizes R, **When**
   the maintainer approves a widget that uses it, **Then** the approval succeeds and the tool
   shows each host as verified for R.
2. **Given** a destination host whose authorization does not list the connector's repository,
   or that publishes none or an invalid one, **When** a widget using that connector is loaded,
   **Then** it is refused, naming the host and the rule.
3. **Given** a connector outside the catalog that declares a host the catalog reserves, **When**
   it is loaded, **Then** it is refused, naming the host, even if that host authorizes the
   repository.
4. **Given** a verified connector, **When** a later verification finds that a host no longer
   lists its repository, **Then** from the next call every call to that connector is refused,
   naming the host.
5. **Given** a verified connector, **When** a later verification cannot complete (the host is
   unreachable), **Then** the last successful verification stays valid up to a maximum age;
   after it, calls are refused until a verification succeeds.

---

### User Story 5 - Changes never widen network access silently (Priority: P2)

The maintainer installs new versions of widgets and connectors. A new version that adds a
destination, or changes an operation the widget calls, does not run until approved; a connector
update that only adds operations the widget does not call runs as before. Optional connectors
start off and, while off, every call answers "not granted". Removing a widget's approval, or
turning off one of its connectors, takes effect from the next call and discards the data the
host kept for the widget.

**Why this priority**: an update that quietly sends data somewhere new is how a trusted widget
or connector turns hostile (feature 002's rule for permissions, applied to destinations).

**Independent Test**: approve a widget; update its connector with an operation the widget does
not call (it runs); update it so that an operation the widget calls goes elsewhere (refused
until approved); turn an optional connector off (calls answer "not granted"); remove the
approval (refused, kept data gone).

**Acceptance Scenarios**:

1. **Given** an approved widget, **When** a new version of its connector only adds operations the
   widget does not call, **Then** the widget runs without a new approval.
2. **Given** an approved widget, **When** a new version of the widget or of a connector adds a
   destination, changes the destination, parameters, request or kind of an operation the widget
   calls, or makes the widget call a new operation, **Then** it is refused until approved,
   naming what changed, and the grants already given stay as they were.
3. **Given** an optional connector that is off, **When** the widget calls it, **Then** every call
   answers "not granted", nothing is sent, and the widget keeps running.
4. **Given** a connector that is on, **When** the maintainer turns it off or removes the widget's
   approval, **Then** from the next call, even within an update already running, calls answer
   "not granted" or the widget is refused, and the data the host kept for the widget is
   discarded.
5. **Given** a widget that depends on major version 1 of a connector, **When** only another major
   version of that connector is available, **Then** the widget is refused, naming the version it
   needs.

---

### User Story 6 - The user chooses where a protocol connector goes (Priority: P3)

Some catalog connectors speak an open protocol instead of one service, starting with RSS and
Atom feeds. Their destinations come from the user, for each widget, through the host's controls;
the widget can only pick among the destinations the user chose. Those destinations follow the
same proxy rules as any other.

**Why this priority**: open protocols cover many sources of a productivity dashboard without
third-party connectors, and the widget still never chooses where a request goes.

**Independent Test**: approve a feed widget, set two feed addresses through the tool and update:
both feeds arrive; make the widget select a destination the user did not choose (refused); set
an address that leads to a private address (refused).

**Acceptance Scenarios**:

1. **Given** the maintainer chose a widget's feed addresses, **When** the widget updates,
   **Then** it can fetch each of them, and the tool shows them as chosen by the user.
2. **Given** a widget that selects a destination the user did not choose, **When** the call is
   made, **Then** it is refused before anything is sent.
3. **Given** an address that is not HTTPS or is an IP address, **When** the maintainer sets it,
   **Then** it is refused, naming the rule; **given** an address whose name leads to a loopback
   or private address, **When** the widget fetches it, **Then** the call is refused.
4. **Given** a widget with no destination chosen, **When** it updates, **Then** its calls fail
   naming that no destination was chosen, and the update fails with that reason.

---

### User Story 7 - Authors build connectors and widgets on contract 1.1.0 (Priority: P3)

Authors write a connector and a widget with the published schemas and documentation, test them
in development mode against their own machine, and build against contract 1.1.0. Widgets built
for contract 1.0.0 keep running; one built for a newer minor version is refused.

**Why this priority**: connectors work only if authors can write them correctly, and existing
widgets must not break when the contract grows.

**Independent Test**: validate the example connector and widget against the schemas and run the
example; run a widget built for 1.0.0 on the new host; check that one claiming a newer minor
version is refused; run a widget with a local connector pointing at the author's machine in
development mode.

**Acceptance Scenarios**:

1. **Given** the published schemas and documentation, **When** an author writes a connector and
   a widget like the example, **Then** they validate and the example runs.
2. **Given** a connector with a mistake (an unknown field, a destination with a path, a
   parameter without a type), **When** it is checked, **Then** the error names the field and the
   problem.
3. **Given** a widget built for contract 1.0.0, **When** it runs on this host, **Then** it works
   as before; **given** a widget built for a newer 1.x than the host's, **When** it is loaded,
   **Then** it is refused as a contract mismatch, naming both versions.
4. **Given** a widget built for contract 1.0.0 whose manifest declares a connector, **When** it
   is loaded, **Then** it is refused, naming the contract version that connectors need.
5. **Given** a local connector whose destination is the author's own machine without HTTPS,
   **When** the maintainer runs a widget with it through the development tool, **Then** it works,
   and its approval is kept only in the development record.
6. **Given** a local connector whose destination is a private address of the local network, or
   a catalog domain, **When** it is loaded, **Then** it is refused, even in development.

### Edge Cases

- A connector declares two hosts and only one of them authorizes its repository: the connector
  is refused, naming the other host.
- A destination's name leads to several addresses, some public and some not: only public ones
  are used, and none at all means the call is refused. A name that starts leading to a private
  address after approval is refused at the call: addresses are checked at every connection.
- A redirect to another path on the same host is followed; to another host the same connector
  declares, it is followed under that host's rules; to any other host, a non-public address or
  plain HTTP, it is not followed and the call fails, naming the target. Past a fixed number of
  redirects, the call fails.
- Host names are compared in one normal form, so `API.Example.com` and `api.example.com` are the
  same host for reservation and verification. A destination given as an IP address is refused
  outside development.
- The catalog's reservation covers each host it declares and every host under the same
  registrable domain: declaring `api.github.com` reserves `github.com` and all its subdomains.
- One host authorizes two repositories: both may publish connectors for it; the owner decides.
- An authorization that is too large, malformed or served through a redirect counts as no
  authorization.
- A connector declares a credential: it is refused until feature 005.
- A widget declares a connector it never calls: allowed; a required connector still needs
  approval, as every required permission does (feature 002).
- A response larger than the limit, or in a format other than the operation declares: the call
  fails naming the limit or the format, and the widget gets nothing of it.
- The service answers with an error status: the widget receives the status and the body, within
  the limits; it is a normal answer, not a refusal.
- Two widgets use the same connector: each has its own approval and its own limits; the rate
  limit per destination host is shared, so together they cannot flood a service.
- An update calls an operation that changes data: refused, naming that changes need a user
  action, which arrives in Phase 3.
- The machine has no network: calls fail as unavailable, the update fails, and the previous data
  is kept.
- The request log reaches its size limit: the oldest entries go first; recording never stops.
- A widget's update returns output that is not valid JSON, or too large: the update fails and the
  previous data is kept.
- A connector file an approved widget needs is gone or changed (development): the widget is
  refused naming the connector, or the change follows User Story 5.
- A plugin tries to read the request log, another widget's kept data or the verification
  results: the host offers no capability for any of them.
- A service on the user's own machine or local network, such as a home server: in this feature
  no connector reaches it; only the developer's own machine, in development. Constitution v2.1.0
  lets a catalog connector reach one at an address the user chooses, from a later feature.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The host MUST offer plugins no network access other than calling connector
  operations: no sockets, no generic HTTP, nothing else that sends data off the machine. A
  component that asks for any of them MUST be refused before it runs.
- **FR-002**: A connector MUST be a declaration, never code. It states its id, version, author,
  destinations and operations. Each operation states whether it reads or changes data, its
  destination, the fixed shape of its request, the parameters a widget may fill in, each with
  its type, and the format of its response (JSON or text). The project MUST publish the
  connector schema, and the host MUST refuse a connector that does not conform, naming the field
  and the problem.
- **FR-003**: A parameter MUST be text with a maximum length and, optionally, a pattern or a
  fixed set of values; a whole number with a range; or true/false; and either required or
  optional. The host MUST check every value of every call against its type before sending
  anything, and refuse the call otherwise, naming the parameter and the rule. A value goes only
  where its operation places it (one path segment, one query value or one field of the request
  body), encoded by the host, so no value can change the destination, reach outside its place or
  add anything to the request.
- **FR-004**: A connector's id MUST be the repository it comes from (`<host>/<owner>/<repository>`,
  at most one connector per repository, which may also hold one widget), a catalog id that no
  repository can claim, or a local id (`local/<name>`) for development. Its destinations depend
  on that: a catalog connector's are those it declares, or the user's choice for a protocol
  connector (FR-015); a repository connector's must authorize its repository (FR-006); a local
  connector's are allowed only in development (FR-020).
- **FR-005**: The official catalog MUST live in the project's repository and ship with the host:
  a connector is a catalog connector only if it is in the catalog the host was built with. Every
  host a catalog connector declares, and every host under the same registrable domain, MUST be
  reserved to the catalog: a connector outside it that declares one MUST be refused, naming the
  host, whatever that host authorizes. The project MUST publish the criteria a connector meets to
  enter the catalog.
- **FR-006**: Every destination host of a repository connector MUST publish, at a fixed address
  under `/.well-known/` on that host, an authorization listing the repositories allowed to publish
  connectors for it, in a format the project publishes. The host MUST verify every destination
  host before approval, at every update of the connector, and whenever its last successful
  verification is older than a fixed period. A host whose authorization does not list the
  connector's repository, or that publishes none or an invalid one, MUST make the connector
  refused, naming the host. When a verification cannot complete, the last successful one MUST
  stay valid for at most a maximum age, after which calls are refused until a verification
  succeeds. Fetching an authorization MUST follow FR-008.
- **FR-007**: A widget's manifest MUST declare each connector it uses: its id, its major version,
  the operations the widget calls, and whether the connector is required or optional, each
  connector being a permission under the rules of feature 002. A widget MUST be refused at load,
  naming what is missing, when a connector it declares is unavailable, is not of the declared
  major version, or lacks an operation the widget declares. A call to an operation the widget did
  not declare MUST be refused before anything is sent.
- **FR-008**: Every connector request MUST go through the host proxy, which MUST: use HTTPS with
  a certificate valid for the destination host; connect only to the exact host and port of the
  operation's destination; refuse a destination given as an IP address; never connect to a
  loopback, private, link-local or other non-public address, checking every address a name leads
  to at every connection; follow a redirect only to a host the same connector declares, under
  these same rules, at most a fixed number of times; and neither send nor keep cookies. Local
  connectors in development are the only exception (FR-020).
- **FR-009**: The host MUST limit each call's time, request size and response size; the number
  of calls and the total response size of each update; and the rate of calls per widget and per
  destination host across all widgets. A call over a limit MUST fail with an error naming the
  limit, and the widget MUST NOT receive any part of a response over the size limit.
- **FR-010**: The host MUST record every connector call, allowed or refused, in a request log:
  time, widget, connector, operation, destination host, parameter values as sent, outcome
  (response status, or the refusal and its rule), bytes sent and received, and duration. It MUST
  NOT record credentials. The log MUST keep a bounded size, dropping the oldest entries first.
  Only the host's own controls, in this phase the command-line tool, MAY read it; no plugin MAY
  read or change it.
- **FR-011**: A widget MAY call connectors only during an update; a call made during render MUST
  be refused. An update MUST have a time limit of its own, larger than render's, and the same CPU
  and memory limits as render. It returns a JSON data document within a size limit. The host MUST
  keep each widget's last valid data across runs, with the time it was fetched. An update that
  fails (an error, a time or other limit, invalid output, a refused call that ends it) MUST leave
  the previous data unchanged and record the reason.
- **FR-012**: Render MUST receive, through the configuration the host gives it, the kept data,
  the time it was fetched and the outcome of the last update. Render's limits stay those of
  feature 001, and render MUST NOT cause any network request.
- **FR-013**: The host MUST update a widget when it is loaded, at the interval its manifest
  declares, and when the user asks, and MUST NOT update it more often than its interval except at
  the user's request. A declared interval below the host's minimum MUST be refused at load,
  naming the minimum. In this phase the command-line tool updates a widget only when the
  maintainer asks; updating on load and on a schedule arrives with the host app (Phase 3).
- **FR-014**: Operations that change data MUST be shown as such before approval. In this
  feature, an update MAY call only operations that read; a call to an operation that changes data
  MUST be refused, naming that it needs a user action, which arrives in Phase 3.
- **FR-015**: A catalog protocol connector MUST take its destinations from the user, for each
  widget, through the host's controls. The widget MAY only select among the destinations the
  user chose; selecting any other MUST be refused before anything is sent. A chosen address that
  is not HTTPS, or is an IP address, MUST be refused when the user sets it, and one whose name
  leads to a non-public address MUST be refused at the call (FR-008).
- **FR-016**: Before approval, the host MUST show, for every connector a widget declares, its id
  and version, where its destinations come from (catalog, verified for its repository, chosen by
  the user, or local development), every destination host, and every operation the widget calls,
  marked as a read or a change. The approval covers those destinations and operations as shown.
  A new version of the widget or of a connector that adds a destination, changes the
  destination, parameters, request or kind of an operation the widget calls, or makes the widget
  call a new operation MUST be refused until approved, naming what changed, with the grants
  already given unchanged. A connector update that only adds operations the widget does not call
  MUST NOT need a new approval.
- **FR-017**: An optional connector MUST start off and, while off, answer every call with "not
  granted" and send nothing (Principle II). Removing a widget's approval or turning off one of
  its connectors MUST take effect from the widget's next call, even within an update already
  running; a request already sent MAY finish within its time limit, but its response MUST NOT
  reach the widget. The host MUST then discard the data it kept for that widget.
- **FR-018**: Contract v1 MUST move to version 1.1.0, adding the connector capability, which can
  answer "not granted", and the update entry point. The host MUST run widgets built for any v1
  minor version up to its own as before, and MUST refuse one built for a newer minor version as
  a contract mismatch, naming both versions; every contract name a component uses MUST carry one
  version. A widget that declares a connector MUST be built for 1.1.0 or later, or be refused,
  naming the version needed. Contract v0 stays as it is.
- **FR-019**: A connector that declares a credential MUST be refused, naming that credentials
  arrive with feature 005. The connector format MUST leave room for credentials without changing
  what this feature defines.
- **FR-020**: The development command-line tool MAY run local connectors whose destination hosts
  are not verified, or are loopback addresses without HTTPS on any port, keeping their approvals
  only in the development record that installed plugins never use (constitution v2.0.0,
  Principle III). Even there, a host the catalog reserves and a non-loopback private address MUST
  be refused. An approval that involves a connector belongs to the files it was approved from,
  as in feature 002: a connector file elsewhere that claims the same id inherits nothing.
- **FR-021**: Every refusal MUST name, in one message, the connector, operation, parameter or
  destination concerned and the rule that applied: operation not declared, unknown operation,
  value outside its type, destination not verified, destination reserved, non-public address,
  not HTTPS, redirect not allowed, over a limit, called outside an update, change needs a user
  action, connector unavailable or of another version, contract mismatch, invalid connector,
  invalid authorization, credentials not supported yet, no destination chosen.
- **FR-022**: The hostile suite MUST gain one case per rule in FR-001, FR-003, FR-005, FR-006,
  FR-007, FR-008 (a non-public address given directly, reached through a name and reached
  through a redirect; plain HTTP), FR-009, FR-011, FR-014, FR-015 and FR-017. It MUST run on
  Windows, macOS and Linux on every change that is not documentation-only, without depending on
  the internet, and fail the build if any case is not refused or if any request reaches a
  destination it was not approved for (Principle VIII).
- **FR-023**: The catalog MUST start with two connectors that need no account: RSS and Atom feeds
  (a protocol connector) and GitHub public data (a few read operations on public repositories).
  The project MUST ship an example widget that uses a catalog connector, and documentation for
  widget authors (declaring connectors, the update entry point), connector authors (the
  connector schema, testing in development) and service owners (the authorization).
- **FR-024**: Every project document that still describes a domain allowlist or the earlier
  security core MUST describe connectors instead: `SECURITY.md`, `CONTRIBUTING.md`, the threat
  model, the contracts' `README`, the plugin author guide and the pull request template's list
  of the security core (Principle IX now includes the connector engine and the catalog).

### Key Entities

- **Connector**: the declaration, as data, of the operations a widget may call on one external
  service; it has an id, a version, an author, destinations and operations, and comes from the
  catalog, a repository or, in development, a local file.
- **Operation**: one request a connector can make: a read or a change, its destination, the
  fixed shape of its request, its parameters and the format of its response.
- **Parameter**: a value a widget fills in when calling an operation, with a type the host
  checks: text, whole number or true/false, with its bounds.
- **Destination**: the host an operation reaches; it is declared by a catalog connector,
  authorized by its owner for a repository connector, chosen by the user for a protocol
  connector, or local in development.
- **Official catalog**: the connectors kept in the project's repository and shipped with the
  host, reviewed line by line, whose domains are reserved to it.
- **Domain authorization**: what a service's owner publishes on each host to list the
  repositories allowed to publish connectors for it.
- **Connector dependency**: what a widget's manifest declares about a connector: its id, major
  version, the operations the widget calls, and whether it is required or optional.
- **Update**: the widget's entry point for fetching data through connectors, with limits of its
  own, separate from render.
- **Kept data**: the last valid data an update returned, with the time it was fetched, kept by
  the host and given to render.
- **Request log**: the host's record of every connector call, readable only through the host's
  own controls.
- **Contract 1.1.0**: contract v1 with the connector capability and the update entry point; the
  host still runs widgets built for 1.0.0.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the new hostile network cases are refused on all three operating systems,
  alongside the cases of features 001 and 002, and no test service receives a request from a
  refused case.
- **SC-002**: For every combination of declared, approved and called operation, and every kind
  of destination, a request leaves the machine only when the widget declared the operation, the
  user approved it, and its destination is allowed for its kind, verified by tests.
- **SC-003**: Every refusal tells the maintainer, in one message, the connector, operation,
  parameter or destination and the rule, without reading source code or host logs.
- **SC-004**: A widget whose service takes 5 s to answer still renders in under 1 s from its
  kept data, every time.
- **SC-005**: With the network down, a widget that updated once renders its last data and when
  it was fetched, every time.
- **SC-006**: Every widget built for contract 1.0.0 in the repository (example and hostile
  suites) passes unchanged on the new host, and a widget built for a newer minor version is
  refused every time.
- **SC-007**: Checking a widget's connectors when it loads, leaving out network verification,
  adds less than 10 ms on every CI system.
- **SC-008**: 100% of the connector calls made in the tests, allowed and refused, appear in the
  request log.
- **SC-009**: Removing a widget's approval, or turning off one of its connectors, takes effect
  from the widget's next call and discards its kept data, every time.
- **SC-010**: The example connectors and widget validate against the published schemas in CI,
  and the example runs end to end in development mode.

## Assumptions

- In this phase the maintainer approves widgets and their connectors, chooses the destinations of
  protocol connectors and runs updates with the command-line tool. The host dialogs of Phase 3
  apply the same rules, and the Phase 3 warning about data from one account reaching another
  destination judges the destinations this feature shows.
- Installation from repositories, keeping each repository's permanent identifier, arrives in
  Phase 3. In this phase widgets and connectors load from local files in development mode. A
  local file may claim a repository id; the host then checks that the destination hosts
  authorize that repository, and proving that the file came from it arrives with installation.
- The numbers (update time limit, call time limit, request and response sizes, calls and bytes
  per update, rates, minimum interval, verification period and maximum age, log size, number of
  redirects) are set in the plan and measured on the three systems, as feature 001 did.
- The host hands a response body to the widget as received, within the limits; it checks only
  its format and size and never runs or shows anything in it. The widget reads it inside the
  sandbox.
- The security tests reach test services on the CI machine, not the internet; how they do so
  without weakening the rules for real connectors is decided in the plan.
- Constitution v2.2.0 lets a catalog connector reach a service on the user's own machine or
  local network at an address the user chooses, which must stay local. There the host may trust
  a certificate the user accepted at first use, and may go without HTTPS, shown as not
  encrypted, only for a connector that sends no credential; a credential travels only over HTTPS
  with a certificate valid for the destination or accepted by the user (Principle IV). Most such
  services need a credential, so the first of these connectors arrives after feature 005. In this feature the proxy refuses every
  non-public address, and the destination kinds (FR-004, FR-016) leave room for this one.
- GitHub limits requests without an account; the catalog's GitHub connector serves public data
  within those limits until accounts arrive (feature 005).
- The catalog is security core (Principle IX): every change to it is reviewed line by line.
- One person uses the host on a machine, as in feature 002.
- Out of scope: credentials, API keys and OAuth (005); catalog connectors for services on the
  user's machine or local network (after 005); user actions, operations that change data driven
  by them, installation dialogs and the cross-account warning (Phase 3); storage (004);
  signatures and the store (Phase 6); updating widgets in the cloud while the computer is off
  (Phase 7), which a separate update entry point is meant to allow.
