# wit

The contracts between the host and plugins: WIT packages, one directory per version, and the
manifest schema. Everything here is security core (constitution, Principle IX).

- [`v1/widget.wit`](v1/widget.wit): plugin contract v1 at its newest minor,
  `dashboard:widget@1.2.0`, from [feature 002](../specs/002-plugin-permissions/spec.md),
  [feature 003](../specs/003-plugin-connectors/spec.md) and
  [feature 004](../specs/004-plugin-storage/spec.md), with four worlds: `widget`, which only
  renders; `connected-widget`, which also fetches data through connectors in `update`;
  `stored-widget`, which writes its own store in `update` and reads it in `render`; and
  `connected-stored-widget`, which does both, the world the host binds. New plugins build
  against it.
- [`v0/widget.wit`](v0/widget.wit): plugin contract v0 (`dashboard:widget@0.1.0`), from
  [feature 001](../specs/001-plugin-runtime/spec.md). The host keeps running v0 plugins, with
  `log` required.
- [`history/`](history): published versions, one directory each (`1.0.0`, `1.1.0`), frozen and
  never edited; `runtime/tests/contract_v1.rs` pins each by its SHA-256. Plugins that build
  against an older minor version, and the test that a new minor version only adds, read them
  from here.
- [`manifest.schema.json`](manifest.schema.json): the JSON Schema (draft 2020-12) of the plugin
  manifest, `quota_bytes` on `storage` included. The host's parser and this schema are tested to
  agree (`runtime/tests/manifest_schema.rs`); the host also checks a quota against its own cap.
- [`connector.schema.json`](connector.schema.json): the JSON Schema of a connector, the
  declaration of a service's operations that the host runs for widgets. The host's connector
  parser and this schema are tested to agree too, with the rules only the host can check listed
  apart; [`docs/connectors.md`](../docs/connectors.md) explains the format.

The host and every plugin build from these files; they are the only copies. A change to a
published version is a new version.

## Capabilities in v1

Each capability is its own interface, carried by one permission that a plugin's manifest
declares and the user grants. Every function of a capability interface returns `result<T, E>`,
where `E` is an enum or variant of that interface whose first case is `not-granted`: the answer
of an optional permission the user has not turned on. `runtime/tests/contract_v1.rs` fails the
build if a function breaks this rule.

A minor version only adds: 1.1.0 holds every interface, function and type of 1.0.0 unchanged,
and 1.2.0 every one of 1.1.0, which `runtime/tests/contract_v1.rs` checks against `history/`.
The host runs plugins built for any v1 minor up to its own and refuses newer ones, naming both
versions. Feature 003 added `connectors`, which carries every connector a manifest declares,
each a permission of its own, and the `updater` export. Feature 004 added `storage`, carried by
the `storage` permission with its quota: the plugin's own store of named values, which no call
can point at another plugin's, read in `render` and written only in `update`, all or nothing.
Feature 005 adds accounts in the next minor.

## The manifest in a package

A plugin package is one component file. Its manifest is the JSON payload of exactly one
top-level custom section named `dashboard-manifest`, at most 16 KiB
([package format](../specs/002-plugin-permissions/contracts/package.md)). `cargo xtask plugins`
and `runtime pack` add it. [`docs/plugin-authors.md`](../docs/plugin-authors.md) explains how to
write one.
