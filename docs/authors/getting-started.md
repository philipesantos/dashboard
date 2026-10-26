# Get started: your first widget

This walkthrough takes you from an empty folder to a widget of your own, built, packed with its
manifest, approved and running in the development CLI. It takes about half an hour, most of it
while cargo builds. Every command and every output below runs on Windows, macOS and Linux in the
project's CI, so what you see is what the docs show; values that change from run to run appear as
`<digest>`, `<path>` and the like.

It was walked with Rust 1.99.1 and its `wasm32-unknown-unknown` target, `wit-bindgen` 0.63.0,
`serde_json` 1.0.154 and the development CLI built from the repository, against contract
`dashboard:widget@1.3.0`. Where a command differs between systems, the page shows each form: the
`$` lines are for bash on Linux and macOS, the `PS>` lines for PowerShell 7 on Windows.

## 1. The tools

- Rust 1.99.1, installed with [rustup](https://rustup.rs), and the target widgets build for:
  `rustup toolchain install 1.99.1` and `rustup target add wasm32-unknown-unknown`.
- git.
- On Linux, the two packages the development CLI's keychain support builds with: on Debian and
  Ubuntu, `sudo apt-get install libdbus-1-dev pkg-config`.

## 2. The development CLI

Until the app has releases, you test a widget with `runtime`, the development CLI, built from the
repository. Clone it next to where your widget will live and install the CLI; the install builds
the whole runtime and takes several minutes.

```console check output=any
$ git clone https://github.com/philipesantos/dashboard
$ cargo install --path dashboard/runtime --locked
```

`runtime` is now on your `PATH`. The clone also holds the contract your widget builds against.

## 3. A crate for the widget

A widget is a Rust library built for WebAssembly. Make its folder beside the clone, with the
contract copied into it: `wit/history/1.3.0/widget.wit` is contract 1.3.0 as published, and it
never changes, so your widget keeps building whatever the clone does later.

```console check os=linux,macos
$ mkdir -p my-widget/src my-widget/wit
$ cd my-widget
$ cp ../dashboard/wit/history/1.3.0/widget.wit wit/
```

```console check os=windows
PS> New-Item -ItemType Directory -Path my-widget/src, my-widget/wit | Out-Null
PS> cd my-widget
PS> Copy-Item ../dashboard/wit/history/1.3.0/widget.wit wit/
```

`Cargo.toml` makes it a library WebAssembly can load (`cdylib`), with the two crates it uses, at
the versions this page was walked with:

```toml file=Cargo.toml check
[package]
name = "my-widget"
version = "0.1.0"
edition = "2024"
publish = false

[lib]
crate-type = ["cdylib"]

[dependencies]
serde_json = "=1.0.154"
wit-bindgen = "=0.63.0"
```

`src/lib.rs` is the widget. `wit_bindgen::generate!` reads the contract in `wit/` and gives it the
`widget` world: the `log` interface to call and the `renderer` interface to implement, whose
`render` gets the widget's configuration as JSON and returns its content as JSON.

```rust file=src/lib.rs check
//! A widget that greets the name in its configuration and writes one log message.

wit_bindgen::generate!({ path: "wit", world: "widget" });

use dashboard::widget::log::{Level, write};
use exports::dashboard::widget::renderer::Guest;
use serde_json::{Value, json};

struct MyWidget;

impl Guest for MyWidget {
    /// `config` is the JSON configuration the host passes; the content returned is JSON too.
    /// An `Err` reaches the host as a `plugin-error` with this message.
    fn render(config: String) -> Result<String, String> {
        // `log` is required in the manifest, so the host runs this widget only with `log`
        // granted, and `write` answers `ok`
        let _ = write(Level::Info, "rendering my-widget");

        let config: Value = serde_json::from_str(&config)
            .map_err(|error| format!("the configuration is not valid JSON: {error}"))?;
        let name = match config.get("name") {
            None => "world",
            Some(Value::String(name)) => name,
            Some(_) => return Err(r#""name" in the configuration must be a string"#.to_owned()),
        };
        Ok(json!({"title": "Hello", "text": format!("Hello, {name}!")}).to_string())
    }
}

export!(MyWidget);
```

`manifest.json` says who the widget is and what it asks for. A widget you build yourself has an id
`local/<name>`; it asks for `log`, as required, so it runs only once you approve it.

```json file=manifest.json check
{
  "manifest_version": 1,
  "id": "local/my-widget",
  "version": "0.1.0",
  "author": "Your name",
  "permissions": [
    { "name": "log", "required": true }
  ]
}
```

## 4. Build it

```console check output=any
$ cargo build --release --target wasm32-unknown-unknown
```

Cargo writes a WebAssembly module, `target/wasm32-unknown-unknown/release/my_widget.wasm`.

## 5. Pack it

The host loads packages: one WebAssembly component with the manifest inside. `runtime pack` turns
cargo's module into a component, checks the manifest and adds it, and prints the package's
digest, which approvals are tied to.

```console check
$ runtime pack target/wasm32-unknown-unknown/release/my_widget.wasm --manifest manifest.json --output my-widget.wasm
packed my-widget.wasm
digest: sha256:<digest>
```

## 6. Approve it and run it

Nothing runs before you approve it:

```console check exit=1
$ runtime run my-widget.wasm
error: not-granted: log (local/my-widget has no approval; approve it first)
```

`runtime permissions` shows what the package asks for and what is granted; `runtime approve`
grants everything it requires:

```console check
$ runtime permissions my-widget.wasm
plugin: local/my-widget 0.1.0
author: Your name
contract: v1 (1.3.0)
approved: no
permissions:
  log  required  not granted
login providers: none
$ runtime approve my-widget.wasm
approved local/my-widget 0.1.0
granted: log
```

The approval is for this package file. Pack the widget again after a change and it still runs as
long as it asks for nothing more; a version that asks for more needs a new approval.

Now it runs. Its log message comes first, then the content `render` returned, which the CLI
prints as indented JSON:

```console check
$ runtime run my-widget.wasm --config '{"name":"Ada"}'
[plugin] info: rendering my-widget
{
  "text": "Hello, Ada!",
  "title": "Hello"
}
```

That is a widget of your own, running. The [guides](README.md#guides) show how to fetch data,
act as the user's account and keep data of its own; the [reference](README.md#reference) lists
every part of the contract.

## When something goes wrong

### A target that imports WASI

`wasm32-wasip2` builds a component directly, but its standard library imports WASI, the system
interface for files, clocks and sockets, and the host refuses every WASI import: widgets reach
nothing but what the contract offers.

```console check output=any
$ rustup target add wasm32-wasip2
$ cargo build --release --target wasm32-wasip2
```

```console check
$ runtime pack target/wasm32-wasip2/release/my_widget.wasm --manifest manifest.json --output wasi.wasm
packed wasi.wasm
digest: sha256:<digest>
```

```console check exit=1
$ runtime approve wasi.wasm
error: capability-refused: wasi:cli/environment@<version>, wasi:cli/exit@<version>, wasi:cli/stderr@<version>, wasi:cli/stdin@<version>, wasi:cli/stdout@<version>, wasi:cli/terminal-input@<version>, wasi:cli/terminal-output@<version>, wasi:cli/terminal-stderr@<version>, wasi:cli/terminal-stdin@<version>, wasi:cli/terminal-stdout@<version>, wasi:clocks/monotonic-clock@<version>, wasi:io/error@<version>, wasi:io/poll@<version>, wasi:io/streams@<version>
```

Build for `wasm32-unknown-unknown`, as in step 4.

### A manifest that is missing or invalid

The module cargo writes is not a package yet: it has no manifest, and it is not a component.

```console check exit=1
$ runtime run target/wasm32-unknown-unknown/release/my_widget.wasm
error: invalid-package: not a WebAssembly component
```

Run what `runtime pack` wrote. `pack` checks the manifest, naming what is wrong:

```json file=broken.json check
{
  "manifest_version": 1,
  "id": "local/my-widget",
  "version": "0.1.0",
  "permissions": [
    { "name": "log", "required": true }
  ]
}
```

```console check exit=1
$ runtime pack target/wasm32-unknown-unknown/release/my_widget.wasm --manifest broken.json --output broken.wasm
error: invalid-manifest: missing field `author` at line 8 column 1
```

### A newer contract than the CLI runs

A widget built against a contract the installed CLI does not have yet, such as a later minor, is
refused, naming both versions. Here the copied contract claims 1.4.0:

```console check os=linux,macos output=any
$ sed -i.bak 's/@1.3.0;/@1.4.0;/' wit/widget.wit
$ cargo build --release --target wasm32-unknown-unknown
```

```console check os=windows output=any
PS> (Get-Content wit/widget.wit) -replace '@1.3.0;', '@1.4.0;' | Set-Content wit/widget.wit
PS> cargo build --release --target wasm32-unknown-unknown
```

```console check
$ runtime pack target/wasm32-unknown-unknown/release/my_widget.wasm --manifest manifest.json --output newer.wasm
packed newer.wasm
digest: sha256:<digest>
```

```console check exit=1
$ runtime approve newer.wasm
error: contract-mismatch: dashboard:widget/renderer@1.4.0 (is built for contract 1.4.0, newer than 1.3.0, the newest this host runs; update the host)
```

Copy the contract the installed CLI runs, `wit/history/1.3.0/widget.wit` of the clone it was built
from, and build again.
