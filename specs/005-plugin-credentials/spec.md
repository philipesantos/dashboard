# Feature Specification: Plugin Credentials

**Feature Branch**: `005-plugin-credentials`

**Created**: 2026-10-24

**Status**: Draft

**Input**: User description: "Feature 005 — secrets vault, OAuth and connector credentials (Phase
2 of the guide; gate case: an attempt to obtain the token must be blocked, the last open case of
the Phase 2 gate; constitution v2.4.0, Principles II, IV, VII, VIII, IX; fixed decision: the
`keyring` crate, OS keychain). A connector may now declare the credential its operations need,
replacing 003's refusal (FR-017): an OAuth account (authorization code with PKCE in the system
browser, first provider GitHub) or a key the user enters. The declaration names the destination it
is for, the scopes and the header it travels in, and it is shown at approval as part of the
connector's grant, so an update that adds a credential or widens its scopes requires a new
approval (narrowing does not). Credentials live only in the OS keychain through `keyring`; host
records hold only which account is connected (connector, login, scopes, date), never a token, and
no token is ever written to the app's files, the request log, kept data, storage, error messages
or terminal output. The host injects the credential only into that connector's requests to the
declared destination, over HTTPS with a certificate valid for it, and only into the declared
header: never into a path, query or body, never into a value a plugin parameter can name or build,
and never past a redirect (already refused). Plugins never receive a credential: no import, file,
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

## Clarifications

### Session 2026-10-24

- Q: When a development connector's file is moved or deleted, should the tool still list its
  account and let the maintainer disconnect it? → A: Yes: the tool lists every account with its
  connector id and origin, marks it when the origin's file no longer exists, and disconnects by id
  and origin whether or not that file exists (FR-014).
- Q: When the maintainer connects a connector that already has an account, may the tool switch to
  a different account, or must the current one be disconnected first? → A: Connecting again is
  allowed only when the service reports the same account (to reconnect or to hold more access); a
  different account, or a new key, is refused, naming the current account, until it is
  disconnected (FR-014).
- Q: For each call that carried a credential, should the request log record the name of the
  account it acted as, or only that a credential was sent? → A: The account's name, escaped when
  shown, or that it was a key; never the credential (FR-007).

### Session 2026-10-25 (planning)

- Q: GitHub's browser-redirect flow needs a client secret even with the proof key, so GitHub
  works only with its device flow (a code the user types at GitHub). Should this feature build
  only the device flow, or also the browser-redirect flow? → A: Both: the device flow, which the
  GitHub connector uses, and the browser-redirect flow with proof key, for connectors that declare
  it, tested with the test sign-in service and development connectors (FR-009, FR-015, FR-019).
- Q: Constitution v2.4.0 fixed "the `keyring` crate", whose project now publishes its API as
  `keyring-core` and each store as a crate of its own. → A: Amended to v2.4.1: the fixed decision
  names keyring-rs, `keyring-core` and its store crates (Assumptions).

## User Scenarios & Testing *(mandatory)*

The people using this feature are the maintainer, who connects accounts and approves widgets
through the command-line tool; widget authors; connector authors; and the project, which keeps the
catalog. End users make the same decisions later, in the host dialogs of Phase 3.

A credential is what a service needs to know who is calling: an account the user connects by
signing in to the service in their browser, or a key the user types. The connector says which
credential its operations need; the host keeps it in the operating system's keychain and adds it
to that connector's requests itself. A widget asks for data through the connector as before and
never holds the credential, so it cannot copy it, send it elsewhere or keep it.

### User Story 1 - A widget shows private data through the user's account (Priority: P1)

The maintainer connects their GitHub account to the catalog's GitHub connector with the tool:
the tool opens the system browser, the maintainer signs in at GitHub and picks the repositories
the connector may read, and the tool reports the account as connected, with its name and the
access it holds. An author's widget declares that it uses the connector's account. The maintainer
asks what the widget requests and sees that it acts as the connected account, with that access,
and approves it. When the widget updates, its calls reach GitHub with the account, and its render
shows releases of a private repository. The widget never receives the token.

**Why this priority**: this is Principle IV made concrete: widgets reach a person's own data
without ever seeing their credentials. Without it, connectors only reach public data.

**Independent Test**: with the test sign-in service, connect an account, approve the example
widget, run an update and a render, and check that the private releases appear, that the request
log shows each call as made with the account, and that no token appears in the widget's output,
the host's files or the tool's output.

**Acceptance Scenarios**:

1. **Given** no account is connected, **When** the maintainer connects one to a connector through
   the tool, **Then** the system browser opens at the service's sign-in page, and after the
   maintainer signs in and accepts, the tool reports the account connected, naming the connector,
   the account and the access it holds.
2. **Given** a widget whose manifest says it uses a connector's account, **When** the maintainer
   asks what it requests, **Then** the tool lists, for that connector, the credential's kind, the
   destinations it is sent to, the access it asks for, whether it is required or optional, and
   the account connected now or that none is.
3. **Given** the widget is approved and an account is connected, **When** an update calls an
   operation, **Then** the request reaches the destination with the credential, the widget gets
   the response, and the request log records the call with the name of the account it acted
   as, never the credential itself.
4. **Given** a connector that takes a key instead of an account, **When** the maintainer connects
   it, **Then** the tool reads the key from the terminal without showing it, and calls carry the
   key as they would a token.
5. **Given** a connector whose credential is optional and no account connected, **When** an
   approved widget calls it, **Then** the request goes without a credential, as in feature 003.
6. **Given** a widget that does not say it uses the connector's account, **When** it calls the
   connector while an account is connected, **Then** the request goes without a credential.

---

### User Story 2 - No plugin can obtain a credential (Priority: P1)

The maintainer runs hostile widgets that each try to get hold of a credential: one looks for it
in its environment, its arguments, the file system and every capability it has; one calls a
destination that sends back the credential it received; one tries to start a sign-in or open the
browser; one is a file that copies an approved widget's id. Each finds nothing or is refused, and
after every case no credential appears in any file the host writes, in the request log, in kept
data, in storage or in what the host and the tool print. The cases run automatically on Windows,
macOS and Linux and break the build if any one is not contained.

**Why this priority**: this is the last case of the Phase 2 gate, "an attempt to obtain the token
is blocked", and every security rule needs a test that breaks the build (Principle VIII).

**Independent Test**: run the hostile suite with an account connected through the test sign-in
service; every new case is contained, a scan of the host's files, the development data directory,
the request log, kept data, storage and the captured output finds no credential, and a
well-behaved widget updates and renders normally right afterwards.

**Acceptance Scenarios**:

1. **Given** an account is connected, **When** a widget looks for a credential in its environment
   variables, arguments, configuration, the file system or any capability, **Then** it finds none:
   the host gives it no credential anywhere, and a component that asks for file system access is
   refused before running.
2. **Given** a destination that returns, in its body or in a header the operation declares, the
   credential its request carried, **When** the widget calls it, **Then** the widget receives no
   part of that response, the call fails as an invalid response, and the log records why.
3. **Given** any widget, **When** it calls a connector whose account is not connected, **Then** the
   call answers that the account is not connected, and no browser opens and no sign-in starts.
4. **Given** every hostile case, **When** it has run, **Then** no credential, sign-in code or
   sign-in secret appears in the host's records, the development data directory, the request log,
   kept data, storage or the captured output of the host and the tool.
5. **Given** any hostile case was just contained, **When** a well-behaved widget updates and
   renders next, **Then** it produces its normal result; the cases of features 001 to 004 still
   pass.

---

### User Story 3 - A credential goes only where its connector sends it (Priority: P1)

The maintainer runs hostile widgets and connectors that each try to make the host send a
credential somewhere else: a connector with a second destination the credential is not for;
values that try to add a header or name the credential; a development connector without HTTPS,
or with an invalid certificate; a widget that was not approved for the account, or whose
connector is turned off; a redirect to another host. The credential reaches only the destinations
it is for, over HTTPS with a valid certificate, in the header the connector declares, and nothing
else.

**Why this priority**: Principle IV binds a credential to its connector's destination; if the host
can be steered to send it elsewhere, a widget gets the credential without ever seeing it.

**Independent Test**: run these cases against test services that record what they receive; only
the destinations the credential is for receive it, and only in the declared header.

**Acceptance Scenarios**:

1. **Given** a connector with two destinations and a credential for one, **When** a widget calls
   an operation on the other, **Then** the request goes without the credential.
2. **Given** values with line breaks, header names or text that names the credential, **When** a
   widget passes them, **Then** each is refused by its type or sent as plain data in its place,
   never as a header and never replaced by the credential.
3. **Given** a development connector over plain HTTP that declares a credential, **When** it is
   loaded, **Then** it is refused, naming that a credential travels only over HTTPS; **given** one
   whose destination's certificate is not valid for it, **When** a widget calls it, **Then** the
   call fails before any header is sent, and the credential never leaves the host.
4. **Given** a widget without a grant for the connector, with an optional connector turned off, or
   without approval to use the account, **When** it calls the connector, **Then** no request carries
   the credential.
5. **Given** a request that carried a credential is redirected, **When** the target is a host the
   credential is for, **Then** the redirect is followed with it; **when** the target is another
   host of the connector, **Then** it is followed without the credential; any other target is not
   followed, as in feature 003.
6. **Given** a development connector file elsewhere that claims the id of a connector with a
   connected account, **When** a widget uses it, **Then** it has no account.

---

### User Story 4 - The maintainer manages accounts (Priority: P2)

The maintainer lists the connected accounts, each with its connector, account name, access held
and when it was connected, every text escaped so nothing a service returned can disguise the
output. Tokens that expire are refreshed by the host; when an account can no longer be used,
because the maintainer revoked it at the service or it expired past refresh, widgets are told the
account is not connected and the tool shows it needs connecting again. The maintainer disconnects
an account: its credential leaves the keychain, the service is asked to revoke it where it offers
that, and widgets stop using it from their next call. A sign-in that someone forges or replays
connects nothing.

**Why this priority**: accounts must be visible and removable at any time, and a sign-in is a
place where an attacker on the same machine could slip in their own account or steal a code.

**Independent Test**: connect an account with the test service; list it; let the test service
expire and then revoke the token; check refresh, then the "not connected" answer; disconnect and
check that the keychain entry is gone and the test service received the revocation; send forged
and replayed callbacks during a sign-in and check that nothing is connected.

**Acceptance Scenarios**:

1. **Given** connected accounts, **When** the maintainer lists them, **Then** the tool shows each
   connector, origin, account name, access asked for and held, when it was connected and whether it
   needs connecting again, escaping every text, and never a credential.
2. **Given** a token that has expired and can be refreshed, **When** a widget calls the connector,
   **Then** the host refreshes it and the call goes through, without the widget seeing either step.
3. **Given** a token that the service refuses, or that cannot be refreshed, **When** a widget calls
   the connector, **Then** the call answers that the account is not connected, the host retries no
   further, and the tool lists the account as needing connecting again.
4. **Given** a connected account, **When** the maintainer disconnects it, **Then** its credential is
   deleted from the keychain, the service is asked to revoke it when it offers that (the tool says
   whether revocation succeeded), widgets that used it lose the data the host kept from them, and
   their next calls go without the account or answer that it is not connected.
5. **Given** a sign-in in progress, **When** a callback arrives with a wrong or missing state, a
   code already used, an error, or after the sign-in's time limit, **Then** the sign-in ends with
   nothing connected and any account connected before stays as it was.
6. **Given** an account name or a service's error text with line breaks or terminal escapes,
   **When** the tool prints it, **Then** it is escaped and cannot pass for the tool's own output.
7. **Given** an account whose development connector file was moved or deleted, **When** the
   maintainer lists accounts, **Then** it appears with its connector id and origin, marked as having
   no file; **when** the maintainer disconnects it by id and origin, **Then** its credential is
   deleted from the keychain and no other account changes.
8. **Given** a sign-in through a device code, **When** the user denies it at the service, the code
   expires, or the service asks the host to poll more slowly, **Then** a denial or an expiry ends
   the sign-in with nothing connected, and the host polls no faster than the service allows.

---

### User Story 5 - Changes never widen account access silently (Priority: P2)

The maintainer loads new versions of widgets and connectors. A connector version that adds a
credential, asks for more access, sends the credential to a new destination or in another header
does not run for widgets that use the account until they are approved again; a widget version
that starts using the account does not run until approved. When a connector asks for more access,
the account must also be connected again, since the credential held was given for less.
Disconnecting, removing an approval or turning a connector off while an update runs takes effect
from the widget's next call.

**Why this priority**: an update that quietly lets a widget act with more of the user's access is
the escalation feature 002 forbids, applied to accounts.

**Independent Test**: approve a widget that uses an account; load a connector version that asks
for more access (refused until approved; once approved, calls answer "not connected" until the
account is connected again); load one that asks for less (runs); disconnect during an update and
check that its response does not reach the widget and that no later request carries the
credential.

**Acceptance Scenarios**:

1. **Given** an approved widget that uses a connector's account, **When** a new version of the
   connector adds a credential, asks for more access, sends the credential to another destination
   or in another header, **Then** the widget is refused until approved, naming what changed, and
   the grants already given stay as they were.
2. **Given** an approved widget, **When** a new version of the widget starts using the connector's
   account, **Then** it is refused until approved.
3. **Given** a connector version that asks for less access, or a widget version that stops using
   the account, **When** it loads, **Then** it needs no new approval.
4. **Given** a connector that now asks for more access than the account holds, **When** a widget
   calls it after approval, **Then** the call answers that the account is not connected until the
   maintainer connects it again.
5. **Given** an update in progress that uses an account, **When** the maintainer disconnects the
   account, removes the widget's approval or turns its connector off, from any process, **Then** a
   request already sent may finish but its response does not reach the widget, and no later
   request carries the credential.

---

### User Story 6 - Authors build on contract 1.3.0 (Priority: P3)

Connector authors declare a credential with the published connector schema; widget authors
declare that they use a connector's account and handle the new answers, "account not connected"
and "subscription required", building against contract 1.3.0. Widgets built for 1.0.0, 1.1.0 and
1.2.0 keep running and never receive the new answers. The manifest's old list of login providers
no longer means anything.

**Why this priority**: credentials are useful only if authors can declare them correctly, and
existing widgets must not break when the contract grows.

**Independent Test**: validate the catalog connector and the example widget against the schemas
and run the example; run the existing 1.0.0, 1.1.0 and 1.2.0 plugins on the new host; check that a
widget that uses an account while built for 1.2.0 is refused naming 1.3.0.

**Acceptance Scenarios**:

1. **Given** the published schemas and documentation, **When** an author writes a connector with a
   credential and a widget that uses it, like the example, **Then** they validate and the example
   runs.
2. **Given** a connector whose credential is declared wrongly (a destination it does not declare, a
   header that is not allowed, sign-in addresses on hosts it does not reserve or declare), **When**
   it is checked, **Then** the error names the field and the problem.
3. **Given** plugins built for contract 1.0.0, 1.1.0 and 1.2.0, **When** they run on this host,
   **Then** they work as before; **given** a widget built for 1.2.0 or older that says it uses an
   account, **When** it is loaded, **Then** it is refused, naming 1.3.0.
4. **Given** a service that refuses a call for lack of a paid plan, with a response the connector
   declares as meaning that, **When** a widget built for 1.3.0 calls it, **Then** the call answers
   "subscription required".
5. **Given** a manifest that still lists login providers, **When** it is loaded, **Then** it is
   valid, the tool shows the list as obsolete, and it grants nothing.

### Edge Cases

- A connector with a required credential and no account connected: every call answers that the
  account is not connected and nothing is sent.
- A connector with an optional credential whose account needs connecting again: calls from widgets
  that use the account answer that the account is not connected rather than go without it, so the
  widget never shows public data as if it were the user's.
- The service grants less access than the connector asked for (the user unticked something or
  picked fewer repositories): the account is connected with what was granted; the tool shows both.
  Calls the service refuses get the service's answer, as any error status does.
- A connector version asks for less access than the account holds: no new approval and no new
  sign-in; the tool shows that the account holds more than the connector asks for and offers to
  connect it again.
- The maintainer signs in again to a connector that has an account: when the service reports the
  same account, the new credential replaces the old one and widgets keep acting as that account;
  when it reports a different account, or the connector cannot tell which account it is, the new
  credential is discarded (revoked when the service offers that), the tool names the current
  account and says to disconnect it first, and nothing changes. A new key is refused the same way
  while one is stored. Several accounts for one connector are out of scope.
- Connecting an account grants nothing to any widget, and approving a widget connects nothing:
  the two are separate actions.
- Two widgets use the same connector's account: each needs its own approval to use it; the
  connector's rate limits are shared, as in feature 003.
- The keychain is locked, missing or refuses access: connecting fails naming the cause, and a call
  that needs the credential fails as unavailable, naming the keychain; it never goes out without
  the credential when the credential is required, or when the account is connected.
- The sign-in page never calls back: the sign-in ends at its time limit with nothing connected,
  and the loopback address it listened on is closed.
- The browser cannot be opened: the tool prints the sign-in address, escaped, for the maintainer to
  open by hand.
- A sign-in address, a token address or a revocation address of a connector is on a host the
  connector neither reserves (catalog) nor declares (development): the connector is refused.
- A connector fixes the header its credential travels in, or a value in it: refused, since the
  host alone writes that header.
- A development connector, outside the catalog, puts its credential in a header other than
  `Authorization`: refused; only a catalog connector, whose review checks it, may use another
  header.
- A key typed by the maintainer is empty, too long or holds characters a header cannot carry:
  refused, naming the rule, and nothing is stored.
- The service's response to a sign-in or a refresh holds an error text: the tool shows the error
  code and text escaped and cut, never the code, token or secret of the exchange.
- A widget's kept data from before an account was connected: kept; data fetched with an account is
  discarded when the account is disconnected.
- The host is stopped during a sign-in: nothing is connected, and no code or secret of the sign-in
  is left on disk.
- A widget built for 1.2.0 or older uses a connector that now declares an optional credential: it
  keeps running and its calls go without the credential, since it cannot declare that it uses the
  account; one that declares a required credential is refused for such a widget, naming 1.3.0.
- A connector from a repository that declares a credential: refused as not verified, as every
  connector from a repository is until Phase 3.
- No GitHub app is configured for development: connecting the GitHub connector fails, naming how to
  configure one; widgets that use GitHub without an account work as before.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: A connector MAY declare one credential, replacing feature 003's refusal (its FR-017):
  an account the user connects by signing in to the service, or a key the user enters. The
  declaration MUST state its kind; whether it is required or optional; which of the connector's
  destinations it is sent to; the header it travels in; for an account, the service's sign-in,
  token and, if any, revocation addresses, the access it asks for, each item named, and, when the
  service offers one, an operation that tells the account's name; and which responses, if any,
  mean that the account lacks a paid plan. The sign-in, token and revocation addresses MUST be on
  hosts the connector reserves (catalog) or declares (development). The host MUST refuse a
  credential declaration that does not conform, naming the field and the problem.
- **FR-002**: A credential MUST travel in the `Authorization` header, with the scheme the
  connector states; only a catalog connector MAY use another header, checked by its review. No
  connector MAY fix that header or any value in it, and no value a connector fixes or a widget
  fills MAY name, contain or be replaced by a credential.
- **FR-003**: A widget's manifest MUST state, for each connector it declares, whether it uses that
  connector's account. A widget that does not MUST have its calls sent without a credential, even
  when an account is connected. A widget that declares a connector with a required credential MUST
  declare that it uses the account, or be refused at load, naming the connector.
- **FR-004**: The account MUST belong to the connector, identified by its id and origin (for a
  development connector, the canonical path of its file, as in feature 003), not to a widget:
  every widget whose approval includes using the account acts as the one account connected to
  that connector. Connecting an account MUST grant nothing to any widget, and approving a widget
  MUST connect nothing.
- **FR-005**: Before approval, the host MUST show, for every connector with a credential that a
  widget declares, the credential's kind, whether it is required or optional, the destinations and
  header it is sent in, the access it asks for, whether the widget uses it, and the account
  connected now or that none is. The approval MUST cover the credential declaration and whether
  the widget uses it, as shown, in the digest of feature 003; it does not cover which account is
  connected. A new version of a connector that adds a credential, asks for more access, sends the
  credential to another destination or in another header, or a new version of a widget that
  starts using an account, MUST be refused until approved, naming what changed, with the grants
  already given unchanged. Asking for less access, or no longer using the account, MUST NOT need a
  new approval.
- **FR-006**: The host MUST keep credentials (tokens, refresh tokens and keys) only in the
  operating system's keychain, and MAY hold them in its own memory while it runs. The host's
  records MUST hold only which account is connected: connector, origin, kind, account name, access
  asked for and held, when it was connected and refreshed, and whether it needs connecting again.
  No credential, sign-in code or sign-in secret MAY be written to any file the host writes, the
  request log, kept data, storage, an error message or the output of the host or the tool, nor
  sent anywhere but under FR-007 and FR-009.
- **FR-007**: The host MUST add a credential only to requests of the connector it belongs to, to a
  destination the credential is for, from widgets whose approval includes using it, through the
  proxy of feature 003: HTTPS with a certificate valid for the destination, the exact host, no
  non-public address. It MUST add it in the declared header only, after building the request from
  the operation, and never in a path, query or body. A redirect MUST carry the credential only to a
  host it is for; to another host of the connector, the redirect is followed without it, and any
  other target is not followed, as in feature 003. A connector that declares a credential MUST be
  refused when any destination the credential is for is not HTTPS, including a development
  connector on a loopback address without HTTPS. The request log of feature 003 MUST record, for
  each call that carried a credential, the name of the account it acted as, or that it was a key,
  escaped when shown, and never the credential.
- **FR-008**: Plugins MUST NOT receive a credential by any means: no capability, environment
  variable, argument, configuration value or file carries one. The host MUST withhold from the
  widget any response whose body or declared headers contain the credential its request carried,
  answering an invalid response and recording why in the log.
- **FR-009**: Only the host's own controls MAY start a sign-in, in this phase the development tool;
  neither a widget nor its output MAY connect, reconnect or disconnect an account, start a sign-in
  or open a browser (Principle VII). A connector's account declares one of two sign-in flows,
  both run in the system browser and neither using a client secret. With the authorization code
  and proof key (PKCE), the callback reaches the host on a loopback address at a port chosen for
  that sign-in, which the host closes after the first callback or at the sign-in's time limit; the
  host MUST check the callback's state, and a callback with a wrong or missing state, a code
  already used, an error, or one arriving after the time limit MUST end the sign-in with nothing
  connected and any earlier account unchanged. With a device code, the tool MUST show the code and
  the service's address only in its own output, poll no faster than the service's interval, slow
  down when the service asks, and end the sign-in with nothing connected when the user denies it,
  the code expires or the time limit passes. The host's own requests for sign-in, refresh and
  revocation MUST follow the proxy's rules and appear in the request log as the host's, without
  any credential, code or secret.
- **FR-010**: The development tool MUST read a key from the terminal without showing it, never from
  an argument, an environment variable or a file, and MUST refuse a key that is empty, longer than
  a limit, or holds characters a header cannot carry.
- **FR-011**: The host MUST refresh an expired token itself, at most once per call. When no account
  is connected, or the account cannot be used (refresh failed, or the service refused a request
  that carried the credential as unauthorized), the call MUST answer "account not connected", the
  host MUST NOT retry further, and the account MUST be marked as needing connecting again. A call
  from a widget that uses the account MUST go without a credential only when the credential is
  optional and no account is connected.
  A call to a connector that asks for more access than the account holds MUST answer "account not
  connected" until the account is connected again.
- **FR-012**: When a response is one the connector declares as meaning the account lacks a paid
  plan, the widget MUST receive "subscription required" and no part of the response.
- **FR-013**: Disconnecting an account, removing a widget's approval or turning off its connector,
  from any process, MUST take effect from the widget's next call, even within an update already
  running; a request already sent MAY finish within its time limit, but its response MUST NOT
  reach the widget, and no later request MAY carry the credential. Disconnecting an account
  MUST discard the data the host kept from every widget that used it.
- **FR-014**: The development tool MUST connect an account (sign-in in the system browser, or a
  key read under FR-010), list every account in the development vault (connector id, origin,
  kind, account name, access asked for and held, when connected, whether it needs connecting
  again, and whether the origin's file still exists), and disconnect one by connector id and
  origin, whether or not that file exists, deleting its credential from the keychain and asking
  the service to revoke it when it offers that, saying whether revocation succeeded (or that it
  could not be asked because the connector's file is gone). It MUST escape every text a service
  or a connector wrote, and never print a credential. Connecting a connector that already has an
  account MUST succeed only when the service reports the same account, the new credential then
  replacing the old; a different account, an account the connector cannot identify, or a new key
  while one is stored MUST be refused, naming the current account, with the new credential
  discarded and nothing changed, until the maintainer disconnects the current one. In this
  phase, accounts live in a development vault, in the operating system's keychain, and their
  records in the development data directory, which installed plugins never use (constitution
  v2.4.0); accounts for installed plugins arrive with installation in Phase 3.
- **FR-015**: The catalog's GitHub connector MUST move to version 1.1.0 with an optional account
  that asks only for read access, to the repositories the user picks at GitHub, so that the
  approval never shows write access for a widget that only reads: a GitHub App with read access
  to contents and metadata, signed in with GitHub's device flow, the only one GitHub completes
  without a client secret; the user picks the repositories by installing the app on them at
  GitHub, at an address the tool gives. GitHub offers no revocation to a client without a
  secret, so disconnecting says where to revoke the app at GitHub. A catalog connector's review
  MUST check that the access its credential asks for is the narrowest the service offers for its
  operations. The app's identity is configured for development by the developer, never by a
  plugin; the project registers its own app before the first public release (Phase 6).
- **FR-016**: The manifest's list of login providers (feature 002) MUST become obsolete: a
  manifest that has it stays valid, the tool shows it as obsolete, and it grants nothing; the
  accounts a widget uses come only from the connectors it declares (FR-003).
- **FR-017**: Contract v1 MUST move to version 1.3.0, adding the answers "account not connected"
  and "subscription required" to connector calls; version 1.2.0 MUST first be kept unchanged as a
  record, and a test MUST fail the build if 1.3.0 does more than add to 1.2.0. The host MUST NOT
  give either answer to a widget built for an older minor version, which cannot declare that it
  uses an account. A widget that declares that it uses an account MUST be built for 1.3.0 or
  later, or be refused, naming the version needed. The host MUST run plugins built for any v1
  minor version up to its own as before, and refuse one built for a newer minor version as a
  contract mismatch, naming both versions.
- **FR-018**: Every refusal and failure MUST name, in one message, the connector, widget, account,
  field or limit concerned and the rule that applied: account not connected, subscription
  required, invalid response (a credential sent back), unavailable (the keychain); when a
  connector or widget loads or is approved: invalid connector (credential declaration, header,
  sign-in addresses), credential needs HTTPS, account use not declared, contract too old, needs a
  new approval (credential, access, destination or header), approval changed; and in the tool:
  sign-in failed (state, code, time limit, the service's error), another account connected, key
  refused, keychain unavailable, no app configured.
- **FR-019**: The hostile suite MUST gain one case per rule, each run with an account connected
  through the test sign-in service: looking for the credential (FR-008), a destination that sends
  it back (FR-008), a second destination (FR-007), values that try to add a header or name the
  credential (FR-002), a development connector without HTTPS or with an invalid certificate
  (FR-007), a widget without a grant, with its connector off, or not approved for the account
  (FR-003, FR-007), a file that copies an approved widget's or connector's id (FR-004), a version
  that adds a credential or asks for more access (FR-005), forged and replayed callbacks, and a
  denied, expired or slowed-down device code (FR-009),
  a widget trying to start a sign-in (FR-009), a revoked and an expired token (FR-011), a
  disconnect during an update (FR-013), and a spoofed account name (FR-014). After every case, a
  scan MUST find no credential, code or secret in the host's records, the development data
  directory, the request log, kept data, storage or captured output (FR-006). The suite MUST run
  on Windows, macOS and Linux on every change that is not documentation-only, without the real
  GitHub, and fail the build if any case is not contained (Principle VIII).
- **FR-020**: The project MUST ship the GitHub connector's account, the example widget
  `github-releases` showing private releases once an account is connected, the connector and
  manifest schemas' new fields, and documentation for widget authors (using an account, the new
  answers) and connector authors (declaring a credential, the narrowest access, testing with a
  development app).
- **FR-021**: The security documents MUST describe the vault and accounts and their rules:
  `SECURITY.md`, the threat model, the contracts' `README`, the plugin author guide, the
  connectors' `README` and the hostile suite's `README`.

### Key Entities

- **Credential**: what a service needs to know who calls: a token from an account, or a key; kept
  only in the keychain and added by the host to its connector's requests.
- **Credential declaration**: what a connector says about its credential: kind, required or
  optional, destinations, header, sign-in addresses, the access it asks for, how to learn the
  account's name, and which responses mean a paid plan is missing.
- **Account**: the user's identity at a service, connected to one connector; it has a name, the
  access it holds, when it was connected and whether it needs connecting again.
- **Access**: what the account lets the connector do at the service (scopes or permissions), shown
  before approval and before sign-in.
- **Account use**: what a widget's manifest says about a connector's account: whether the widget
  acts as it; approved with the connector.
- **Vault**: the keychain entries where the host keeps credentials; in this phase, the development
  vault.
- **Sign-in**: one run of the browser flow that connects an account, with its state, proof key,
  loopback callback and time limit.
- **Contract 1.3.0**: contract v1 with the answers "account not connected" and "subscription
  required"; the host still runs plugins built for 1.0.0, 1.1.0 and 1.2.0.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: 100% of the new hostile cases are contained on all three operating systems,
  alongside the cases of features 001 to 004, and after every case the scan of FR-019 finds no
  credential, code or secret anywhere.
- **SC-002**: For every combination of declared, approved, turned-on and used account, connected or
  not, and every kind of destination, a request carries a credential only when the connector
  declares it, the widget declares and was approved to use the account, the connector is on, an
  account is connected, and the destination is one the credential is for, over HTTPS with a valid
  certificate, verified by tests.
- **SC-003**: Every refusal tells the maintainer, in one message, what is concerned and the rule,
  without reading source code or host logs.
- **SC-004**: A forged, replayed or late callback, and a denied or expired device code, connect
  nothing, every time.
- **SC-005**: Disconnecting an account, removing an approval or turning a connector off takes effect
  from the widget's next call, every time, and no later request carries the credential.
- **SC-006**: Adding a credential to a call adds less than 5 ms once the host has read it from the
  keychain, on every CI system; the first read and a refresh are recorded in the feature's
  performance record.
- **SC-007**: Every plugin built for contract 1.0.0, 1.1.0 or 1.2.0 in the repository passes
  unchanged on the new host, and a plugin built for a newer minor version is refused every time.
- **SC-008**: The catalog connector and the example widget validate against the published schemas
  in CI, the example shows private releases end to end with the test sign-in service, and the
  quickstart walks the maintainer through the same with a real GitHub account.

## Assumptions

- In this phase the maintainer connects accounts and approves widgets with the command-line tool
  (constitution v2.4.0, Principle VII). The host dialogs of Phase 3 apply the same rules, and the
  Phase 3 warning about data from one connected account reaching another destination judges the
  accounts and destinations this feature shows.
- The fixed technical decision applies: credentials live in the operating system's keychain
  through keyring-rs, `keyring-core` and its store crates (constitution v2.4.1). Plugins never
  see that choice. How CI reaches a keychain on each system, and whether macOS or Linux asks the
  user for access, is decided in the plan.
- A classic GitHub OAuth app can read private repositories only with full access to all of them,
  writing and deleting included; GitHub's other kind of app can ask for read-only access to the
  repositories the user picks, with tokens that expire after eight hours and are refreshed. For
  both kinds, GitHub's browser-redirect flow requires a client secret at the code exchange, even
  with the proof key; its device flow requires none, and neither does refreshing a token it gave
  (research R1).
- The response check of FR-008 catches a credential sent back as it was received. A destination
  that encodes it on purpose already holds it, since the credential is its own; that is why only
  reviewed catalog connectors and, from Phase 3, owner-verified domains carry credentials.
- The numbers (sign-in time limit, key length, the limits on refresh) are set in the plan; the
  costs are measured on the three systems, as in features 001, 003 and 004.
- The vault is security core (Principle IX): every change to it gets line-by-line review, as the
  catalog does.
- Another process of the same user on the machine may be able to read the keychain entries; that
  is outside the threat model, as for the host's other records.
- The security table's case of a credential sent to a local address without a valid or accepted
  certificate arrives with the catalog connectors for local services, right after this feature; in
  this feature a credential never goes to a loopback address without HTTPS (FR-007).
- One person uses the host on a machine, as in features 002 to 004.
- Out of scope: host dialogs for connecting accounts and the cross-account warning (Phase 3);
  sign-in for authors' own services on verified domains, including paid ones (Phase 3, with
  repository connectors; the credential declaration already allows it); catalog connectors for
  local services and certificates accepted at first use (right after this feature); AI provider
  keys (Phase 4); several accounts for one connector; turning a widget's use of an account on and
  off apart from its connector; flows other than the authorization code with proof key and the
  device flow; registering the project's GitHub app (Phase 6); sync or export
  of the vault; the TypeScript example (its own Phase 2 item).
