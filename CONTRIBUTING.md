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

  <!-- WEBKITGTK-ONLY(system-libs): the window's engine on Linux, see docs/webkitgtk-register.toml -->

  ```bash
  sudo apt-get install libwebkit2gtk-4.1-dev libxdo-dev libssl-dev \
    libayatana-appindicator3-dev librsvg2-dev
  ```

Then, from the repository root:

```bash
pnpm install
cargo xtask plugins --locked
pnpm tauri dev
```

See the [README](README.md) for the repository layout.

## Checks

CI runs these on Linux, macOS and Windows; run them locally before opening a pull request. On
Linux the vault's Secret Service store needs `libdbus-1-dev` (and `pkg-config`) to build, and its
tests use the real keychain: run them in a desktop session whose keyring is unlocked, or in a
session bus of their own, as CI does:
`dbus-run-session -- bash -c "gnome-keyring-daemon --components=secrets --daemonize --unlock <<< 'dev' && cargo test --workspace --locked"`.


```bash
cargo xtask plugins --locked                                   # the plugins: first, the app embeds some
pnpm --filter ./design check                                   # the visual system (below)
pnpm --filter ./design test
pnpm --filter ./design run licenses                            # npm licenses
pnpm audit --audit-level high                                  # npm advisories
pnpm --filter ./app build                                      # UI type-check and build
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
`cargo test` fails, never skips, when a plugin it loads was not built, and the app does not build
without the widgets it embeds: run `cargo xtask plugins` first, after every change under
`plugins/`.

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

[`DESIGN.md`](DESIGN.md) is the only source of the app's look, in Google's open `DESIGN.md`
format at version 0.4.0 plus the project's `design-rules` blocks. Never edit a color, size or
shadow anywhere else:

1. Edit `DESIGN.md`, with the token's rationale in bold-named prose beside it.
2. Run `pnpm --filter ./design gen`, which writes `app/src/theme.generated.css` and
   `design/widget-map.generated.json`. Commit them with `DESIGN.md`.
3. Run `pnpm --filter ./design check`. It fails, one line per problem, when:
   - **format**: the pinned design.md checker reports an error (its warnings only print);
   - **project layer**: a role lacks its `-dark` twin or its bold rationale, a section is out of
     order, or a `design-rules` name does not resolve;
   - **contrast and coverage**: a declared pairing is under WCAG AA in either theme (text 4.5:1;
     large text, a control's edge, the focus ring and chart marks 3:1), or a color role is in no
     pairing and not marked decorative;
   - **color usage**: a component in `app/src` draws a color that is not a role, or a role at an
     opacity no pairing declares;
   - **host-only**: a tone or variant widget content can select reaches a role only the host may
     draw (the chrome, the frame, dialogs, the scrim, the focus ring);
   - **drift**: a generated file differs from what `DESIGN.md` generates;
   - **criteria coverage**: a criterion in
     [`docs/design/host-surfaces.md`](docs/design/host-surfaces.md) lacks its `Check:` or
     `Feature:`, or a threat-model row on the plugin UI boundary in Phase 3 cites none, or cites
     one checked only by review;
   - **font licenses**: a font family has no license text in `app/public/licenses/fonts/`.

`pnpm --filter ./design preview` writes `design/preview/index.html`, a static page with both
themes, every pairing's ratio, the scales, the tones and variants and a sketch of the host's
surfaces; open it in a browser. CI uploads it from the Linux build as the `design-preview`
artifact. A change to `DESIGN.md`, `design/`, `docs/design/` or the threat model runs all of CI,
even when it touches only Markdown.

Work that draws the chrome, a widget frame or a host dialog must pass the criteria in
[`docs/design/host-surfaces.md`](docs/design/host-surfaces.md) that name its feature, or change
the criterion first.

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

### The window's protections

The host window loads only the app's bundle and can reach no host command unless the reviewed
list says so (`specs/008-tauri-shell/`). Three things keep it that way.

**The shell probe** is the release build plus one page module and two commands. Inside the real
page it tries every forbidden action (injected script, loads from the internet, navigation, new
windows, host commands, engine permissions, downloads and dialogs) and reads each engine's
settings, then exits non-zero if anything was allowed. CI runs it on all three systems. Locally:

```bash
pnpm --filter ./app build --mode probe          # adds src/probe/run.ts to the page
cargo build -p dashboard --release --features shell-probe,tauri/custom-protocol --locked
DASHBOARD_PROBE=1 ./target/release/dashboard    # on Linux: xvfb-run -a, with
                                                # WEBKIT_DISABLE_DMABUF_RENDERER=1 (needs xvfb)
pnpm --filter ./app build                       # put the normal dist back
```

It prints `probe: running <id>` before each case, one `held` or `broken` line per case, and a
summary. A missing report after 60 s means a case unloaded or blocked the page: the last
`running` line names it. Add a case under `app/src/probe/` whenever you add a protection, and
its id to `PAGE_CASES` in `probe.rs`: a case missing from either breaks the run.

**The reviewed list.** `app/src-tauri/window-surface.toml` lists every permission, command and
Tauri plugin the window may use, each with its reason, and `tests/surface.rs` fails when the
list and the app disagree. To add a host command, follow the three steps in that file's comment.

**No engine pickers.** The window cancels every file input, and `showPicker()` (file, date
and color) and the browser's file APIs do nothing; the console says so. A feature that needs a
file goes through a host dialog, Tauri's dialog plugin, added to `window-surface.toml` with its
reason (FR-021).

**Run-time styles carry the nonce.** The content policy refuses a `<style>` element without the
response's nonce. A Radix or shadcn component that draws one takes `nonce={getNonce()}` (from
`get-nonce`), and `shadcn add` must not drop it: `tests/style_nonce.rs` fails otherwise.

**WebKitGTK-only changes are registered.** While Linux draws the window with WebKitGTK
([decision 0002](docs/decisions/0002-linux-window-engine.md)), anything you add only because of
it, in code, configuration, dependencies, CI or the UI, gets a `WEBKITGTK-ONLY(<id>)` comment
where it is made and an entry in [`docs/webkitgtk-register.toml`](docs/webkitgtk-register.toml),
so the move to the embedded engine can remove it. `tests/webkitgtk_register.rs` fails when the
two disagree, or when a line in `app/` or `.github/` names the engine with no marker nearby. It
cannot see a change that never names the engine, such as a CSS value picked for how WebKitGTK
draws, so reviewers ask about those. Code the macOS engine shares is not WebKitGTK-only; name
the engines by system in its comments.

### Widgets

A widget never draws anything itself. Its `render` returns content in the host's format
([`docs/authors/reference/content.md`](docs/authors/reference/content.md), schema
`wit/content.schema.json`); `widget-content/` checks it against the schema's kinds and the
host's bounds, and `app/src/widgets/` draws it with the host's own components inside the host's
frame, every string as text inside `<bdi>`. The frame's header, the plugin's name and its icon
belong to the host. A button or form runs only an action the manifest declares, through
`widget_act`, under the grants and limits of an update; the data an action returns stays in
memory until the app quits.

- **A new component, field kind, tone or bound** changes the schema, `widget-content/`, the
  drawing in `app/src/widgets/content/` and the author page together; `widget-content`'s tests
  and `runtime/tests/docs_reference.rs` fail when they disagree. A tone or variant that widget
  content can select must reach only shared roles (`pnpm --filter ./design check`, host-only).
- **Bundled widgets** are built by `cargo xtask plugins` (run it before building the app) and
  embedded by `app/src-tauri/build.rs` from `BUNDLED`. One that declares no permission, connector
  or storage is part of the app and approved at start-up with nothing granted (constitution,
  Principle VII); the app refuses to load a bundled package that declares any, and
  `tests/widgets.rs` checks that a release build embeds exactly `showcase`.
- **Hostile widgets** live in `plugins/hostile/ui-*`, one broken rule each, and are embedded
  only in the probe build (`PROBE_BUNDLED`). The probe's widget cases in
  `app/src/probe/widgets.ts` break each frame and content rule in the real page: the header kept
  and hittable, content clipped and drawn as text, isolated strings, nothing outside the frame,
  no credential inputs, status icons, foreign actions refused, error states shown, the window
  responsive under a looping action. A new rule on widget content gets a fixture and a case
  here, listed in `run.ts` and `PAGE_CASES`.

## Security rules for every change

- A change that adds or touches a security rule comes with an automated test that fails when
  the rule is broken. The security test suite only grows: do not delete or weaken its tests.
- Changes to the security core (plugin runtime, permission engine, connector engine and
  network proxy, plugin storage in `runtime/src/storage/`, the secrets vault in
  `runtime/src/vault/` with the accounts and the sign-in, the official connector catalog
  in `connectors/`, the window's protections and the widget host listed below) get
  line-by-line human review before merge, including code written with AI help.
  They always go through a pull request, even when a maintainer could push to `master`
  directly, and the pull request records who reviewed them.
- The window's protections, in the security core since constitution v2.6.0:
  - `app/src-tauri/src/window.rs`, `lib.rs` (the `commands!` macro and the invoke handler),
    `commands.rs` and `probe.rs`;
  - `app/src-tauri/build.rs`, `permissions/` and `capabilities/`;
  - `app/src-tauri/window-surface.toml`;
  - `tauri.conf.json`'s `app.security` and `bundle` blocks, and the `tauri` features in
    `app/src-tauri/Cargo.toml`;
  - `app/index.html`'s empty `<style>`, and in `app/src/main.tsx` the nonce hand-off and the
    context-menu cancel; the `nonce` props in `app/src/components/ui/`;
  - `app/vite.config.ts`'s development policy;
  - the tests that guard them: `app/src-tauri/tests/surface.rs` and `style_nonce.rs`,
    `app/src/probe/` and the probe step in `ci.yml`.
- The widget host, in the security core since feature 009:
  - `app/src-tauri/src/widgets.rs` (the bundled widgets, their records and log, and the checks
    before a plugin runs), `name.rs` (the drawn name) and the three commands `widgets_list`,
    `widget_render` and `widget_act` in `commands.rs`;
  - `app/src/widgets/`, which draws the frame and every component of widget content;
  - `widget-content/`, which checks content and turns it into the tree the page draws, and the
    content schema `wit/content.schema.json`.
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
