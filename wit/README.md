# wit

The contracts between the host and plugins: WIT packages, one directory per version, and the
manifest schema. Everything here is security core (constitution, Principle IX).

- [`v1/widget.wit`](v1/widget.wit): plugin contract v1 at its newest minor,
  `dashboard:widget@1.1.0`, from [feature 002](../specs/002-plugin-permissions/spec.md) and
  [feature 003](../specs/003-plugin-connectors/spec.md), with two worlds: `widget`, which only
  renders, and `connected-widget`, which also fetches data through connectors in `update`. New
  plugins build against it.
- [`v0/widget.wit`](v0/widget.wit): plugin contract v0 (`dashboard:widget@0.1.0`), from
  [feature 001](../specs/001-plugin-runtime/spec.md). The host keeps running v0 plugins, with
  `log` required.
- [`history/`](history): published versions, one directory each, frozen and never edited.
  Plugins that build against an older minor version, and the test that a new minor version only
  adds (`runtime/tests/contract_v1.rs`), read them from here.
- [`manifest.schema.json`](manifest.schema.json): the JSON Schema (draft 2020-12) of the plugin
  manifest. The host's parser and this schema are tested to agree
  (`runtime/tests/manifest_schema.rs`).
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
which `runtime/tests/contract_v1.rs` checks against `history/1.0.0`. The host runs plugins built
for any v1 minor up to its own and refuses newer ones, naming both versions. Feature 003 added
`connectors`, which carries every connector a manifest declares, each a permission of its own,
and the `updater` export; features 004 and 005 add storage and accounts, each in the next
minor.

## The manifest in a package

A plugin package is one component file. Its manifest is the JSON payload of exactly one
top-level custom section named `dashboard-manifest`, at most 16 KiB
([package format](../specs/002-plugin-permissions/contracts/package.md)). `cargo xtask plugins`
and `runtime pack` add it. [`docs/plugin-authors.md`](../docs/plugin-authors.md) explains how to
write one.
