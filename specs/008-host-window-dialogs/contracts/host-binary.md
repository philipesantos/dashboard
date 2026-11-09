# Contract: the `dashboard` binary

## Start

```text
dashboard [--grants <path>] [--data <dir>] [--connectors <dir>]
```

The options mean what they mean for the development tool (`runtime`): the grant record, the data
directory and the local connectors directory, with the same defaults. The window lists the record's
approvals and loads each from its origin (research R5).

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
| `<data dir>/dev.philsantos.dashboard/host/appearance.json` | the appearance choice (data-model) | nobody; no plugin can reach it |
| the development grant record | only through `GrantStore::remove`, from "Remove from dashboard" | the development tool |

## Runtime calls the host makes

- `GrantStore::open` / `GrantStore::approvals()` (new, read-only) / `GrantStore::remove`
- `Runtime::new`, `Runtime::load`, `Plugin::render`, `Runtime::discard_kept_data`

No other runtime call; none of them on the UI thread except reading the approvals.
