# Review of contract 1.3.0

The review of the contract before it is frozen (spec FR-004 and FR-005, research R12). The
reviewer is someone who did not write the contract. Each finding is checked at the source
before anything is done about it. The contract is frozen only when no finding is `open`.

Outcomes:

- `docs`: fixed in the docs or in a doc comment of 1.3.0.
- `host`: fixed in the host, with a test that fails without the fix.
- `breaking`: fixed by a new major, which the maintainer chose (FR-006).
- `kept`: left as it is, with the reason.

## Reviewer

An independent agent with none of this feature's context, on 2026-10-25, over commit 8fae67b
(the tree `wit/v1` at 1.3.0, `runtime/src/`, `runtime/tests/`, the schemas and the docs of
feature 005). It reproduced F1 and F7 with tests in a copy of the tree, and ran the `contract`,
`contract_v1` and `permissions` suites there. Each finding below was then checked at the cited
source before it got its outcome. Line numbers are at 8fae67b.

## Findings

| id | item | what | checked | outcome | cost |
| --- | --- | --- | --- | --- | --- |
| F1 | `accounts`, `accounts/call` | The account a widget acts as is not fixed at load, as the WIT and `calls.rs:103-104` say: every relink, after any change to the grant record (even another plugin's approval), reads the accounts record again, so an account connected after load is used without a reload. | `lib.rs:1691-1705` relinks when the generation moved; `lib.rs:1224-1243` reads the accounts at link time; reproduced by the reviewer (third update sent the credential) | host: T055 keeps the accounts read at load across relinks, with a test that fails without it; it closes the adoption of an account the plugin was not loaded with (005's rule, Principle II) |  |
| F2 | `connectors/rule/over-limit` (response size) | `connectors` and `accounts` each get their own 4 MiB of response bodies per update, so one update can receive 8 MiB, against `limits.rs:57-58` and the docs. | `lib.rs:1509` and `lib.rs:1522` each start a `Calls` with `max_update_response_bytes` | host: T056 shares one budget between them per run, with a test that fails without it; it enforces a documented limit |  |
| F3 | `connectors/call-error/not-granted`, `accounts/account-call-error/not-granted` | "Nothing was sent" is not always true: step 12 drops a response received after the approval was removed in flight. | `calls.rs:646-649`; `tests/hostile/connectors.rs:834-852` asserts it | docs: both comments of 1.3.0 (T005) and the reference (T021) |  |
| F4 | `connectors/call-error/failed`, `accounts/account-call-error/failed` and `refused`, `connectors/rule/unavailable` | `failed` ("the request was attempted") is also answered when nothing was sent: keychain unreadable, a stored secret the host cannot send, time used up before the first hop; an `accounts` `refused` can follow the host's own token refresh. | `calls.rs:846`, `calls.rs:1128-1129`, `network/mod.rs:523-530`, `calls.rs:771` before `calls.rs:552-558` | docs: T005 and T021 |  |
| F5 | `accounts/account-call-error/account-not-connected` | Also answered for a 401 to one request while the identity check shows the account works (not marked), and when the accounts record cannot be read. | `calls.rs:1093-1096` and `tests/hostile/credentials.rs:1066-1094`; `calls.rs:797` | docs: T005 and T021 |  |
| F6 | `accounts/call` | After a 401 the host refreshes, resends the request and may call the identity operation: one call can reach the service three times plus a token request, counted once. | `calls.rs:584-631`, `calls.rs:1039-1086` | docs: T005 and T021 |  |
| F7 | `storage`, `storage/rule/run-aborted`, `updater/update` | In `update`, a store that cannot be opened for writing (busy, damaged, unavailable) aborts the run, and the update fails as `storage-failed` even when the plugin handled the error and returned `ok`, so its data is not kept; writes are also dropped when the returned data is refused. | `storage/connection.rs:104`, `storage/mod.rs:699-704`, `storage/mod.rs:494-502`, `lib.rs:1431-1437`; reproduced by the reviewer | docs: T005 and T021. The behaviour is kept: an update that cannot take its store never keeps part of its run (`storage/mod.rs:700-701`), the safe side of the choice |  |
| F8 | `connectors/rule/redirect-not-allowed` | Also answered for more than 5 redirects and for a redirect to a declared destination whose name leads only to non-public addresses. | `network/mod.rs:595-603`, `network/mod.rs:551-558` | docs: T005 and T021 |  |
| F9 | `connectors/rule/value-outside-type`; the limits table | More than 16 arguments is `refused(value-outside-type)`, not `over-limit` as `docs/plugin-authors.md:444` says. | `connector.rs:507-511`, reached from `calls.rs:498-502` | docs: T005, and the reference's limits page (T025) |  |
| F10 | `renderer/render` | `last_update.error` is present only when the outcome is `failed`; a configuration that is not JSON arrives as a JSON string of its text. | `kept.rs:69-71`; `lib.rs:1353-1354` | docs: T005 and T021 |  |
| F11 | `connectors/header/value` | A header the response repeats arrives as its values joined with `, `, non-UTF-8 bytes replaced; undocumented. | `network/mod.rs:652-668` | docs: T005 and T021 |  |
| F12 | `accounts/account-call-error/not-granted` | "(or its approval to use the account) was removed": no control removes account use alone; it comes from the manifest at approval, and a change refuses the load. | `grants.rs:686-690`, `grants.rs:246-248`, `lib.rs:1152-1161` | docs: T005 drops the parenthesis |  |
| F13 | `accounts/account-call-error/subscription-required` | Also answered for a request sent without an account (optional credential, none connected): the check is that the connector has a binding, not an account. | `calls.rs:636-642`, `AccountBinding` at `calls.rs:106-110` | docs: T005 and T021 |  |

No finding is `breaking`, and none is `open`. The reviewer's close calls that hold (the order of
answers past the call limit, `invalid-response` checking only the media type, an updater export
in a hand-made 1.0.0 component, a `subscription_required` status of 401) need nothing; the
reference states the first two (T021, T025).
