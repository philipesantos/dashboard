## What and why

<!-- What this pull request changes and why. -->

## Spec

<!-- The spec and tasks it implements, for example specs/001-plugin-runtime, T012–T018. -->

## Security core (Principle IX)

<!-- The security core today is runtime/, wit/ and connectors/ (the official catalog), and
Cargo.lock when it moves a crate of the network stack (ureq, ureq-proto, rustls, rustls-webpki,
ring, webpki-roots, url or regex) or of storage (rusqlite or libsqlite3-sys, which bundles
SQLite). The security tests are in plugins/hostile/, plugins/fixtures/ (store-probe included),
runtime/tests/hostile.rs, runtime/tests/hostile/ and runtime/tests/common/servers.rs. -->

- [ ] Does not touch the security core or the security tests
- [ ] Every commit touching the security core or the security tests was reviewed line by line by: @
