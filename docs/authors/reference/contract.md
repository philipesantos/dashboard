# The contract

Contract v1 at 1.3.0, the WIT package `dashboard:widget@1.3.0`, is everything a widget and the host
say to each other. It is published as `wit/history/1.3.0/widget.wit` in the repository and never
changes; a widget binds its own copy ([the walkthrough](../getting-started.md)). The names below
are the WIT names; `wit-bindgen` turns them into Rust ones (`call-error` into `CallError`,
`not-granted` into `NotGranted`).

Each item says the minor it arrived in. A widget built for an older minor keeps running; one built
for a newer minor than the host's is refused as `contract-mismatch` ([errors](errors.md)).

**The rule of every capability.** Every function of a capability interface (`log`,
`connectors`, `accounts`, `storage`) returns `result<T, E>`, and the first case of `E` is
`not-granted`: the answer of an optional permission the person has not turned on, or one whose
approval was removed. A widget handles it like any other answer, usually by showing less, and
never fails because of it.

## Worlds

A world is what a widget imports and exports. Pick the smallest one that has what the widget
uses; the manifest then asks for each import it has ([the manifest](manifest.md)).

<!-- ref: wit widget -->
### `widget`

Imports `log`; exports `renderer`. A widget that only renders. Since 1.0.0.

<!-- ref: wit connected-widget -->
### `connected-widget`

Imports `log` and `connectors`; exports `renderer` and `updater`. Fetches data through connectors
in `update`, and renders it. Since 1.1.0.

<!-- ref: wit stored-widget -->
### `stored-widget`

Imports `log` and `storage`; exports `renderer` and `updater`. Writes its own store in `update` and
reads it in `render`. Since 1.2.0.

<!-- ref: wit connected-stored-widget -->
### `connected-stored-widget`

Imports `log`, `connectors` and `storage`; exports `renderer` and `updater`. Since 1.2.0.

<!-- ref: wit account-widget -->
### `account-widget`

Imports `log`, `connectors` and `accounts`; exports `renderer` and `updater`. Calls some
connectors as the person's account. Since 1.3.0.

<!-- ref: wit account-stored-widget -->
### `account-stored-widget`

Imports `log`, `connectors`, `accounts` and `storage`; exports `renderer` and `updater`; the world
the host binds. Since 1.3.0.

## `log`

<!-- ref: wit log -->
Writes log messages, attributed to the widget; carried by the `log` permission. Since 1.0.0.

<!-- ref: wit log/level -->
### `level`

| Case | Meaning |
| --- | --- |
| `debug` | Detail for the author <!-- ref: wit log/level/debug --> |
| `info` | What the widget is doing <!-- ref: wit log/level/info --> |
| `warn` | Something the maintainer may want to look at <!-- ref: wit log/level/warn --> |
| `error` | Something went wrong <!-- ref: wit log/level/error --> |

<!-- ref: wit log/log-error -->
### `log-error`

| Case | When | What to do |
| --- | --- | --- |
| `not-granted` | `log` is optional and off: nothing was written | Carry on without logging <!-- ref: wit log/log-error/not-granted --> |

<!-- ref: wit log/write -->
### `write(level, message) -> result<_, log-error>`

Writes a message. The host keeps at most 100 messages per call and cuts each to 1 KiB; the excess
is dropped and reported, and the call still answers `ok`. Every message is escaped when shown, so
none can add a line or pass for the host's own output.

## `connectors`

<!-- ref: wit connectors -->
Calls the operations of the connectors the manifest declares, each a permission of its own. The
host makes every request: it checks the call against the connector and the manifest, and refuses
it before sending anything when a rule says so. Calls work only while the host runs `update`.
Since 1.1.0. [The connectors guide](../guides/connectors.md) walks through one.

<!-- ref: wit connectors/value -->
### `value`

A value for one parameter, of the type the operation declares for it.

| Case | Meaning |
| --- | --- |
| `text(string)` | A `text` parameter <!-- ref: wit connectors/value/text --> |
| `integer(s64)` | An `integer` parameter <!-- ref: wit connectors/value/integer --> |
| `flag(bool)` | A `flag` parameter <!-- ref: wit connectors/value/flag --> |

<!-- ref: wit connectors/argument -->
### `argument`

One argument of a call.

| Field | Meaning |
| --- | --- |
| `name` | The parameter's name <!-- ref: wit connectors/argument/name --> |
| `value` | Its value <!-- ref: wit connectors/argument/value --> |

<!-- ref: wit connectors/header -->
### `header`

A response header the operation declares. Headers it does not declare never reach the widget.

| Field | Meaning |
| --- | --- |
| `name` | The header's name, as the operation declares it <!-- ref: wit connectors/header/name --> |
| `value` | `none` when the response did not carry it. A header the response repeats arrives as its values joined with `, `, and bytes that are not UTF-8 are replaced <!-- ref: wit connectors/header/value --> |

<!-- ref: wit connectors/response -->
### `response`

What the service answered. Any status is a normal answer, error statuses included: a widget
decides what a 404 means for it.

| Field | Meaning |
| --- | --- |
| `status` | The HTTP status <!-- ref: wit connectors/response/status --> |
| `headers` | The declared headers, in the order the operation declares them <!-- ref: wit connectors/response/headers --> |
| `body` | The body as received, within the size limits; never part of a larger one <!-- ref: wit connectors/response/body --> |

<!-- ref: wit connectors/rule -->
### `rule`

The rule behind a call that was refused or failed. With `refused`, nothing the widget asked for
was sent (through `accounts`, a token refresh of the host's own may have gone first); with
`failed`, the call could not complete and nothing reached the widget.

| Case | When | What to do |
| --- | --- | --- |
| `operation-not-declared` | The manifest does not declare this operation for this connector | Declare it; it is a mistake to fix, not to handle <!-- ref: wit connectors/rule/operation-not-declared --> |
| `value-outside-type` | An argument is unknown, repeated, missing or outside its parameter's type, or the call passes more than 16 arguments | Check the value before calling, or show that it is invalid <!-- ref: wit connectors/rule/value-outside-type --> |
| `called-outside-update` | The call came from `render`, which never reaches the network | Call from `update`, and render what it kept <!-- ref: wit connectors/rule/called-outside-update --> |
| `change-needs-user-action` | The operation changes data; until user actions arrive (Phase 3), `update` only reads | Call reads only <!-- ref: wit connectors/rule/change-needs-user-action --> |
| `over-limit` | A limit: calls per run or per minute, the request's address or body, the response's body or headers, the update's bodies together, or time ([limits](limits.md)) | Call less; to keep the previous data, return an error from `update`: the host keeps it, while `ok` replaces it <!-- ref: wit connectors/rule/over-limit --> |
| `non-public-address` | The destination's name led only to addresses that are not public | Nothing the widget can do; show that the service is unreachable <!-- ref: wit connectors/rule/non-public-address --> |
| `redirect-not-allowed` | The service redirected to a destination the connector does not declare, to one whose name leads only to non-public addresses, or more than five times | As above <!-- ref: wit connectors/rule/redirect-not-allowed --> |
| `unavailable` | The request could not complete: name resolution, connection or certificate; through `accounts`, also the system's keychain or a stored credential the host cannot send | Show that the service is unreachable; the next update tries again <!-- ref: wit connectors/rule/unavailable --> |
| `invalid-response` | The response is not in the format the operation declares, or, through `accounts`, it carried the credential back | Show that the answer could not be read <!-- ref: wit connectors/rule/invalid-response --> |

<!-- ref: wit connectors/problem -->
### `problem`

| Field | Meaning |
| --- | --- |
| `rule` | The rule <!-- ref: wit connectors/problem/rule --> |
| `detail` | A message for the widget's author; show it to the maintainer, never parse it <!-- ref: wit connectors/problem/detail --> |

<!-- ref: wit connectors/call-error -->
### `call-error`

| Case | When | What to do |
| --- | --- | --- |
| `not-granted` | The connector is optional and off, its approval was removed, or the manifest does not declare it. Nothing was sent; or, when the approval was removed while the request was in flight, the response was dropped | Show the widget without this connector's data <!-- ref: wit connectors/call-error/not-granted --> |
| `refused(problem)` | The host refused the call before sending anything | See the rule <!-- ref: wit connectors/call-error/refused --> |
| `failed(problem)` | The call could not complete and no response reached the widget: the request failed, or the host stopped it before sending, such as when the update's time ran out | See the rule; to keep the previous data, return an error from `update`: the host keeps it, while `ok` replaces it <!-- ref: wit connectors/call-error/failed --> |

<!-- ref: wit connectors/call -->
### `call(connector, operation, arguments) -> result<response, call-error>`

Calls `operation` of `connector`, an id the manifest declares, such as `catalog/github`, with
`arguments`. One call at a time; the host follows allowed redirects itself. A connector the
manifest declares with `"account": true` is called through `accounts`: here it answers
`not-granted`.

## `accounts`

<!-- ref: wit accounts -->
Calls the operations of connectors the manifest declares with `"account": true`, as the account
connected to that connector. The host adds the credential to the request itself: the widget never
sees it, and nothing it passes can name it. Calls work only while the host runs `update`. A
connector declared without an account answers `not-granted` here, and one declared with an
account answers `not-granted` through `connectors`. Which account a widget acts as is fixed when
the host loads it: one connected later is used from the next load. Since 1.3.0.
[The accounts guide](../guides/accounts.md) walks through one.

<!-- ref: wit accounts/argument -->
### `argument`

The type of `connectors`, used here as it is.

<!-- ref: wit accounts/response -->
### `response`

The type of `connectors`, used here as it is.

<!-- ref: wit accounts/problem -->
### `problem`

The type of `connectors`, used here as it is: its rule is one of `connectors/rule`.

<!-- ref: wit accounts/account-call-error -->
### `account-call-error`

| Case | When | What to do |
| --- | --- | --- |
| `not-granted` | The connector is optional and off, its approval was removed, or the manifest does not declare it with an account. Nothing was sent; or, when the approval was removed while the request was in flight, the response was dropped | Show the widget without this connector's data <!-- ref: wit accounts/account-call-error/not-granted --> |
| `refused(problem)` | The host refused the call before sending it; a token refresh of the host's own may have gone first | See the rule <!-- ref: wit accounts/account-call-error/refused --> |
| `failed(problem)` | The call could not complete and no response reached the widget: the request failed, or the host stopped it before sending, such as when the keychain could not be read. A response that carried the credential back is `invalid-response` | See the rule <!-- ref: wit accounts/account-call-error/failed --> |
| `account-not-connected` | No account is connected and the credential is required, or the account must be connected again: the service refused it, it could not be refreshed, or the connector now asks for more access than it holds. Also when the service refused this one request as unauthorized even after a refresh while the account still works for its identity operation, or to a key or an account that declares no identity operation, and when the host cannot read its record of accounts. Nothing was sent, or the response was dropped | Show that the account may need connecting; only the person can connect it, in the host's controls. When only this request was refused, the account needs nothing and other operations may work <!-- ref: wit accounts/account-call-error/account-not-connected --> |
| `subscription-required` | The service answered, with a status the connector declares as meaning that, that a paid plan this operation needs is missing; also for a request sent without an account. The response was dropped | Show that the operation needs a plan the account does not have <!-- ref: wit accounts/account-call-error/subscription-required --> |

<!-- ref: wit accounts/call -->
### `call(connector, operation, arguments) -> result<response, account-call-error>`

Calls `operation` of `connector`, an id the manifest declares with an account. When no account
was connected as the host loaded the widget and the credential is optional, the request goes
without one, and the service answers as it does for anyone. One call at a time; the host follows
allowed redirects itself and refreshes an expired token at most once per call. When a request
that carried the credential gets 401, the host refreshes an account's token and sends the request
once more, unless this call refreshed it already. If the answer is still 401, the host calls the
connector's identity operation, when it declares one, to tell whether the account still works. A
key is never refreshed, and neither a key nor an account without an identity operation can be
confirmed: a 401 then stops that call only and marks nothing, so one widget cannot lock the
credential out for the others. A key the service no longer accepts keeps being sent, and refused,
until the person connects another. All
of that counts as one call for the calls-per-run and per-minute limits; each request has its own
time limit, and every response body counts toward the update's.

## `storage`

<!-- ref: wit storage -->
The widget's own store of named values, carried by the `storage` permission with its quota. The
host picks the store from the widget's id and origin: no function names a store, a file or a
place, and no widget reaches another's. Names are any non-empty text up to 256 bytes of UTF-8,
compared and sorted as bytes; values are bytes, up to 256 KiB. Reads work in `render` and
`update`; writes only in `update`, kept together only when `update` returns `ok` with data the
host keeps. In `update`, the first call that reaches the store opens it for writing; when it cannot
(`busy`, `damaged`, `unavailable`), that call answers so and every later call of the run answers
`run-aborted`: the run reads and writes nothing, and what `update` returns stands. When the data
directory was unavailable as the host loaded a widget whose storage is optional, every call
answers `unavailable` instead; required storage is then `storage-failed` at load. Since 1.2.0.
[The storage guide](../guides/storage.md) walks through one.

<!-- ref: wit storage/rule -->
### `rule`

| Case | When | What to do |
| --- | --- | --- |
| `render-only-reads` | A write or delete during `render` | Write in `update` <!-- ref: wit storage/rule/render-only-reads --> |
| `over-quota` | The write would take the store past the quota the manifest declares | Delete something first, or declare a larger quota (a new approval) <!-- ref: wit storage/rule/over-quota --> |
| `over-limit` | A limit: calls per run, bytes written per update, a value's size, or the store's file size. A write that finds no room on disk answers this too and ends the run, as `run-aborted` says | Write less per update <!-- ref: wit storage/rule/over-limit --> |
| `invalid-name` | The name is empty or longer than 256 bytes, or a `list` prefix or `after` is longer than that | A mistake to fix <!-- ref: wit storage/rule/invalid-name --> |
| `damaged` | The store's file is not one the host wrote for this widget; the host left it as it is | Show that the stored data is unavailable; the person can clear the store <!-- ref: wit storage/rule/damaged --> |
| `busy` | Another process is writing the store, past the time the call could wait | Try again in the next update <!-- ref: wit storage/rule/busy --> |
| `unavailable` | The store could not be read or written: the data directory is unavailable, or the file is read-only. A full disk answers `over-limit` | When opening the store answers it, carry on without the store. When a write answers it, the run's writes have ended and the update fails as `storage-failed`, whatever it returns <!-- ref: wit storage/rule/unavailable --> |
| `run-aborted` | An earlier call of this run ended its transaction, such as by reaching the file's size: nothing was done, nothing this run wrote is kept, and the update fails. Or an earlier call of `update` could not open the store: nothing was done, and the run reads and writes nothing more | Stop calling storage in this run <!-- ref: wit storage/rule/run-aborted --> |

<!-- ref: wit storage/problem -->
### `problem`

| Field | Meaning |
| --- | --- |
| `rule` | The rule <!-- ref: wit storage/problem/rule --> |
| `detail` | A message for the widget's author <!-- ref: wit storage/problem/detail --> |

<!-- ref: wit storage/storage-error -->
### `storage-error`

| Case | When | What to do |
| --- | --- | --- |
| `not-granted` | `storage` is optional and off, or it was turned off or its approval removed during this run: nothing was read or written, and nothing this run wrote will be kept; turned off during an `update`, it fails the update as `not-granted` | Show the widget without its stored data <!-- ref: wit storage/storage-error/not-granted --> |
| `refused(problem)` | The host refused the call before changing anything | See the rule <!-- ref: wit storage/storage-error/refused --> |
| `failed(problem)` | The store could not do it | See the rule <!-- ref: wit storage/storage-error/failed --> |

<!-- ref: wit storage/page -->
### `page`

One page of names, in byte order.

| Field | Meaning |
| --- | --- |
| `names` | At most 100 names <!-- ref: wit storage/page/names --> |
| `next` | The last name of this page when more remain: pass it as `after` for the next page <!-- ref: wit storage/page/next --> |

<!-- ref: wit storage/store-usage -->
### `store-usage`

| Field | Meaning |
| --- | --- |
| `used-bytes` | The bytes of every name and value, plus 64 per entry <!-- ref: wit storage/store-usage/used-bytes --> |
| `quota-bytes` | The quota the manifest declares <!-- ref: wit storage/store-usage/quota-bytes --> |

<!-- ref: wit storage/get -->
### `get(name) -> result<option<list<u8>>, storage-error>`

The value stored under `name`, or `none` when there is none.

<!-- ref: wit storage/set -->
### `set(name, value) -> result<_, storage-error>`

Stores `value` under `name`, replacing any value there. `update` only.

<!-- ref: wit storage/delete -->
### `delete(name) -> result<_, storage-error>`

Removes `name`; removing a name that is not there succeeds. `update` only.

<!-- ref: wit storage/list -->
### `list(prefix, after) -> result<page, storage-error>`

The names that start with `prefix` (empty: every name) and come after `after`, at most 100.

<!-- ref: wit storage/usage -->
### `usage() -> result<store-usage, storage-error>`

How much of its quota the store uses.

## `renderer`

<!-- ref: wit renderer -->
The entry point every widget implements. Since 1.0.0. [The render and update
guide](../guides/render-and-update.md) walks through both entry points.

<!-- ref: wit renderer/render -->
### `render(config) -> result<string, string>`

`config` is a JSON document chosen by the host. On success the widget returns its content as a
JSON document of at most 64 KiB, whose shape is the widget's own until Phase 3 defines the content
the host frame draws ([the index](../README.md)); on failure, a message for the maintainer, shown
as `plugin-error`.

For a widget that also exports `updater`, `config` is:

```json
{
  "config": "<the widget's configuration>",
  "data": "<the data the last successful update returned, or null>",
  "fetched_at_ms": "<when, or null>",
  "last_update": null
}
```

`data` is `null` before the first successful update, and again once the host discards what it
kept: when the package or its approval changes (a rebuild included), when a connector it used is
turned off, or when the account it was fetched as is disconnected. `fetched_at_ms` and
`last_update` go with it. `last_update` is either `null` or `{"at_ms": …, "outcome": "ok" | "failed", "error":
"<kind>: <detail>"}`, `error` only when the outcome is `failed`. The configuration is JSON, or a
JSON string of its text when it is not JSON. Each call runs in a fresh instance, with nothing kept
from the last one, and render never waits on the network.

## `updater`

<!-- ref: wit updater -->
The entry point of widgets that fetch data through connectors or write their store. Since 1.1.0.

<!-- ref: wit updater/update -->
### `update(config) -> result<string, string>`

`config` is the widget's configuration, chosen by the host. On success the widget returns its data
as a JSON document of at most 256 KiB, which the host keeps and gives to `render`, unless storage
fails the update: a write of the run failed and ended its transaction (storage `run-aborted`), the
store could not commit (`busy`, `damaged`, `unavailable`), or storage was turned off during the
run (`not-granted`). On
failure, a message for the maintainer, and the host keeps the previous data. Each call runs in a
fresh instance, with a 10-second limit and render's fuel and memory.
