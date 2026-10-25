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

A catalog connector that declares a credential gets three more checks. Its `access` is the
narrowest the service offers for the operations it declares, in words the user can judge
before approving and before signing in. Every sign-in address and page of it (`authorize`,
`access_page`, `revoke_page`) is under its `reserves`, which `runtime/src/catalog.rs` tests, and
its `token` and `revoke` addresses are on the credential's own destinations. And its `header`,
when it is not `Authorization`, is the one the service reads the credential from and no other.

Every change to a file here changes what every user's widgets can reach, so it gets the same
line-by-line review as host code. The criteria for adding a connector to the catalog arrive in
Phase 3.

## Contributing a connector

[`docs/connectors.md`](../docs/connectors.md) explains the format and how to test a connector
against your own service first, as a local connector. To propose one for the catalog, add
`catalog/<name>.json` with the id `catalog/<name>` and the whole domains of its service in
`reserves`, add it to the list in `runtime/src/catalog.rs`, and say in the pull request why each
operation is a read or a change: the reviewers check each kind by hand, since a catalog
connector may declare a read with a body. `runtime check-connector` and the schema tests
(`runtime/tests/manifest_schema.rs`) check every file here.
