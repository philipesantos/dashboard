# Contract: the formats the docs checks read

Pages under `docs/authors/` follow these rules so that the drift test
(`runtime/tests/docs_reference.rs`) and the docs runner (`runtime/tests/docs_walk.rs`) can read
them. Readers on GitHub see ordinary Markdown: markers are HTML comments, and the extra words of
a fence's info string are not shown.

## Reference markers

```markdown
<!-- ref: <kind> <id> -->
```

One marker per entry, on the line before the entry's heading or, in a table, at the end of the
entry's row. Kinds and ids:

| Kind | Id | Example |
| --- | --- | --- |
| `wit` | `<world>`, `<interface>`, `<interface>/<function>`, `<interface>/<type>`, `<interface>/<type>/<case or field>` | `wit connectors/call-error/not-granted` |
| `error` | the name `ErrorKind::as_str` returns | `error capability-refused` |
| `manifest` | a field's path in `manifest.schema.json`, `[]` for array items | `manifest connectors[]/account` |
| `connector` | a field's path in `connector.schema.json` | `connector operations[]/kind` |
| `limit` | the field's name in `Limits` | `limit max_package_bytes` |

A `limit` marker sits in a table row whose second cell is the default, written as `10 MiB`,
`64 KiB`, `1 s`, `500 ms`, `500,000,000` or `30 per minute`; the test formats `Limits::default()`
the same way and compares.

The drift test fails, naming each item, when: an item of a source has no marker; a marker names
an item no source has; a marker repeats; a limit's default differs; or a source holds a
definition the test cannot classify.

## Runnable blocks

````markdown
```console check
$ runtime pack target/wasm32-unknown-unknown/release/my_widget.wasm --manifest manifest.json --output my-widget.wasm
packed my-widget.wasm
digest: <digest>
```

```console check os=windows
PS> Copy-Item ..\dashboard\wit\history\1.3.0\widget.wit wit\
```

```console check exit=2
$ runtime run my-widget.wasm --config '{"name":"Ada"}'
error: not-granted: log (local/my-widget has no approval; approve it first)
```

```toml file=Cargo.toml check
[package]
name = "my-widget"
```
````

- Info string: the language (`console` for commands, any other for a file), then `check`, then
  optional `os=linux|macos|windows` (comma-separated), `exit=<n>`, `file=<path>`, `output=any`.
- `output=any` checks only the exit code, for tools whose progress lines vary between runs
  (cargo, git); the runner refuses it on a block with a `runtime` command, whose output is always
  compared whole.
- In `console` blocks, a line starting `$ ` is a command run by bash on Linux and macOS; `PS> `
  by PowerShell 7 on Windows. A block without `os=` must use only one prompt kind that works on
  every system it runs on, or both forms in separate blocks with `os=`.
- Each command's stdout and stderr are merged as the shell does with `2>&1`; every line up to the
  next prompt or the end of the block is its expected output, compared line by line after
  trimming trailing spaces and normalizing line endings.
- Placeholders: `<digest>` (64 lowercase hex), `<path>` (a path under the session's folder or
  the data directory), `<n>` (an integer), `<time>` (a date-time or duration as the CLI prints
  it), `<version>` (a semver). Each matches only its kind; any other `<…>` is literal.
- A page whose front matter comment says `<!-- check: service=github,notes -->` runs its
  `runtime` commands through the test-only CLI, with those test services started, and has its
  accounts disconnected after it (R9). Services are declared only there, never on a block.
- Blocks without `check` are not run; the walkthrough and the guides may use them only for text
  that is not a command or a file (such as a widget's JSON content shown inline).

## What the runner guarantees per page

- A fresh temporary folder as the working directory.
- The walkthrough's `git clone` of the repository's address runs as written and gets the
  checkout under test (its committed `HEAD`), through git's `url.<base>.insteadOf` set in the
  environment; `cargo install` installs into the session's folder, first on `PATH`, building in
  the job's target folder (R3).
- The walkthrough's file blocks equal the files of `docs/authors/getting-started/`, and its
  `Cargo.lock` is in the session's folder before the page and unchanged after it (R8).
- The development data directory, found by the CLI's own lookup, cleared before the page; the
  runner refuses to start unless `CI` or `DASHBOARD_DOCS_WALK_CLEARS_DEVELOPMENT_DATA=1` is set.
  The test-only CLI uses a data directory of the session's own instead.
- Blocks run in order; the page fails at the first block that differs, showing the expected and
  the actual lines.
