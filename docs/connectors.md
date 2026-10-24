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

A field named `credential`, `credentials` or `auth`, anywhere, is refused
(`credentials-not-supported`): connectors that log in to an account arrive with feature 005,
and the format leaves room for them. Any other unknown field, or a key given twice, is
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
put a key or a token there: credentials arrive with feature 005, kept by the host.

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
change. The criteria for entering the catalog arrive in Phase 3; connectors from repositories,
whose destinations the service's owner verifies, arrive with installation.
