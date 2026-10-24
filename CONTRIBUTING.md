# Contributing

Thanks for your interest in the project. Its goal is to be the place where installing other
people's plugins is safe, so every change must respect the rules in the
[project constitution](.specify/memory/constitution.md). Please read it before you start.

The project is pre-alpha. The current focus is the plugin sandbox; the user interface comes
after it.

## Before you start

- Report security vulnerabilities privately, as described in [SECURITY.md](SECURITY.md).
  Never open a public issue for them.
- For anything beyond a small fix, open an issue first so we can agree on the approach.
- Everyone taking part follows the [Code of Conduct](CODE_OF_CONDUCT.md).

## Development setup

You need:

- Rust (stable), installed with [rustup](https://rustup.rs/), with the WebAssembly targets
  plugins build for: `rustup target add wasm32-unknown-unknown wasm32-wasip2`
- Node.js 24 or newer
- [pnpm](https://pnpm.io/installation) 12 (the exact version is pinned in `package.json`)
- The [Tauri system prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS.
  On Debian or Ubuntu:

  ```bash
  sudo apt-get install libwebkit2gtk-4.1-dev libxdo-dev libssl-dev \
    libayatana-appindicator3-dev librsvg2-dev
  ```

Then, from the repository root:

```bash
pnpm install
pnpm tauri dev
```

See the [README](README.md) for the repository layout.

## Checks

CI runs these on Linux, macOS and Windows; run them locally before opening a pull request:

```bash
pnpm --filter ./app build                                      # UI type-check and build
cargo xtask plugins --locked                                   # the plugins the tests load
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy --manifest-path plugins/Cargo.toml --target wasm32-unknown-unknown \
  --target-dir target/plugins-build --locked -- -D warnings    # the plugins themselves
cargo test --workspace --locked                                # includes the hostile suite
cargo audit                                                    # known vulnerabilities
cargo audit --file plugins/Cargo.lock
cargo deny check                                               # licenses, sources, advisories
cargo deny --manifest-path plugins/Cargo.toml check
```

Install `cargo audit` and `cargo deny` with `cargo install --locked cargo-audit cargo-deny`.
`cargo test` fails, never skips, when a plugin it loads was not built: run
`cargo xtask plugins` first.

Performance is measured by hand, on the three systems: a maintainer adds the `bench` label to
the pull request, which runs [`bench.yml`](.github/workflows/bench.yml). Compare its results
with the targets in
[`specs/001-plugin-runtime/performance.md`](specs/001-plugin-runtime/performance.md).

## How features are built

New features start as a spec in [`specs/`](specs/) and go through
[GitHub Spec Kit](https://github.com/github/spec-kit): `/speckit-specify`, `/speckit-plan`,
`/speckit-tasks`, `/speckit-implement`. Every plan must pass the Constitution Check. Bug fixes
and small changes do not need a spec.

The repository holds what every contributor shares: the specs, the constitution, and Spec Kit's
templates and scripts in `.specify/`. The commands for your coding agent are not committed, so
the project does not depend on any one agent. Generate them with the pinned Spec Kit version,
which needs [uv](https://docs.astral.sh/uv/):

```bash
# Spec Kit v1.2.2
uv tool install specify-cli --from git+https://github.com/github/spec-kit.git@74be0a63e93c7087c5b06556b7561810d8f78a45
specify integration install claude --script sh   # or copilot, gemini, cursor-agent, codex…
```

`specify integration list` shows every supported agent. The repository ships the bash scripts
(`--script sh`); on Windows, run your agent with Git Bash or WSL. Keep the generated files out
of your commits (for Claude Code: `.claude/`, `.specify/integration.json`,
`.specify/integrations/claude.manifest.json` and `.specify/init-options.json`) by adding them
to your global gitignore or to `.git/info/exclude`.

You do not need Spec Kit to read or edit a spec: specs are plain Markdown.

## Security rules for every change

- A change that adds or touches a security rule comes with an automated test that fails when
  the rule is broken. The security test suite only grows: do not delete or weaken its tests.
- Changes to the security core (plugin runtime, permission engine, connector engine and
  network proxy, plugin storage in `runtime/src/storage/`, secrets vault, and the official
  connector catalog in `connectors/`) get
  line-by-line human review before merge, including code written with AI help.
  They always go through a pull request, even when a maintainer could push to `master`
  directly, and the pull request records who reviewed them.
- New Rust dependencies must come from crates.io and use a license allowed in
  [`deny.toml`](deny.toml); `cargo deny check` enforces both.

## Pull requests

- Keep each pull request focused on one change, and explain what it does and why.
- Link the issue or spec it implements.
- CI must be green on all three systems before merge.
- Pull requests merge with a merge commit, so `master` keeps the commits as they were reviewed.
  After a review, push fixes as new commits instead of rewriting reviewed ones.
- Use a short summary line for commits, optionally with a type prefix such as `feat:`,
  `fix:`, `docs:` or `ci:`.

## License

By contributing, you agree that your contributions are licensed under the
[Apache License 2.0](LICENSE), as stated in its section 5.
