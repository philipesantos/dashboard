# Writing a connector

A connector describes, as data, the operations a widget may call on one external service:
where each request goes, which values the widget fills in, and what it gets back. It is never
code: the host reads it and makes every request itself, so a widget can reach nothing but the
operations a connector declares, with values the host checks. Widgets depend on a specific
connector, by its id and major version.

The format is the published schema [`wit/connector.schema.json`](../wit/connector.schema.json);
point `$schema` at it and your editor checks the file as you type. The host checks a few rules
no schema can express, listed below; `runtime check-connector` checks them all.

## An example

```json
{
  "$schema": "../../wit/connector.schema.json",
  "connector_version": 1,
  "id": "local/weather",
  "version": "0.1.0",
  "author": "You",
  "description": "Forecasts from my weather service",
  "destinations": { "api": "https://api.weather.example" },
  "operations": {
    "forecast": {
      "kind": "read",
      "method": "GET",
      "destination": "api",
      "path": "/v1/{city}/forecast",
      "query": { "days": "{days}", "units": "metric" },
      "headers": { "Accept": "application/json" },
      "parameters": {
        "city": { "type": "text", "max_length": 64, "pattern": "[a-z-]+" },
        "days": { "type": "integer", "minimum": 1, "maximum": 7, "required": false }
      },
      "response": { "format": "json", "headers": ["ETag"] }
    }
  }
}
```

## The fields

| Field | Rule |
| --- | --- |
| `connector_version` | `1` |
| `id` | `catalog/<name>` in the official catalog, `local/<name>` in development; `<name>` is 1–64 lowercase letters, digits and hyphens, a hyphen never first or last |
| `version` | [SemVer 2.0.0](https://semver.org). Widgets depend on its major version |
| `author`, `description` | 1–100 and 1–200 characters, no control characters. The user sees them |
| `reserves` | Catalog connectors only, and required there: the domains of the service, each reserving itself and every host under it |
| `destinations` | 1–8 names, each an origin: `https://<host>[:<port>]`, nothing after it but an optional `/` |
| `operations` | 1–64 names (a lowercase letter, then lowercase letters, digits and hyphens), each one request |
| `credential` | Optional: the account or key the operations need, which the host adds itself (see [Credentials](#credentials)) |

A field named `credential` anywhere but at the top, or `credentials` or `auth` anywhere, is
refused (`credentials-not-supported`): a connector declares its credential only in the
top-level `credential` field. Any other unknown field, or a key given twice, is
`invalid-connector`, and the error names the field.

### Destinations

Requests go only to the destinations a connector declares, by name, and the user sees every
one before approving. A destination is a DNS name over HTTPS, compared in one normal form
(lowercase, internationalized names as `xn--`): a name ending with a dot, an IP address, a
path, a query, a fragment, a user or a password are refused. A redirect is followed only to an
origin the same connector declares, scheme, host and port exactly, at most five times.

The host connects only to public addresses: a name that leads to the user's machine, the local
network or a cloud metadata address is refused at every connection, and it never uses a network
proxy set in the operating system. Certificates are checked against Mozilla's root list, the
same on every system.

### Operations

| Field | Rule |
| --- | --- |
| `kind` | `read` (changes nothing in the service) or `change` |
| `method` | `GET`, `HEAD`, `POST`, `PUT`, `PATCH` or `DELETE` |
| `destination` | A name of `destinations` |
| `path` | At most 512 characters, starting with `/`; each segment literal, or exactly `{<parameter>}` |
| `query` | At most 16 pairs: a name, then a literal value of at most 256 characters, or exactly `{<parameter>}` |
| `headers` | At most 8: `Accept`, `Accept-Language` and, in the catalog only, `X-…` names; values printable ASCII, at most 256 characters |
| `body` | A JSON object of at most 4 KiB; a string exactly `{<parameter>}` becomes that value. Only for changes, and for reads in the catalog |
| `parameters` | At most 16: a name (a lowercase letter, then lowercase letters, digits and underscores) and its type |
| `response.format` | `json` (`application/json` or `…+json`) or `text` (`text/…` or `…+xml`); a success (2xx) in another media type fails, while an error status reaches the widget whatever its body, such as an HTML error page |
| `response.headers` | At most 8 header names passed to the widget, never `Set-Cookie` or `Cookie`. No other header reaches it |

Every placeholder names a declared parameter, and every parameter appears in exactly one place.
The host sets `Host`, `User-Agent`, `Content-Type` and the framing headers itself and refuses a
connector that fixes them, or `Authorization`, `Cookie` or `Proxy-*`.

### Reads and changes

The kind says whether the service's data changes; it never says whether data leaves: a read
still sends its parameters to its destination. Until user actions arrive (Phase 3), a widget's
update may call only reads: a change is shown before approval, and refused when called.

The host cannot see what a service does with a request, but it can check its shape. So outside
the catalog, a read must be a `GET` or `HEAD` without a body, and a connector may not fix an
`X-` header, since some servers read headers such as `X-HTTP-Method-Override` to turn a read
into a change. Declare anything else as a change. The catalog's reviewers check each kind by
hand, so a catalog connector may declare a read with a body, such as a GraphQL query.

### Parameters

| Type | Fields | Checked on every call |
| --- | --- | --- |
| `text` | `max_length` (1–2048), and optionally `pattern` (a regular expression matching the whole value, 1–512 characters) or `values` (1–64 allowed strings) | Within the length, matching the pattern or among the values; in a path segment never `""`, `.` or `..` |
| `integer` | `minimum`, `maximum` (64-bit) | A whole number in the range |
| `flag` | — | `true` or `false` |

`required` defaults to `true`; an optional parameter may not fill a path segment, and when the
widget leaves it out, its query pair or body field is dropped. An unknown name, the same name
twice, a missing required parameter or a value outside its type is refused before anything is
sent (`value-outside-type`), naming the parameter. Patterns run in linear time, so no value can
stall a call.

The host encodes every value in its own place: a path value is percent-encoded except letters,
digits and `-._~`, so `/`, `?`, `#` and `%2F` stay inside their segment; a query value is
form-encoded; a body value becomes a JSON string, number or boolean.

### Fixed values are public

Every value a connector writes into its requests (literal query values, headers, body fields)
is shown to the user before approval, and query values also appear in the request log. Never
put a key or a token there: declare a [credential](#credentials), which the host keeps in the
system's keychain and adds to requests itself.

## Credentials

A connector whose service needs an account or a key declares one `credential`. The connector
never holds the secret: the user connects the account or types the key in the host's controls,
the host keeps it in the system's keychain, and adds it to each request to the destinations
the credential names, in the one header it names. No widget ever sees it.

```json
"credential": {
  "kind": "account",
  "required": false,
  "destinations": ["api"],
  "header": "Authorization",
  "scheme": "Bearer",
  "subscription_required": [402],
  "account": {
    "flow": "authorization-code",
    "client": "notes",
    "authorize": "https://notes.example/oauth/authorize",
    "token": "https://notes.example/oauth/token",
    "revoke": "https://notes.example/oauth/revoke",
    "scopes": ["notes:read"],
    "access": [{ "name": "notes", "level": "read" }],
    "identity": { "operation": "get-me", "name": "/name", "id": "/id" }
  }
}
```

[`plugins/examples/private-notes/connectors/notes.json`](../plugins/examples/private-notes/connectors/notes.json)
is a whole connector with an account.

| Field | Rule |
| --- | --- |
| `kind` | `account`, signed in to in the browser, or `key`, typed by the user |
| `required` | `false` (the default): without an account, calls go without it and the service answers as it does for anyone. `true`: without one, calls answer `account-not-connected` |
| `destinations` | 1–8 names of the connector's `destinations` the credential goes to; never any other |
| `header` | `Authorization`, the only value outside the catalog. A catalog connector may name its service's own header, which its review checks |
| `scheme` | Required with `Authorization`: a token such as `Bearer`; the header is `<scheme> <secret>` |
| `account` | With `kind: account` only: how the user signs in (below) |
| `access` | With `kind: key` only, optional: what the key lets the connector do, as below |
| `subscription_required` | Up to 4 statuses from 400 to 499 that mean the account lacks a paid plan: the widget gets `subscription-required`, without the response |

`account`:

| Field | Rule |
| --- | --- |
| `flow` | `authorization-code`: the authorization code with PKCE, in the system's browser, with the redirect to a loopback address of this machine. The host keeps no client secret, so the service must accept a public client |
| `client` | The name of the client registration the host uses; in development, the id you give with `--client` (see below) |
| `authorize` | An `https` address on a host of one of the connector's destinations (in the catalog, under its `reserves`): the browser goes there, never the credential |
| `token`, `revoke` | `https` addresses on one of the credential's own destinations, so a refresh token, or a token being revoked, goes only where the credential goes. `revoke` is optional |
| `scopes` | Up to 32 scope names asked for at sign-in |
| `access` | 1–32 `{ "name", "level" }` items, `level` `read` or `write`: what the account lets the connector do, in words the user reads before approving and before signing in |
| `identity` | Optional: a read operation of the connector on one of the credential's destinations, `GET` without parameters, and JSON pointers to the account's `name` and stable `id` in its answer. The host calls it after sign-in to name the account, and to tell a revoked token from a refusal of one operation. Without it, the account is named `account`, and connecting another one is refused until it is disconnected |
| `access_page`, `revoke_page` | Optional `https` addresses on the same hosts as `authorize`: where the user chooses what the account reaches, and where they revoke it by hand when there is no `revoke` |

Rules the host checks beyond the schema:

- **Every destination is HTTPS** when the connector declares a credential, not only the
  credential's own (`credential-needs-https`): a plain-HTTP destination on this machine is for
  connectors that send none.
- **The header is the host's.** A connector that fixes the credential's header, or any
  `Authorization`, in an operation is `invalid-connector`. No parameter reaches a header, so no
  value a widget passes can name, build or replace the credential.
- **Ask for the narrowest access** the service offers: only the scopes your operations need,
  and `access` items that say exactly what they let the connector do. A new version that adds
  an item, or raises one from `read` to `write`, is approved again by every widget that uses it,
  and the account is connected again; one that asks for less is not.
- **Tokens are bearer tokens**: letters, digits and `-._~+/=`. A key is 1 to 1,024 of them. A
  token outside that set is refused at sign-in.
- **A response never carries the credential back**: one that does, in its body, a header or a
  redirect, raw or escaped, is `failed(invalid-response)`, and the widget gets nothing of it.
- Redirects keep the credential only to the hosts it is for; to any other destination of the
  connector they go without it.

A new version of a connector that adds a credential or changes it (its kind, destinations,
header, addresses, scopes or identity) needs a new approval from every widget that uses it.

## Testing against your own service

In development, a connector `local/<name>` is read from `<data>/connectors/<name>.json` (or the
directory given with `--connectors <dir>`), and its file must carry that id. Besides HTTPS on a
public name, it may reach this machine over plain HTTP: `http://localhost:<port>`,
`http://127.0.0.1:<port>` or `http://[::1]:<port>`. Never another address of the local network,
and never a host the catalog reserves. Only the development command-line tool loads local
connectors, and their approvals stay in the development record.

```bash
rt() { cargo run -q -p runtime --release -- "$@"; }
rt check-connector ~/.local/share/dev.philsantos.dashboard/development/connectors/weather.json
```

```text
ok: local/weather 0.1.0, 1 destination, 1 operation (1 read)
```

For a connector with an account, register a development app at your service: a public client
(no secret) that allows the authorization code with PKCE and a loopback redirect,
`http://127.0.0.1/callback` on any port (RFC 8252). Then connect it with its client id, which
the tool remembers for the connector:

```bash
rt accounts connect local/notes --client <your app's client id>
```

```text
account for local/notes (local file …/connectors/notes.json, development)
asks for: notes read
opening https://notes.example/oauth/authorize?response_type=code&client_id=… in the browser
waiting for the browser (10 min)…
connected: ada (id 1234567)
```

A key is typed at the terminal, which does not show it, never given as an argument. The account
goes to a development vault in your system's keychain; `rt accounts list` shows it, and
`rt accounts disconnect local/notes` deletes it and revokes it at the service.

Then declare it in your widget's manifest, with major `0` for version `0.1.0`, and follow the
flow of the [plugin author guide](plugin-authors.md#approving-and-running-in-development):
`permissions` shows what your connector reaches and the approval digest, `approve --expect`
grants exactly that, and `update`, `run` and `requests` show it working. A change to your
connector that adds a destination or changes an operation the widget calls needs a new
approval.

## The official catalog

The [catalog](../connectors) holds the connectors every user can trust, built into the host and
reviewed line by line with the rest of the security core. A catalog connector lists in `reserves`
the whole domains of its service, such as `github.com`; every host under them is the catalog's,
so no other connector can claim it, and a destination outside them is refused.

To propose one, add `connectors/catalog/<name>.json` with the id `catalog/<name>`, a test that it
works with the example of a widget, and say in the pull request why each operation is a read or a
change. A catalog connector with a credential also says why its access is the narrowest the
service offers; every sign-in address and page of it is under its `reserves`. The criteria for entering the catalog arrive in Phase 3; connectors from repositories,
whose destinations the service's owner verifies, arrive with installation.
