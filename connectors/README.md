# connectors

The official connector catalog: the connectors every user can trust, kept in this repository
and reviewed line by line with the rest of the security core (constitution, Principle IX).

- [`catalog/`](catalog): one file per connector, `catalog/<name>.json` for the id
  `catalog/<name>`, in the format of [`wit/connector.schema.json`](../wit/connector.schema.json).

The catalog is built into the host: `runtime/src/catalog.rs` embeds each file when the host is
compiled, so a connector is a catalog connector only if it is in the catalog the host was built
with, and no file next to the binary can add one.

Each catalog connector lists in `reserves` the whole domains of its service, such as
`github.com`. Every host under a reserved domain belongs to the catalog: a connector from
anywhere else that declares one is refused. A reviewer checks that the list names whole domains
of the service the connector is for, and that it covers every host the connector declares.

Every change to a file here changes what every user's widgets can reach, so it gets the same
line-by-line review as host code. The criteria for adding a connector to the catalog arrive in
Phase 3.
