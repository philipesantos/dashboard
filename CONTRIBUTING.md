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

- Rust (stable), installed with [rustup](https://rustup.rs/)
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
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo audit                                                    # known vulnerabilities
cargo deny check                                               # licenses, sources, advisories
```

Install the last two tools with `cargo install --locked cargo-audit cargo-deny`.

## How features are built

New features start as a spec in [`specs/`](specs/) and go through
[GitHub Spec Kit](https://github.com/github/spec-kit): `/speckit-specify`, `/speckit-plan`,
`/speckit-tasks`, `/speckit-implement`. Every plan must pass the Constitution Check. Bug fixes
and small changes do not need a spec.

## Security rules for every change

- A change that adds or touches a security rule comes with an automated test that fails when
  the rule is broken. The security test suite only grows: do not delete or weaken its tests.
- Changes to the security core (plugin runtime, permission engine, network proxy, secrets
  vault) get line-by-line human review before merge, including code written with AI help.
- New Rust dependencies must come from crates.io and use a license allowed in
  [`deny.toml`](deny.toml); `cargo deny check` enforces both.

## Pull requests

- Keep each pull request focused on one change, and explain what it does and why.
- Link the issue or spec it implements.
- CI must be green on all three systems before merge.
- Use a short summary line for commits, optionally with a type prefix such as `feat:`,
  `fix:`, `docs:` or `ci:`.

## License

By contributing, you agree that your contributions are licensed under the
[Apache License 2.0](LICENSE), as stated in its section 5.
