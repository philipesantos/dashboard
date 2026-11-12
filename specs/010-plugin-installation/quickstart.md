# Quickstart: Validating Plugin Installation

Run from the repository root. The order of the build commands is CONTRIBUTING.md's.

## 1. Build and test

```bash
cargo xtask plugins --locked
cargo test --workspace --locked          # the R11 cases below the page
pnpm --filter ./design check             # host-only roles, criteria cited by the threat model
```

Expected: every case of research R11 passes; `compile-slow` and `compile-memory` finish in
seconds, because the tests lower the limits.

## 2. The probe, with the dialog cases

```bash
pnpm --filter ./app build --mode probe
cargo build -p dashboard --profile probe --features shell-probe,tauri/custom-protocol
DASHBOARD_PROBE=1 ./target/probe/dashboard     # xvfb-run on Linux
```

Expected: exit 0, with the HS-10 to HS-15 and HS-17 to HS-23 cases reported as passed, beside
features 008 and 009's cases. CI runs this on the three systems.

## 3. A real installation

Prerequisite: a public test repository whose latest release follows
[contracts/release.md](contracts/release.md), with one required permission (`log`), a `storage`
quota, and the catalog GitHub connector as optional, and one release later that makes the
connector required.

1. `pnpm --filter ./app tauri dev`, then in the chrome choose to add a widget and type the
   repository address.
2. Check the dialog: the manifest's name and the repository id; the author, marked as declared by
   the plugin; the `log` and `storage` sentences; the connector off, with its destinations,
   operations, sends and "revisado"; no warning.
3. Wait one second, approve. The widget renders, its frame shows `github.com/<owner>` after the
   name; the connector answers "not granted".
4. Quit and start the app again: the widget is there, with no recompilation (the compiled form is
   reused).
5. Publish the later release; check for updates on the widget. The dialog lists every permission,
   connector and destination, the connector marked as changed to required; cancel, and the old
   version keeps running; check again and approve.

## 4. Break it by hand

- Type `file:///tmp/x.wasm`, `https://example.com/x` and `gitlab.com/a/b`: each is refused as
  `bad-address`.
- Flip a byte of `compiled.cwasm` under `<app data dir>/host/packages/…`; restart: the widget
  loads after a recompile, never from the altered file.
- Flip a byte of `package.wasm`; restart: the widget is refused as `package-changed`.
- Delete the test repository and create one with the same name; check for updates: refused as
  `repository-changed`, the installed version kept.

## 5. A heavy compile

Publish `target/plugins/compile-heavy.wasm` (built by `cargo xtask plugins`, research R11) as the
release of a test repository whose name matches its manifest id, then install it. While it
compiles, move and type in the window: it keeps answering (SC-003). With the default limits it
installs. The refusals at each limit are the automated `compile-*` cases of §1, which lower the
limits; no setting changes them in the app.

On each system, also confirm the confinement by hand once: while a compile runs, the child
process shows in the system's process list with its sandbox (Linux: `Seccomp: 2` in
`/proc/<pid>/status`; macOS: Activity Monitor's Sandbox column; Windows: Process Explorer's
AppContainer integrity).

## 6. Review steps on the three systems

These steps cover what research R11's table says the probe cannot reach with untrusted events.
With a keyboard only, then with Narrator (Windows), VoiceOver (macOS) and Orca (Linux), open the
installation dialog and check: a real click on Approve and a real Enter within the first second do
nothing, and again right after switching back to the window (HS-12); focus starts on Cancel and
Tab through every control stays inside (HS-18); Escape cancels from every control (HS-19); focus
returns to the add button (HS-20); the dialog is announced with its title, who is asking and what
is decided (HS-21); the screen reader's heading and landmark lists reach nothing outside it, and
clicks on the chrome and on a frame do nothing (HS-11, HS-22). Repeat once with Windows high
contrast (HS-17) and with reduced motion on (HS-23).
