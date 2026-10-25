# Contract: `runtime` command-line tool

Extends [feature 004's contract](../../004-plugin-storage/contracts/cli.md), whose common rules
still hold: development mode only, local files, every text from a package, a connector, a store
or a service escaped before it is printed, a refusal printed as `error: <kind>: <detail>` on
stderr with exit 1, usage errors with exit 2.

## Common rules

- `--data <dir>` now also holds `accounts.json` ([data-model.md](../data-model.md#accounts-record)).
  The credentials themselves are in the development vault, in the system's keychain, under
  entries named by the record's vault id; no command prints, reads from a file, or takes as an
  argument any credential.
- Every command that waits on the keychain waits at most 2 minutes, then fails as
  `keychain-unavailable`.

## `runtime accounts connect <connector-id> [--origin <path>] [--client <id>] [--no-browser] …`

Connects an account to a connector: a local connector by id (its file in `--connectors <dir>`;
`--origin` picks the file when the id is ambiguous) or, from a later feature, a catalog connector.
For `kind: key`, reads the key from the terminal without echo. For an account, runs the sign-in of
[data-model.md](../data-model.md#sign-in). `--client` supplies the client registration when the
host has none built in (R4) and is remembered for the connector; without it and without a
remembered one, the command fails as `no-client-configured`, naming the option. `--no-browser`
prints the address without opening it.

```text
account for local/notes (local file /home/ada/dashboard/plugins/examples/private-notes/connectors/notes.json, development)
asks for: notes read
opening https://notes.test/oauth/authorize?response_type=code&client_id=dashboard-dev&… in the browser
waiting for the browser (10 min)…
connected: ada (id 1234567)
```

- The sign-in address is printed as the host will open it, escaped; the `state` and challenge in
  it are not secrets, and the verifier never leaves the host.
- An account already connected: the same account (same id) replaces its credential and prints
  `reconnected: <name>`; another account fails as `another-account-connected`, naming the current
  one and `runtime accounts disconnect`, and nothing changes; for a key, any new key fails the
  same way.
- The widgets that use the account are listed after `connected:` as `used by: <plugin id>, …`.
- Failures: `sign-in-failed` (the time limit, a wrong or missing state, a used code, the
  service's error code and text, escaped and cut), `key-refused` (empty, too long, a
  character outside `!`–`~`), `keychain-unavailable`, `no-client-configured`,
  `invalid-connector`, `connector-unavailable`.

## `runtime accounts list …`

```text
local/notes  local file /home/ada/dashboard/plugins/examples/private-notes/connectors/notes.json
  account: ada (id 1234567), connected 2026-10-25 14:02, refreshed 14:02, expires 15:02
  access: notes read (as asked)
  client: dashboard-dev (from --client)
  used by: local/private-notes
local/weather  local file /home/ada/connectors/weather.json (no file)
  account: ada@example, needs connecting again: the service refused it (401)
```

- Every account in the development vault's record, sorted by connector id and origin; `(no file)`
  when a local connector's file no longer exists (clarification 1).
- `needs connecting again:` names the reason: refused, refresh failed, missing from the keychain,
  or `the connector asks for more access than it holds (…)`.
- `access:` adds `(more than the connector asks for now: connect again to narrow it)` when the
  connector's access shrank.
- `nothing connected` when the record is empty.

## `runtime accounts disconnect <connector-id> [--origin <path>] …`

```text
disconnected local/notes: ada
deleted from the keychain
revoked at notes.test
kept data discarded: local/private-notes
```

- Works whether or not the connector's file exists; `--origin` is required when the id has
  accounts at several origins.
- With a `revoke` address: `revoked at <host>` or `revocation failed: <reason>; revoke it at the
  service` (the account is gone from the vault either way); without one, `the service offers no
  revocation: revoke it at <revoke_page>` or `…: revoke it at the service`.
- No account: `error: account-not-connected: …`, exit 1.

## `runtime permissions <plugin> …`

As in feature 004, plus, for each connector with a credential, after its `destinations:` line:

```text
  local/notes 1.0.0  required  granted
    from: local file /home/ada/dashboard/plugins/examples/private-notes/connectors/notes.json (development)
    destinations: https://notes.test
    credential: account (optional), Authorization: Bearer, sent to https://notes.test
      asks for: notes read (declared by the developer)
      this widget acts as the account: ada (id 1234567)
    list-notes  read (declared by the developer)  GET https://notes.test/notes
…
data flow: data read as the account of local/notes can reach https://api.github.com
login providers: none
approval digest: sha256:…
```

- `this widget acts as the account:` is followed by the connected account, or `none connected
  (calls go without one)` for an optional credential, `none connected (calls answer account not
  connected)` for a required one; a widget that does not declare `account` reads `this widget
  does not use the account`.
- `asks for:` marks `(reviewed)` for the catalog and `(declared by the developer)` for local
  connectors. The `credential:` block also prints the `sign-in:` and `token:` addresses.
- For a widget that uses an account, a `data flow:` line names every origin of its connectors,
  other than the credential's own destinations, that data read as the account can reach
  (Principle III), beside 003's line for data read from this machine; none is printed when there
  is no other origin.
- `login providers:` reads `none`, or the list followed by `(obsolete: accounts come from
  connectors)`.

## `runtime approve <plugin> --expect <digest> …`

As before; the digest covers each connector's credential and whether the widget uses the account
([data-model.md](../data-model.md#approval-digest)). One more line per connector the widget uses
with an account: `account use: local/notes (notes read)`.

## `runtime requests …`

Each entry of a call that carried a credential shows `as <account name>` (or `as key`) after the
connector; the host's own requests appear as `host  local/notes  token  POST notes.test/oauth/token  200`,
never with a body.

## `runtime update <plugin> …`

As before. A call that answered `account-not-connected` or `subscription-required` is counted
with the refused ones, and the update output adds `account: local/notes needs connecting again
(runtime accounts connect local/notes)` when that is why.
