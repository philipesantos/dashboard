# Feature Specification: Plugin Credentials

**Feature Branch**: `005-plugin-credentials`

**Created**: 2026-10-24

**Status**: Draft

**Input**: User description: "Feature 005 — secrets vault, OAuth and connector credentials (Phase 2
of the guide; gate case: an attempt to obtain the token must be blocked, the last open case of the
Phase 2 gate; constitution v2.4.0, Principles II, IV, VII, VIII, IX; fixed decision: the `keyring`
crate, OS keychain). A connector may now declare the credential its operations need, replacing
003's refusal (FR-017): an OAuth account (authorization code with PKCE in the system browser,
first provider GitHub) or a key the user enters. The declaration names the destination it is for,
the scopes and the header it travels in, and it is shown at approval as part of the connector's
grant, so an update that adds a credential or widens its scopes requires a new approval (narrowing
does not). Credentials live only in the OS keychain through `keyring`; host records hold only
which account is connected (connector, login, scopes, date), never a token, and no token is ever
written to the app's files, the request log, kept data, storage, error messages or terminal
output. The host injects the credential only into that connector's requests to the declared
destination, over HTTPS with a certificate valid for it, and only into the declared header: never
into a path, query or body, never into a value a plugin parameter can name or build, and never
past a redirect (already refused). Plugins never receive a credential: no import, file,
environment variable or argument carries one, and a response that contains the credential sent
with its request is withheld from the widget as invalid-response. The account belongs to the
connector, not to a widget: every widget granted that connector uses the same connected account,
and each grant shows that it acts as that account. The manifest's login_providers field (feature
002) becomes obsolete: the accounts a widget uses come from the connectors it declares; a manifest
that still carries it stays valid, and the field grants nothing. A connector's credential is
required or optional; the catalog's GitHub connector moves to 1.1.0 with an optional GitHub
account (public data without one; private data and a higher rate limit with one). The host
refreshes tokens itself; a token that is missing, expired past refresh or revoked answers
account-not-connected, and a service that refuses for lack of a paid plan answers
subscription-required (the connector declares which responses mean it), so the widget can render
the state without learning more. Neither a widget nor its output can connect, reconnect or
disconnect an account or open the browser (Principle VII). Until the Phase 3 dialogs, the
development CLI connects, lists and disconnects accounts for local packages (the system browser
for OAuth; a key read from the terminal without echo, never from an argument), in a development
vault, in the OS keychain, that installed plugins never use (constitution v2.4.0); disconnecting
deletes the keychain entry and revokes the token at the provider when it supports that. OAuth
waits for the redirect on a loopback address with a one-time port and checks state, so a forged or
replayed callback connects nothing. CI uses only a test OAuth provider; local development uses an
OAuth app the developer registers and configures; the project's own GitHub app is registered
before the first release (Phase 6, with the project's name). The plan checks whether GitHub's flow
needs a client secret, which a desktop app cannot keep, and if so uses GitHub's device flow, also
in the system browser. Contract v1 moves to 1.3.0: freeze 1.2.0 under wit/history/1.2.0 first,
then add the account-not-connected and subscription-required answers to connector calls, and
extend runtime/tests/contract_v1.rs to the 1.2.0→1.3.0 pair; the host never returns those answers
to a widget built for an older minor: such a widget calls a connector with an optional credential
without it, and one with a required credential is contract-too-old. Deliverables: the
github-releases example showing private releases once an account is connected, the hostile cases
below, docs (SECURITY.md, docs/threat-model.md, wit/README.md, docs/plugin-authors.md,
connectors/README.md, plugins/hostile/README.md), and bench numbers (call latency with and without
a credential, a token refresh) in the feature's performance.md. Hostile cases (CI uses a test
OAuth provider and test services, never the real GitHub): token-hunt (environment, arguments,
wasi:filesystem on the keychain files and host records, every import: capability-refused, nothing
found); echo-credential (a destination returns the Authorization header in its body or in a
declared response header: withheld, the token never reaches the widget); other-destination (a
connector with two destinations calls the one the credential is not for: sent without it);
parameter-smuggling (values with CR/LF, header names or text naming the credential: refused or
sent as data, never as a header and never replaced by the token); plain-http-dev (a local
development connector over plain HTTP or with an invalid certificate that declares a credential:
refused, nothing sent); credential-escalating-update (adding a credential or a scope:
needs-new-approval); same-id-other-origin (a file copying an approved id: no account); not-granted
(a widget without the connector's grant, or with an optional one off, never causes a credential to
be sent); forged-callback (wrong state, replayed code, a callback from another flow: nothing
connected, the existing account kept); widget-connect (no widget call opens the browser or starts
a flow); revoked-and-expired (account-not-connected after a failed refresh, no retry loop);
disconnect-mid-run (a request in flight may finish, its response does not reach the widget, no
later request carries the credential); account-spoof (a login name with line breaks and terminal
escapes printed escaped by the CLI); secrets-out-of-files (after every case, no token bytes in the
host records, the development data directory, the request log, kept data, storage or captured
output). Out of scope: host dialogs for connecting accounts and the cross-account warning (Phase
3); OAuth for authors' own services on verified domains (Phase 3, with repository connectors; the
format leaves room for endpoints on the connector's verified domain); local-network connectors
that need a credential and certificates accepted at first use (right after 005); AI provider keys
(Phase 4); several accounts for one connector; OAuth grants other than authorization code with
PKCE and, if GitHub needs it, the device flow; registering the project's GitHub app (Phase 6);
sync or export of the vault; the TypeScript example (separate Phase 2 item)."

## User Scenarios & Testing *(mandatory)*

The people using this feature are the maintainer, who connects accounts, runs widgets and approves
them through the command-line tool; widget authors; connector authors, who in this phase write
connectors for the official catalog or, in development, for their own service; and the project,
which keeps the catalog. End users make the same decisions later, in the host dialogs of Phase 3.

A credential is what a service needs to know who is calling: an account the user signs in to
through the service's own login page, or a key the user copies from the service. The connector
says which credential its operations need and where it goes. The host keeps it, adds it to the
connector's requests itself, and never hands it to a widget: the widget calls the same operations
as before and gets the answers, never the credential.

The host learns how to sign in to a service only from the connector's declaration: the addresses
of the service's sign-in and token pages, the access it asks for and the header the credential
travels in. It runs one sign-in procedure, the published standard, for every service; GitHub is
the first service described this way, not a case of its own.

### User Story 1 - A widget shows private data through the user's account (Priority: P1)

The maintainer connects their GitHub account to the catalog's GitHub connector through the tool:
the system browser opens GitHub's own sign-in page, the maintainer signs in there and chooses the
repositories the connector may read, and the tool reports the account as connected, with its name
and the access it holds. An author's widget shows the releases of a repository and declares that it
uses the connector's account. The maintainer asks what the widget requests, sees that it acts as
the connected GitHub account with read-only access, and approves it. When the widget updates, it
calls the same operation as before; the host adds the credential to the request, and the widget
receives the releases of a private repository. Nothing the widget can reach holds the credential.

**Why this priority**: this is Principle IV made concrete: services that need an account become
usable without any plugin ever seeing the account's credential.

**Independent Test**: connect an account on the test provider, approve the example widget, run an
update and a render, and check that the private data appears, the request log shows the call as
made with the account, and no file, log or output holds the token.

**Acceptance Scenarios**:

1. **Given** a connector whose credential is an account, **When** the maintainer connects it
   through the tool, **Then** the system browser opens the service's sign-in page at the address
   the connector declares, and after the maintainer signs in the tool shows the account as
   connected, with the account's name, the access asked for and the access granted.
2. **Given** a widget that declares it uses a connector's account, **When** the maintainer asks
   what it requests, **Then** the tool shows, for that connector, the kind of credential, the
   destinations it is sent to, the access it asks for, whether it is required or optional, that the
   widget acts as that account, and which account is connected now, or that none is.
3. **Given** the widget is approved and an account is connected, **When** an update calls an
   operation, **Then** the host sends the request with the credential to the operation's
   destination and gives the widget the response, and the request log records that the call was
   made with the account, never the credential.
4. **Given** a connector whose credential is optional and no account connected, **When** an
   approved widget calls an operation, **Then** the request goes without a credential, as in
   feature 003, and the widget receives the public answer.
5. **Given** a connector whose credential is a key, **When** the maintainer connects it, **Then**
   the tool reads the key from the terminal without showing it and reports the key as stored,
   with the date; **When** an approved widget calls an operation, **Then** the request carries
   the key.
6. **Given** an approved widget that does not declare it uses the account, **When** it calls the
   same connector while an account is connected, **Then** its requests go without the credential.

---

### User Story 2 - No plugin obtains a credential (Priority: P1)

The maintainer runs hostile widgets that each try to get hold of a token or a key: one looks
through every capability it is given, its environment, its arguments and the files it can name;
one uses a destination that copies the credential it received into its answer; one tries to
start a sign-in or open the browser; one tries to read the host's records or the keychain's files.
Each finds nothing, or is refused, and after every case no file the host writes, no log and no
output holds any credential. The cases run automatically on Windows, macOS and Linux, without the
real GitHub, and break the build if any one is not contained.

**Why this priority**: this is the last Phase 2 gate case, "an attempt to obtain the token is
blocked", and every security rule needs a test that breaks the build (Principle VIII).

**Independent Test**: run the hostile suite with an account connected on the test provider; every
new case is contained, a scan of the host's records, the development data directory, the request
log, kept data, stores and captured output finds no credential, and a well-behaved widget updates
and renders normally right afterwards.

**Acceptance Scenarios**:

1. **Given** a connected account, **When** a widget looks for its credential in every capability
   it is linked to, its environment, its arguments and its configuration, **Then** it finds
   nothing; **given** a component that asks for file system access to reach the keychain or the
   host's records, **When** it is loaded, **Then** it is refused before running.
2. **Given** a destination that returns the credential it received, in its body or in a header the
   operation passes on, **When** the widget calls it, **Then** the widget receives no part of that
   response, the call fails as an invalid response, and the request log records why.
3. **Given** any widget, **When** it runs, **Then** no call it makes starts a sign-in, opens the
   browser, connects, reconnects or disconnects an account; a call that needs an account that is
   not connected answers "account not connected" and nothing else happens.
4. **Given** every hostile case of this and earlier features, **When** it has run, **Then** no
   credential, authorization code or sign-in secret appears in any file the host wrote, the
   request log, kept data, any store, any error message or any output.
5. **Given** any hostile case was just contained, **When** a well-behaved widget updates and
   renders next, **Then** it produces its normal result; the cases of features 001 to 004 still
   pass.

---

### User Story 3 - A credential goes only where its connector sends it (Priority: P1)

A credential is bound to its connector and to the destinations the connector names for it. The
maintainer runs hostile widgets and connectors that try to send it anywhere else: one calls a
second destination of the same connector; one fills parameters with line breaks, header names or
text that names the credential; one is a local development connector over plain HTTP or with an
invalid certificate; one is a widget without the connector's grant, or with an optional connector
turned off; one is a file that copies the id of an approved widget or of a local connector that
holds an account. In every case the credential is not sent, or nothing is sent at all.

**Why this priority**: Principle IV requires a credential to reach only its connector's own
destination over authenticated HTTPS; a credential sent anywhere else is stolen.

**Independent Test**: with an account connected, run each case against test services that record
what they receive; only the destinations the credential is for ever receive it, and only over
HTTPS with a valid certificate.

**Acceptance Scenarios**:

1. **Given** a connector with two destinations whose credential is for one, **When** a widget
   calls an operation on the other, **Then** the request goes without the credential.
2. **Given** parameter values with line breaks, header names, or text that names the credential
   or its header, **When** a widget calls an operation, **Then** each value is refused by its type
   or sent as data in its place, never as a header, and never replaced by the credential.
3. **Given** a local development connector whose credential would travel over plain HTTP, or to a
   destination whose certificate is not valid, **When** it is loaded or called, **Then** it is
   refused and nothing is sent.
4. **Given** a widget without the connector's grant, with the connector turned off, or whose
   approval was removed, **When** it calls the connector, **Then** the call answers "not granted"
   and nothing is sent.
5. **Given** a file from another origin that claims an approved widget's id, or a local connector
   file elsewhere that claims the id of a local connector with an account, **When** it runs,
   **Then** it gets no grant and no account.
6. **Given** a redirect from a request that carried the credential, **When** it leads to a host
   the credential is for, **Then** it is followed with the credential under feature 003's rules;
   **When** it leads to another host the connector declares, **Then** it is followed without the
   credential; anywhere else it is not followed, as before.

---

### User Story 4 - The maintainer connects, checks and disconnects accounts safely (Priority: P2)

The maintainer manages accounts through the tool: connects one, lists what is connected,
disconnects one. A sign-in can be completed only by the browser the tool opened for it: a forged
answer, an old one, or one meant for another sign-in connects nothing. Tokens that expire are
renewed by the host; one the service has revoked, or that cannot be renewed, makes the account
show as needing to be connected again, and the widgets that use it are told so. Everything the
tool prints about an account is escaped, so nothing a service returns can disguise its output.

**Why this priority**: connecting an account is a trust decision (Principle VII); a sign-in that
anyone on the machine could complete, or a revoked token that keeps being retried, would undo the
rest of the feature.

**Independent Test**: connect, list and disconnect an account on the test provider; send the
tool's waiting address a forged, a replayed and a late answer and check nothing is connected; make
the test provider expire and then revoke the token and check the account shows as needing to be
connected again after one renewal attempt.

**Acceptance Scenarios**:

1. **Given** the maintainer connects an account, **When** an answer arrives at the address the
   tool waits on with the wrong or no check value, with a code already used, after the tool's time
   limit, or meant for another sign-in, **Then** the sign-in ends with nothing connected, and an
   account already connected stays as it was.
2. **Given** a connected account whose token expired, **When** a widget calls the connector,
   **Then** the host renews the token once and sends the request; **given** renewal fails or the
   service answers that the credential is no longer valid, **Then** the call answers "account not
   connected", the account shows as needing to be connected again, and the host makes no further
   attempt until the maintainer connects it again.
3. **Given** connected accounts, **When** the maintainer lists them, **Then** the tool shows each
   connector, where it comes from, the kind of credential, the account's name, the access asked
   for and granted, when it was connected, and whether it needs to be connected again, escaping
   every text a service or a connector supplied.
4. **Given** a connected account, **When** the maintainer disconnects it, **Then** the credential
   is deleted from the keychain, the host asks the service to revoke it where the connector
   declares how, and the tool says whether the service confirmed it or the authorization remains
   with the service until the user removes it there.
5. **Given** a service that refuses a request because the account lacks a paid plan, as the
   connector declares, **When** a widget calls it, **Then** the call answers "subscription
   required" and the widget renders that state.
6. **Given** a key to connect, **When** the maintainer passes it as an argument of the tool,
   **Then** the tool refuses and asks for it from the terminal.

---

### User Story 5 - Changes never widen account access silently (Priority: P2)

A connector update that adds a credential, asks for more access, or sends the credential to a new
destination does not take effect for any widget until approved; a widget update that starts using
the account does not run until approved. Asking for less access, or a widget that stops using the
account, needs no approval. Disconnecting an account, turning off its connector or removing a
widget's approval takes effect from the next call, even within an update already running.

**Why this priority**: an update that quietly lets a widget act as the user's account, or with
more access, is how a trusted widget or connector turns hostile (feature 002's rule for
permissions, applied to accounts).

**Independent Test**: approve a widget that uses an account; update its connector to ask for more
access (refused until approved, and the account must be connected again); disconnect the account
during an update (no later request carries the credential, and the response in flight does not
reach the widget).

**Acceptance Scenarios**:

1. **Given** an approved widget, **When** a new version of its connector adds a credential, asks
   for more access, sends the credential to another destination or in another header, or a new
   version of the widget starts using the account, **Then** the widget is refused until approved,
   naming what changed, and the grants already given stay as they were.
2. **Given** a connector update that asks for more access than the connected account holds,
   **When** a widget calls it after approval, **Then** the call answers "account not connected"
   until the maintainer connects the account again with the new access.
3. **Given** a connector update that asks for less access, or a widget update that stops using
   the account, **When** it runs, **Then** it needs no new approval.
4. **Given** an update in progress that has sent a request with the credential, **When** the
   maintainer disconnects the account, turns the connector off or removes the widget's approval,
   from any process, **Then** the request in flight may finish within its time limit, its response
   does not reach the widget, no later request carries the credential, and the data the host kept
   for widgets that used the account is discarded.

---

### User Story 6 - Authors declare credentials on contract 1.3.0 (Priority: P3)

Connector authors declare a credential with the published connector schema and documentation;
widget authors declare that they use a connector's account and build against contract 1.3.0,
which adds the answers "account not connected" and "subscription required". Widgets built for
1.0.0, 1.1.0 and 1.2.0 keep running, never receive those answers, and never use an account; one
built for a newer minor version is refused. A manifest that still lists login providers stays
valid; the list grants nothing.

**Why this priority**: credentials are useful only if authors can declare them correctly, and
existing widgets must not break when the contract grows.

**Independent Test**: validate the catalog's GitHub connector and the example widget against the
schemas and run the example on the test provider; run the existing 1.0.0, 1.1.0 and 1.2.0 plugins
of the repository on the new host; check that a widget declaring account use while built for
1.2.0 is refused naming 1.3.0.

**Acceptance Scenarios**:

1. **Given** the published schemas and documentation, **When** an author declares a credential
   and its use like the example, **Then** they validate and the example runs.
2. **Given** a credential declaration with a mistake (a sign-in page on a host the connector does
   not declare or reserve, a destination the connector does not declare, a header not allowed, a
   destination without HTTPS), **When** it is checked, **Then** the error names the field and the
   problem.
3. **Given** plugins built for contract 1.0.0, 1.1.0 and 1.2.0, **When** they run on this host,
   **Then** they work as before; **given** one built for a newer 1.x than the host's, **When** it
   is loaded, **Then** it is refused as a contract mismatch, naming both versions.
4. **Given** a widget built for 1.2.0 or older whose manifest declares it uses an account, or that
   depends on a connector whose credential is required, **When** it is loaded, **Then** it is
   refused, naming 1.3.0 as the version accounts need.
5. **Given** a manifest that lists login providers, **When** it is loaded, **Then** it is valid,
   the tool shows the field as obsolete, and it gives the widget no account.

### Edge Cases

- A connector declares a credential and an id that names a repository: refused as not verified, as
  in feature 003; authors' own services, with their sign-in on their verified domain, arrive with
  repository connectors in Phase 3.
- A connector declares more than one credential: refused; one connector has at most one
  credential, and one account at a time.
- A sign-in, token or revocation address on a host the connector neither declares nor (for a
  catalog connector) reserves: refused, naming the address.
- A credential header outside the catalog other than the standard authorization header: refused,
  since a local connector may not fix `X-` headers (feature 003) and a credential header is fixed
  by the connector. A catalog connector may use another header its review accepts.
- A connector that fixes a value in the credential's header, or a parameter placed there: refused;
  only the host writes that header.
- An account is connected before any widget is approved, or a widget is approved before any
  account is connected: both are allowed; connecting grants nothing, and approving connects
  nothing.
- The maintainer connects a different account to a connector: every widget whose grant includes
  that connector's account now acts as the new one; the tool names those widgets when connecting.
  Showing this in a host dialog, and the warning about data from one account reaching another
  destination, arrive in Phase 3.
- The service grants less access than the connector asked for (the user unticked a choice on the
  service's page): the account is connected with what was granted, the tool shows both, and an
  operation the service refuses gets the service's answer.
- The connector now asks for less access than the account holds: the account keeps working; the
  tool shows that it holds more than asked and offers connecting again.
- A connector with an optional credential whose account needs to be connected again: calls answer
  "account not connected" rather than going without the credential, so the widget does not show
  public data as if it were the user's.
- A connector with a required credential and no account: calls answer "account not connected" and
  nothing is sent.
- The keychain is locked, missing or refuses access: connecting fails naming the cause; a call that
  would carry the credential fails as unavailable, naming the keychain, and nothing is sent.
- The maintainer closes the browser or never finishes signing in: the tool stops waiting after its
  time limit, and nothing is connected.
- The system browser cannot be opened: the tool prints the sign-in address, escaped, for the
  maintainer to open, and waits as before.
- Two sign-ins for the same connector at once: the second is refused while the first waits.
- A service answers the token request with an error: the tool shows the service's error code and
  description, escaped and cut to a maximum length, never a code or a token.
- A widget makes thousands of calls that each need a renewal: the host renews at most once per
  account per run, and calls past the limits of feature 003 are refused as before.
- A response carries the credential in an encoding of the service's own choosing: the host's check
  finds the credential as sent; a service that hides it on purpose could pass it on, which is why
  only reviewed catalog connectors and, later, owner-verified domains carry credentials (see
  Assumptions).
- A key with line breaks, control characters or over its length limit: refused when the maintainer
  enters it.
- A widget's update fails after a call that carried the credential: the previous kept data stays,
  as in feature 003; the failure is recorded without the credential.
- The developer has not configured the sign-in client for a connector that needs one: connecting
  fails, naming how to configure it; nothing opens.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: A connector MAY declare one credential, which replaces feature 003's refusal of
  credentials: an account the user connects through the service's sign-in, or a key the user
  enters. The declaration MUST state its kind; whether it is required or optional; the
  destinations of the connector it is sent to; the header it travels in; for an account, the
  service's sign-in, token and, if any, revocation addresses, the access it asks for, the answers
  that mean the credential is no longer valid, and optionally the read operation that tells the
  account's name; and the answers that mean a paid plan is required. Sign-in, token and revocation
  addresses MUST be HTTPS on hosts the connector reserves (catalog) or declares (local
  development). Outside the catalog, the header MUST be the standard authorization header. The
  host MUST refuse a declaration that does not conform, naming the field and the problem.
- **FR-002**: The host MUST run one sign-in procedure for every service, from the published
  standards (authorization code with proof key, an answer to a loopback address on the user's
  machine, token renewal, and revocation where offered), driven only by the connector's
  declaration. The host MUST NOT contain code for a particular service; a service the declaration
  cannot describe is not supported until the connector format can describe it.
- **FR-003**: A catalog connector's credential MUST ask for the narrowest access the service offers
  for the connector's operations, and read-only access when they all read; the catalog review
  checks it. The catalog's GitHub connector MUST move to version 1.1.0 with an optional account
  whose access is read-only and limited to the repositories the user chooses on the service's
  page; public data stays readable without an account.
- **FR-004**: A widget's dependency on a connector MUST state whether the widget uses the
  connector's account; a widget that depends on a connector whose credential is required MUST
  state it. The account's use is part of the connector's grant. The host MUST send the credential
  only on calls of a widget whose approved grant includes the account; every other call MUST go
  without it, even while an account is connected.
- **FR-005**: An account MUST belong to the connector, identified by its id and origin (in
  development, the canonical path of the connector file, as for grants): one account per
  connector, used by every widget whose grant includes it. Connecting an account MUST grant
  nothing to any widget, and approving a widget MUST connect nothing. A connector file from
  another origin that claims the same id MUST inherit no account.
- **FR-006**: Before approval, the host MUST show, for every connector with a credential that a
  widget uses, the kind of credential, whether it is required or optional, the destinations and
  header it is sent with, the access it asks for, that the widget acts as that account, and which
  account is connected, or that none is. The approval digest MUST cover the declaration and the
  widget's use of the account, not which account is connected. A new version of a connector that
  adds a credential, asks for more access, sends it to another destination or in another header,
  or a new version of a widget that starts using an account, MUST be refused until approved,
  naming what changed, with the grants already given unchanged. Asking for less access, or a widget
  that stops using the account, MUST NOT need a new approval.
- **FR-007**: Credentials (tokens, renewal tokens and keys) MUST be kept only in the operating
  system's keychain, through the host's vault; the host MAY hold them in its own memory while it
  runs. The host's records MUST hold only the connector, its origin, the kind of credential, the
  account's name, the access asked for and granted, when it was connected and renewed, and whether
  it must be connected again. No credential, authorization code or proof key MAY be written to any
  file the host writes, the request log, kept data, any store, an error message or any output, or
  sent anywhere but the addresses and destinations its connector declares for it.
- **FR-008**: The host MUST add the credential only to requests of its own connector, to the
  destinations the credential is for, after building the request from the operation, in the
  declared header only, and only over HTTPS with a certificate valid for the destination under
  feature 003's rules. It MUST NOT place a credential in a path, a query, a body, or any value a
  connector fixes or a widget fills, and no connector or parameter MAY write the credential's
  header. A redirect MUST carry the credential only to a host the credential is for; to another
  host of the connector it MUST be followed without the credential, under feature 003's rules.
- **FR-009**: Plugins MUST NOT receive any credential: no capability, environment value, argument,
  configuration or file reachable by a plugin may hold one. The host MUST withhold from the widget
  any response whose body or passed-on headers contain the credential sent with its request,
  failing the call as an invalid response and recording the reason in the request log.
- **FR-010**: Only the host's own controls MAY start a sign-in, connect, reconnect or disconnect an
  account, or open the browser; in this phase, the development tool (constitution v2.4.0,
  Principle VII). No capability lets a widget do any of these, and no widget call or output may
  cause one.
- **FR-011**: A sign-in MUST open the service's sign-in page, at the address the connector
  declares, in the system browser, and wait for the answer on a loopback address of the user's
  machine at a port chosen for that sign-in, for a bounded time. It MUST accept only one answer:
  with the check value it sent, a code not used before, before the time limit. Any other answer
  MUST end the sign-in with nothing connected and the account already connected unchanged. The
  host's own requests to the service (token, renewal, revocation, account name) MUST follow
  feature 003's proxy rules and appear in the request log as the host's own, without any secret.
  If the plan finds that GitHub's sign-in from a desktop app needs a secret the app cannot keep,
  the host MUST use the standard device sign-in instead, also in the system browser and under the
  same rules.
- **FR-012**: The tool MUST read a key only from the terminal without showing it, never from an
  argument, and refuse a key with line breaks, control characters or over its length limit.
- **FR-013**: The host MUST renew an expired token itself, at most once per account per run. When
  no account is connected and the credential is required, when renewal fails, or when the service
  gives an answer the connector declares as meaning the credential is no longer valid, the call
  MUST answer "account not connected", send nothing more, and mark the account as needing to be
  connected again; the host MUST make no further attempt for it until the maintainer connects it
  again. With an optional credential, calls go without it only while no account is connected;
  while an account needs to be connected again, they answer "account not connected". After a
  connector asks for more access than the account holds, calls MUST answer "account not
  connected" until it is connected again.
- **FR-014**: A response the connector declares as meaning a paid plan is required MUST reach the
  widget only as the answer "subscription required", with nothing of the response.
- **FR-015**: Disconnecting an account, turning off its connector or removing a widget's approval,
  from any process, MUST take effect from the next call, even within an update already running: a
  request already sent MAY finish within its time limit, its response MUST NOT reach the widget,
  and no later request MAY carry the credential. Disconnecting MUST also discard the data the host
  kept for the widgets that used the account.
- **FR-016**: The development tool MUST connect an account, list accounts and disconnect one, for
  the connectors of local packages, in a development vault in the operating system's keychain and
  development records that installed plugins never use (constitution v2.4.0). Listing MUST show
  each connector, its origin, the kind of credential, the account's name, the access asked for and
  granted, when it was connected and whether it needs to be connected again, escaping every text a
  service or a connector supplied. Disconnecting MUST delete the credential from the keychain and
  ask the service to revoke it where the connector declares how, and MUST say whether the service
  confirmed it. Accounts for installed plugins arrive with installation in Phase 3.
- **FR-017**: The sign-in client a connector uses MUST come from the host's build or, in
  development, from the developer's configuration, never from a plugin. Tests MUST use only a test
  service that implements the sign-in standards, never the real GitHub.
- **FR-018**: The manifest's list of login providers (feature 002) MUST become obsolete: a
  manifest that has it MUST stay valid, the tool MUST show it as obsolete, and it MUST give no
  widget any account. The accounts a widget uses come only from the connectors it declares.
- **FR-019**: Contract v1 MUST move to version 1.3.0, adding the answers "account not connected"
  and "subscription required" to connector calls; version 1.2.0 MUST first be kept unchanged as a
  record, and a test MUST fail the build if 1.3.0 does more than add to 1.2.0. The host MUST run
  plugins built for any v1 minor version up to its own as before, MUST NOT give those answers to a
  plugin built for an older one, and MUST refuse one built for a newer minor version as a contract
  mismatch, naming both versions. A widget that uses an account, or depends on a connector whose
  credential is required, MUST be built for 1.3.0 or later, or be refused, naming the version
  needed.
- **FR-020**: Every refusal and failure MUST name, in one message, the connector, operation,
  account or field concerned and the rule that applied: account not connected, subscription
  required, invalid response (a credential returned), unavailable (keychain); when a connector or
  widget loads or is approved: invalid connector (credential declaration), credential needs HTTPS,
  contract too old, needs a new approval (credential, access, destination, header or account use),
  approval changed; and, in the tool: sign-in ended (check value, used code, time limit, service
  error), sign-in already waiting, keychain unavailable, sign-in client not configured, key not
  accepted.
- **FR-021**: The hostile suite MUST gain one case per rule: looking for the credential through
  every capability, the environment, the arguments and the file system (FR-007, FR-009); a
  destination that returns the credential (FR-009); a second destination of the connector and
  redirects (FR-008); parameter values with line breaks, header names or the credential's name
  (FR-008); a local connector carrying a credential over plain HTTP or to an invalid certificate
  (FR-008); a version that adds a credential or asks for more access (FR-006); a file that copies
  an approved widget's id or a local connector's id (FR-005); a widget without the grant, without
  account use, or with the connector off (FR-004); forged, replayed, late and foreign sign-in
  answers (FR-011); a widget that tries to start a sign-in (FR-010); an expired and a revoked
  token (FR-013); disconnecting during an update (FR-015); a spoofed account name in the tool's
  output (FR-016); and, after every case of this and earlier features, a scan that finds no
  credential in any file the host wrote, the request log, kept data, stores or captured output
  (FR-007). The suite MUST run on Windows, macOS and Linux on every change that is not
  documentation-only, without depending on the internet, and fail the build if any case is not
  contained (Principle VIII).
- **FR-022**: The project MUST ship the example widget showing private releases once an account is
  connected (public ones without), the connector schema's credential declaration, the manifest
  schema's account use, and documentation for connector authors (declaring a credential, its
  access, its addresses) and widget authors (using an account, the new answers).
- **FR-023**: The security documents MUST describe the vault, accounts and their rules:
  `SECURITY.md`, the threat model, the contracts' `README`, the plugin author guide, the
  connectors' `README` and the hostile suite's `README`.

### Key Entities

- **Credential**: what a service needs to know who calls: an account's tokens or a key; kept only
  in the keychain, never seen by a plugin.
- **Credential declaration**: what a connector says about its credential: kind, required or
  optional, the destinations and header it goes with, and for an account the service's addresses,
  the access asked for and the answers that mean the credential is no longer valid or a paid plan
  is required.
- **Account**: a credential connected to one connector (id and origin), with the account's name,
  the access granted, when it was connected, and whether it must be connected again.
- **Account use**: what a widget's connector dependency says about using the connector's account;
  part of the connector's grant.
- **Vault**: the host's part that keeps credentials in the keychain and adds them to requests; in
  this phase, the development vault.
- **Sign-in**: one run of the standard procedure that connects an account, started only from the
  host's own controls and ended by one valid answer or its time limit.
- **Contract 1.3.0**: contract v1 with the answers "account not connected" and "subscription
  required"; the host still runs plugins built for 1.0.0, 1.1.0 and 1.2.0.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the new hostile credential cases are contained on all three operating
  systems, alongside the cases of features 001 to 004, and after every case a scan finds no
  credential in any file the host wrote, the request log, kept data, stores or captured output.
- **SC-002**: For every combination of declared credential, account use, approval, connector on or
  off, account connected or not, and destination, a credential leaves the machine only when the
  connector declares it for that destination, the widget's approved grant includes the account,
  the connector is on, an account is connected, and the connection is HTTPS with a valid
  certificate, verified by tests.
- **SC-003**: Every refusal tells the maintainer, in one message, the connector, operation,
  account or field and the rule, without reading source code or host logs.
- **SC-004**: No forged, replayed, late or foreign sign-in answer connects an account in any test.
- **SC-005**: Disconnecting an account, turning off its connector or removing a widget's approval
  takes effect from the next call, every time.
- **SC-006**: A call that carries a credential takes at most 5 ms longer than the same call without
  one on every CI system, and a token renewal is measured, as recorded in the feature's
  performance record.
- **SC-007**: Every plugin built for contract 1.0.0, 1.1.0 or 1.2.0 in the repository passes
  unchanged on the new host, and a plugin built for a newer minor version is refused every time.
- **SC-008**: The catalog's GitHub connector and the example widget validate against the published
  schemas in CI, and the example shows private data end to end against the test service.

## Assumptions

- In this phase the maintainer connects accounts, approves widgets and runs updates with the
  command-line tool (constitution v2.4.0). The host dialogs of Phase 3 apply the same rules, show
  the same facts before approval, and add the warning about data from one connected account
  reaching another destination.
- The fixed technical decision applies: credentials are kept in the operating system's keychain,
  through the `keyring` crate (constitution, Fixed Technical Decisions). Plugins never see that
  choice. How the tests reach a keychain on each CI system is decided in the plan.
- The vault is security core: every change to it gets line-by-line review (Principle IX), as does
  every change to the catalog's credential declarations.
- GitHub offers two kinds of sign-in client. A classic one can read private repositories only with
  access that also writes to all of them, so the catalog's GitHub connector uses GitHub's other
  kind, whose access is set read-only when the client is registered and limited to the
  repositories the user chooses (FR-003). Both use the same standard sign-in, so the host does not
  depend on the choice. The plan checks how such a client renews tokens from a desktop app, which
  cannot keep a secret; if renewal needs one, the declaration uses tokens that do not expire or the
  device sign-in. Revoking may need a secret too; then disconnecting deletes the credential and the
  tool says the authorization remains with GitHub until the user removes it there (FR-016).
- The project's own GitHub sign-in client is registered before the first release, with the
  project's name (Phase 6). Until then, a developer who wants to try the example against the real
  GitHub registers a client of their own and configures it (FR-017); CI never reaches GitHub.
- The host's check for a returned credential finds it as sent. A service could pass on the
  credential it receives in a form of its own; the destination already holds the credential, so
  this check catches careless reflection, while the binding of credentials to reviewed catalog
  connectors (and, from Phase 3, to owner-verified domains) is what keeps them with their service.
- The numbers (sign-in time limit, key length limit, the bound on service error text) are set in
  the plan, as earlier features did.
- Another process run by the user on the same machine can read the user's keychain entries or the
  host's records, as for the stores of feature 004; that is outside the threat model.
- One person uses the host on a machine, as in features 002 to 004.
- The guide's security row "a credential sent to a local address without a valid or accepted
  certificate" is tested with local-network connectors, the feature right after this one; here a
  local development connector carrying a credential over plain HTTP or to an invalid certificate
  is refused (FR-008, FR-021).
- Out of scope: host dialogs for connecting accounts and the cross-account warning (Phase 3);
  connectors from repositories and the sign-in of authors' own services on verified domains
  (Phase 3; the format leaves room for them); local-network connectors that need a credential and
  certificates accepted at first use (after this feature); AI provider keys (Phase 4); several
  accounts for one connector; sign-in procedures other than the authorization code with proof key
  and, if needed, the device sign-in; turning a widget's account use on and off apart from its
  connector; registering the project's GitHub client (Phase 6); sync, backup or export of the
  vault; the TypeScript example (its own Phase 2 item).
