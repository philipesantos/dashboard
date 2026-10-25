# Data Model: Plugin Credentials

What feature 005 adds to the data of features 002–004. Field rules are in [research.md](research.md);
this file says what each record holds and how the steps use it.

## Connector: `credential`

The optional field of [R3](research.md#r3-the-credential-declaration)
([draft](contracts/connector-credential.schema.json)). Checked when the connector loads (003, load
step 6a), in this order, each failure `invalid-connector` naming the field:

1. The schema: kinds, flows, lengths, patterns; `account` only with `kind: account`.
2. Each of `destinations` names a declared destination whose origin is `https`. A local connector on
   a loopback address without HTTPS that declares a credential is `credential-needs-https`
   (FR-007).
3. `header`: `Authorization` with a `scheme`; another name only in a catalog connector, never a
   name the host forbids or a header an operation fixes.
4. Sign-in addresses and pages: `https`, no user information or fragment, each host under a
   domain the connector `reserves` (catalog) or equal to the host of one of its destinations
   (local).
5. `identity.operation` is a read the connector declares, `GET` without parameters.
6. `client` names a registration (R4); whether one is available is checked only when connecting.

## Manifest: `account`

| Field | Rule |
| --- | --- |
| `connectors[].account` | Optional, `true` or `false` (default `false`): the widget calls this connector as its account, through `accounts` |
| `login_providers` | Optional; obsolete, accepted as before and never used (FR-016) |

A connector whose credential is `required: true` declared without `account: true` is
`account-use-not-declared`. `account: true` on a connector without a credential is
`invalid-manifest`, naming the connector.

## Permission registry

| Permission | Contract v1 | Since |
| --- | --- | --- |
| `log` | `dashboard:widget/log@1.<m>.0` | 1.0 |
| each connector without an account | `dashboard:widget/connectors@1.<m>.0` | 1.1 |
| `storage` | `dashboard:widget/storage@1.<m>.0` | 1.2 |
| each connector with an account | `dashboard:widget/accounts@1.<m>.0` | 1.3 |

## Contract versions

| Contract | Package | Accepted from a plugin |
| --- | --- | --- |
| v1 | `dashboard:widget@1.3.0` | Any 1.m.p not newer than 1.3.0, one version on every contract name |

`account: true` needs 1.3 or later (`contract-too-old`, naming 1.3.0). Plugins built for 1.0.0,
1.1.0 and 1.2.0 cannot import `accounts`, so the host never answers them `account-not-connected`
or `subscription-required`.

## Client registration

`{ "client_id": "<text, 1–100 visible characters>", "app": "<optional name, 1–100 of a–z, 0–9, ->" }`,
from the host's built-in table (empty in this feature) or, in development, from `--client` and
`--app`, remembered in the accounts record for the connector. Never a secret.

## Vault entry

| Part | Value |
| --- | --- |
| Service | `dev.philsantos.dashboard.development` |
| User | `<vault id>:<account key>`: the record's vault id and the SHA-256 (hex) of `"<connector id>\n<origin>"` |
| Secret | JSON: `{"access": "…", "refresh": "…"}` for an account (`refresh` absent when the service gives none), `{"key": "…"}` for a key; at most 2,560 bytes |

## Accounts record

`<data dir>/accounts.json`, written under a lock with an atomic rename:

```json
{
  "record_version": 1,
  "vault": "5f0c…(32 hex)",
  "accounts": [
    {
      "connector": "catalog/github",
      "origin": "catalog",
      "kind": "account",
      "flow": "device",
      "client": { "client_id": "Iv23li…", "app": "dashboard-dev-ada", "from": "option" },
      "account_id": "9b1e…(32 hex)",
      "name": "ada",
      "service_id": "1234567",
      "access_asked": [{ "name": "Contents", "level": "read" }, { "name": "Metadata", "level": "read" }],
      "access_reported": [],
      "connected_at_ms": 1761400920000,
      "refreshed_at_ms": 1761400920000,
      "expires_at_ms": 1761429720000,
      "needs_connecting": null
    }
  ]
}
```

| Field | Rule |
| --- | --- |
| `vault` | 16 random bytes, hex; created with the record, never changed |
| `connector`, `origin` | The account's key; `origin` is `catalog` or the local file's canonical path |
| `account_id` | 16 random bytes, hex; new for a new account, kept when the same account reconnects |
| `name`, `service_id` | From `identity`: the account's name (at most 100 characters) and stable id; for a key, `name` is `key` and `service_id` absent |
| `access_asked` | The connector's `access` when the account was connected |
| `access_reported` | The scopes the token response named, as given (empty for GitHub Apps) |
| `expires_at_ms` | From `expires_in`; absent when the token does not expire |
| `needs_connecting` | `null`, or `{ "reason": "refused" \| "refresh-failed" \| "missing-from-keychain" \| "less-access", "at_ms": … }` |

## Account states

```text
            connect (sign-in or key)                     disconnect
   absent ───────────────────────────▶ connected ───────────────────────▶ absent
                                        │   ▲
         401 after refresh, refresh     │   │ connect again, same account
         failed, entry missing, the     ▼   │
         connector asks for more   needs connecting ──── disconnect ───▶ absent
```

Connecting while `connected` or `needs connecting` succeeds only for the same account (same
`service_id`; for a key, never), keeping `account_id`.

## Grant record, version 4

Version 3 (feature 004) plus, in each connector entry, only when the connector declares a credential:

```json
"account": {
  "uses": true,
  "credential": "sha256:4d1a…",
  "access": [{ "name": "Contents", "level": "read" }, { "name": "Metadata", "level": "read" }]
}
```

| Field | Rule |
| --- | --- |
| `uses` | The manifest's `account` when approved |
| `credential` | SHA-256 of the canonical declaration without `access` (R8) |
| `access` | The approved access items, sorted by name |

Versions 1–3 are read as 4 without it and written back as 4. Load step 7 applies the table of
[R8](research.md#r8-the-grant-record-version-4-and-the-approval-digest).

## Approval digest

The document of features 003 and 004 gains, in each connector that declares a credential:

```json
"account": { "uses": true, "credential": "sha256:4d1a…", "access": [{ "name": "Contents", "level": "read" }] }
```

Connectors without a credential, and widgets without one, keep the digests they had.

## Kept data

The file of 003 gains `accounts`: for each connector the widget uses with an account, the
`account_id` the data was fetched with, or `null` when it was fetched without one. A load (step
7a) that finds another `account_id`, or none where one was recorded, discards the file.

## Request log entry

The entry of 003 gains `account`: the account's name, `key`, or absent when the call carried no
credential. The host's own requests are entries with `plugin: "host"`, the connector, `operation`
one of `sign-in`, `token`, `refresh`, `revoke`, `identity`, the method, host and path (no query
for `token`, `refresh` and `revoke`, whose bodies hold the secrets), the status or the failure,
and no arguments.

## Load decision

`Runtime::load` as in feature 004, with these changes:

| Step | Check | Refusal |
| --- | --- | --- |
| 3 | The manifest, with `account` | `invalid-manifest` |
| 5 | v1 accepts any minor up to 1.3.0; `account: true` needs 1.3 | `contract-mismatch`, `contract-too-old` |
| 6 | Imports through the registry; `accounts` needs a connector declared with `account: true` | `capability-refused`, `not-declared` |
| 6a | Each declared connector, now with its credential checked; a required credential needs `account: true` | `invalid-connector`, `credential-needs-https`, `account-use-not-declared` |
| 7 | Grants, plus each credential that applies against `account` in the record (R8) | `needs-new-approval`, naming the change |
| 7a | Kept data whose `accounts` no longer match | Discarded, no refusal |
| 9 | Link: `connectors` over the table of granted connectors without account use; `accounts` over the table of those with it; stand-ins when declared and none granted | `contract-mismatch` |

## An account call

For `accounts.call(connector, operation, arguments)`: the steps of 003's connector call
(`specs/003-plugin-connectors/data-model.md`, A connector call), where step 1 looks in the
account table, plus:

- **7a** After the grant re-check: the account state of [R11](research.md#r11-account-states-and-refresh),
  steps 1–5. `account-not-connected` sends nothing.
- **8a** The request carries the credential only to origins it is for, over `https`, re-checked
  per redirect hop (R10).
- **9a** A `401` to a request that carried it: refresh once and resend, or mark the account and
  answer `account-not-connected`.
- **11a** The body, a declared header or `Location` contains the secret: `failed(invalid-response)`.
- **11b** A status of `subscription_required`: `subscription-required`.
- **12a** Before handing the response over, the accounts record still has the same `account_id`
  connected: else `account-not-connected`, response dropped.

Steps 1, 7, 7a, 12 and 12a only take away.

## Sign-in

| Step | Device flow | Authorization code flow |
| --- | --- | --- |
| 1 | Load the connector; read its credential; get the client registration | Same |
| 2 | — | Bind `127.0.0.1:0`; make `state`, verifier, challenge |
| 3 | `POST device` (`client_id`, `scope`); check `verification_uri` and `user_code` (R2) | Open `authorize?response_type=code&client_id&redirect_uri&scope&state&code_challenge&code_challenge_method=S256` |
| 4 | Print the code and the warning; open the verification address; poll `token` | Wait for `GET /callback`; check `state`; refuse `error` |
| 5 | Token response: `access_token`, `token_type` `bearer`, optional `refresh_token`, `expires_in`, `scope` | `POST token` with `code`, `redirect_uri`, `code_verifier`; same response |
| 6 | Call `identity` with the new credential; read name and id | Same |
| 7 | Compare with the connected account (clarification 2); refuse another | Same |
| 8 | Write the vault entry, then the record | Same |

Nothing is written before step 8; a failure at any step leaves the vault and the record as they
were, and a token obtained before a refusal at step 7 is revoked when the connector declares
`revoke`, then dropped.

## Error kinds

New load and tool kinds: `credential-needs-https`, `account-use-not-declared`, `sign-in-failed`,
`another-account-connected`, `key-refused`, `keychain-unavailable`, `no-client-configured`,
`account-not-connected` (the tool, when disconnecting nothing). Existing kinds gain cases:
`invalid-connector` (the credential), `invalid-manifest` (`account` without a credential),
`contract-too-old` (`account` before 1.3), `needs-new-approval` (credential, access, use),
`approval-changed`.

Calls through `accounts` answer `not-granted`, `refused` and `failed` with the rules of 003, plus
`account-not-connected` and `subscription-required`; a keychain that does not answer is
`failed(unavailable)`, naming the keychain.

## Combinations (SC-002)

A table-driven test over: credential absent, optional, required × manifest `account` false, true ×
approval `uses` false, true × connector required, optional on, optional off × account absent,
connected, needs connecting × destination the credential is for, another of the connector's. For
each, it records whether the test service received the `Authorization` header. It does exactly
when: credential declared, `account: true`, `uses: true`, connector granted, account connected,
destination one it is for.

## Hostile cases (FR-019)

All with an account connected through the test sign-in service, a fresh random token per case,
and the scan of R15 afterwards. Fixtures: `net-probe` (calls and dumps), `hostile/token-hunt`
(WASI, looks everywhere).

| Case | What it does | Expected |
| --- | --- | --- |
| `token-hunt` | Reads its environment, arguments and configuration; a `wasm32-wasip2` build opens the keychain files and host records | Nothing found; the WASI build refused before running |
| `echo-body` | Destination returns the `Authorization` value in its body | `failed(invalid-response)`; the widget gets nothing |
| `echo-header` | …in a declared response header | Same |
| `echo-location` | …in a redirect's `Location` | Same; the target not logged |
| `other-destination` | Calls the connector's destination the credential is not for | Sent without the header |
| `redirect-other-host` | Credential-bearing request redirected to another host of the connector | Followed without the header |
| `header-smuggling` | Values with CR/LF, `Authorization:` text, the scheme, the header's name | Refused by type or sent as data; no header added; never replaced |
| `fixed-auth-header` | A local connector fixes `Authorization` or the credential's header | `invalid-connector` |
| `plain-http-credential` | A local loopback connector without HTTPS declares a credential | `credential-needs-https`; nothing sent |
| `bad-certificate` | The destination's certificate is not valid for it | `failed(unavailable)` before any header is sent |
| `wrong-interface` | Calls an account connector through `connectors`, and a plain one through `accounts` | `not-granted` both ways |
| `no-account-use` | Widget without `account: true` calls while an account is connected | Sent without the header |
| `optional-off` | Account connector optional and off | `not-granted`; nothing sent |
| `copied-widget-id` | A file elsewhere claims an approved widget's id | Not granted; no account |
| `copied-connector-id` | A local connector file elsewhere claims the id of one with an account | No account at that origin |
| `credential-added-update` | Connector version adds a credential the widget uses | `needs-new-approval` |
| `access-widened-update` | Connector version adds an access item or raises a level | `needs-new-approval`; once approved, `account-not-connected` (less access) until connected again |
| `forged-callback` | Callback with a wrong or missing state | Sign-in ends; nothing connected; earlier account kept |
| `replayed-code` | Callback with a code already exchanged | Sign-in ends; nothing connected |
| `late-callback` | Callback after the time limit | Listener closed; nothing connected |
| `device-denied` | The test service answers `access_denied` | Nothing connected |
| `device-expired` | `expired_token` | Nothing connected |
| `device-slow-down` | `slow_down` twice | Polls at 5, then 10, then 15 s; never faster |
| `foreign-verification-address` | Device response names a verification address on another host | `sign-in-failed`; nothing shown to open |
| `another-account` | Sign-in returns another account's identity | `another-account-connected`; new token revoked or dropped; earlier account kept |
| `widget-connect` | Widget tries every call while no account is connected | `account-not-connected`; no browser, no sign-in |
| `revoked-token` | Test service answers 401 | One refresh; then `account-not-connected`; no further retries |
| `expired-refresh` | Refresh refused | `account-not-connected`; marked `refresh-failed` |
| `disconnect-mid-run` | Account disconnected by another process during an update | In-flight response dropped; later requests without the header or `account-not-connected` |
| `account-spoof` | Account name with line breaks and terminal escapes | Printed escaped by `accounts list`, `permissions`, `requests` |

## Other tests named

- `vault_round_trip` (real keychain, three systems): write, read, overwrite, delete, `NoEntry`;
  the 2,560-byte bound.
- `accounts_record` concurrency: two processes refreshing at once leave one valid token.
- `kept_data_account`: disconnect and reconnect another account discard kept data; reconnecting
  the same account keeps it.
- `contract_v1`: 1.3.0 only adds to 1.2.0; `accounts` answers `not-granted` first.
- `manifest_schema` and `connector_schema`: the schemas and the parsers agree on `account` and
  `credential`.
- `catalog`: every sign-in address and page of a catalog credential is under its `reserves`.
- `pkce`: the challenge of RFC 7636 appendix B's verifier.
