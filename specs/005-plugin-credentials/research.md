# Research: Plugin Credentials

Decisions behind [plan.md](plan.md), each with what it rests on. GitHub's behavior comes from its
documentation, read on 2026-10-24 (each claim the plan rests on checked again in the raw article);
`keyring-rs` comes from its published sources and a spike on Linux on 2026-10-24, outside the
repository. The plan's first tasks repeat on the three systems the parts that matter.

## R1. GitHub: no account in this feature

**Decision** (planning session): the catalog's GitHub connector stays 1.0.0, public data without
an account. GitHub cannot sign in a client that keeps no secret except through its device flow,
which the maintainer left out, so its account waits for a later feature, after the project
registers its GitHub App (Phase 6) and decides how a public client signs in to GitHub.

What GitHub documents:

| Question | Answer | Source |
| --- | --- | --- |
| Read-only private access with a classic OAuth app | None: `repo` "Grants full access to public and private repositories including read and write access to code" | [Scopes for OAuth apps](https://docs.github.com/en/apps/oauth-apps/building-oauth-apps/scopes-for-oauth-apps) |
| Releases with a GitHub App | "Contents" repository permission (read); `GET /repos/{owner}/{repo}` is under "Metadata" | [Permissions required for GitHub Apps](https://docs.github.com/en/rest/authentication/permissions-required-for-github-apps) |
| What a user token reaches | "only access resources that both the user and app can access"; the user picks repositories when installing the app ("Only select repositories") | [Generating a user access token](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/generating-a-user-access-token-for-a-github-app), [Installing a GitHub App](https://docs.github.com/en/apps/using-github-apps/installing-a-github-app-from-a-third-party) |
| Web flow without a secret | Not possible: at the code exchange `client_secret` is "**Required.**", and `code_verifier` only "Strongly recommended", for GitHub Apps and OAuth apps alike; PKCE is supported, S256 only, since 2025-07-14 | Same page; [changelog](https://github.blog/changelog/2025-07-14-pkce-support-for-oauth-and-github-app-authentication/) |
| Device flow | Polls with `client_id`, `device_code`, `grant_type` only; must be enabled in the app's settings; code valid 900 s, interval 5 s, `slow_down` adds 5 s; "desktop applications should use the device flow" | Generating a user access token |
| Refresh | Token 8 h, refresh token 6 months, both rotate on use; `client_secret` "**Required** unless the user access token was generated using the device flow" | [Refreshing user access tokens](https://docs.github.com/en/apps/creating-github-apps/authenticating-with-a-github-app/refreshing-user-access-tokens) |
| Revocation | `DELETE /applications/{client_id}/token` needs Basic auth with the secret; the user revokes at Settings → Applications → Authorized GitHub Apps | [OAuth applications API](https://docs.github.com/en/rest/apps/oauth-applications), [Reviewing GitHub Apps](https://docs.github.com/en/apps/using-github-apps/reviewing-and-revoking-authorization-of-github-apps) |
| Header and identity | `Authorization: Bearer <token>`; `GET /user` "does not require any permissions" and returns `login` and `id` | [Users API](https://docs.github.com/en/rest/users/users) |
| Statuses | A private repository the token cannot see answers 404; invalid credentials answer 401 first, then 403 for a while | [Troubleshooting](https://docs.github.com/en/rest/using-the-rest-api/troubleshooting-the-rest-api), [Authenticating](https://docs.github.com/en/rest/authentication/authenticating-to-the-rest-api) |
| Rate limits | 60 requests per hour per address without a token; 5,000 per hour per user with one | [Rate limits](https://docs.github.com/en/rest/using-the-rest-api/rate-limits-for-the-rest-api) |

So no flow under FR-009 completes against GitHub: the browser-redirect flow needs the secret
at the code exchange (and at refresh), and the device flow, the one GitHub recommends for desktop
apps, is what GitHub's best practices warn about: "an attacker can use the device flow to remotely
impersonate your app as part of a phishing attack", since the client id is public and anyone can
request a code under the app's name. When the GitHub account comes, FR-015's narrow access needs
a GitHub App, not a classic OAuth app.

**Alternatives considered**: the device flow, with a warning to enter only a code the tool showed
(planned first, then removed by the maintainer); the browser-redirect flow with the project's
secret in the binary, as GitHub's best practices suggest for public clients (the secret is then
public, and GitHub Apps document no run-time loopback port, so the callback port would be fixed);
a classic OAuth app (`repo` is full write access to every repository, against FR-015).

## R2. The sign-in flow

**Decision**: the authorization code with PKCE (RFC 7636) on a loopback callback (RFC 8252), the
one flow of FR-009; a connector's account declares `flow: authorization-code`, so a later flow
adds a value without changing what this one means.

- A loopback listener on `127.0.0.1`, port 0 (the system's choice), redirect address
  `http://127.0.0.1:<port>/callback`; `state` and the verifier are 32 random bytes each, from
  `ring::rand::SystemRandom` (already in the tree), sent base64url without padding; the challenge is
  the S256 of the verifier (`sha2`, already in the tree).
- The listener answers only `GET /callback`: the first such request ends the sign-in, whatever it
  carries; any other path gets `404` and does not count. The page it returns is fixed text, never
  anything from the request, with `Cache-Control: no-store`. The listener closes after that request
  or at the time limit.
- The code is exchanged with `client_id`, `code`, `redirect_uri` and `code_verifier`, never a
  secret.

The sign-in ends with the token response parsed (R10), the account's name learned (R3), the
account compared with the one already connected (clarification 2 of the spec), and only then is
anything written to the vault or the accounts record.

The listener is the only socket the host opens that a peer connects to; it lives in
`runtime/src/network/callback.rs`, and `std::net::TcpListener` joins the types `clippy.toml`
forbids outside `runtime/src/network/` (R19). Request heads are read within 8 KiB and 2 s and
parsed with `httparse`, already in the tree through `ureq-proto`.

**Rationale**: the flow RFC 8252 recommends for native apps, which services that accept public
clients offer, and the one authors' services of Phase 3 will use.

**Alternatives considered**: the device flow (R1); an embedded web view (the user must see the real
browser and its address bar, and Principle V keeps plugins out of host windows anyway);
`localhost` as the redirect host (RFC 8252 §8.3 recommends the literal address).

## R3. The credential declaration

**Decision**: a connector gains one optional field, `credential`
([draft](contracts/connector-credential.schema.json)); `credentials` and `auth` stay refused as feature 003
refused them, now naming `credential` as the field to use.

| Field | Rule |
| --- | --- |
| `kind` | `account` or `key` |
| `required` | `true` or `false` (default `false`) |
| `destinations` | 1–8 keys of the connector's `destinations`. Every destination of a connector with a credential is `https`, not only these (Principle III: only a connector that sends no credential may go without HTTPS) |
| `header` | `Authorization` (outside the catalog, the only value); a catalog connector may name another, which its review checks; never one of the names the host forbids (003, R1). Header names are compared without regard to case, here and against every header an operation fixes |
| `scheme` | Required with `Authorization`: 1–32 characters of a token (RFC 9110), such as `Bearer`; the header is `<scheme> <secret>`. Other headers carry the secret alone |
| `account` | With `kind: account` only: the sign-in (below) |
| `subscription_required` | 0–4 statuses from 400–499 that mean a paid plan is missing (FR-012) |

`account`:

| Field | Rule |
| --- | --- |
| `flow` | `authorization-code` (R2) |
| `client` | The name of the client registration the host uses (R4) |
| `authorize` | Absolute `https` address without credentials or fragment, on a host under a domain the connector reserves (catalog) or the host of one of its destinations (development): the browser goes there, not the credential |
| `token`, `revoke` | Absolute `https` addresses whose origin is one of the credential's `destinations`, so a refresh token or a token being revoked goes only where the credential goes (FR-001); `revoke` optional (RFC 7009) |
| `scopes` | 0–32 scope names sent at sign-in |
| `access` | 1–32 `{ "name", "level" }` items, `level` `read` or `write`: what the account lets the connector do, shown before approval and before sign-in; a catalog review checks it is the narrowest the service offers (FR-015) |
| `identity` | Optional `{ "operation", "name", "id" }`, `id` required within: a read operation of the connector on one of the credential's destinations, `GET` without parameters, called by the host with the new credential, and JSON pointers to the account's name and stable id in its response. Without `identity`, the account is named `account`, has no id, and connecting again is refused until it is disconnected, as for a key |
| `access_page` | Optional `https` address, on the same hosts as above, where the user chooses what the account reaches, printed after sign-in |
| `revoke_page` | Optional `https` address where the user revokes the account by hand, printed when `revoke` is absent |

The header and every value in it belong to the host: a connector that fixes the credential's
header, or a header named like it, is `invalid-connector` (FR-002); 003 already refuses
`Authorization` among fixed headers. A key has no sign-in, no identity and no access beyond what
the service gives it; `access` is then optional and shown when present.

**Rationale**: one declaration covers authors' services, keys and, later, GitHub; every address is
checked against the same reservation rules as destinations, so a connector cannot send the token
exchange, or the user, to a host nobody reviewed.

**Alternatives considered**: OpenID Connect discovery (a request to a document the service
controls decides the endpoints, after approval); scopes alone as the access (some services have
none, such as GitHub Apps; `access` is what the person reads).

## R4. Client registrations

**Decision**: a client registration is `{ "client_id" }`, never a secret. The host binary carries
a built-in table from registration name to registration, empty in this feature (no catalog
connector declares a credential yet). In development, `accounts connect … --client <id>` supplies
one and the accounts record remembers it for that connector, so later refreshes use the same
client. A plugin never supplies or sees one.

**Rationale**: refresh must use the client the token came from; a developer's own app must not be
written into a connector file others may copy.

## R5. The vault

**Decision**: `keyring-core` `=1.0.1` with one store per system, held as an
`Arc<CredentialStore>` built once by the `Runtime` and never through the crate's global default:

| System | Store | Features |
| --- | --- | --- |
| Windows | `windows-native-keyring-store` `=1.1.0` | `default-features = false` (no `search`, which pulls `regex` features) |
| macOS | `apple-native-keyring-store` `=1.0.2` | `keychain` (the login keychain; `protected` needs a provisioning profile) |
| Linux | `dbus-secret-service-keyring-store` `=1.0.1` | `crypto-rust`; **not** `vendored` |

- Entries: service `dev.philsantos.dashboard.development` (the development vault; installed plugins
  will use `dev.philsantos.dashboard` from Phase 3), user `<vault id>:<account key>`, where the
  vault id is 16 random bytes in hex, created with the accounts record, so every data directory,
  including each test's, has entries of its own; the account key is the SHA-256 of the connector
  id and origin.
- The secret is a JSON document (`access`, `refresh`, or `key`) written with `set_secret` and at
  most 2,560 bytes, Windows' `CRED_MAX_CREDENTIAL_BLOB_SIZE`; a token response that would not fit
  is `sign-in-failed`.
- `runtime/src/vault/` is the only code that touches `keyring-core` or a store, enforced by Clippy
  (R19). It hands out a `Secret` (`zeroize::Zeroizing<String>`, already in the tree) whose
  `Debug` prints `[credential]` and which has no `Display`.
- Every vault operation runs on **one vault thread per process**, which takes operations from a
  queue one at a time, in order: the Windows store does not order operations on one entry made from
  several threads (its README: "setting a password on one thread and then immediately spawning
  another to get the password may return a `NoEntry` error"). A caller waits at most a bound: the
  call's time limit for a widget's call, 2 minutes for the tool (time for the person to unlock a
  keychain). One that does not answer in time fails as `unavailable`, naming the keychain: the
  D-Bus store can block indefinitely on an unlock prompt, and macOS shows its unlock dialog on its
  own. An operation given up on still runs in order, so a later one never overtakes it.
- A write that matters for rotation (a refresh) holds the accounts record's lock until the vault
  thread has answered, not only until the caller stopped waiting; a refresh whose write fails or
  does not finish marks the account as needing connecting again (reason `refresh-failed`) and is
  logged, since the only valid refresh token may then be lost.
- `NoStorageAccess` and `PlatformFailure` are "keychain unavailable"; `NoEntry` for an account the
  record says is connected marks it as needing connecting again.
- Errors never carry a secret: `keyring-core`'s `BadEncoding(Vec<u8>)` and `BadDataFormat(Vec<u8>,
  _)` hold the raw secret bytes and its `Error` derives `Debug`, so `store.rs` maps every error to a
  payload-free enum of its own before it leaves the module, and the vault's code never formats a
  `keyring-core` value. Parsing a document that holds a secret (the vault's JSON, a token
  response) reports fixed text, never the `serde_json` error, which quotes the offending value. A
  test injects `BadEncoding(<token bytes>)` through the mock's `set_error` and scans the output.

What the spike found (Linux, cargo-deny 0.20.2 with this repository's `deny.toml`): 49 crates in
the Linux tree, MIT or Apache-2.0 apart from `subtle` (BSD-3-Clause) and `byteorder` (Unlicense OR
MIT, through the Windows store), all allowed: `bans ok, licenses ok`. Set, read, delete and
`NoEntry` worked against the mock store, a desktop Secret Service and an isolated gnome-keyring.
A locked collection without a display answers `NoStorageAccess(Prompt)` in 0.1 s; no daemon
answers `PlatformFailure` after about 25 s (hence the bound). The D-Bus store links the system's
`libdbus-1`: building needs `libdbus-1-dev` and `pkg-config` (the Tauri dependencies CI installs
already include `pkg-config`). The `vendored` feature would compile libdbus's C sources, licensed
AFL-2.1 or GPL-2.0, which `cargo deny` cannot see behind the crate's MIT/Apache label: it stays
off, and `deny.toml` cannot catch it, so the PR template names it (R19).

On macOS the item is created with the creating application trusted; another application of the
same user gets a confirmation prompt unless the user chose "Always Allow" (Apple, access control
lists). A rebuilt binary counts as another application, so developers see a prompt after each
rebuild; that is expected and documented. In CI, the binary that creates an entry is the one that
reads and deletes it (R15), so a headless runner never meets a prompt.

**Rationale**: constitution v2.4.1 names `keyring-core` and its stores; the D-Bus store is
synchronous, as the host is (003, R5), where the zbus store brings `async-io` executors (126
crates). One injected store keeps tests off the global default and lets them use the mock.

**Alternatives considered**: `keyring` 4.x's wrapper (zbus on Linux, stores chosen by the wrapper);
`linux-keyutils` (kernel keyring, lost at every reboot); a file encrypted with a key in the
keychain (more code in the security core for nothing the keychain does not do).

## R6. The accounts record

**Decision**: `<data dir>/accounts.json`, host data like the grant record: written under a lock
with an atomic rename, `record_version` 1, never holding a credential
([data-model.md](data-model.md#accounts-record)). It maps each connector id and origin to its
account: kind, flow, client registration, name and stable id, account id (R12), access declared
at connection and access the service reported, connected, refreshed and expiry times, and
whether and why it needs connecting again. It also holds the vault id (R5).

Refreshing takes the record's lock, re-reads the record and the vault, and refreshes only if no
other process did meanwhile: services that rotate refresh tokens (RFC 6749 §10.4, OAuth 2.1),
GitHub among them, invalidate the old one on use, so two processes refreshing with one refresh
token would leave one of them holding a dead token.

## R7. Account use in the manifest, and the login providers

**Decision**: a connector entry of the manifest gains `account` (`true` or `false`, default
`false`). A connector with a required credential needs `account: true` or the widget is refused
at load as `account-use-not-declared` (FR-003). `login_providers` stops being required in
`wit/manifest.schema.json` and is described as obsolete; the parser still accepts its old values,
the tool prints `login providers: github (obsolete: accounts come from connectors)`, and nothing
reads it (FR-016).

## R8. The grant record, version 4, and the approval digest

**Decision**: `record_version` 4. A connector entry gains `account`, present only when the
connector declares a credential: `uses` (the widget's `account` as approved), `credential` (the
SHA-256 of the canonical declaration without `access`: kind, required, destinations, header,
scheme, flow, client, addresses, scopes, identity, subscription statuses) and `access` (the
approved items, sorted). Versions 1–3 are read as 4 without it and written back as 4.

At load (step 7), for a connector whose credential applies (the widget uses the account, or the
credential is required):

| Change since the approval | Effect |
| --- | --- |
| Credential added, or `credential` digest differs | `needs-new-approval`, naming what changed |
| `uses` was false and the manifest now says `true` | `needs-new-approval` |
| An `access` item added, or a level raised from `read` to `write` | `needs-new-approval`, naming the items |
| Access narrowed, or the widget no longer uses the account | Runs; the approval is left as it is, and the table uses what both allow |

The approval digest of 003 gains, per connector with a credential, `"account": { "uses",
"credential", "access" }`, so widgets whose connectors declare none keep their digests and their
kept data.

## R9. Contract 1.3.0: the `accounts` interface

**Decision**: `wit/v1/widget.wit` becomes `dashboard:widget@1.3.0`
([draft](contracts/widget-v1.3.wit)), with 1.2.0 frozen first under `wit/history/1.2.0`:

- A new interface `accounts` with one function, `call`, taking the same arguments as
  `connectors.call` and answering `result<response, account-call-error>`, where
  `account-call-error` is `not-granted`, `refused(problem)`, `failed(problem)`,
  `account-not-connected` and `subscription-required`. Its types come from `connectors` by `use`.
- Two new worlds, `account-widget` (log, connectors, accounts) and `account-stored-widget` (with
  storage); the host binds `account-stored-widget`.

The answers cannot be added to `connectors`: `call-error` and `rule` are 1.1.0 types, and a 1.1.0
component's import type must match the host's definition exactly, so a grown enum would stop
every older widget from linking; `tests/contract_v1.rs` forbids it anyway.

Linking follows Principle II's rule for shared imports with two tables, both built at link time:
`connectors` holds the granted connectors the widget calls without an account, `accounts` those
it calls with one (`account: true` in the manifest and `uses` in the approval). A connector is in
one table or neither, so a call through the other interface misses and answers `not-granted`. An
`accounts` import with no connector declared `account: true` is `not-declared`, as `connectors`
without connectors is; with some declared and none granted, the stand-in answers `not-granted`.
`account: true` needs contract 1.3 or later (`contract-too-old`), and the registry records
`accounts` since 1.3.

A component that calls only `accounts` still imports `connectors`, as a type-only instance (the
types `accounts` takes by `use`, no `call`): a spike with wit-bindgen 0.63 and wit-component 0.262
built that way. So `connectors` is never refused as `not-declared` while the manifest declares any
connector (today's rule, `permissions.rs`), and when every connector is `account: true` it is
linked to the stand-in; a named test covers a widget whose connectors are all used with an
account.

**Alternatives considered**: one interface for both kinds of call (the new answers have nowhere
to go for 1.1.0 types); a status function for the widget to ask whether an account is connected
(a call already answers that, and the widget learns nothing else).

## R10. Adding the credential to a request

**Decision**, in the network module:

- `Request` gains an optional credential: the header name, the value (a `Secret`) and the origins
  it is for. `Session::once` adds the header only when the request's origin is one of them and
  its scheme is `https`, marks the value sensitive (`HeaderValue::set_sensitive`), and checks
  again on every redirect, so a redirect to another of the connector's hosts goes without it
  (FR-007). `build_request` never sees the secret: no template, value or placeholder can reach it.
- The credential never touches what is logged: the logged target is path and query, which hold
  no credential, and byte counts count it without recording it.
- Every credential is in the bearer-token set, letters, digits and `-._~+/=` (RFC 6750's
  `b64token`): a key outside it is `key-refused`, a token outside it fails the sign-in or refresh.
  After reading a response, before anything reaches the widget or the log: if the body, a declared
  header or the `Location` header contains the secret in any of the forms an encoder can give it
  (as is; with `/` written `\/`; with `+`, `/` and `=` as `\u002b`, `\u002f`, `\u003d` in
  either case), or the whole header value, the call fails as `invalid-response` ("the response
  carried the credential back"), and a redirect target is not logged. Restricting the set is what
  makes the list of forms complete: JSON encoders escape `"`, `\`, `<`, `>` and `&`, none of which
  a credential holds. The check is a byte search (`memchr::memmem`, already in the tree through
  `regex`) over at most the body limit.
- A `401` answer to a request that carried the credential: if the account can be refreshed and
  this call has not refreshed yet, the host refreshes (R11) and sends the request once more, as a
  logged hop. A `401` that remains is confirmed with the identity operation (when declared): if
  identity is refused too, the account is marked as needing connecting again; if it succeeds, the
  `401` came from this request, not the token, and only this call answers `account-not-connected`,
  so one widget cannot lock the shared account out for the others (FR-011). The response of the
  `401` never reaches the widget.
- The refresh, the resend and the identity confirmation are the host's, one each at most per
  call; they are not counted against the widget's calls per run or its rate, which bound the calls
  that cause them, and the destination host's rate counts them.
- A status listed in `subscription_required` answers `subscription-required`, without the
  response.
- The host's own requests (token, refresh, revocation, identity) go through the same
  agents: HTTPS, public addresses only, Mozilla's roots, no proxy, no cookies; they follow no
  redirect at all. Bodies are form-encoded (`application/x-www-form-urlencoded`) with `Accept:
  application/json`; answers are parsed into typed structs within 64 KiB. They are logged with the
  plugin `host`, the connector, an operation `token`, `refresh`, `revoke` or
  `identity`, and the status, never a body.

**Rationale**: adding the header at the last moment, per hop, is what makes "only to its own
destinations" checkable in one place; the echo check closes the one path by which a reviewed
destination could hand the token to the widget.

## R11. Account states and refresh

**Decision** ([data-model.md](data-model.md#account-states)): an account is `connected`, `needs
connecting again` (with a reason: refused, refresh failed, missing from the keychain, less access
than the connector asks for) or absent. For each call through `accounts`, after the grant
re-check (003 step 7):

0. When the widget is linked, the host takes for each connector in the account table the
   `account_id` connected then, or none. That snapshot is what the run can use: an account
   connected afterwards, by any process, applies from the next load (Principle II; 003 does the
   same for a connector turned on by another process), and every check below only takes away.
1. No account in the snapshot and the credential optional: the request goes without a
   credential, even if one has been connected since.
2. No account in the snapshot, or one needing connecting again: `account-not-connected`, nothing
   sent.
3. The access recorded at connection lacks an item the connector now declares:
   `account-not-connected` (reason: less access), nothing sent.
4. The secret is read from the vault once per run of `update` and held until the run ends.
5. A token that expires within 60 s is refreshed first, under the record's lock (R6), at most once
   per call (FR-011). A failed refresh marks the account and answers `account-not-connected`.
6. Just before sending and again before handing the response over, the accounts record is read
   again: an account no longer connected with the snapshot's `account_id` answers
   `account-not-connected` and the response is dropped (FR-013), as 003 re-reads the grants.

## R12. Disconnecting, replacing and kept data

**Decision**: `accounts disconnect` takes the accounts record's lock and holds it while it deletes
the vault entry and then the record's account, so no call between the two marks the account as
missing; then it deletes, at once, the kept data of every widget whose file names that account
(FR-013: private data must not wait for a load that may never come); then it asks the service to
revoke when it declares `revoke` (with `client_id` and the token), printing whether that
succeeded, or prints `revoke_page`. Each account
gets an account id, 16 random bytes in hex, when a new account is connected; reconnecting the same
account (same stable id from `identity`) keeps it. Kept data (003, R14) gains, for each connector
the widget uses with an account, the account id it was fetched with; a load that finds another or
none also discards the file, the backstop for a disconnect another process made while a load was
reading. Connecting a different account, or a new key while one is stored, is refused
(clarification 2), and the token just obtained is revoked when the service allows, then dropped.

## R13. Limits

| Limit | Value |
| --- | --- |
| Sign-in time limit | 10 min |
| Callback request head | 8 KiB, read within 2 s |
| Sign-in and refresh responses | 64 KiB; each request within the call's time limit (5 s) |
| Stored secret | 2,560 bytes (the JSON document) |
| Key | 1–1,024 bytes of letters, digits and `-._~+/=` |
| Account name and id shown | 100 characters, then cut |
| Vault operation | The call's time limit for widgets; 2 min for the tool |
| Refresh margin | 60 s before expiry |
| Refreshes per call | 1 |

## R14. CLI

**Decision** ([contracts/cli.md](contracts/cli.md)): `runtime accounts connect <connector-id>
[--client <id>]`, `runtime accounts list` and `runtime accounts disconnect
<connector-id> [--origin <path>]`; `permissions` shows each connector's credential and the account
connected and, for a widget that uses an account, a `data flow:` line saying which other
destinations of its connectors data read as that account can reach (constitution, Principle III:
"before approval, the host MUST show … whether data read from one connected account … can reach
another destination"), next to 003's line for data read from this machine; `requests` shows the
account of each call and the host's own entries. A key is read from the terminal without echo
through `rpassword` (only in the binary), whose `read_password` opens `/dev/tty` or `CONIN$`. It
turns the echo off only once it starts reading, after the prompt, so on Unix the binary turns it
off first, with `libc` and `TCSAFLUSH`, as glibc's `getpass` does, and only then shows the prompt
(found when CI typed the key in that gap, after the merge of 005). On Windows the gap stays; no
test types there. The key's checks live in a library function, tested directly and through
`read_password_from_bufread`, and the CLI's key path is tested in a pseudo-terminal on Linux and
macOS, on Linux checking the echo is off when the prompt appears; the browser is opened
with the `open` crate (only in the binary, with `shellexecute-on-windows`, so no shell parses the
address on Windows), and the address is always printed too.

## R15. Testing

**Decision**:

- A **test sign-in service** in `tests/common/signin.rs`, on the HTTPS test servers of feature 003
  (test CA, routes through the seam): an authorization page that redirects to the loopback
  callback (or with a wrong state, a used code, an error, or late), token, refresh, revocation and
  identity addresses; scripted per test to rotate tokens, refuse, answer another account, or echo
  the credential. It never reaches a real service.
- The **vault seam**: `Runtime::with_test_network` also takes a credential store, so library tests
  inject `keyring_core::mock::Store` (in-memory, errors injectable). The seam keeps 003's guard:
  only debug builds with `test-network`.
- **Where each path is tested** (planning session): the CLI keeps 003's rule, no option reaches the
  seam. The sign-in, the example end to end and every hostile case run as library tests through the
  seam (test CA, routes, mock or real store). CLI tests cover the `accounts` commands themselves:
  output, escaping, refusals (`no-client-configured`, `credential-needs-https`,
  `account-not-connected`, `keychain-unavailable`), a key typed in a pseudo-terminal on Linux and
  macOS (Windows: the key's checks through the library function only), and `list` and
  `disconnect` on entries the CLI itself made. No automated test runs `accounts connect` with a
  sign-in through the binary; the library's sign-in code is the same code the binary calls.
- **Real keychains**: the CLI tests and a `vault` test file run against the system's keychain on
  the three CI systems, each test with its own data directory and so its own vault id. Each entry
  is read and deleted by the binary that created it (the `vault` tests in the test binary, the CLI
  tests through the CLI's own `accounts disconnect`), since macOS trusts only the creating binary.
  CI installs `gnome-keyring` and `libdbus-1-dev` on `ubuntu-24.04` and runs the Linux test step
  as `dbus-run-session -- bash -c "gnome-keyring-daemon --components=secrets --daemonize --unlock
  <<< 'ci'; cargo test --workspace --locked"` (the recipe of keyring-rs's own CI, which needs the
  newline the here-string adds), a step of its own beside the one macOS and Windows keep;
  `bench.yml` gets the same packages and session on Linux.
- **The scan** of FR-019: every token and key the test service issues is a fresh random value;
  after each case the test searches for it in every file under the data directory, the grant
  record, the request log, kept data, stores, the captured output of the host and the tool, and
  everything the fixture returns.
- The browser is never opened in tests: the tool's `--no-browser` option (prints only) is used,
  and the test service plays the person.

## R16. The example

**Decision**: a new example, `plugins/examples/private-notes` (contract 1.3.0, `account-widget`),
with its development connector `local/notes` beside it: one read, `list-notes`, on
`https://notes.test`, with an optional account (`authorization-code`, `Authorization: Bearer`,
`access` "notes: read", `identity` through `get-me`). The widget declares `account: true`;
without an account it shows the public notes, with one also the account's own. CI runs it end to
end through the library with the test sign-in service on `notes.test` (routes and test CA of the
seam), since a credential needs HTTPS and the development tool's loopback connectors are plain
HTTP. `github-releases` stays a 1.1.0 widget, unchanged, and `catalog/github` stays 1.0.0. A new
fixture, `account-probe` (contract 1.3.0, `account-widget`), makes the calls its configuration
lists through `accounts` or `connectors` and returns every answer, or returns everything it can
find in its configuration, for `token-hunt`; `net-probe` stays a 1.1.0 plugin, as the contract
tests need it.

**Rationale**: the example shows the whole path (declaration, approval, sign-in, calls, private
data) without a real service, which no catalog connector offers in this feature.

## R17. Dependencies

| Crate | Where | Why | In `Cargo.lock` | License |
| --- | --- | --- | --- | --- |
| `keyring-core` `=1.0.1` | runtime | Vault API (R5) | No | MIT OR Apache-2.0 |
| `windows-native-keyring-store` `=1.1.0` | runtime, Windows | Credential Manager | No | MIT OR Apache-2.0 |
| `apple-native-keyring-store` `=1.0.2` | runtime, macOS | Login keychain | No | MIT OR Apache-2.0 |
| `dbus-secret-service-keyring-store` `=1.0.1` | runtime, Linux | Secret Service over libdbus | No | MIT OR Apache-2.0 |
| `zeroize` 1 | runtime | Secrets wiped on drop | Yes | MIT OR Apache-2.0 |
| `memchr` 2 | runtime | The echo check (R10) | Yes | Unlicense OR MIT |
| `httparse` 1 | runtime | The callback request (R2) | Yes | MIT OR Apache-2.0 |
| `rpassword` 7 | runtime (binary only) | Reading a key without echo | No | Apache-2.0 |
| `libc` 0.2 | runtime (binary only), Unix | The echo turned off before the key prompt (R14) | Yes (Wasmtime) | MIT OR Apache-2.0 |
| `open` 5 | runtime (binary only) | Opening the system browser at the sign-in address | Yes (workspace) | MIT |

Pins are exact for the vault crates, since they are security core. `rpassword` and `open` are used
only by `src/bin/runtime.rs`. Each must pass `cargo deny` and `cargo audit` in CI; `memchr`'s
`Unlicense OR MIT` is satisfied by MIT, so `deny.toml` needs no new license.

## R18. Measuring

**Decision**: the bench runs in release, where the seam cannot compile (003, R15), so
`bench/src/bin/credential_cost.rs` measures what a credential adds in-process, with no network:
the median of 100 requests built and checked with and without a credential (the header added per
hop and the echo check over a 1 MiB body: SC-006, the difference under 5 ms), and the first read
of an entry from the system's keychain and a later one, on the three systems through `bench.yml`,
into `specs/005-plugin-credentials/performance.md`, with the limits of R13 as measured. A refresh
is a network round trip, timed in the library tests, not benched.

## R19. Keeping the review on what matters

**Decision**:

- `clippy.toml` disallows `keyring_core` types and the store crates' outside `runtime/src/vault/`,
  and `std::net::TcpListener` with `std::net::TcpListener::bind` outside `runtime/src/network/`, as
  it lists `TcpStream` and its `connect`.
- The pull request template lists `runtime/src/vault/` and `connectors/` as security core, the
  four vault crates and `libdbus-sys` under `Cargo.lock`, and asks that the `vendored` feature of
  `libdbus-sys` never be turned on (its C sources are AFL-2.1 or GPL-2.0).
- The catalog's unit test (003, R20) also checks that every sign-in address and page of a catalog
  credential is under the domains the connector reserves; no catalog connector declares one in
  this feature, so the test runs on catalog connectors added through the seam.
