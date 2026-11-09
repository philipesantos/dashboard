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
- On Linux, the packages the secrets vault and the host window build with. On Debian or Ubuntu:

  ```bash
  sudo apt-get install libdbus-1-dev pkg-config libxkbcommon-dev libxkbcommon-x11-dev \
    libxcb1-dev libx11-xcb-dev libfreetype-dev libfontconfig-dev libwayland-dev
  ```

Then, from the repository root, build the plugins and check everything:

```bash
cargo xtask plugins
cargo check --workspace
```

See the [README](README.md) for the repository layout.

## Checks

CI runs these on Linux, macOS and Windows; run them locally before opening a pull request. On
Linux the vault's Secret Service store needs `libdbus-1-dev` (and `pkg-config`) to build, and its
tests use the real keychain: run them in a desktop session whose keyring is unlocked, or in a
session bus of their own, as CI does:
`dbus-run-session -- bash -c "gnome-keyring-daemon --components=secrets --daemonize --unlock <<< 'dev' && cargo test --workspace --locked"`.


```bash
cargo xtask plugins --locked                                   # the plugins the tests load
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo clippy --manifest-path plugins/Cargo.toml --target wasm32-unknown-unknown \
  --target-dir target/plugins-build --locked -- -D warnings    # the plugins themselves
cargo test --workspace --locked                                # includes the hostile suite
cargo audit                                                    # known vulnerabilities
cargo audit --file plugins/Cargo.lock
cargo deny check                                               # licenses, sources, advisories
cargo deny --manifest-path plugins/Cargo.toml check
cargo audit --file docs/authors/getting-started/Cargo.lock    # what the walkthrough builds
cargo deny --manifest-path docs/authors/getting-started/Cargo.toml check
```

Install `cargo audit` and `cargo deny` with `cargo install --locked cargo-audit cargo-deny`.
`cargo test` fails, never skips, when a plugin it loads was not built: run
`cargo xtask plugins` first.

The runtime's integration tests build as one executable, `runtime/tests/it.rs`, since each
executable links the whole runtime (about 100 MB in a debug build). Each file in `runtime/tests/`
is a module of it: a new file goes in with `mod <name>;` there, and a test fails until it does.
Run one file's tests with `cargo test -p runtime --test it <file>::`. When the disk fills up,
`cargo xtask clean-space` removes the incremental caches, the executables a newer build replaced
and `plugins/target/`, without a full rebuild.

### The docs for authors

The docs under [`docs/authors/`](docs/authors/) are checked like code, so a change to them, or
to `wit/`, runs all of CI even when it touches only Markdown:

- **The reference stays in step.** A change to the contract, the manifest or connector schema, a
  limit, an error kind or a command or option of the CLI updates
  [`docs/authors/reference/`](docs/authors/reference/) in the same pull request: every entry
  carries a marker `<!-- ref: <kind> <id> -->`, and `cargo test -p runtime --test it
  docs_reference::` names each item the reference lacks or names wrongly, and each limit whose
  default differs from the host's. The format is in
  [`contracts/docs-format.md`](specs/006-contract-v1-docs/contracts/docs-format.md).
- **The commands run as written.** Every block marked `check` in the walkthrough, the guides and
  the reference runs in the `docs` job on the three systems, its output compared whole. The
  runner clears your development data directory and grant record before and after each page,
  so outside CI it asks for an explicit opt-in. It clones the committed `HEAD`, so commit
  first; on Linux it needs a Secret Service, as the tests do:

  ```bash
  DASHBOARD_DOCS_WALK_CLEARS_DEVELOPMENT_DATA=1 cargo test -p runtime --test it --locked -- --ignored docs_walk::
  DASHBOARD_DOCS_WALK_PAGES=getting-started.md DASHBOARD_DOCS_WALK_CLEARS_DEVELOPMENT_DATA=1 \
    cargo test -p runtime --test it --locked -- --ignored docs_walk::  # one page
  ```

- **The walkthrough's crate moves with its page.** `docs/authors/getting-started/` is the crate
  the walkthrough builds: the runner checks that the page shows each of its files as they are,
  and builds with its `Cargo.lock`, which `cargo audit` and `cargo deny` check. Change a
  dependency there, its lock file and the page together; the docs job pins Rust 1.99.1, the
  version the walkthrough names.
- **How the contract changes** from now on is in [`wit/README.md`](wit/README.md#how-v1-changes-from-now-on).

### The visual system

[`DESIGN.md`](DESIGN.md) is the only hand-edited source of the host's look; it, `design/`,
[`docs/design/`](docs/design/) and the threat model are read by checks, so a change to any of them
runs all of CI. To change the look:

1. Edit `DESIGN.md`: a token in the front matter, or a role, tone, elevation level, pick or icon
   in a `design-roles` block. Every token says why.
2. Run `cargo xtask design`. It checks the file and rewrites `design/tokens/src/generated.rs`,
   `design/host-surfaces/src/generated.rs` and `design/tokens.dtcg.json`; commit them with the
   change. The front matter's `colors` are the roles resolved at the default appearance: when a
   role changes, the check prints the value each color must take.
3. `cargo xtask design --check` runs every check without writing; CI runs it, and
   `cargo xtask design --preview <dir>` draws the SVG preview CI publishes as the `design-preview`
   artifact.

What a failure means:

- **A contrast failure** names the pairing, its kind, the theme, the appearance where it is worst
  and the ratio. Change the step the role uses, never a threshold (constitution v5.0.0,
  Development Workflow).
- **A drift failure** names the generated file and the token: run `cargo xtask design`.
- **A pin failure** means a file under `design/radix/`, `design/format/` or `design/fonts/` changed.
  Those are upstream files, unmodified; moving Radix, the format or a font to a new version is a
  commit of its own, with the new hashes in [`design/README.md`](design/README.md), reviewed like a
  dependency upgrade.
- **A criteria failure** means a threat-model row on boundary B4 tested from Phase 3 is neither
  answered by a criterion in [`docs/design/host-surfaces.md`](docs/design/host-surfaces.md) nor
  listed under its "Decided elsewhere", or a criterion quotes a row that is not there.
- **The `deny` job's ban fixture** must fail: only the `host` crate may depend on `host-surfaces`,
  and only `xtask` on `design-gen`.

The host compiles in Radix's colors and ships Barlow and IBM Plex Mono: when it is packaged,
Radix's MIT notice (`design/radix/3.0.0/LICENSE`) and the fonts' licenses
(`design/fonts/barlow/OFL.txt`, `design/fonts/plex-mono/LICENSE.txt`) go with it.

### The host window

The `host` crate draws the dashboard with GPUI (`cargo run -p host --bin dashboard`). Its checks
run with the others, and three of its test targets need more:

- `cargo test -p host --test pixels` draws offscreen on the GPU; without a GPU it needs Mesa's
  software Vulkan (`mesa-vulkan-drivers` on Debian or Ubuntu). Windows has no headless renderer
  and skips it.
- `cargo test -p host --test window` opens real windows and drives them with the system's own
  input and accessibility tree. It runs in the "Host window" CI job on the three systems; locally
  it takes over the keyboard and pointer for about a minute, and on Linux it needs X11 (the job
  runs it under Xvfb with the AT-SPI bus, `xdotool` and Python's AT-SPI bindings).
- `host/ban-fixture` calls each GPUI method `clippy.toml` bans and must fail clippy with one
  "disallowed method" error per call; the "Host window" job checks that it does.

A screen-reader walk of every host dialog, with VoiceOver, Narrator and Orca, comes before each
release ([`docs/accessibility/screen-reader-walk.md`](docs/accessibility/screen-reader-walk.md)).

**Upgrading GPUI.** GPUI comes from gpui-unofficial, a third party's republish of Zed's GPUI, and
its nineteen crates are pinned with `=` to one version, recorded in
[`host/README.md`](host/README.md). Upgrade at most once a month, or at once for a security
advisory:

1. Take a version published from a Zed stable tag at least two weeks old, never a pre-release.
2. Read the changes to gpui-unofficial's patch list (`docs/upstream-patches.md` in its repository)
   since the commit `host/README.md` records, and note anything that touches input, windows,
   prompts, URLs, assets, credentials or accessibility.
3. Run `cargo test -p host --test checks`. It reads the new GPUI's source and fails on every public
   function whose name or signature may load, read, open, prompt, decode, or reach the network or
   the keychain, unless it is banned in `clippy.toml` or listed in `host/gpui-reviewed.txt` under
   the reason it is safe; it also fails on a listed function GPUI no longer has. It is a word
   filter (a function's name, its arguments' names and types), so a new way out named like none
   of them passes it: step 2's reading by hand still decides. For each new one it flags,
   decide: ban it (in `clippy.toml`, in `BANNED` in `host/tests/checks.rs` and as a call in
   `host/ban-fixture/src/lib.rs`, raising the count the "Host window" job expects), or add it to
   the list under its reason. The check reads the crates for other systems only on those systems,
   so let CI run on all three.
4. Move all nineteen pins in `host/Cargo.toml` (and the fixture's in `host/ban-fixture/`)
   together, then update the table in `host/README.md`: version, Zed tag and commit, publish date
   and the patch list's commit. `host/tests/checks.rs` fails while any of them disagree.
5. Run `cargo test -p host` and `cargo clippy --manifest-path host/ban-fixture/Cargo.toml` (it
   must still fail with every ban), and let the "Host window" job run on the three systems before
   merging.

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
  network proxy, plugin storage in `runtime/src/storage/`, the secrets vault in
  `runtime/src/vault/` with the accounts and the sign-in, and the official connector catalog
  in `connectors/`) get
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
