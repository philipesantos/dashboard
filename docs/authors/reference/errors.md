# Errors

Every error the host reports for a widget, by the name the development CLI prints it with:

```text
error: <name>: <detail>
```

The detail says what happened. Every text in it that came from the package, the manifest, a
connector or a service is escaped: a newline shows as `\n` and a control character as `\u{…}`, so
no detail can add a line or change the terminal. The CLI exits with 1 when the host refused or
stopped the widget, and with 2 when the command itself was wrong (a usage error).

These are the host's errors. What a call answers a widget inside `render`, `update` or an
action, such as `not-granted` from a capability, is in [the contract](contract.md).

## The package and its contract

<!-- ref: error invalid-package -->
### `invalid-package`

The file is not a WebAssembly component, is corrupted, or is larger than the package limit
(10 MiB, [limits](limits.md)). From `runtime pack`, also: a core module without the contract's
world, because it was not built with `wit-bindgen`, or one the component encoder refuses, with
the encoder's reason. Build with `wit_bindgen::generate!` for `wasm32-unknown-unknown`, and pack
the module cargo writes, as in [the walkthrough](../getting-started.md).

<!-- ref: error contract-mismatch -->
### `contract-mismatch`

The component implements no contract version the host runs, or imports from another version than
the one it implements. That includes a newer minor than the host's: the detail names both
versions. Bind the contract the host runs: the copy of `wit/history/1.4.0` from the clone the CLI
was built from.

<!-- ref: error contract-too-old -->
### `contract-too-old`

The manifest declares a connector or storage, but the widget is built for a contract older than
the minor that added it: connectors arrived in 1.1.0, storage in 1.2.0, accounts in 1.3.0. Build
against 1.4.0.

<!-- ref: error capability-refused -->
### `capability-refused`

The component imports something the host does not offer, such as WASI, which the standard library
of `wasm32-wasip2` imports. A widget imports only the contract's interfaces. Build for
`wasm32-unknown-unknown`.

<!-- ref: error missing-updater -->
### `missing-updater`

The manifest declares a connector or storage, but the component exports neither `updater` nor
`actor`, where it would call or write them; or `runtime update` was run on a widget without an
`updater`. Implement `update`, in a world that has it, such as `connected-widget`.

## The manifest

<!-- ref: error no-manifest -->
### `no-manifest`

The package has no manifest section: it is a component that was never packed. Run `runtime pack`
with `--manifest`.

<!-- ref: error manifest-mismatch -->
### `manifest-mismatch`

The package has more than one manifest section, or one inside a nested module or component; or
`runtime pack` was given a component that already has one. Pack the component cargo's module was
turned into, once.

<!-- ref: error invalid-manifest -->
### `invalid-manifest`

The manifest is larger than 16 KiB, is not JSON, or breaks [the manifest's schema](manifest.md):
the detail names the field. A storage quota above the host's cap is refused this way too, and so
is an icon that is not a PNG within its bounds. When the host loads the widget, also: the
manifest declares actions and the component exports no `actor`, or it exports `actor` and the
manifest declares no action ([actions](manifest.md#actions)).

<!-- ref: error unknown-permission -->
### `unknown-permission`

The manifest asks for a permission this host does not know. The permissions are `log` and
`storage`; each connector is a permission of its own, but it is declared under `connectors`, not
`permissions` ([the manifest](manifest.md)).

<!-- ref: error optional-needs-v1 -->
### `optional-needs-v1`

A widget built for contract v0 marks a permission optional; only v1 has the `not-granted` answer
an optional permission needs. Build against v1.

<!-- ref: error not-declared -->
### `not-declared`

The component imports a capability its manifest does not ask for. Add the permission to the
manifest, or stop importing it: a world with fewer imports, such as `widget`, has less to declare.

<!-- ref: error account-use-not-declared -->
### `account-use-not-declared`

The manifest declares a connector whose credential is required, without `"account": true`. A
widget that uses such a connector acts as the person's account and must say so.

## Approval

<!-- ref: error not-granted -->
### `not-granted`

The widget has no approval for this package, it was approved from another file, or the grant
record cannot be read. Run `runtime permissions`, then `runtime approve`. An approval is for one
package file: another file, even with the same id, has none. An update also fails with it when
its storage was turned off or its approval removed while it ran: nothing it wrote was kept.

<!-- ref: error needs-new-approval -->
### `needs-new-approval`

An earlier version was approved, and this one asks for something it did not have: a permission,
a connector or operation, a larger quota, a credential, more access. The detail names each. Run
`runtime permissions` to see it and `runtime approve` to grant it.

<!-- ref: error approval-changed -->
### `approval-changed`

`runtime approve --expect <digest>` was given a digest other than the one of what it would approve
now: the package, a connector or the manifest changed since `runtime permissions` showed it. Run
`runtime permissions` again and approve what it shows.

<!-- ref: error required-together -->
### `required-together`

`runtime optional` was asked to turn a required permission on or off. Required permissions are
granted and removed only together, by `runtime approve` and `runtime remove`.

## Running

<!-- ref: error cpu-budget-exceeded -->
### `cpu-budget-exceeded`

The call ran out of its CPU budget (500,000,000 units of fuel). Do less per call: `render` should
only lay out what `update` fetched.

<!-- ref: error time-limit-exceeded -->
### `time-limit-exceeded`

The call ran past its time limit: 1 s for `render`, 10 s for `update` and an action.

<!-- ref: error memory-limit-exceeded -->
### `memory-limit-exceeded`

The call tried to grow its memory past 32 MiB, all memories together.

<!-- ref: error plugin-crashed -->
### `plugin-crashed`

The widget trapped: a panic, an out-of-bounds access, an `unreachable`. In Rust, a panic in
`render`, `update` or `act` ends here; return `Err` instead.

<!-- ref: error plugin-error -->
### `plugin-error`

`render`, `update` or `act` returned `Err`; the detail is the widget's message, escaped. For
`update` and an action, the host keeps the previous data.

<!-- ref: error invalid-output -->
### `invalid-output`

What `render`, `update` or `act` returned is not valid JSON. Content that is JSON but not in
[the content format](content.md) is not an error of the call: the app draws the content's error
in the widget's frame, and `runtime run --check-content` prints it.

<!-- ref: error output-too-large -->
### `output-too-large`

What `render` returned is over 64 KiB, or the data `update` or `act` returned is over 256 KiB.
The host checks the size before it parses anything, and keeps the previous data.

<!-- ref: error undeclared-action -->
### `undeclared-action`

A button, a form or `runtime act` asked for an action the manifest does not declare in `actions`,
or the widget is built for a contract older than 1.4.0, which has no actions. The host refused it
before the widget ran. Declare the action ([actions](manifest.md#actions)), or fix its name in the
content.

## Connectors

<!-- ref: error invalid-connector -->
### `invalid-connector`

A connector file is too large, is not JSON, or breaks [the connector format](connector-format.md)
or a rule only the host checks; the detail names it. `runtime check-connector <file>` checks one.

<!-- ref: error credentials-not-supported -->
### `credentials-not-supported`

A connector declares a credential under `credentials` or `auth`. The field is `credential`.

<!-- ref: error credential-needs-https -->
### `credential-needs-https`

A connector that declares a credential has a destination without HTTPS. Only a local connector
that sends no credential may use plain HTTP, in development, and only to this machine
(`localhost`, `127.0.0.1` or `[::1]`).

<!-- ref: error destination-not-verified -->
### `destination-not-verified`

A connector's id names a repository, whose destinations nothing verifies yet, or it is a local id
(`local/<name>`) while the runtime is not in development mode. Local connectors load only in the
development CLI, from `--connectors <dir>`, or else from `<data>/connectors/`.

<!-- ref: error destination-reserved -->
### `destination-reserved`

A connector outside the catalog claims a catalog id, or a host the catalog reserves. Give a local
connector an id and a destination of its own.

<!-- ref: error connector-unavailable -->
### `connector-unavailable`

A connector the manifest declares is not found, has another major version, or lacks an operation
the widget declares. The detail names which.

## Storage

<!-- ref: error storage-failed -->
### `storage-failed`

The writes of an update could not be committed: the store was busy, damaged or unavailable, a
write reached the file's size cap, or an earlier failure ended the run's writes; or a widget's
required store is unavailable as it loads. Nothing the update wrote was kept. From
`runtime storage show` and `runtime storage clear`, also: the store is busy or unavailable, and,
for `show`, damaged; `clear` deletes a damaged store instead. An update
whose store could not be opened at all writes nothing and keeps what it returned
([storage](contract.md#storage)).

## Accounts

<!-- ref: error account-not-connected -->
### `account-not-connected`

No account is connected to the connector, or the one connected must be connected again: run
`runtime accounts connect <connector-id>`. The CLI also gives it when the record of accounts is
damaged or cannot be written, which connecting does not fix until the record is fixed or deleted,
when the system's random source is unavailable, and when `accounts disconnect` cannot find the
connector and needs `--origin`. Inside `update`, a widget gets this as an answer of
`accounts`, never as an error that stops it.

<!-- ref: error another-account-connected -->
### `another-account-connected`

Another account, or another key, is already connected to the connector. Disconnect it first, with
`runtime accounts disconnect`.

<!-- ref: error sign-in-failed -->
### `sign-in-failed`

A sign-in ended without connecting an account: the browser's answer had a wrong or missing state,
a code already used, or the service's own error; the 10-minute limit passed; or the service gave a
token outside the bearer-token set. Run `runtime accounts connect` again.

<!-- ref: error key-refused -->
### `key-refused`

A key typed at `runtime accounts connect` is empty, longer than 1 KiB, or holds a character a
bearer token cannot (letters, digits and `-._~+/=` only). The key itself is never shown.

<!-- ref: error keychain-unavailable -->
### `keychain-unavailable`

The system's keychain is locked, missing or did not answer within 2 minutes, or what it would
keep for the account is too large. Unlock it and try again. On Linux, the keychain is the Secret
Service (GNOME Keyring or KWallet).

<!-- ref: error no-client-configured -->
### `no-client-configured`

The connector's account needs a client registration with the service, and the host has none for
it. In development, register an app with the service and give its client id with
`runtime accounts connect --client <id>`.
