# Contract: The Content Policies

The release policy lives in `app/src-tauri/tauri.conf.json`, under `app.security.csp`. Tauri
sends it as a response header with every page and asset it serves (`manager/mod.rs:422-439`).
It adds a hash for inline `<script>` text and a per-response nonce for `<style>` elements in
the bundled HTML. The bundle has neither, since its scripts and styles are files that load by
`'self'`. Nothing beyond what is listed here may be added.

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
| `style-src-elem` (added only then, as `'self' 'unsafe-inline'`) | `'unsafe-inline'`, only if the probe shows a component needs it | Fill in at implementation: name the component that inserts a `<style>` element at run time. It goes in `style-src-elem`, because a browser ignores `'unsafe-inline'` in a directive that also holds a nonce. |

## Development (Vite's `server.headers`)

On desktop, `tauri dev` loads the page straight from Vite, and Tauri attaches no policy to it
(research R3), so `devCsp` is not set. `app/vite.config.ts` sends the development policy as a
`Content-Security-Policy` header from Vite's server at `http://localhost:1420`. It is the
release policy with these changes only:

| Directive | Change | Reason |
| --- | --- | --- |
| `script-src` | + `'unsafe-inline'` | The React refresh preamble `@vitejs/plugin-react` writes into the page |
| `connect-src` | + `ws://localhost:1420` | Hot module reload, on Vite's own port |

`'self'` is the development server's origin there. No other directive changes.
`vite.config.ts` builds this header from `tauri.conf.json`'s `csp`, adding these two sources, so
the two policies cannot drift.

## What the probe proves

Every directive above has at least one probe case ([probe.md](probe.md)) that breaks it in the
release build and must be refused. `frame-ancestors` is the exception: no page in the window
frames another, so the header's value is checked instead.
