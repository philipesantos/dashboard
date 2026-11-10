# Contract: The Shell Probe

## Building and running it

```sh
pnpm --filter ./app build --mode probe          # adds src/probe/run.ts to index.html
cargo build -p dashboard --release --features shell-probe --locked
./target/release/dashboard                      # xvfb-run on Linux; exits by itself
pnpm --filter ./app build                       # put the normal dist back
```

The probe build is the release build plus:
- one bundled module, `src/probe/run.ts`;
- one run-time capability with one command, `shell_probe_report`, added through
  `Manager::add_capability`.

Its content policy, `capabilities/`, `window-surface.toml` and window settings are the
release ones.

## Cases

Each case has an id, the rule it breaks and how a refusal is detected:
- **violation**: the engine fired `securitypolicyviolation` for it;
- **absent**: the side effect did not happen;
- **error**: the call rejected;
- **handler**: `on_navigation` or `on_new_window` refused it;
- **setting**: the engine setting read back as the release value.

| Id | Rule | Detected by |
| --- | --- | --- |
| `script-inline-element` | FR-001 | violation and absent |
| `script-inline-attribute` (`<img onerror>`) | FR-001 | violation and absent |
| `script-eval` | FR-001 | error |
| `script-function` | FR-001 | error |
| `script-timeout-string` | FR-001 | violation and absent |
| `load-script-remote` | FR-001 | violation |
| `load-style-remote` | FR-001 | violation |
| `load-style-import` | FR-001 | violation |
| `load-image-remote` | FR-001 | violation |
| `load-font-remote` | FR-001 | violation |
| `connect-fetch` | FR-001 | violation |
| `connect-xhr` | FR-001 | violation |
| `connect-websocket` | FR-001 | violation |
| `connect-eventsource` | FR-001 | violation |
| `embed-iframe` | FR-001 | violation |
| `embed-object` | FR-001 | violation |
| `base-element` | FR-001 | violation |
| `form-remote-submit` | FR-001 | violation and handler |
| `style-runtime-element` | FR-002 | the demo's dialog and select open and close, with no violation unless the allowance is recorded |
| `command-unlisted` (`greet`) | FR-007 | error |
| `command-opener` (`plugin:opener\|open_url`) | FR-007 | error |
| `command-core` (`plugin:event\|listen`) | FR-007 | error |
| `navigate-location` | FR-009 | handler |
| `navigate-assign` | FR-009 | handler |
| `navigate-meta-refresh` | FR-009 | handler |
| `navigate-link` | FR-009 | handler |
| `navigate-file-scheme` | FR-009, FR-012 | handler |
| `window-open` | FR-010 | handler |
| `window-target-blank` | FR-010 | handler |
| `devtools-setting` | FR-011 | setting |
| `context-menu` | FR-011 | `contextmenu` event cancelled; WebView2 setting off |
| `autofill-general` (Windows) | FR-016 | setting |
| `autofill-password` (Windows) | FR-016 | setting |
| `store-incognito` | FR-016 | setting: InPrivate, non-persistent or ephemeral |
| `entitlements-no-webcredentials` (macOS) | FR-016 | the app's entitlements carry no associated domains |

With `DASHBOARD_PROBE_HOLD=1`, the probe prints its report but keeps the window open. It shows a
form with a field labelled "Senha" and named `password`, for the manual autofill walk of
`quickstart.md` §6.

All remote addresses use `.invalid` hosts, which never resolve, so the network can never be what
refused them.

## Report

The probe prints one line per case and a summary, then exits:

```text
probe: held script-inline-element (violation script-src, side effect absent)
probe: broken load-image-remote (no violation; the image request started)
probe: 34 held, 1 broken
```

| Exit | Meaning |
| --- | --- |
| 0 | Every case held |
| 1 | At least one case broken |
| 2 | No report within 60 s: the page unloaded, which means a navigation succeeded, or it hung |
