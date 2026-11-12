# Data Model: Plugin Installation and the Installation Dialog

## Installed record (`<app data dir>/host/installed.json`)

Host data; no plugin reaches it. Written atomically under its lock (`runtime/src/records.rs`).

```json
{
  "record_version": 1,
  "plugins": {
    "github.com/owner/repo": {
      "repository_id": 123456789,
      "tag": "v1.2.0",
      "version": "1.2.0",
      "package_digest": "sha256:…",
      "compiled_digest": "sha256:…",
      "installed_at": "2026-11-20T14:03:00Z"
    }
  }
}
```

| Field | Rule |
| --- | --- |
| key | A repository plugin id (`manifest.rs:759-765`); never `local/` |
| `repository_id` | GitHub's permanent identifier, kept from the first installation; updates are fetched by it only |
| `tag`, `version` | The pinned release; `version` equals the manifest's and the tag's |
| `package_digest` | `sha256:` of `package.wasm` (`package.rs:78-85`); names its directory |
| `compiled_digest` | `sha256:` of `compiled.cwasm`, checked before every `deserialize` (research R3) |

Unknown fields refuse the record, as the grant record does.

## Grant entry (existing, `runtime/src/grants.rs:55-107`)

For an installed plugin, `origin` is `github:<repository_id>`; `approved_digest` is the package
digest. The app's record moves from memory to `<app data dir>/host/grants.json`.

One field is added (record version 5; versions 1 to 4 still read, with it empty):

| Field | Rule |
| --- | --- |
| `account_sources` | Sorted connector ids: every account connector the plugin held while it also held `storage`. Never shrinks while the store exists; judged as sources by every later grant change (research R7) |

An installation entry is built fresh: required items plus the optionals turned on in the dialog
(`GrantStore::install`), never optionals kept from an earlier entry of the same origin.

## Install request (in memory only)

| Field | Meaning |
| --- | --- |
| `request` | Opaque host token, one pending at a time |
| `kind` | `install` or `update` |
| `checked` | The `CheckedPackage`, compiled form and release info, in a temporary directory |
| `model` | The `DialogModel` last returned |
| `optional_on` | The optionals the person has turned on |

States: `fetching → compiling → awaiting-decision → (approved | cancelled | refused)`. The page
gets the token only with the model, at `awaiting-decision`; only then do `request_judge`,
`request_approve` and `request_cancel` apply. Before that, the page cannot cancel: the window
closing or the app quitting stops the request host-side, killing the compile child. An approved,
cancelled or refused token is gone: using it again is `unknown-request`.

## Dialog model (`runtime::install::DialogModel`, sent to the page)

```text
DialogModel {
  kind: install | update | update-confirm,
  asking: { name, id, author /* sanitised, "declarado pelo plugin" */, version, installed_version? },
  permissions: [ { sentence, params, required, change?: added|changed|unchanged } ],
  connectors: [ { id, version, origin: "catalog", required, change?,
                  destinations: [host],
                  account?: { service, access, connected: false },
                  operations: [ { name, kind: read|change, review: reviewed|declared,
                                  method, url, sends: [param], fixed: [..] } ] } ],
  warning: [ { source, destinations: [host] } ],   // empty: no warning (research R7)
  digest: "sha256:…",          // approval digest: package, permissions, connectors, storage
  choice: "sha256:…",          // choice digest: {digest, optional_on, warning}; echoed by request_approve
}
```

For an update, `permissions` and `connectors` list everything the new version would hold, each
marked `added`, `changed` or `unchanged`. `update-confirm` is an update that needs no new grant:
the dialog shows the versions and a confirm button; its `choice` covers that confirmation.

## Flow (`runtime::flows::Flow`)

`{ source: connector id (account, held now or remembered), destinations: [host] }`: computed from
the grants the plugin would hold after the change and its `account_sources`. Empty list means no
dangerous combination.

## Frame info (feature 009's `FrameInfo`, extended)

`source: "github.com/<owner>"` for an installed widget, absent for a bundled one; drawn by the
frame after the name (research R14).

## Compile limits (`runtime::compile::CompileLimits`)

| Field | Default | Note |
| --- | --- | --- |
| `wall_time` | 20 s | Parent kill |
| `cpu_time` | 60 s | `RLIMIT_CPU`, job time limit |
| `memory_bytes` | 1.5 GiB | `RLIMIT_DATA`, job memory limit, macOS watchdog |
| `threads` | max(1, cores / 2) | `RAYON_NUM_THREADS` |
| `max_compiled_bytes` | 64 MiB | Output cap |
| `max_stderr_bytes` | 64 KiB | Kept for the app's log only |

Tests lower them to break each one (research R11). No environment variable or setting changes
them in a release build. Child exit code 86 means an allocation failed (`compile-memory-limit`).

## Release fetch limits (`runtime::install::FetchLimits`)

| Field | Default |
| --- | --- |
| `api_body_bytes` | 256 KiB |
| `api_time` | 10 s |
| `package_time` | 60 s |
| `max_redirects` | 3, to the two asset hosts only |

## New error kinds

Runtime (`error.rs`, added to `error_kinds!` and the author error reference):
`identity-mismatch`, `tag-mismatch`, `changed-release`, `repository-changed` (deleted,
renamed or another permanent identifier), `not-newer`, `source-unreachable`, `rate-limited`,
`no-package-asset`, `compile-time-limit`, `compile-memory-limit`, `compile-failed`,
`package-changed`.

App refusals (`widgets.rs`, like `widget-busy`): `request-pending`, `unknown-request`,
`already-installed`, `bad-address`, `bad-choice`.
