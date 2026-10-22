# hello-widget

The example plugin for contract v0, written in Rust. It greets the name in its configuration
and writes one log message. Start a new plugin from a copy of this folder.

## Contract v0

The contract is [`wit/widget.wit`](../../../wit/widget.wit), the only copy, shared by the
host and every plugin.

- The plugin exports `render(config: string) -> result<string, string>`. It receives the
  configuration as JSON text and returns the widget content as JSON text. An `err` reaches the
  host as a `plugin-error` with your message.
- The plugin may import one thing: `log`, to write `debug`, `info`, `warn` or `error` messages.

Nothing else exists in v0: no files, network, clock, random numbers or environment. A plugin
that imports anything else is refused when it loads, with every refused import named
(`capability-refused`). That includes plugins built for `wasm32-wasip2`, because Rust's
standard library then imports WASI; build for `wasm32-unknown-unknown`.

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

This builds every crate of `plugins/` and writes `target/plugins/hello-widget.wasm`. By hand,
it is two steps: `cargo build --manifest-path plugins/Cargo.toml -p hello-widget --release
--target wasm32-unknown-unknown`, then turning the module into a component with
[wit-component](https://crates.io/crates/wit-component) (or `wasm-tools component new`).

## Run

```bash
cargo run -p runtime --release -- run target/plugins/hello-widget.wasm --config '{"name":"Ada"}'
```

```text
[plugin] info: rendering hello-widget        (stderr)
{                                            (stdout)
  "text": "Hello, Ada!",
  "title": "Hello"
}
```

Without `--config`, the configuration is `{}` and the plugin greets `world`.
[`contracts/cli.md`](../../../specs/001-plugin-runtime/contracts/cli.md) describes the output
and exit codes.

## Your own plugin

Copy this folder, rename the crate and add it to `members` in `plugins/Cargo.toml`. This
folder holds the project's examples; plugins from other authors live in their own
repositories.
