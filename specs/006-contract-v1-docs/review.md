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
| F7 | `storage`, `storage/rule/run-aborted`, `updater/update` | In `update`, a store that cannot be opened for writing (busy, damaged, unavailable) aborts the run, and the update fails as `storage-failed` even when the plugin handled the error and returned `ok`, so its data is not kept; writes are also dropped when the returned data is refused. | `storage/connection.rs:104`, `storage/mod.rs:699-704`, `storage/mod.rs:494-502`, `lib.rs:1431-1437`; reproduced by the reviewer | host, chosen by the maintainer: T057. An update that cannot open its store gets the error, then `run-aborted` for every later call, so it still never keeps part of a run, and what it returns stands; a damaged store no longer fails every update of a widget that handles the error. Writes dropped with refused data stay as they are and are documented (T005, T021) | Two tests of feature 004 change their expectation, the hostile `foreign_store` and `storage.rs`'s busy case; no case is removed |
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

## Review of the prose (T058)

The drift test checks that every item is named with its default, and the runner that every command
prints what the page shows; neither reads what the text says. Three independent agents with none
of this feature's context, on 2026-10-26, over commit df4bb6c, read the pages sentence by sentence
against `wit/history/1.3.0/widget.wit`, `runtime/src/`, the schemas and the examples, first where
safety rests on the text: one read `contract.md` and `errors.md`, one the other reference pages,
one the walkthrough and the guides. The second also ran the release CLI against scratch
directories to confirm P23, P24 and P25. Each finding was checked at the cited source before the
page changed; line numbers are at df4bb6c.

| id | page | what | checked | outcome |
| --- | --- | --- | --- | --- |
| P1 | `contract.md` (`accounts/call`) | "When the service answers 401, the host refreshes the token and sends the request once more, then may call the identity operation": a key is never refreshed, an account refreshed earlier in the call is not resent, and without an identity operation (every key) one 401 marks the credential for every widget that uses it | `calls.rs:594-596`, `calls.rs:1047-1048`, `calls.rs:1099-1100`; 005 research §302 confirms only "when declared" | docs: the paragraph says each case; the marking without an identity operation is 005's design and stays |
| P2 | `errors.md` (`credential-needs-https`) | Plain HTTP "only to the local network": only a local connector in development, only to this machine | `connector.rs:1020-1034`, `network/resolver.rs:22-23` | docs |
| P3 | `contract.md` (`over-limit`, `failed`) | "Keep what the previous update fetched": `update` cannot read it, and `ok` replaces it | `kept.rs:158-163`, `kept.rs:173-181` | docs: return an error to keep it |
| P4 | `contract.md` (storage `unavailable`) | "Carry on without the store": a write that answers it ends the run's writes and fails the update | `storage/mod.rs:740-760`, `storage/mod.rs:496-507`; hostile `unwritable_store` | docs |
| P5 | `contract.md` (render's `data`) | "The last data update returned": `null` again once kept data is discarded (package, approval, connector off, account) | `kept.rs:124-150` | docs |
| P6 | `contract.md` (storage `not-granted`, `update`), `errors.md` (`not-granted`) | Storage turned off during an update fails it as `not-granted`; a failed commit fails it too; host `not-granted` also means approved from another file | `storage/mod.rs:525-536`, `permissions.rs:483-491` | docs |
| P7 | `contract.md` (`storage`) | "The first call opens the store": the first that reaches it, and optional storage whose data directory was unavailable at load answers `unavailable` on every call, never `run-aborted` | `storage/mod.rs:688-700`, `lib.rs:1312-1320` | docs |
| P8 | `contract.md` (`account-not-connected`) | "Show that the account needs connecting" also for the case where only this request was refused and the account needs nothing | `calls.rs:1104-1107`; the WIT's own comment | docs |
| P9 | `contract.md` (`rule`) | "With `refused`, nothing was sent": through `accounts`, a token refresh may have gone first | `calls.rs:536-566` | docs |
| P10 | `contract.md` (`accounts/call`) | "Counts as one call for the limits": one for calls per run and per minute; each request has its own time limit and every body counts toward the update's | `calls.rs:440`, `network/mod.rs:521`, `calls.rs:610-619` | docs |
| P11 | `errors.md` (`account-not-connected`) | Also given for a damaged or unwritable accounts record, no random source, and `disconnect` without `--origin` | `accounts.rs:370`, `:414`, `:437`; `lib.rs:713-721` | docs |
| P12 | `errors.md` (`unknown-permission`) | "`log`, `storage` and one per connector": connectors are declared under `connectors` | `permissions.rs:140-156` | docs |
| P13 | `errors.md` (`destination-not-verified`), `guides/accounts.md` | Local connectors also load from `<data>/connectors/` | `lib.rs:190-194` | docs, both pages |
| P14 | `errors.md` (`storage-failed`) | `clear` deletes a damaged store; both commands can fail as unavailable | `storage/mod.rs:136-153` | docs |
| P15 | `cli.md` (`pack`), `getting-started.md` | "The package's digest, which approvals are tied to": an approval belongs to the file; the digest is display, part of the approval digest, and the key of kept data | `grants.rs:64-66`, `grants.rs:1001-1021`, `kept.rs:126-127` | docs, both pages |
| P16 | `cli.md` (`remove`), `guides/storage.md` | "Removes its store too" / "the data the host kept": the store is kept, as the command itself prints | `cli/mod.rs:1490-1500`; `tests/cli.rs:2064` | docs, both pages |
| P17 | `manifest.md` (`id`) | "A repository id is refused until Phase 3": it is accepted (002 decided so), and nothing checks it | `manifest.rs:174-189`; 002 spec, Assumptions | docs; the check belongs to installing from repositories (Phase 3) |
| P18 | `manifest.md` (approvals) | A connector that was optional and off and a new version requires loaded without a new approval, its calls answering `not-granted`; a permission in the same case is refused as `needs-new-approval` | `grants.rs:171`; `permissions.rs:844-849` for permissions; reproduced by the reviewer | host: `connector_changes` names it (`connector now required`), with a test that fails without it; the page now holds |
| P19 | `manifest.md` (approvals) | "An optional permission a new version adds needs no approval" read as covering connectors, which need one; optional `storage` turns on only with an approved quota | `grants.rs:166-169`, `grants.rs:874` | docs |
| P20 | `limits.md` (storage file cap) | "Ends the run's writes": later calls answer `run-aborted`, nothing is kept, the update fails | `storage/mod.rs:481-507` | docs |
| P21 | `connector-format.md` (`minimum`, `maximum`) | Required "No": required for `integer` | `connector.rs:1427`; the schema | docs |
| P22 | `connector-format.md` (credential) | "Adds it to each request to the credential's destinations": only to calls through `accounts` | `calls.rs:119-145` | docs |
| P23 | `connector-format.md`, `guides/writing-connectors.md` | Non-public addresses are "refused": the call fails as `failed(non-public-address)` (`redirect-not-allowed` after a redirect); in development too, only `http://` loopback destinations reach this machine | `network/mod.rs:550-560`, `calls.rs:647` | docs, both pages |
| P24 | `connector-format.md` (new version) | Approved again "by every widget that uses it": by those that use its account, or call it when the credential is required | `grants.rs:236` | docs |
| P25 | `connector-format.md` (`access_page`, `revoke_page`) | "On a host of `authorize`": on a destination's host (catalog: under `reserves`) | `connector.rs:1919-1979` | docs |
| P26 | `limits.md` (`max_calls_per_run`) | Order of checks reversed and incomplete; calls past it are one summary entry | `calls.rs:459-525` | docs |
| P27 | `limits.md` (per-minute rates) | Buckets live in one running host; each CLI command starts afresh | `calls.rs:338-344` | docs |
| P28 | `limits.md` (`max_callback_head_bytes`) | Past it the request is dropped and the sign-in keeps waiting, not `sign-in-failed` | `network/callback.rs:85-95` | docs |
| P29 | `limits.md` (`max_sign_in_response_bytes`) | The identity operation has the 1 MiB response limit; a revocation over it is reported as failed | `signin.rs:230`, `signin.rs:294` | docs |
| P30 | `limits.md` (`refresh_margin`) | Only when the service said when the token expires | `calls.rs:773-776` | docs |
| P31 | `cli.md` (`accounts connect --origin`) | Not for a connector the CLI cannot find: it is checked against the one found | `cli/mod.rs:1757-1770` | docs |
| P32 | `cli.md` (`approve`) | Without `--expect` the CLI prints usage; `approval-changed` is a digest that differs | `cli/mod.rs:1237-1241` | docs |
| P33 | `cli.md` (`requests`) | Also render's refused calls, a summary entry past the limit, and the host's own sign-in and refresh requests | `calls.rs` `RunLog`, `host_entry`, `identity_entry` | docs |
| P34 | `manifest.md` (kept data) | Read as a complete list: kept data also goes when the package or the approval changes | `kept.rs:124-130` | docs |
| P35 | `guides/accounts.md` (disconnecting) | "No widget shows the account's notes once it is gone": only kept data fetched as the account is discarded; what a widget copied to its store stays, and a required credential answers `account-not-connected` | `lib.rs:706-783`, `kept.rs:186-213`, `calls.rs:741-747`; 005 FR-013 discards kept data only | docs: says so and tells authors to keep account data out of storage. The maintainer chose, on 2026-10-26, to leave the host as it is in 006 and decide in the Phase 4 spec, with the taint review of Principle VI: the host cannot tell which part of a store came from an account |
| P36 | `guides/writing-connectors.md` (testing) | Plain HTTP to this machine read as usable for a connector with an account: every destination of a credential is HTTPS on a public name | `connector.rs:1684-1690` | docs |
| P37 | `guides/connectors.md` (`not-granted`) | Also a response dropped in flight, and the answer for an undeclared connector or the wrong interface | `calls.rs:459-462`, `calls.rs:654-657`; the WIT | docs |
| P38 | `guides/writing-connectors.md` (new version) | "One that asks for less is not" approved again: any credential change, scopes included, is; only access items decide reconnecting | `connector.rs:3323-3344`, `grants.rs:235-262`, `calls.rs:752-770` | docs |
| P39 | `guides/storage.md` (answers) | `not-granted` mid-update and a failed write both fail the update keeping nothing | `storage/mod.rs:494-536`, `storage/mod.rs:740-800` | docs |
| P40 | `guides/connectors.md` (request log) | Values are cut at 256 characters, the destination exists once the request is built, and the log keeps 2 MiB plus one previous file | `requests.rs:1-9`, `requests.rs:155-162`, `limits.rs:148-149` | docs |
| P41 | `guides/accounts.md` (`account-not-connected`) | Also when only this request was refused, or the record cannot be read | the WIT; `calls.rs:793-820` | docs |
| P42 | `guides/accounts.md` (answers) | A `refused` through `accounts` may follow a token refresh | `calls.rs:536-566` | docs |

No finding is `breaking`. P18 is the one host fix. Three findings rest on decisions of earlier
features that the pages now state, and are left to the maintainer: P1 (one 401 marks a key for
every widget that uses it, since a key has no identity operation to confirm with), P17 (any local
file may carry a repository id until Phase 3) and P35 (disconnecting an account does not clear
what widgets copied from it into their stores; deferred to the Phase 4 spec).
