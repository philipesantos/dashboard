# hello-widget

The example plugin for contract v1, written in Rust. It greets the name in its configuration
and writes one log message. Start a new plugin from a copy of this folder.

## Contract v1

This example builds from [`wit/history/1.0.0/widget.wit`](../../../wit/history/1.0.0/widget.wit),
the published contract 1.0.0, frozen: the host runs plugins built for any v1 minor up to its
own, so this one keeps running as the contract grows. A new plugin builds from
[`wit/v1/widget.wit`](../../../wit/v1/widget.wit), the current minor; the
[github-releases](../github-releases/README.md) example shows what 1.1.0 adds.

- The plugin exports `render(config: string) -> result<string, string>`. It receives the
  configuration as JSON text and returns the widget content as JSON text. An `err` reaches the
  host as a `plugin-error` with your message.
- The plugin may import `log`, to write `debug`, `info`, `warn` or `error` messages. `write`
  returns `result<_, log-error>`: when `log` is an optional permission the user has not turned
  on, it answers `err(not-granted)` and writes nothing. This plugin requires `log`, so it always
  gets `ok`.

Nothing else exists yet: no files, network, clock, random numbers or environment. A plugin that
imports anything else is refused when it loads, with every refused import named
(`capability-refused`). That includes plugins built for `wasm32-wasip2`, because Rust's
standard library then imports WASI; build for `wasm32-unknown-unknown`.

## Manifest

[`manifest.json`](manifest.json) says who the plugin is and what it needs: its id
(`local/hello-widget`), version, author, and the `log` permission, required. The build puts it
inside the package. Every permission the plugin uses must be declared there, or it is refused
(`not-declared`).

## Limits

The host sets these for every `render` call; a plugin can neither read nor change them. The
values were measured on all three systems
([performance.md](../../../specs/001-plugin-runtime/performance.md)).

| Limit | Default | When exceeded |
| --- | --- | --- |
| Plugin package | 10 MiB | `invalid-package` at load |
| CPU budget | 500,000,000 fuel units, about 0.35 s of computation | `cpu-budget-exceeded` |
| Time | 1 s | `time-limit-exceeded` |
| Memory | 32 MiB, all memories together | `memory-limit-exceeded` |
| Widget content | 64 KiB, and it must be valid JSON | `output-too-large`, `invalid-output` |
| Log messages | 100 per call, 1 KiB each | the rest are dropped and counted; longer ones are cut |

Each call runs in a fresh instance: nothing your plugin keeps in memory survives to the next
call. Log messages and error text are shown escaped, so line breaks and control characters
appear as `\n` or `\u{1b}` instead of acting on the terminal.

## Build

```bash
rustup target add wasm32-unknown-unknown
cargo xtask plugins
```

This builds every crate of `plugins/` and writes `target/plugins/hello-widget.wasm`, with the
manifest inside it.

## Approve and run

A plugin runs only after you approve its required permissions:

```bash
cargo run -p runtime --release -- approve target/plugins/hello-widget.wasm
cargo run -p runtime --release -- run target/plugins/hello-widget.wasm --config '{"name":"Ada"}'
```

```text
[plugin] info: rendering hello-widget        (stderr)
{                                            (stdout)
  "text": "Hello, Ada!",
  "title": "Hello"
}
```

The approval belongs to that file: rebuilding it in place keeps it, while a copy elsewhere
needs its own. Without `--config`, the configuration is `{}` and the plugin greets `world`.
[`contracts/cli.md`](../../../specs/002-plugin-permissions/contracts/cli.md) describes every
command, its output and its exit codes.

## Your own plugin

Copy this folder, rename the crate, give `manifest.json` your plugin's id, and add the crate to
`members` in `plugins/Cargo.toml`. This folder holds the project's examples; plugins from other
authors live in their own repositories.
