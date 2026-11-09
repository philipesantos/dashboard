# Contract: the `dashboard` binary

## Start

```text
dashboard [--grants <path>] [--data <dir>]
```

The options mean what they mean for the development tool (`runtime`): the grant record and the
data directory, with the same defaults. There is no `--connectors`: the window runs the runtime
without development mode, so local connectors never load in it, and a plugin that declares one
shows "Did not render" (research R5). The window lists the record's approvals and loads each from
its origin.

## Exit codes

| Code | When |
| --- | --- |
| 0 | the person closed the last window or quit |
| 2 | a usage error (unknown option, missing value) |
| 3 | no GPU driver GPUI can use: a message on standard error says why and what to install (FR-013) |
| 4 | the host could not start the runtime (the error, in host words, on standard error) |

## Files the host writes

| File | Content | Who else reads it |
| --- | --- | --- |
| `<data dir>/dev.philsantos.dashboard/host/appearance.json`, in the system's data directory whatever `--data` says | the appearance choice (data-model) | nobody; no plugin can reach it |
| the development grant record | only through `GrantStore::remove`, from "Remove from dashboard" | the development tool |

## Runtime calls the host makes

- `GrantStore::open` / `GrantStore::approvals()` (new, read-only) / `GrantStore::remove`
- `Runtime::new`, `Runtime::load`, `Plugin::render`, `Runtime::discard_kept_data`

No other runtime call, and none of them on the UI thread: reading the record (which takes its file
lock), reading a package (capped at the package size limit), loading, rendering and removing all
run on the background executor. `RuntimeOptions::development` is never called.
