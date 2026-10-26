<!-- check: service=notes -->
# Accounts

Some services answer differently for a signed-in person: their own notes, their private
repositories. A widget can call such a connector **as the person's account**, and still never see
the account's credential: the person connects the account in the host's controls, the host keeps
the credential in the system's keychain, and adds it to each request to the destinations the
connector names. Nothing a widget passes can name, build or replace it, and a response that
carries it back reaches no one.

This guide starts from the example `private-notes` and its development connector `local/notes`,
for a notes service whose public notes anyone reads and whose own notes only an account reads. It
starts in the clone of [the walkthrough](../getting-started.md). In the project's CI, its commands
reach a test notes service and sign in to a test sign-in service; for your own service, see
[writing connectors](writing-connectors.md#testing-against-your-own-service).

## The widget

`private-notes` builds the world `account-widget`: it imports `accounts`, and calls `local/notes`
through it, because its manifest declares the connector with `"account": true`:

```json
"connectors": [
  { "id": "local/notes", "major": 1, "operations": ["list-notes"], "required": true, "account": true }
]
```

The connector, `connectors/notes.json` beside the example, declares the credential: an account of
the service, signed in to in the browser, sent as a bearer token to its API, for reading notes
([the connector format](../reference/connector-format.md#credential)). A local connector loads
only in the development CLI, from the folder `--connectors` names.

```console check output=any
$ cd dashboard/plugins/examples/private-notes
$ cargo build --release --target wasm32-unknown-unknown
```

```console check
$ runtime pack ../../target/wasm32-unknown-unknown/release/private_notes.wasm --manifest manifest.json --output private-notes.wasm
packed private-notes.wasm
digest: sha256:<digest>
$ runtime permissions private-notes.wasm --connectors connectors
plugin: local/private-notes 0.1.0
author: Dashboard contributors
contract: v1 (1.3.0)
approved: no
permissions: none
connectors:
  local/notes 1.0.0  required  not granted
    from: local file <path> (development)
    destinations: https://notes.test
    credential: account (optional), Authorization: Bearer, sent to https://notes.test
      sign-in: https://notes.test/oauth/authorize
      token: https://notes.test/oauth/token
      asks for: notes read (declared by the developer)
      this widget acts as the account: none connected (calls go without one)
    list-notes  read (declared by the developer)  GET https://notes.test/notes
      sends: nothing
      fixed: Accept: application/json
login providers: none
approval digest: sha256:<digest>
$ runtime approve private-notes.wasm --expect sha256:<digest> --connectors connectors
approved local/private-notes 0.1.0
connectors: local/notes (https://notes.test)
account use: local/notes (notes read)
```

## Without an account

The credential of `local/notes` is optional: without an account, a call goes without it, and the
service answers as it does for anyone.

```console check
$ runtime update private-notes.wasm --connectors connectors
updated local/private-notes 0.1.0
calls: 1 (0 not granted, 0 refused, 0 failed), 37 bytes received
$ runtime run private-notes.wasm --connectors connectors
{
  "account": null,
  "fetched_at_ms": <n>,
  "items": [
    {
      "private": false,
      "title": "Welcome"
    }
  ],
  "last_update": {
    "at_ms": <n>,
    "outcome": "ok"
  },
  "text": null,
  "title": "Notes"
}
```

## Connecting the account

Only the person connects an account, here with the development CLI. It opens the service's
sign-in in the browser; the person signs in there, never in the widget, and the host keeps what
the service gives back. `--client` is the client id of the app registered with the service.

```console check
$ runtime accounts connect local/notes --connectors connectors --client dashboard-dev
account for local/notes (local file <path>, development)
asks for: notes read
opening <url> in the browser
waiting for the browser (10 min)…
connected: ada (id 1234567)
used by: local/private-notes
$ runtime accounts list --connectors connectors
local/notes  local file <path>
  account: ada (id 1234567), connected <time>, refreshed <time>
  access: notes read (as asked)
  client: dashboard-dev (from --client)
  used by: local/private-notes
```

## With the account

The widget acts as the account connected when the host loaded it. With the account, the service
answers with the account's own notes too:

```console check
$ runtime update private-notes.wasm --connectors connectors
updated local/private-notes 0.1.0
calls: 1 (0 not granted, 0 refused, 0 failed), 79 bytes received
$ runtime run private-notes.wasm --connectors connectors
{
  "account": null,
  "fetched_at_ms": <n>,
  "items": [
    {
      "private": false,
      "title": "Welcome"
    },
    {
      "private": true,
      "title": "Ada's own note"
    }
  ],
  "last_update": {
    "at_ms": <n>,
    "outcome": "ok"
  },
  "text": null,
  "title": "Notes"
}
```

## Every answer of a call

`call` answers like a connector's ([connectors](connectors.md#every-answer-of-a-call)), with two
answers more ([the contract](../reference/contract.md#accounts)):

- `account-not-connected`: no account is connected and the credential is required, or the account
  must be connected again: the service refused it, it could not be refreshed, or the connector now
  asks for more access than it holds. Show that the account needs connecting; only the person can
  connect it. `private-notes` returns no notes with a note saying so, and keeps working.
- `subscription-required`: the service said the account lacks a paid plan the operation needs.
  The response is dropped.

`not-granted` comes first, as in every capability: the connector is optional and off, or its
approval was removed.

## Disconnecting

```console check
$ runtime accounts disconnect local/notes --connectors connectors
disconnected local/notes: ada (id 1234567)
deleted from the keychain
revoked at notes.test
kept data discarded: local/private-notes
```

Disconnecting deletes the account from the vault, revokes it at the service, when the connector
declares how, and discards the data the host kept from the widgets that used it, so no widget
shows the account's notes once it is gone. The widget's next load goes without it.

## For real use

A connector of your own with an account needs an app registered with your service: a public
client that allows the authorization code with PKCE and a loopback redirect.
[Writing connectors](writing-connectors.md#testing-against-your-own-service) says how. No connector
of the official catalog has an account yet; the first, for GitHub, arrives in Phase 6.
