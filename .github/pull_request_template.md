## What and why

<!-- What this pull request changes and why. -->

## Spec

<!-- The spec and tasks it implements, for example specs/001-plugin-runtime, T012–T018. -->

## Security core (Principle IX)

<!-- The security core today is runtime/ (the secrets vault in runtime/src/vault/, the CLI in
runtime/src/cli/, the test-only CLI in runtime/src/bin/runtime-test-network.rs and its sign-in
follower in runtime/src/network/test_signin.rs included), wit/ and connectors/ (the official
catalog), and Cargo.lock when it moves a crate of the network stack (ureq, ureq-proto, rustls,
rustls-webpki, ring, webpki-roots, url or regex), of storage (rusqlite or libsqlite3-sys, which
bundles SQLite), of the vault (keyring-core, windows-native-keyring-store,
apple-native-keyring-store, dbus-secret-service-keyring-store or libdbus-sys, never with its
`vendored` feature) or of packing (wit-component). The security tests are in plugins/hostile/
(token-hunt and hostile/connectors/ included), plugins/fixtures/ (store-probe and account-probe
included), runtime/tests/hostile.rs, runtime/tests/hostile/, runtime/tests/common/servers.rs and
runtime/tests/common/signin.rs. -->

- [ ] Does not touch the security core or the security tests
- [ ] Every commit touching the security core or the security tests was reviewed line by line by: @
