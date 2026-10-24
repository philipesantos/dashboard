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
