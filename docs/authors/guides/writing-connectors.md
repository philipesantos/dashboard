# Writing connectors

A connector describes, as data, the operations a widget may call on one service: where each
request goes, which values the widget fills in, and what it gets back. It is never code: the host
reads it and makes every request itself, so a widget reaches nothing but the operations a
connector declares, with values the host checks. Widgets depend on a connector by its id and major
version.

This guide writes one for a service of your own. Every field, with what it accepts, is in
[the connector format](../reference/connector-format.md); here is how they fit together. It starts
in the clone of [the walkthrough](../getting-started.md).

## An example

`local/weather` reads forecasts from a weather service. Its schema is `wit/connector.schema.json`
in the clone: point `$schema` at it and an editor checks the file as you write it.

```json file=weather.json check
{
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

`runtime check-connector` checks a file against the schema and the rules only the host checks:

```console check
$ runtime check-connector weather.json
ok: local/weather 0.1.0, 1 destination, 1 operation (1 read)
```

## Destinations

Requests go only to the destinations a connector declares, by name, and the person sees every one
before approving. A destination is an origin, `https://<host>[:<port>]`: a DNS name over HTTPS,
never an IP address, a path or a user. A redirect is followed only to an origin the same connector
declares, at most five times.

The host connects only to public addresses: a name that leads to this machine, the local network
or a cloud metadata address is never connected to, checked at every connection, and the call fails
as `non-public-address`, so a connector cannot be turned on the person's own network. It uses no proxy set in the system and checks certificates against
Mozilla's root list, the same on every system.

## Operations, reads and changes

Each operation is one request: a method, a destination, a path, and the query, headers and body
it sends. A value the widget fills in appears as exactly `{<parameter>}`, in one place, and its
parameter says what it accepts: text of a length, matching a pattern or one of a list; a whole
number in a range; a flag. The host checks every value on every call and encodes it in its own
place, so a value never leaves its path segment, query value or JSON field.

`kind` says whether the service's data changes; it never says whether data leaves, since a read
still sends its parameters. Until user actions arrive (Phase 3), `update` calls only reads: a
change is shown at approval and refused when called. Outside the catalog, a read is a `GET` or
`HEAD` without a body: the host cannot see what a service does with a request, only its shape.

## Fixed values are public

Every value a connector writes into its requests, such as a literal query value, a header or a
body field, is shown to the person before approval, and query values also appear in the request
log. Never put a key or a token there: declare a credential, which the host keeps in the system's
keychain and adds to requests itself.

## Credentials

A connector whose service needs an account or a key declares one `credential`: its kind, the
destinations it goes to, the header it goes in, and, for an account, how the person signs in and
what the account lets the connector do, in words the person reads before approving
([the connector format](../reference/connector-format.md#credential)). The connector never holds the
secret. `plugins/examples/private-notes/connectors/notes.json` in the clone is a whole connector
with an account, and [the accounts guide](accounts.md) uses it.

Ask for the narrowest access the service offers: only the scopes the operations need, and access
items that say exactly what they let the connector do. Any change to the credential, its
scopes included, is approved again by every widget that uses it. A new version that adds access
items also needs the account connected again; one that only removes access items needs neither.

## Testing against your own service

In development, a connector `local/<name>` is read from the folder given with
`--connectors <dir>`, or from `<data>/connectors/`, and its file carries that id. Besides HTTPS on
a public name, it may reach this machine over plain HTTP, `http://localhost:<port>`,
`http://127.0.0.1:<port>` or `http://[::1]:<port>`, for a service you run yourself; never another
address of the local network, and never a host the catalog reserves. Plain HTTP is only for a
connector without a credential: one with an account or a key reaches every destination over HTTPS
on a public name, so test it against a deployment of your service the internet can reach. Only the development CLI
loads local connectors, and their approvals stay in its development record.

For a connector with an account, register a development app with your service: a public client,
without a secret, that allows the authorization code with PKCE and a loopback redirect,
`http://127.0.0.1/callback` on any port (RFC 8252). Then connect it with the app's client id, as
the accounts guide does: `runtime accounts connect local/<name> --client <your app's client id>`.
The CLI remembers the client id for the connector. A key is typed at the terminal, which does not
show it, never given as an argument.

Then declare the connector in your widget's manifest, with major `0` for version `0.1.0`, and
follow [the connectors guide](connectors.md): `permissions` shows what the connector reaches and
the approval digest, `approve --expect` grants exactly that, and `update`, `run` and `requests`
show it working. A change to the connector that adds a destination or changes an operation the
widget calls needs a new approval.

## The official catalog

The catalog holds the connectors every person can trust, built into the host and reviewed line by
line with the rest of its security core. A catalog connector lists in `reserves` the whole domains
of its service, such as `github.com`; every host under them is the catalog's, so no other connector
can claim it. To propose one, add `connectors/catalog/<name>.json` to the repository with a test
that it works with an example widget, and say in the pull request why each operation is a read or
a change. The criteria for entering the catalog arrive in Phase 3, and connectors from
repositories, whose destinations the service's owner verifies, arrive with installation.
