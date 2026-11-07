# Limits

The host sets every limit; a widget can neither read nor change one. Each row gives the limit's
name in the host (`runtime/src/limits.rs`), its default, what it bounds, and what happens past it.
The names of the answers are those of [the contract](contract.md) and of [the errors](errors.md).

## Packages and calls

| Limit | Default | What it bounds | Past it |
| --- | --- | --- | --- |
| `max_package_bytes` | 10 MiB | A package file, and the component `runtime pack` makes from a module | `invalid-package` <!-- ref: limit max_package_bytes --> |
| `fuel` | 500,000,000 | The CPU budget of one call of `render` or `update`, in Wasmtime fuel units | `cpu-budget-exceeded` <!-- ref: limit fuel --> |
| `time_limit` | 1 s | The wall-clock time of one call of `render` | `time-limit-exceeded` <!-- ref: limit time_limit --> |
| `update_time_limit` | 10 s | The wall-clock time of one call of `update`, which waits on the network; its fuel and memory are render's | `time-limit-exceeded` <!-- ref: limit update_time_limit --> |
| `max_memory_bytes` | 32 MiB | Every linear memory of one call together | `memory-limit-exceeded` <!-- ref: limit max_memory_bytes --> |
| `max_output_bytes` | 64 KiB | What `render` returns, checked before it is parsed | `output-too-large` <!-- ref: limit max_output_bytes --> |
| `max_update_output_bytes` | 256 KiB | The data `update` returns, checked before it is parsed | `output-too-large`; the previous data stays <!-- ref: limit max_update_output_bytes --> |
| `max_log_messages` | 100 | Log messages kept per call | The rest are dropped and counted; `write` still answers `ok` <!-- ref: limit max_log_messages --> |
| `max_log_message_bytes` | 1 KiB | One log message | Cut, and marked as cut <!-- ref: limit max_log_message_bytes --> |

## Connector calls

| Limit | Default | What it bounds | Past it |
| --- | --- | --- | --- |
| `max_calls_per_run` | 20 | Connector calls per run of `update`, refused ones included | `refused(over-limit)`, after `not-granted`, `called-outside-update`, `operation-not-declared`, `change-needs-user-action` and `value-outside-type`; the calls past it are logged as one summary entry <!-- ref: limit max_calls_per_run --> |
| `calls_per_minute_per_widget` | 30 per minute | Calls of one widget, within one running host; each CLI command starts afresh | `refused(over-limit)` <!-- ref: limit calls_per_minute_per_widget --> |
| `calls_per_minute_per_host` | 60 per minute | Calls to one destination host, every widget together, within one running host; each CLI command starts afresh | `refused(over-limit)` <!-- ref: limit calls_per_minute_per_host --> |
| `call_time_limit` | 5 s | One call: name resolution, connection, TLS and response, cut to what is left of the update | `failed(over-limit)` <!-- ref: limit call_time_limit --> |
| `max_arguments` | 16 | Arguments of one call | `refused(value-outside-type)` <!-- ref: limit max_arguments --> |
| `max_request_address_bytes` | 8 KiB | The request's path and query | `refused(over-limit)` <!-- ref: limit max_request_address_bytes --> |
| `max_request_body_bytes` | 64 KiB | The request's body | `refused(over-limit)` <!-- ref: limit max_request_body_bytes --> |
| `max_response_body_bytes` | 1 MiB | The response body of one call; a widget never gets part of a larger one | `failed(over-limit)` <!-- ref: limit max_response_body_bytes --> |
| `max_update_response_bytes` | 4 MiB | The response bodies of one update together, through `connectors` and `accounts` | `failed(over-limit)` <!-- ref: limit max_update_response_bytes --> |
| `max_response_header_bytes` | 16 KiB | The response headers of one call | `failed(over-limit)` <!-- ref: limit max_response_header_bytes --> |
| `max_redirects` | 5 | Redirects the host follows in one call | `failed(redirect-not-allowed)` <!-- ref: limit max_redirects --> |

## The request log

What `runtime requests` shows. These bound the log, not the calls.

| Limit | Default | What it bounds | Past it |
| --- | --- | --- | --- |
| `max_request_log_bytes` | 2 MiB | The request log's file | Rotated, keeping one previous file <!-- ref: limit max_request_log_bytes --> |
| `max_logged_value_chars` | 256 | An argument value written to the log, in characters | Cut, with ` [truncated]` <!-- ref: limit max_logged_value_chars --> |
| `max_logged_name_chars` | 160 | A connector or operation name written to the log, in characters | Cut <!-- ref: limit max_logged_name_chars --> |
| `max_argument_name_chars` | 32 | An argument name written to the log, in characters | Cut <!-- ref: limit max_argument_name_chars --> |

## Storage

| Limit | Default | What it bounds | Past it |
| --- | --- | --- | --- |
| `max_storage_quota_bytes` | 8 MiB | The largest quota a manifest may declare | `invalid-manifest` <!-- ref: limit max_storage_quota_bytes --> |
| `storage_entry_cost_bytes` | 64 bytes | What each entry counts in usage on top of its name and value, the same on every system | Counted against the quota (`over-quota`) <!-- ref: limit storage_entry_cost_bytes --> |
| `max_storage_name_bytes` | 256 bytes | A name, in bytes of UTF-8, and `list`'s `prefix` and `after` | `refused(invalid-name)` <!-- ref: limit max_storage_name_bytes --> |
| `max_storage_value_bytes` | 256 KiB | One value | `refused(over-limit)` <!-- ref: limit max_storage_value_bytes --> |
| `max_storage_calls_per_run` | 1,000 | Storage calls per run of `render` or `update`, refused ones included | `refused(over-limit)` <!-- ref: limit max_storage_calls_per_run --> |
| `max_storage_written_bytes_per_update` | 1 MiB | Names and values of every `set` of one update together | `refused(over-limit)` <!-- ref: limit max_storage_written_bytes_per_update --> |
| `storage_list_page` | 100 | Names in one page of `list` | The rest come in the next page, from `next` <!-- ref: limit storage_list_page --> |
| `storage_file_cap_factor` | 5 | The store's file may grow to this many times the approved quota, plus the next limit | `failed(over-limit)`; every later storage call of the run answers `run-aborted`, nothing the update wrote is kept, and the update fails, keeping the previous data <!-- ref: limit storage_file_cap_factor --> |
| `storage_file_cap_extra_bytes` | 64 KiB | Added to the file's cap, for its own pages | `failed(over-limit)`; every later storage call of the run answers `run-aborted`, nothing the update wrote is kept, and the update fails, keeping the previous data <!-- ref: limit storage_file_cap_extra_bytes --> |
| `max_storage_busy_wait` | 2 s | How long a call waits for another process writing the store, within the run's time | `failed(busy)` <!-- ref: limit max_storage_busy_wait --> |

## Accounts and the development CLI

These bound what the host does for an account, and what `runtime accounts connect` reads.

| Limit | Default | What it bounds | Past it |
| --- | --- | --- | --- |
| `sign_in_time_limit` | 10 min | How long a sign-in waits for the browser | `sign-in-failed` <!-- ref: limit sign_in_time_limit --> |
| `max_callback_head_bytes` | 8 KiB | The browser's request to the sign-in's loopback address | That request is dropped, and the sign-in keeps waiting <!-- ref: limit max_callback_head_bytes --> |
| `callback_read_time` | 2 s | How long the loopback listener waits for a request once a connection is open | The connection is dropped; the sign-in keeps waiting <!-- ref: limit callback_read_time --> |
| `max_sign_in_response_bytes` | 64 KiB | An answer of the service's token, refresh or revocation address; the identity operation's answer has `max_response_body_bytes` | `sign-in-failed`; in a call, `account-not-connected`; at disconnecting, the revocation is reported as failed <!-- ref: limit max_sign_in_response_bytes --> |
| `max_secret_bytes` | 2560 bytes | What the keychain keeps for one account | `keychain-unavailable` <!-- ref: limit max_secret_bytes --> |
| `max_key_bytes` | 1 KiB | A key typed at `runtime accounts connect` | `key-refused` <!-- ref: limit max_key_bytes --> |
| `max_account_name_chars` | 100 | An account's name or id as shown, in characters | Cut <!-- ref: limit max_account_name_chars --> |
| `vault_wait_tool` | 2 min | How long the CLI waits for the keychain, for the person to unlock it | `keychain-unavailable` <!-- ref: limit vault_wait_tool --> |
| `refresh_margin` | 1 min | A token that expires within this is refreshed before it is sent | The refresh happens first; a call never sends a token about to expire, when the service said when it expires <!-- ref: limit refresh_margin --> |
| `refreshes_per_call` | 1 | Token refreshes one call may cause | A call that would need a second refresh answers `account-not-connected` <!-- ref: limit refreshes_per_call --> |
