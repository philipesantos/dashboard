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
