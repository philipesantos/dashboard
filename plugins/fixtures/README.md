# fixtures

Well-behaved plugins that the tests use. They are test fixtures, not examples for authors: start
a plugin from [`examples/hello-widget`](../examples/hello-widget) instead.

- `hello-v0`: a contract v0 plugin that logs, to check that the host keeps running v0 plugins.
- `probe`: a contract v1 plugin that calls `log` once and returns `{"log":"ok"}` or
  `{"log":"not-granted"}`, depending on what it answered. Built without a manifest; each test
  packs it with its own.
- `quiet`: a contract v1 plugin that never calls `log`, so its component imports nothing. Built
  without a manifest; each test packs it with its own.
- `net-probe`: a contract 1.1.0 widget (`connected-widget`) that makes, in `update`, the
  connector calls its configuration lists and returns every answer; it can also call from
  `render`, fail its update, return output that is not JSON or of a given size, and report body
  sizes instead of bodies. Built without a manifest; each test packs it with its own.
  The connector cases of the hostile suite run on it
  ([`plugins/hostile/README.md`](../hostile/README.md#connector-cases)).
- `store-probe`: a contract 1.2.0 widget (`stored-widget`) that makes, in `update` and in
  `render`, the storage calls its configuration lists and returns every answer; it can spin
  between calls, end its update with an error, a panic, an endless loop or a memory grab after
  them, and return output that is not JSON or of a given size. Built without a manifest; each
  test packs it with its own. The storage cases of the hostile suite run on it
  ([`plugins/hostile/README.md`](../hostile/README.md#storage-cases)).
- `account-probe`: a contract 1.3.0 widget (`account-widget`) that makes, in `update`, the calls
  its configuration lists, each through `accounts` or `connectors`, and returns every answer;
  with `dump` it returns everything the host gave it instead, so a test can check that no
  credential is in it. Built without a manifest; each test packs it with its own. The credential
  cases of the hostile suite run on it
  ([`plugins/hostile/README.md`](../hostile/README.md#credential-cases)).

## Install fixtures

[`install/`](install) holds manifests that `cargo xtask plugins` packs with a crate's component, as
listed in `install/index.json`, into `target/plugins/<name>.wasm`. Each has a repository id under
`github.com/dashboard-fixtures/`, so the installation tests and the probe's release source can
serve it as the package of a release (`specs/010-plugin-installation/research.md`, R11, R16).

- `install-basic`: `action-steps` with `log` and `storage` (a 64 KiB quota) required and the
  catalog's `catalog/github` optional, declaring the action `save`.
- `install-more`: `install-basic` at version 0.2.0, with `catalog/github` required: the update that
  needs a new approval.
- `install-account`: `account-probe` acting as the account of `catalog/account-echo`, a connector
  with a credential that only the tests' catalog holds, beside `catalog/github` optional: the
  dangerous combination once both are granted. The built-in catalog has no connector with a
  credential yet, so outside the tests this fixture is refused as `connector-unavailable`.
- `install-repo-connector` and `install-local-connector`: `net-probe` with an optional connector
  from a repository and a `local/` one, which installation refuses even when optional.
- `compile-heavy`: `hello-widget` with a core module of generated code that is never instantiated
  (`xtask/src/padding.rs`, the code `bench/src/bin/compile_cost.rs` measures), just under the 10 MiB
  package cap: it renders as `hello-widget` does and costs what the largest packages cost to
  compile.
