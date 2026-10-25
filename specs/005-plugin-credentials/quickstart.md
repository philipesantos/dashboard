# Quickstart: Plugin Credentials

How to check feature 005 end to end. Commands and output follow [contracts/cli.md](contracts/cli.md);
records follow [data-model.md](data-model.md).

## Prerequisites

- The toolchain of features 001–004 (Rust 1.97, `wasm32-unknown-unknown`, `wasm32-wasip2`).
- A keychain: on Windows and macOS, the user's own; on Linux, a Secret Service (GNOME Keyring or
  KWallet) and, to build, `libdbus-1-dev` and `pkg-config` (`sudo apt-get install libdbus-1-dev
  pkg-config`).
- On a headless Linux machine, run everything inside a session bus with an unlocked keyring:

  ```sh
  sudo apt-get install -y gnome-keyring
  dbus-run-session -- bash -c "gnome-keyring-daemon --components=secrets --daemonize --unlock <<< 'ci'; cargo test --workspace"
  ```

## 1. The automated checks

```sh
cargo test --workspace --locked              # unit, integration, CLI and hostile cases
cargo test -p runtime --test hostile credentials   # only the 26 cases of FR-019
cargo test -p runtime --test vault           # the real keychain round trip
```

Expected: every test passes; the hostile cases print each case's rule. The tests reach only the
test sign-in service and test servers on loopback, never GitHub.

## 2. Contract 1.3.0 and older plugins

```sh
cargo test -p runtime --test contract_v1     # 1.3.0 only adds to 1.2.0
cargo test -p runtime --test contract        # 1.0.0, 1.1.0 and 1.2.0 plugins on the 1.3.0 host
```

## 3. The example end to end

```sh
cargo test -p runtime --test accounts private_notes
```

Runs `private-notes` with its development connector `local/notes` against the test sign-in service
on `https://notes.test` (test CA and routes of the seam): connect an account, approve, update,
render, disconnect. Expected: the render shows the account's own notes after connecting and only
the public ones after disconnecting; the request log has entries `as ada` and the host's `token`
entry without a body; the scan finds no token in `$D`, the grant record or the output. There is
no run against a real service in this feature: the catalog's GitHub connector gains its account
later (research R1).

## 4. What the tool shows, without a service

```sh
cargo xtask plugins
export D=$(mktemp -d)
alias rt="cargo run -q -p runtime --release -- "   # every rt command takes --grants $D/grants.json --data $D
rt permissions target/plugins/private-notes.wasm --connectors plugins/examples/private-notes/connectors
rt accounts list
rt check-connector plugins/hostile/connectors/plain-http-key.json
```

Expected: `permissions` shows the `credential:` lines of `local/notes`, `asks for: notes read
(declared by the developer)` and `this widget acts as the account: none connected (calls go
without one)`; `accounts list` prints `nothing connected`; the last command fails with
`credential-needs-https`, since a credential never travels to a loopback address without HTTPS.

## 4. Numbers

`bench.yml` runs `credential_cost` on the three systems (by hand, `gh workflow run bench.yml --ref
005-plugin-credentials`) and the numbers go into `performance.md`: a call with a credential within
5 ms of one without (SC-006), the first keychain read and a refresh.
