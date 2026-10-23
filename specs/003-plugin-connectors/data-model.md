# Data Model: Plugin Connectors

Host types and records of this feature. The data models of
[feature 001](../001-plugin-runtime/data-model.md) and [feature 002](../002-plugin-permissions/data-model.md)
still hold for packages, manifests, grants, limits and their error kinds; this file says what
changes.

## Connector

A JSON file, published as `wit/connector.schema.json` ([draft](contracts/connector.schema.json)),
at most 64 KiB. Unknown fields are refused, except `$schema`, which the host ignores
([research R1](research.md#r1-the-connector-format)).

| Field | Rule |
| --- | --- |
| `connector_version` | `1` |
| `id` | `catalog/<name>`, `local/<name>`, or a repository id (refused for now, see Load decision); `<name>` is `[a-z0-9](?:[a-z0-9-]{0,62}[a-z0-9])?` |
| `version` | SemVer 2.0.0; its major is what widgets depend on |
| `author` | 1–100 characters, no control characters |
| `description` | 1–200 characters, no control characters |
| `destinations` | 1–8 entries: name (`[a-z][a-z0-9-]{0,31}`) → origin (Destination) |
| `operations` | 1–64 entries: name (`[a-z][a-z0-9-]{0,63}`) → Operation |

A field named `credential`, `credentials` or `auth`, anywhere, is `credentials-not-supported`
(FR-017). Text fields are untrusted and escaped on display, as in feature 002.

### Destination

| Form | Allowed for |
| --- | --- |
| `https://<domain>[:<port>]` | Every connector |
| `http://localhost:<port>`, `http://127.0.0.1:<port>`, `http://[::1]:<port>` | Local connectors, in development (FR-018) |

`<domain>` is a DNS name, compared in one normal form: lowercase ASCII, internationalized labels
in their `xn--` form. No path (other than `/`), query, fragment, user or password. An IP address
outside the loopback forms above is `invalid-connector`. A destination whose host the catalog
reserves, in a connector outside the catalog, is `destination-reserved` (FR-005).

### Operation

| Field | Rule |
| --- | --- |
| `kind` | `read` (changes nothing in the service) or `change` |
| `method` | `GET`, `HEAD`, `POST`, `PUT`, `PATCH` or `DELETE`. Outside the catalog a `read` must be `GET` or `HEAD` |
| `destination` | A key of `destinations` |
| `path` | Starts with `/`; each segment is literal or exactly `{<parameter>}`; literal segments contain no `{`, `}`, `?`, `#` |
| `query` | Optional, at most 16 pairs: name → literal or exactly `{<parameter>}` |
| `headers` | Optional, at most 8: `Accept`, `Accept-Language` or `X-…` → literal value |
| `body` | Optional JSON object, at most 4 KiB as written; strings exactly `{<parameter>}` are replaced by typed values. Only for changes and for catalog reads |
| `parameters` | 0–16 entries: name (`[a-z][a-z0-9_]{0,31}`) → Parameter |
| `response.format` | `json` (media type `application/json` or `…+json`) or `text` (`text/…` or `…+xml`) |
| `response.headers` | 0–8 names passed to the widget; never `Set-Cookie` or `Cookie` |

Every placeholder names a declared parameter, and every declared parameter appears in exactly
one place. An optional parameter may not fill a path segment.

### Parameter

| Type | Fields | Check on each call |
| --- | --- | --- |
| `text` | `max_length` (1–2048), optional `pattern` (whole match, compiled at most 64 KiB) or `values` (1–64 strings) | A string within the length, matching the pattern or among the values; in a path segment, never `""`, `.` or `..` |
| `integer` | `minimum`, `maximum` (64-bit) | A whole number in the range |
| `flag` | — | `true` or `false` |

`required` defaults to `true`. Arguments are checked before anything is sent: an unknown name,
the same name twice, a missing required parameter or a value outside its type is
`value-outside-type`, naming the parameter and the rule.

## Official catalog

The connectors in `connectors/catalog/`, built into the host ([R2](research.md#r2-where-connectors-come-from-in-this-feature)).

| Field | Rule |
| --- | --- |
| id | `catalog/<name>`, the file's name |
| reserved hosts | Every host a catalog destination names, exactly (registrable domains: Phase 3) |

This feature ships `catalog/github` ([draft](contracts/catalog-github.json)).

## Manifest: `connectors`

A new optional field of the manifest (`wit/manifest.schema.json`,
[draft](contracts/manifest.schema.json)); `manifest_version` stays `1`, and a manifest without it
declares no connector.

| Field | Rule |
| --- | --- |
| `connectors` | At most 16 entries, unique by `id` |
| `id` | A connector id, as above |
| `major` | 0–65535: the connector's major version the widget is built for |
| `operations` | 1–32 unique operation names the widget calls |
| `required` | As for permissions (feature 002): required connectors are approved with the other required permissions; optional ones start off |

## Permission registry

| Permission | Contract v0 | Contract v1 | Since |
| --- | --- | --- | --- |
| `log` | `dashboard:widget/log@0.1.0` | `dashboard:widget/log@1.<m>.0` | 1.0 |
| each connector | — | `dashboard:widget/connectors@1.<m>.0` | 1.1 |

Every connector the manifest declares is a permission of its own, carried by the one
`connectors` interface ([R12](research.md#r12-linking-the-connectors-capability)).

## Contract versions

| Contract | Package | Accepted from a plugin |
| --- | --- | --- |
| v0 | `dashboard:widget@0.1.0` | Exactly 0.1.0 |
| v1 | `dashboard:widget@1.1.0` | Any 1.m.p not newer than 1.1.0; every contract name of the component carries the same version |

A v1 plugin exports `renderer`, and also `updater` when its manifest declares a connector.

## Grant record, version 2

```json
{
  "record_version": 2,
  "plugins": {
    "local/github-releases": {
      "origin": "/home/ada/dashboard/target/plugins/github-releases.wasm",
      "granted": [],
      "approved_version": "0.1.0",
      "approved_digest": "sha256:9a4e…",
      "connectors": {
        "catalog/github": {
          "origin": "catalog",
          "version": "1.0.0",
          "granted": true,
          "destinations": ["https://api.github.com"],
          "operations": { "list-releases": "sha256:51c0…" }
        }
      }
    }
  }
}
```

| Field | Rule |
| --- | --- |
| `record_version` | `2`; a version 1 record is read as version 2 with no connectors, and written back as 2 |
| `connectors` | One entry per connector the approved manifest declared |
| `origin` | `catalog`, or the canonical path of the local connector file |
| `granted` | Required connectors: `true`. Optional ones: `true` only while on |
| `destinations` | Every origin of the connector, sorted |
| `operations` | For each operation the widget declared: SHA-256 of its canonical declaration |

| Change | Effect |
| --- | --- |
| `approve` | As in feature 002 for permissions; each declared connector recorded with its current origin, destinations and digests; required ones granted, optional ones keep their state when nothing changed and start off otherwise |
| `optional <plugin> <connector-id> on\|off` | That connector's `granted`; refused for a required one (`required-together`); off discards the kept data |
| `remove` | Entry deleted, kept data discarded |

## Kept data

`<data>/widget-data/<hex SHA-256 of "<id>\n<origin>">.json`, written atomically.

| Field | Meaning |
| --- | --- |
| `id`, `origin`, `digest` | The package that produced the data; checked on read |
| `approval` | The approved digest and the connectors that were on when the data was fetched |
| `fetched_at_ms` | When the last successful update finished |
| `data` | The JSON document `update` returned (at most 256 KiB) |
| `last_update` | `at_ms`, `outcome` (`ok` or `failed`) and, when failed, `error` (`<kind>: <detail>`) |

Discarded (clarification 4): by `remove`; by `optional … off`; and on load when the package digest
differs (a new version), the approval differs, or a connector recorded as on is now off. A
failed update leaves `data` and `fetched_at_ms` and updates `last_update` only.

### What render receives

For a widget that exports `updater`, the configuration `render` gets is
([contracts/widget-v1.1.wit](contracts/widget-v1.1.wit)):

```json
{
  "config": { "owner": "rust-lang", "repo": "rust" },
  "data": { "releases": [] },
  "fetched_at_ms": 1792761600000,
  "last_update": { "at_ms": 1792761600000, "outcome": "ok" }
}
```

`data` and `fetched_at_ms` are `null` before the first successful update; `last_update` is `null`
before the first update. A widget that exports no `updater` gets its configuration as before.

## Request log entry

One JSON object per line of `<data>/requests.jsonl` ([R14](research.md#r14-kept-data-and-the-request-log)).

| Field | Meaning |
| --- | --- |
| `at_ms` | When the call started |
| `plugin` | Plugin id |
| `connector`, `operation`, `kind` | What was called |
| `method`, `host`, `path` | The request as built (absent when refused before building) |
| `arguments` | Name → value as sent, each cut to 256 characters with ` [truncated]` |
| `outcome` | `{ "status": 200 }`, `{ "refused": "<rule>", "detail": "…" }` or `{ "failed": "<rule>", "detail": "…" }` |
| `sent_bytes`, `received_bytes`, `duration_ms` | Totals for the call, redirects included |

## Load decision

`Runtime::load` as in feature 002, with these changes:

| Step | Check | Refusal |
| --- | --- | --- |
| 3 | The manifest, now with `connectors` | `invalid-manifest` |
| 5 | Contract: v1 accepts any minor up to 1.1.0, one version across all contract names; connectors need 1.1 or later; a widget with connectors exports `updater` | `contract-mismatch`, `contract-too-old`, `missing-updater` |
| 6 | Imports, as before; `connectors` needs at least one declared connector | `capability-refused`, `not-declared` |
| 6a | Each declared connector: found (catalog, or local file in development), parses, follows the destination rules, has the declared major and every declared operation | `invalid-connector`, `credentials-not-supported`, `destination-not-verified`, `destination-reserved`, `connector-unavailable` |
| 7 | Grants, as before, plus each required connector, and each optional one that is on, matches the entry (origin, destinations, operation digests) | `not-granted`, `needs-new-approval` (naming what changed) |
| 7a | Kept data still matches the package and the approval, or is discarded | — |
| 9 | Link: `connectors` real over a table of the granted connectors when there is one; the stand-in when none is granted (Principle II, v2.3.0) | `contract-mismatch` |

## A connector call

Inside `update`, for `call(connector, operation, arguments)`, in order; the first rule that
applies answers, and every call is logged:

1. The store runs `update`, not `render` — else `refused(called-outside-update)`.
2. The connector is in the plugin's table, built at link time from the granted connectors, and
   the grant store's generation still matches, or it is still granted — else `not-granted`.
3. The widget declared the operation — else `refused(operation-not-declared)`; the connector has
   it — else `refused(unknown-operation)`.
4. It is a read — else `refused(change-needs-user-action)`.
5. The arguments match the parameters — else `refused(value-outside-type)`.
6. Calls per update, rate per widget and per destination host — else `refused(over-limit)`.
7. Build the request; resolve; keep public addresses; connect; TLS — `failed(non-public-address)`
   or `failed(unavailable)`.
8. Response headers within 16 KiB, body within the per-call and per-update limits, time within
   the call's limit — else `failed(over-limit)`; the widget gets none of the body.
9. A redirect: follow it under R7 — else `failed(redirect-not-allowed)`.
10. The media type matches `response.format` — else `failed(invalid-response)`.
11. The generation still matches — else `not-granted`, and the response is dropped.
12. `ok(response)`: status, the declared headers, the body.

## Error kinds

New kinds a load, an approval or an update can end with (CLI contract names):

| Kind | When |
| --- | --- |
| `invalid-connector` | A connector file is too large, not JSON, breaks the schema or a rule above |
| `credentials-not-supported` | A connector declares a credential (FR-017) |
| `destination-not-verified` | A connector id names a repository (FR-004) |
| `destination-reserved` | A connector outside the catalog claims a catalog id or a reserved host (FR-005, FR-018) |
| `connector-unavailable` | A declared connector is not found, has another major, or lacks a declared operation (FR-006) |
| `contract-too-old` | A widget declares a connector but is built for a contract older than 1.1.0 (FR-016) |
| `missing-updater` | A widget declares a connector but exports no `updater`, or `update` is asked of a widget without one |

An update that fails for its own reasons uses feature 001's kinds (`plugin-error`,
`time-limit-exceeded`, `cpu-budget-exceeded`, `memory-limit-exceeded`, `plugin-crashed`,
`invalid-output`, `output-too-large`).

Rules of a refused or failed call, seen by the widget in `call-error` and in the log (FR-019):

| Rule | Refused (nothing sent) or failed |
| --- | --- |
| `operation-not-declared`, `unknown-operation`, `value-outside-type`, `called-outside-update`, `change-needs-user-action` | Refused |
| `over-limit` | Refused (calls, rate) or failed (size, time) |
| `non-public-address`, `redirect-not-allowed`, `unavailable`, `invalid-response` | Failed |
| `not-granted` | Its own case of `call-error`: nothing sent |

## Combinations (SC-002)

For one connector and one operation, a request leaves the machine only in the first row:

| Declared by the widget | Connector granted | Operation declared | Called in | Kind | Destination | Result |
| --- | --- | --- | --- | --- | --- | --- |
| Yes | Yes | Yes | `update` | Read | Allowed for its kind | Request sent |
| No | — | — | — | — | — | `not-declared` at load (import without a connector) or `refused(operation-not-declared)` |
| Yes, optional, off | No | — | `update` | — | — | `not-granted` |
| Yes | Yes | No | `update` | — | — | `refused(operation-not-declared)` |
| Yes | Yes | Yes | `render` | — | — | `refused(called-outside-update)` |
| Yes | Yes | Yes | `update` | Change | — | `refused(change-needs-user-action)` |
| Yes | Yes | Yes | `update` | Read | Non-public address | `failed(non-public-address)` |
| Yes | — | — | — | — | Repository id | `destination-not-verified` at load |
| Yes | — | — | — | — | Reserved host, outside the catalog | `destination-reserved` at load |

## Hostile cases (FR-020)

Each runs in `runtime/tests/hostile.rs` on the three systems; the test services record every
request, and a refused case must leave them empty.

| Case | Rule | Expected |
| --- | --- | --- |
| `generic-http` | FR-001 | A component importing `wasi:http/outgoing-handler` is `capability-refused` |
| `read-with-body` | FR-002 | A local connector with a `POST` read is `invalid-connector` |
| `value-outside-type` | FR-003 | Too long, wrong pattern, out of range, unknown, missing, `..` in a path: refused, nothing sent |
| `repository-connector` | FR-004 | A connector with a repository id is `destination-not-verified` |
| `reserved-host` | FR-005 | A local connector on `api.github.com`, and one claiming `catalog/github`, are `destination-reserved` |
| `undeclared-operation` | FR-006 | An operation the manifest does not declare, and one the connector lacks: refused |
| `private-address` | FR-007 | A name leading to `10.0.0.1`, `127.0.0.1`, `::1`, `169.254.169.254`, `::ffff:192.168.0.1`; an IP destination; plain HTTP outside development: refused, nothing sent |
| `rebinding` | FR-007 | A name public on the first lookup and private on the second: the second call is refused |
| `redirect-escape` | FR-007 | Redirects to an undeclared host, to a private address and to plain HTTP: not followed |
| `system-proxy` | FR-007 | `HTTPS_PROXY`, `HTTP_PROXY`, `ALL_PROXY` set: the proxy server receives nothing |
| `cookie` | FR-007 | A `Set-Cookie` answer: the next request carries no `Cookie` |
| `over-limits` | FR-008 | A body over 1 MiB, 21 calls, a slow service, a rate burst: refused or stopped, widget gets nothing over the limit |
| `call-in-render` | FR-010 | A call from `render`: refused, nothing resolved or sent |
| `change-in-update` | FR-013 | A call to a change: refused |
| `optional-off` | FR-015 | An optional connector that is off has no entry in the table and answers `not-granted`, nothing sent; turning it off mid-update drops the response in flight; turning one on mid-update leaves it off until the next update |
