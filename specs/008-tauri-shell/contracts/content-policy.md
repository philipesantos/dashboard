# Contract: The Content Policies

Both policies live in `app/src-tauri/tauri.conf.json`, under `app.security`. Tauri adds, at
build time, a hash for each bundled script and a nonce for each bundled style. Nothing else
may be added beyond what is listed here.

## Release (`csp`)

```json
{
  "default-src": "'self'",
  "script-src": "'self'",
  "style-src": "'self'",
  "font-src": "'self'",
  "img-src": "'self'",
  "connect-src": "ipc: http://ipc.localhost",
  "object-src": "'none'",
  "frame-src": "'none'",
  "child-src": "'none'",
  "worker-src": "'none'",
  "base-uri": "'none'",
  "form-action": "'none'",
  "frame-ancestors": "'none'"
}
```

**Allowances** (each needs a reason here, and the probe must show it is needed):

| Directive | Added source | Reason |
| --- | --- | --- |
| `style-src` | `'unsafe-inline'`, only if the probe shows a component needs it | Fill in at implementation: name the component that inserts a `<style>` element at run time. |

## Development (`devCsp`)

The release policy with these changes only, for Vite's server at `http://localhost:1420`:

| Directive | Change | Reason |
| --- | --- | --- |
| `script-src` | + `'unsafe-inline'` | The React refresh preamble `@vitejs/plugin-react` writes into the page |
| `connect-src` | + `ws://localhost:1421` | Hot module reload |

`'self'` is the development server's origin there. No other directive changes.

## What the probe proves

Every directive above has at least one probe case
([probe.md](probe.md)) that breaks it in the release build and must be refused.
