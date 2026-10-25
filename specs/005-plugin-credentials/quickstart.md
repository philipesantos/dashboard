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
cargo test -p runtime --test hostile credentials   # only the 30 cases of FR-019
cargo test -p runtime --test vault           # the real keychain round trip
```

Expected: every test passes; the hostile cases print each case's rule. The tests reach only the
test sign-in service and test servers on loopback, never GitHub.

## 2. Contract 1.3.0 and older plugins

```sh
cargo test -p runtime --test contract_v1     # 1.3.0 only adds to 1.2.0
cargo test -p runtime --test contract        # 1.0.0, 1.1.0 and 1.2.0 plugins on the 1.3.0 host
```

## 3. A real GitHub account (manual, once per developer)

1. Register a GitHub App under your account (Settings → Developer settings → GitHub Apps → New):
   any name and homepage; no callback address; webhook off; **Enable Device Flow** on; **Expire
   user authorization tokens** on; repository permissions **Contents: Read-only** (Metadata is
   read-only already); nothing else. Note its client id and its name in the address
   (`https://github.com/apps/<name>`).
2. Build the example and connect, with a temporary data directory and grant record as in
   features 003 and 004:

   ```sh
   cargo xtask plugins
   export D=$(mktemp -d)
   alias rt="cargo run -q -p runtime --release -- "   # every rt command takes --grants $D/grants.json --data $D
   rt accounts connect catalog/github --client <client id> --app <name>
   ```

   Enter the code the tool shows at `https://github.com/login/device`, approve, then open the
   address printed after `choose the repositories it may read:` and install the app on one private
   repository.
3. Approve and run the example on that repository:

   ```sh
   rt permissions target/plugins/github-releases.wasm    # shows "this widget acts as the account: <you>"
   rt approve target/plugins/github-releases.wasm --expect <digest>
   rt update target/plugins/github-releases.wasm --config '{"owner":"<you>","repo":"<private repo>"}'
   rt run target/plugins/github-releases.wasm
   rt requests                                           # entries "as <you>"; no token anywhere
   ```

   Expected: the private repository's releases render; `runtime requests` shows the calls as your
   account and the host's own `token` entry, with no body.
4. Check that nothing leaked: `grep -r "ghu_" $D` finds nothing; `rt accounts list`
   shows the account without any token.
5. Disconnect:

   ```sh
   rt accounts disconnect catalog/github
   rt update target/plugins/github-releases.wasm --config '…'
   ```

   Expected: the keychain entry is gone, the tool prints where to revoke the app at GitHub, the kept
   data is discarded, and the private repository now answers 404 (public repositories still work
   without an account). Revoke the app at `https://github.com/settings/apps/authorizations`.

## 4. Numbers

`bench.yml` runs `credential_cost` on the three systems (by hand, `gh workflow run bench.yml --ref
005-plugin-credentials`) and the numbers go into `performance.md`: a call with a credential within
5 ms of one without (SC-006), the first keychain read and a refresh.
