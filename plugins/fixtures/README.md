# fixtures

Well-behaved plugins that the tests use. They are test fixtures, not examples for authors: start
a plugin from [`examples/hello-widget`](../examples/hello-widget) instead.

- `hello-v0`: a contract v0 plugin that logs, to check that the host keeps running v0 plugins.
- `probe`: a contract v1 plugin that calls `log` once and returns `{"log":"ok"}` or
  `{"log":"not-granted"}`, depending on what it answered. Built without a manifest; each test
  packs it with its own.
- `quiet`: a contract v1 plugin that never calls `log`, so its component imports nothing. Built
  without a manifest; each test packs it with its own.
