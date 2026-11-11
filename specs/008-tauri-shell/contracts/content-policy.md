# Contract: The Content Policies

The release policy lives in `app/src-tauri/tauri.conf.json`, under `app.security.csp`. Tauri
sends it as a response header with every page and asset it serves (`manager/mod.rs:422-439`).
It adds a hash for inline `<script>` text and a per-response nonce for `<style>` elements in
the bundled HTML. The bundle has no inline script. It has one empty `<style>`, whose nonce
`main.tsx` hands to the components that insert `<style>` elements at run time (research R3).
Nothing beyond what is listed here may be added.

## Release (`csp`)

```json
{
  "default-src": "'self'",
  "script-src": "'self'",
  "style-src": "'self'",
  "font-src": "'self'",
  "img-src": "'self'",
  "connect-src": "ipc:",
  "object-src": "'none'",
  "frame-src": "'none'",
  "child-src": "'none'",
  "worker-src": "'none'",
  "base-uri": "'none'",
  "form-action": "'none'",
  "frame-ancestors": "'none'"
}
```

On Windows, `app/src-tauri/tauri.windows.conf.json`, which Tauri merges over `tauri.conf.json`
there, sets `connect-src` to `ipc: http://ipc.localhost`: Windows reaches the IPC channel by its
`http` form. Elsewhere that address is a real request to the machine's port 80, so it is allowed
on Windows only (review finding of T028). The development policy merges the same file when Vite
runs on Windows.

**Allowances** (each needs a reason here, and the probe must show it is needed): none. The two
components that insert a `<style>` at run time, Radix's scroll lock and the Select's viewport,
carry the response's nonce instead (research R3).

## Development (Vite's `server.headers`)

On desktop, `tauri dev` loads the page straight from Vite, and Tauri attaches no policy to it
(research R3), so `devCsp` is not set. `app/vite.config.ts` sends the development policy as a
`Content-Security-Policy` header from Vite's server at `http://localhost:1420`. It is the
release policy with these changes only:

| Directive | Change | Reason |
| --- | --- | --- |
| `script-src` | + `'nonce-dashboard-development'` | The React refresh preamble and Vite's client, which Vite marks with the nonce (`html.cspNonce`, set only for the server) |
| `style-src` | + `'nonce-dashboard-development'` | The `<style>` elements Vite inserts for CSS in development, and the run-time styles of R3, which get the same nonce through `main.tsx` |
| `connect-src` | + `ws://localhost:1420` | Hot module reload, on Vite's own port |

A fixed nonce is no weaker than `'unsafe-inline'` here, and it keeps injected markup refused in
development too. The probe, run against the development server, holds every case
(T009).

`'self'` is the development server's origin there. No other directive changes.
`vite.config.ts` builds this header from `tauri.conf.json`'s `csp`, adding these sources, so the
two policies cannot drift.

## What the probe proves

Every directive above has at least one probe case ([probe.md](probe.md)) that breaks it in the
release build and must be refused. `frame-ancestors` is the exception: no page in the window
frames another, so the header's value is checked instead.
