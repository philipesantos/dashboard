# The connector format

A connector describes, as data, the operations a widget may call on one service. Its JSON Schema is
`wit/connector.schema.json` in the repository; `runtime check-connector <file>` checks a file
against it and against the rules only the host checks, listed with each field. How to write one,
and test it against your own service, is in [writing connectors](../guides/writing-connectors.md).

Paths below name a field inside the file: `operations/*/kind` is `kind` in every operation, and
`[]` stands for each item of an array. An unknown field, or a key given twice, is
`invalid-connector`, naming it; a field named `credentials` or `auth`, or `credential` anywhere but
at the top, is `credentials-not-supported` ([errors](errors.md)).

## The connector

| Field | Accepts | Required |
| --- | --- | --- |
| `$schema` | Any text; the host ignores it | No <!-- ref: connector $schema --> |
| `connector_version` | `1`, the version of this format | Yes <!-- ref: connector connector_version --> |
| `id` | `catalog/<name>` in the official catalog, `local/<name>` in development; `<name>` is 1–64 lowercase letters, digits and hyphens, never starting or ending with a hyphen. Repository ids arrive with the verification of their owners' domains, a later item of Phase 3; the app does not install a widget that declares one | Yes <!-- ref: connector id --> |
| `version` | A Semantic Versioning 2.0.0 version. Widgets depend on its major version | Yes <!-- ref: connector version --> |
| `author` | 1–100 characters, no control characters; the person sees it | Yes <!-- ref: connector author --> |
| `description` | 1–200 characters, no control characters; the person sees it | Yes <!-- ref: connector description --> |
| `reserves` | Catalog connectors only, and required there: the domains of the service, each reserving itself and every host under it, so no other connector can claim them | In the catalog <!-- ref: connector reserves --> |
| `destinations` | 1–8 names, each an origin `https://<host>[:<port>]`, with nothing after it but an optional `/`. A DNS name, compared lowercase with international names as `xn--`; a trailing dot, an IP address, a path, a query, a user or a password are refused. In development only, also `http://localhost:<port>`, `http://127.0.0.1:<port>` or `http://[::1]:<port>` | Yes <!-- ref: connector destinations --> |
| `operations` | 1–64 operations by name: a lowercase letter, then lowercase letters, digits and hyphens | Yes <!-- ref: connector operations --> |
| `credential` | The one account or key the operations need, which the host adds itself (below) | No <!-- ref: connector credential --> |

The host connects only to public addresses: a name that leads to this machine, the local network
or a cloud metadata address is never connected to, checked at every connection, and the call fails
as `failed(non-public-address)`, or `failed(redirect-not-allowed)` after a redirect. Only a
development connector's `http://` destinations on `localhost`, `127.0.0.1` or `[::1]` reach this
machine. It uses no proxy set in the system, and checks certificates against
Mozilla's root list, the same on every system. Redirects are followed only to an origin the same
connector declares, at most five times.

## Operations

| Field | Accepts | Required |
| --- | --- | --- |
| `operations/*/kind` | `read`, which changes nothing in the service, or `change`. Until user actions arrive (Phase 3), `update` may call only reads: a change is shown at approval and answers `refused(change-needs-user-action)`. Outside the catalog, a read is a `GET` or `HEAD` without a body or a fixed `X-` header | Yes <!-- ref: connector operations/*/kind --> |
| `operations/*/method` | `GET`, `HEAD`, `POST`, `PUT`, `PATCH` or `DELETE` | Yes <!-- ref: connector operations/*/method --> |
| `operations/*/destination` | A name of `destinations` | Yes <!-- ref: connector operations/*/destination --> |
| `operations/*/path` | At most 512 characters, starting with `/`; each segment literal, or exactly `{<parameter>}` | Yes <!-- ref: connector operations/*/path --> |
| `operations/*/query` | At most 16 pairs: a name, then a literal value of at most 256 characters or exactly `{<parameter>}`. Literal values are shown at approval and written to the request log: never a secret | No <!-- ref: connector operations/*/query --> |
| `operations/*/headers` | At most 8 fixed headers: `Accept`, `Accept-Language` and, in the catalog only, `X-…`; values printable ASCII, at most 256 characters. The host sets `Host`, `User-Agent`, `Content-Type` and the framing headers, and refuses `Authorization`, `Cookie` and `Proxy-*` | No <!-- ref: connector operations/*/headers --> |
| `operations/*/body` | A JSON object of at most 4 KiB; a string exactly `{<parameter>}` becomes that value. Only for changes, and for reads in the catalog | No <!-- ref: connector operations/*/body --> |
| `operations/*/parameters` | At most 16 parameters by name: a lowercase letter, then lowercase letters, digits and underscores. Each appears in exactly one place of the request | No <!-- ref: connector operations/*/parameters --> |
| `operations/*/response` | What the service answers | Yes <!-- ref: connector operations/*/response --> |
| `operations/*/response/format` | `json` (`application/json` or `…+json`) or `text` (`text/…` or `…+xml`). A success (2xx) with a body in another media type is `failed(invalid-response)`; an error status reaches the widget whatever its body | Yes <!-- ref: connector operations/*/response/format --> |
| `operations/*/response/headers` | At most 8 header names passed to the widget, never `Set-Cookie` or `Cookie`; no other header reaches it | No <!-- ref: connector operations/*/response/headers --> |

## Parameters

Every value a widget passes is checked against its parameter before anything is sent, and
encoded in its own place: a path value is percent-encoded except letters, digits and `-._~`; a
query value is form-encoded; a body value becomes a JSON string, number or boolean. An unknown
name, a name twice, a missing required parameter, a value outside its type, or more than 16
arguments is `refused(value-outside-type)`.

| Field | Accepts | Required |
| --- | --- | --- |
| `operations/*/parameters/*/type` | `text`, `integer` or `flag` | Yes <!-- ref: connector operations/*/parameters/*/type --> |
| `operations/*/parameters/*/required` | `true`, the default, or `false`. An optional parameter may not fill a path segment; when it is left out, its query pair or body field is dropped | No <!-- ref: connector operations/*/parameters/*/required --> |
| `operations/*/parameters/*/max_length` | For `text`, 1–2048 characters | For `text` <!-- ref: connector operations/*/parameters/*/max_length --> |
| `operations/*/parameters/*/pattern` | For `text`, a regular expression of 1–512 characters the whole value must match; it runs in linear time, so no value can stall a call. In a path segment, `""`, `.` and `..` are refused whatever the pattern | No <!-- ref: connector operations/*/parameters/*/pattern --> |
| `operations/*/parameters/*/values` | For `text`, instead of `pattern`: 1–64 allowed strings | No <!-- ref: connector operations/*/parameters/*/values --> |
| `operations/*/parameters/*/minimum` | For `integer`, the smallest value (64-bit) | For `integer` <!-- ref: connector operations/*/parameters/*/minimum --> |
| `operations/*/parameters/*/maximum` | For `integer`, the largest value (64-bit) | For `integer` <!-- ref: connector operations/*/parameters/*/maximum --> |

## Credential

The connector never holds the secret. The person connects the account or types the key in the
host's controls, the host keeps it in the system's keychain and adds it, in the one header it names, to each
request to the credential's destinations that a widget makes through `accounts`, as a widget whose
manifest declares the connector with `"account": true`; calls through `connectors` never carry
it. No widget ever sees it. A response that
carries the credential back, in its body, a header or a redirect, raw or escaped, is
`failed(invalid-response)`. A connector with a credential has only HTTPS destinations
(`credential-needs-https`). A new version that adds or changes a credential, or asks for more
access, is approved again by every widget that uses its account, and by every widget that calls
the connector at all when the credential is required.

| Field | Accepts | Required |
| --- | --- | --- |
| `credential/kind` | `account`, signed in to in the browser, or `key`, typed by the person | Yes <!-- ref: connector credential/kind --> |
| `credential/required` | `false`, the default: without an account, calls go without it. `true`: without one, calls answer `account-not-connected` | No <!-- ref: connector credential/required --> |
| `credential/destinations` | 1–8 names of `destinations` the credential goes to, and no other; redirects to any other destination go without it | Yes <!-- ref: connector credential/destinations --> |
| `credential/header` | `Authorization`, the only value outside the catalog; a catalog connector may name its service's own header | Yes <!-- ref: connector credential/header --> |
| `credential/scheme` | With `Authorization`, and required there: a token such as `Bearer`; the header is `<scheme> <secret>` | With `Authorization` <!-- ref: connector credential/scheme --> |
| `credential/access` | With `kind: key` only: what the key lets the connector do, as `access` items below | No <!-- ref: connector credential/access --> |
| `credential/access[]/name` | What the item reaches, in words the person reads at approval | Yes <!-- ref: connector credential/access[]/name --> |
| `credential/access[]/level` | `read` or `write` | Yes <!-- ref: connector credential/access[]/level --> |
| `credential/subscription_required` | Up to 4 statuses from 400 to 499 that mean a paid plan is missing: the widget gets `subscription-required`, without the response | No <!-- ref: connector credential/subscription_required --> |
| `credential/account` | With `kind: account` only: how the person signs in | With `account` <!-- ref: connector credential/account --> |
| `credential/account/flow` | `authorization-code`: the authorization code with PKCE in the system's browser, redirected to a loopback address of this machine. The host keeps no client secret, so the service must accept a public client | Yes <!-- ref: connector credential/account/flow --> |
| `credential/account/client` | The client registration the host uses; in development, the id given with `runtime accounts connect --client <id>` | Yes <!-- ref: connector credential/account/client --> |
| `credential/account/authorize` | An `https` address on a host of a destination (in the catalog, under `reserves`), where the browser goes | Yes <!-- ref: connector credential/account/authorize --> |
| `credential/account/token` | An `https` address on one of the credential's destinations | Yes <!-- ref: connector credential/account/token --> |
| `credential/account/revoke` | An `https` address on one of the credential's destinations; `runtime accounts disconnect` revokes there | No <!-- ref: connector credential/account/revoke --> |
| `credential/account/scopes` | Up to 32 scope names asked for at sign-in: only those the operations need | No <!-- ref: connector credential/account/scopes --> |
| `credential/account/access` | 1–32 items: what the account lets the connector do, read at approval and at sign-in | Yes <!-- ref: connector credential/account/access --> |
| `credential/account/access[]/name` | What the item reaches | Yes <!-- ref: connector credential/account/access[]/name --> |
| `credential/account/access[]/level` | `read` or `write`. Adding an item, or raising one to `write`, needs a new approval and a new sign-in | Yes <!-- ref: connector credential/account/access[]/level --> |
| `credential/account/identity` | A read operation of the connector, `GET` without parameters, on one of the credential's destinations, that names the account. The host calls it after sign-in, and after a 401 to tell a revoked token from a refusal of one operation. Without it, a 401 never marks the account as needing to be connected again | No <!-- ref: connector credential/account/identity --> |
| `credential/account/identity/operation` | The operation's name | Yes <!-- ref: connector credential/account/identity/operation --> |
| `credential/account/identity/name` | A JSON pointer to the account's name in the answer | Yes <!-- ref: connector credential/account/identity/name --> |
| `credential/account/identity/id` | A JSON pointer to the account's stable id in the answer | Yes <!-- ref: connector credential/account/identity/id --> |
| `credential/account/access_page` | An `https` address on a host of a destination (in the catalog, under `reserves`), where the person chooses what the account reaches | No <!-- ref: connector credential/account/access_page --> |
| `credential/account/revoke_page` | An `https` address on a host of a destination (in the catalog, under `reserves`), where the person revokes the account by hand when there is no `revoke` | No <!-- ref: connector credential/account/revoke_page --> |

Tokens and keys are bearer tokens: letters, digits and `-._~+/=`; a key is 1 to 1,024 of them.
