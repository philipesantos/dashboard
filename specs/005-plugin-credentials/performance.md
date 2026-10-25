# Performance: Plugin Credentials

Measurements for SC-006: adding a credential to a call costs less than 5 ms once the host has
read it from the keychain, on every CI system. Also the first read from the system's keychain, a
later one, and a refresh (research R18).

## How it was measured

- **Run**: [Bench workflow, run 40193702314](https://github.com/philipesantos/dashboard/actions/runs/40193702314),
  2026-10-25, at commit `cceffbe`, started by hand on the `005-plugin-credentials` branch of pull
  request #5 (`.github/workflows/bench.yml`). The commit after it only adds the refresh test below.
- **Build**: release profile (LTO, `codegen-units = 1`), Rust 1.99.1, `keyring-core` 1.0.1 with
  `windows-native-keyring-store` 1.1.0, `apple-native-keyring-store` 1.0.2 and
  `dbus-secret-service-keyring-store` 1.0.1.
- **Systems**: GitHub's standard runners for public repositories, `ubuntu-24.04`,
  `macos-latest` and `windows-latest`. Linux runs in a session bus of its own with GNOME Keyring
  unlocked, as CI's tests do.
- **Command**: `cargo run -p dashboard-bench --release --locked --bin credential_cost`.
  - **The request:** the median of 100 builds and checks of one request to `local/bench`, with
    and without an account token. The host's own code runs, through `runtime::measure`, with no
    network, since the test seam cannot compile in a release build. Each run builds the hop's
    request and looks for the credential in a 1 MiB response, as a call does with what comes
    back.
  - **The keychain:** the first read of an entry in the development vault, by a vault of its
    own, which opens the keychain; then the median of 100 later reads.

Shared runners are noisy, and this is one run per system: read the numbers as an order of
magnitude, which is what the targets need.

## A credential on a request

| System | Request without | With a credential | Added |
| --- | --- | --- | --- |
| Linux (`ubuntu-24.04`) | 0 µs | 26 µs | 26 µs |
| macOS (`macos-latest`) | 0 µs | 100 µs | 100 µs |
| Windows (`windows-latest`) | 0 µs | 45 µs | 44 µs |

**SC-006 passes on every system.** The most a credential adds is 100 µs, on macOS, a fiftieth of
the 5 ms target. Almost all of it is the echo check, which reads every byte of a 1 MiB response
looking for the token. Building the request rounds to 0 µs with and without the header. A
smaller response costs less: the check scales with the response, and 1 MiB is the most a call
receives.

## The keychain

| System | Keychain | First read | Later reads |
| --- | --- | --- | --- |
| Linux (`ubuntu-24.04`) | Secret Service (GNOME Keyring) | 4.8 ms | 310 µs |
| macOS (`macos-latest`) | Login keychain | 1.1 ms | 657 µs |
| Windows (`windows-latest`) | Credential Manager | 723 µs | 154 µs |

A run reads an account's entry once and keeps it until it ends (research R11), so a widget pays
the read at most once per update. The first read, which opens the keychain, takes at most
4.8 ms, on Linux, where it opens a D-Bus session to GNOME Keyring. Every read stays far under
the call's 5 s time limit, which bounds a widget's vault operations (research R13).

## A refresh

A refresh is a round trip to the service's token address, so the bench cannot time it.
`a_refresh_timed` in `runtime/tests/accounts.rs` times it instead. Every token the test sign-in
service issues expires within the refresh margin, so each of 20 updates of `private-notes`
refreshes first. Then the same updates run with a token that does not expire. Locally, on Linux,
in a debug build with the mock vault, over loopback to the test service:

| Measure | Median | Slowest |
| --- | --- | --- |
| The refresh request, as the request log records it | 6 ms | 9–14 ms |
| An update that refreshes | 16 ms | |
| An update that does not | 11 ms | |

A refresh adds about 5 ms to an update here, a TLS connection to the token address and one
exchange. Against a real service, the network round trip adds to it. That runs within the
call's 5 s limit, and a call refreshes at most once (research R13).

## The limits of research R13

Every limit stands as planned:

| Limit | Value | Measured |
| --- | --- | --- |
| Vault operation | The call's time limit (5 s) for widgets, 2 min for the tool | The slowest read, the first on Linux, takes 4.8 ms |
| Refreshes per call | 1 | A refresh takes about 6 ms over loopback; `revoked-token` checks there is no second one |
| Refresh margin | 60 s before expiry | `a_refresh_timed` and `accounts_record` refresh within it |
| Stored secret | 2,560 bytes | `vault_round_trip` keeps a document of 2,560 bytes on each system's keychain and refuses one byte more |
| Sign-in and refresh responses | 64 KiB, each request within the call's time limit | The test service's answers are under 1 KiB |
