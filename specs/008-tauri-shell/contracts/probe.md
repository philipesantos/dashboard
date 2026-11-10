# Contract: The Shell Probe

## Building and running it

```sh
pnpm --filter ./app build --mode probe          # adds src/probe/run.ts to index.html
cargo build -p dashboard --release --features shell-probe,tauri/custom-protocol --locked
DASHBOARD_PROBE=1 ./target/release/dashboard    # exits by itself; on Linux under xvfb-run
                                                # with WEBKIT_DISABLE_DMABUF_RENDERER=1
pnpm --filter ./app build                       # put the normal dist back
```

`tauri/custom-protocol` is what `tauri build` turns on. Without it the binary runs in dev mode,
loads `devUrl` and serves no policy. The probe build is the release build plus:
- one bundled module, `src/probe/run.ts`;
- two app commands, compiled in only with the feature:
  - `shell_probe_report`, granted by a run-time capability added through
    `Manager::add_capability`;
  - `shell_probe_ungranted`, which nothing grants.

Its content policy, `capabilities/`, `window-surface.toml`, ACL mode and window settings are the
release ones. Without `DASHBOARD_PROBE=1`, the binary refuses to start.

## Cases

Each case has an id, the rule it breaks and how a refusal is detected:
- **violation**: the engine fired `securitypolicyviolation` for it;
- **absent**: the side effect did not happen;
- **error**: the call rejected;
- **handler**: `on_navigation`, `on_new_window`, `on_permission_request` or `on_download` refused
  it;
- **stayed**: after a wait, the page is still the app's own page and the app still has exactly
  one window, whatever refused it: the engine, the policy or a handler;
- **setting**: the engine setting read back as the release value.

| Id | Rule | Detected by |
| --- | --- | --- |
| `page-load-clean` | FR-004, FR-020 | no violation before the first case: the page, its fonts and its styles load under the release policy |
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
| `form-remote-submit` | FR-001 | violation (`form-action`) and stayed |
| `worker-blob` | FR-001 | violation |
| `load-media-remote` | FR-001 | violation |
| `load-image-data` | FR-001 | violation |
| `style-runtime-element` | FR-002 | the demo's dialog and select open and close, with no violation unless the allowance is recorded |
| `command-ungranted` (`shell_probe_ungranted`, registered, no grant) | FR-007 | error naming the ACL, not a missing command. It proves the ACL refuses what nothing grants; the surface test proves the release build keeps the ACL on |
| `command-opener` (`plugin:opener\|open_url`) | FR-007 | error |
| `command-core` (`plugin:event\|listen`) | FR-007 | error |
| `navigate-location` | FR-009 | handler and stayed |
| `navigate-assign` | FR-009 | handler and stayed |
| `navigate-meta-refresh` | FR-009 | handler and stayed |
| `navigate-link` | FR-009 | handler and stayed |
| `navigate-own-asset` (another bundled file) | FR-009 | handler and stayed |
| `navigate-file-scheme` | FR-009, FR-012 | stayed |
| `window-open` | FR-010 | stayed (no second window) |
| `window-target-blank` | FR-010 | stayed (no second window) |
| `permission-camera` (`getUserMedia`) | FR-021 | error, and the handler denied it where the engine asked |
| `permission-location` | FR-021 | error |
| `permission-notification` | FR-021 | `denied` |
| `download-link` (`a[download]`) | FR-021 | handler (`on_download`) or no download started |
| `dialog-alert`, `dialog-confirm`, `dialog-prompt`, `dialog-print` | FR-021 | the call returns at once with the replaced value (`undefined`, `false`, `null`) |
| `devtools-setting` | FR-011 | setting |
| `context-menu` | FR-011 | `contextmenu` event cancelled; WebView2 setting off |
| `autofill-general` (Windows) | FR-016 | setting |
| `autofill-password` (Windows) | FR-016 | setting |
| `store-incognito` | FR-016 | setting: InPrivate, non-persistent or ephemeral |

The absence of macOS web-credential entitlements is checked by the surface test, not the probe:
an unsigned probe binary has no entitlements to read.

With `DASHBOARD_PROBE_HOLD=1`, the probe prints its report but keeps the window open. It shows a
form with a field labelled "Senha" and named `password`, for the manual autofill walk of
`quickstart.md` §6. Its button removes the form, which engines treat as a submission, since
`form-action 'none'` blocks a real one.

All remote addresses use `.invalid` hosts, which never resolve, so the network can never be what
refused them.

## Report

The probe prints one line per case and a summary, then exits:

```text
probe: held script-inline-element (violation script-src, side effect absent)
probe: broken load-image-remote (no violation; the image request started)
probe: 47 held, 1 broken     (about 48 cases in all)
```

| Exit | Meaning |
| --- | --- |
| 0 | Every case held |
| 1 | At least one case broken |
| 2 | No report within 60 s: the page unloaded, which means a navigation succeeded, or it hung |
