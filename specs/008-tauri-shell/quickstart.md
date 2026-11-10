# Quickstart: Validating the Desktop Shell

Commands run from the repository root. They need the Tauri prerequisites of `CONTRIBUTING.md`,
and on Linux a display or `xvfb-run`.

## 1. The surface is empty and reviewed (FR-005 to FR-008)

```sh
cargo test -p dashboard --test surface --locked
```

**Expected**: passes. `window-surface.toml` has no entries. `capabilities/default.json` grants
nothing. `tauri-plugin-opener` and `greet` are gone.

Then add `"core:event:default"` to the capability and run the test again.

**Expected**: it fails with
`window-surface.toml: permission "core:event:default" is granted … but not listed`. Revert.

## 2. The probe holds on this system (FR-001, FR-007, FR-009 to FR-012, FR-016)

Run [contracts/probe.md](contracts/probe.md)'s four commands.

**Expected**: the summary reads `N held, 0 broken` and the exit status is 0. Record the time the
probe takes (target: under 60 s).

## 3. A broken rule fails the probe

1. In `tauri.conf.json`, add `https:` to `img-src`, then rebuild the probe and run it.

   **Expected**: `probe: broken load-image-remote …`, exit 1.
2. Revert. In `window.rs`, make `on_navigation` return `true`, then rebuild and run.

   **Expected**: exit 2, because the page navigated away and no report came. Revert.

## 4. The demo still works

```sh
pnpm tauri dev
```

**Expected**: the dashboard demo looks and behaves as before (feature 007), except that the
"Tauri" tab is gone. The development policy allows hot reload.

## 5. Release look and start-up

```sh
pnpm tauri build
```

Open the built app on each system. In a release build:
- right-click shows no engine menu;
- F12 and Ctrl+Shift+I (Cmd+Option+I on macOS) open nothing;
- dragging a file onto the window does nothing.

Record the cold start time beside today's build.

## 6. No password offer (HS-26, the macOS review step)

On each system, run the probe build with `DASHBOARD_PROBE_HOLD=1`. It keeps the window open after
the report, showing the probe's form with a field labelled "Senha" and named `password`. Type
into it and submit.

**Expected**: no autofill or save offer appears. On macOS, this walk is the review step that
covers the residual in research R7.

## 7. The weekly engine watch

Run the `audit.yml` job `cef-watch` by hand (`workflow_dispatch`).

**Expected**: it passes while `tauri-runtime-cef` has only pre-releases. Its log names the
newest version it saw.
