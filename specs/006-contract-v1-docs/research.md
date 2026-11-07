# Research: Contract v1 Closed and Docs for Plugin Authors

Decisions for the plan, each with its rationale and the alternatives weighed. Facts about the
repository were checked on `master` at a8cd948.

## R1. Where an author outside the repository gets the contract

**Decision**: the walkthrough copies the frozen version, `wit/history/1.3.0/widget.wit` (or
2.0.0, if chosen), from the clone into a `wit/` folder of the author's crate, and the crate binds
it with `wit_bindgen::generate!({ path: "wit", world: "widget" })`, as `hello-widget` binds
`wit/history/1.0.0`.

**Rationale**: the frozen copy never moves, so a widget built from it keeps building after the
host adds a minor; the author already has the clone to install the CLI (R3). A copy is one
command on each system.

**Alternatives considered**: pointing `generate!` at the clone's `wit/v1` (the author's build
then breaks or changes when the clone is updated); a CLI command that writes the contract it
runs (more host code for something a copy does; may come with installation in Phase 3).

## R2. Turning the author's build into a component: `pack` accepts a core module

**Decision**: `runtime pack` accepts either a component, as today, or the core module cargo
builds for `wasm32-unknown-unknown`, which it turns into a component with `wit-component`
(`ComponentEncoder`, validation on), exactly as `cargo xtask plugins` does, before adding the
manifest. The runtime takes `wit-component` `=0.262.0`, the release `xtask` uses
(`xtask/Cargo.toml:12`) and the one `wit-bindgen` 0.63.0 embeds its component type with
(`plugins/Cargo.lock`), so `pack` decodes what the author's build wrote with the encoder that
wrote it. A module whose embedded world is missing or invalid is refused as `invalid-package`,
naming the cause, escaped as every plugin text is: the encoder's messages quote import, export
and section names taken from the module. Nothing else about the package changes, and the host
checks the result on load as any package. Two tests come with it: `pack` on the core module of
every example gives the same bytes as `cargo xtask plugins`, and a core module whose import name
and embedded world hold newlines and control characters is refused on one escaped line.

**Rationale**: the walkthrough must not need the repository's xtask (FR-007), and the fixed
decision for Rust plugins is wit-bindgen on `wasm32-unknown-unknown` componentized with
wit-component. With `pack` doing it, an author needs cargo and the CLI, nothing more, on three
systems.

**Alternatives considered**: `wasm-tools component new` (a second tool to install and pin on
three systems, one more step to explain; it runs the same library); `wit-component` `=0.261.0`,
the release Wasmtime 50's own build uses (one copy fewer of `wasmparser` in the binary, but a
decoder one release behind what `wit-bindgen` 0.63.0 writes); a `cargo` build script in
the author's crate (cannot post-process the final artifact); `wasm32-wasip2`, which builds a
component directly but imports WASI, which the host refuses.

## R3. Installing the development CLI

**Decision**: the walkthrough clones the repository and runs `cargo install --path
<clone>/runtime --locked`, which puts the `runtime` binary on the author's `PATH`; every later
command calls `runtime …`. On Linux the walkthrough lists `libdbus-1-dev` and `pkg-config`, which
the vault's store needs to build.

In the runner, both commands run as written without reaching GitHub or the contributor's own
install: git's `url.<base>.insteadOf`, set through the `GIT_CONFIG_COUNT`/`GIT_CONFIG_KEY_*`/
`GIT_CONFIG_VALUE_*` variables, sends the clone to the checkout under test (its committed `HEAD`);
`CARGO_INSTALL_ROOT` points at the session's folder, put first on `PATH`. For a `cargo install`
command only, the runner adds `CARGO_TARGET_DIR` (the job's cached target folder),
`CARGO_PROFILE_RELEASE_LTO=false` and `CARGO_PROFILE_RELEASE_CODEGEN_UNITS=16` to that command's
environment. Set for the whole session, they would move the author's own `cargo build` output
away from the `target/` the walkthrough packs from.
Measured on an 8-core Linux machine: the release build takes 279 s from nothing and 149 s with
every dependency built, because fat LTO optimizes Wasmtime again at every link; without LTO and
with the dependencies cached it takes 16 s. The binary is built from the same code and only less
optimized, and no output the guides show depends on speed (the limits count fuel, and the time
limits are far from what the examples use), so the command still runs as written.

**Rationale**: there are no releases until Phase 6 (spec, Assumptions); an installed binary
reads the same on three systems, unlike the `cargo run -p runtime --release --` alias of today's
docs, which only works inside the clone.

**Alternatives considered**: the `rt()` shell function (bash only, inside the clone); prebuilt
binaries (Phase 6, with the project's name).

## R4. Pinned versions

**Decision**: the walkthrough is walked with Rust 1.99.1 and the `wasm32-unknown-unknown`
target, `wit-bindgen` `=0.63.0` and `serde_json` `=1.0.154` (the versions in
`plugins/Cargo.lock`), and says so. The docs job installs exactly Rust 1.99.1; moving a pin is a
reviewed change to the walkthrough and the job together. The build job keeps `stable`.

**Rationale**: FR-007 asks for the versions it was walked with, and the edge case of a tool
release that breaks a step: pinning the docs job keeps the check about the docs, not about the
newest compiler.

**Alternatives considered**: `stable` everywhere (a new release could fail the docs job for
reasons unrelated to the change under review); a `rust-toolchain.toml` in the author's crate (an
extra file to explain, and it would not pin the CLI's build).

## R5. Layout of the docs

**Decision**: `docs/authors/` with an index, `getting-started.md`, five guides (render and
update, connectors, accounts, storage, writing connectors) and six reference pages (contract,
errors, manifest, connector format, limits, CLI). `docs/plugin-authors.md` and
`docs/connectors.md` become short pointers. The specs' CLI contracts stay where they are, as
background; every fact an author needs is in the reference (FR-015).

**Rationale**: one page per thing an author is doing keeps pages short and gives the checks
fixed targets; pointers keep the links in specs, READMEs and SECURITY.md working.

**Alternatives considered**: one long `docs/plugin-authors.md` (already 454 lines before the
reference; the drift markers would make it longer still); a generated reference from the WIT
comments (the comments cannot say what a widget should do about each case, and frozen comments
cannot be corrected).

## R6. How the reference names what it covers

**Decision**: every entry carries a marker, an HTML comment invisible on GitHub:
`<!-- ref: <kind> <id> -->`, with kinds `wit`, `error`, `manifest`, `connector`, `limit`, `cli` and
ids such as `wit connectors/call-error/not-granted`, `error capability-refused`, `manifest
connectors[]/account`, `limit max_package_bytes`. A limit's marker sits in the table row whose
"Default" cell must equal the host's default in the reference's units. The format is in
[contracts/docs-format.md](contracts/docs-format.md).

**Rationale**: the drift test needs an exact, unambiguous list from the docs; parsing headings
or tables written for people would make the test fragile, and markers do not change what readers
see.

**Alternatives considered**: headings in a fixed form (constrains the writing, still ambiguous
for enum cases listed in a table); a separate index file (a second place to keep in step).

## R7. The drift test

**Decision**: `runtime/tests/docs_reference.rs`, in every `cargo test`. It builds the expected
set from: the contract `wit/v1` parsed with `wit-parser` (worlds, interfaces, functions, every
type and every case or field of enums, variants, records and flags); `wit/manifest.schema.json`
and `wit/connector.schema.json`, walked through `properties` and `items`; `ErrorKind::ALL`, a
new constant declared by one macro together with the enum and `as_str`, so a kind cannot exist
without being in it (a wildcard-free `match` would check `as_str`, not the array);
`Limits`, destructured field by field, so a new field fails to compile until the test names it;
and the commands and options of `runtime::cli::USAGE`, reachable once the CLI is a library
module (R9), except `bench`, which is for the project's own measurements (FR-020). It also fails
on a link from `docs/authors/` into `specs/` on a line that does not start with `Background:`
(FR-015, SC-007).
It reads every marker under `docs/authors/reference/` and fails on a missing item, an item that
does not exist, a limit default that differs, or a definition it cannot classify, naming each.

**Rationale**: every source of truth is already in the runtime; deriving the list from it, with
the compiler catching new error kinds and limits, leaves no hand-kept list to drift (FR-017).

**Alternatives considered**: an xtask (would need the runtime's internals anyway); checking the
docs against the WIT's comments (says nothing about manifest, limits or errors).

## R8. The docs runner

**Decision**: `runtime/tests/docs_walk.rs`, `#[ignore]`d, run by the docs job with
`--ignored`. It runs every page's marked blocks in order, one session per page, in a fresh
temporary folder: ```` ```console check ```` blocks hold commands (`$ ` on Linux and macOS, run by
bash; `PS> ` on Windows, run by PowerShell 7) and the whole output each prints, stdout and
stderr merged by the shell (`2>&1`); `os=` limits a block to systems; ```` ```<lang> file=<path>
check ```` blocks write a file. A command must exit 0 unless its block says `exit=<n>`. Output is
compared line by line, with named placeholders (`<digest>`, `<path>`, `<n>`, `<time>`,
`<version>`) each matching only its kind of value. A block may say `output=any` only when none
of its commands is `runtime`: cargo's and git's progress lines vary from run to run and are not
this project's text, so for them only the exit code is checked. Before each page, the runner
clears the development data directory and grant record through the functions the CLI uses to
find them, so pages do not share grants or stores; it refuses to start unless `CI` is set or
`DASHBOARD_DOCS_WALK_CLEARS_DEVELOPMENT_DATA=1` is, so a contributor's own development record is
never cleared by surprise. The crate the walkthrough builds is also kept in the repository,
`docs/authors/getting-started/` (`Cargo.toml`, `Cargo.lock`, `src/lib.rs`, excluded from the
root workspace like `plugins/`): the runner checks that the page's file blocks equal its files,
puts its `Cargo.lock` in the session's folder before the page and checks it unchanged after, so
the walkthrough builds with the dependencies `cargo audit` and `cargo deny` checked (R11). The format is in
[contracts/docs-format.md](contracts/docs-format.md).

**Rationale**: FR-018 and the clarification ask for whole outputs; one session per page lets a
guide build on its own earlier steps; clearing the data directory through the CLI's own lookup
works on Windows too, where the data folder comes from a system call, not an environment
variable.

**Alternatives considered**: `trycmd` or `mdbook test` (neither runs bash and PowerShell blocks
per system, nor knows our placeholders); comparing only marked lines (rejected in clarification).

## R9. Commands that reach a service: the test-only CLI

**Decision**: the CLI's code moves from `src/bin/runtime.rs` into a library module,
`runtime::cli`, with one entry point that takes how to start the runtime (today five places build
it, `runtime.rs:528`, `640`, `1428`, `1459`, `1653`) and how to open the sign-in address
(`runtime.rs:1749`); `src/bin/runtime.rs` calls it with `Runtime::new` and the system browser,
as today. A second binary, `runtime-test-network` (`required-features = ["test-network"]`, so it
never builds without the feature, and the feature refuses release builds, which CI already
checks), calls it with:

- `Runtime::with_test_network_on_keychain`: the seam, on routes and a test CA the runner writes
  to a file named by `DASHBOARD_TEST_NETWORK`, and the development vault on the system's
  keychain, because each command is a process of its own and the mock vault lives in one
  process's memory (`vault/store.rs:25`); the same file names the data directory and the grant
  record, which the binary uses instead of the default ones (`RuntimeOptions::
  default_development_data_dir`, `GrantStore::default_development_path`), so its grants and
  accounts never land in the records the real CLI reads;
- an opener that, instead of a browser, follows the test sign-in service's page. Its HTTP client
  lives in `runtime/src/network/` behind `test-network`, beside the rest of the network code, so
  Clippy's rule that only that module opens connections keeps holding with no new `#![allow]`.

The runner runs a page's `runtime` commands through this binary only when the page's front
matter comment says `service=<names>` (the connectors guide: `github`; the accounts guide:
`notes`), after starting the matching test servers from `runtime/tests/common`, and disconnects
the page's accounts after it. The command text never changes. A test in `cli.rs` runs the real
`runtime` with `DASHBOARD_TEST_NETWORK` routing a `localhost` destination to a live test server
and checks that the server gets no request and the output is the same as without the variable.
The output is production's refusal of a destination that is not public, so nothing leaves the
machine. Pages other than the walkthrough start from a local clone and the CLI this test build
made, put first on `PATH` (`contracts/docs-format.md`).

The work goes in two commits: first the code moves unchanged, reviewed as moved lines
(`git diff --color-moved` shows them identical); then the runtime and the opener become
parameters, reviewed line by line.

**Rationale**: the seam is library-only by design (003, R15; 005 kept "no test seam in the
CLI"); a test binary with the same CLI code is the only way the guides' commands run as written
against test services (FR-018). Splitting the move from the change keeps the large diff
mechanical and the small one readable (Principle IX).

**Alternatives considered**: the real GitHub in CI (rate limits per runner address, output
changes as releases are published, and CI never reaches GitHub since 003); leaving those
commands unchecked (breaks FR-018); a hidden flag in the real CLI (breaks 003's rule that no
build anyone runs contains the seam).

## R10. The accounts guide without a real account

**Decision**: the accounts guide follows `private-notes` and its development connector
`local/notes`, as 005's quickstart did; in CI the test-only CLI runs it on the development vault
in the system's keychain (as `accounts.rs` does for two processes), the test sign-in service and
the test notes service; the runner disconnects the account at the end of the page, so the
keychain keeps nothing of a run. The guide tells authors to register an app with
their own service for real use; no catalog connector has an account until Phase 6.

**Rationale**: 005 left the catalog's GitHub connector without an account; the example that
exists is `private-notes`.

**Alternatives considered**: a real provider in CI (a secret in CI, and Principle IV's spirit of
no real tokens in tests).

## R11. CI

**Decision**: a `docs` job, matrix `ubuntu-24.04`, `macos-latest`, `windows-latest`, `needs:
changes`, run when `code` is true, in parallel with `build`, and listed in `result`'s `needs`.
Steps: checkout; Rust 1.99.1 with `wasm32-unknown-unknown`; the Linux packages the runtime needs
to build and the vault needs to run (`libdbus-1-dev`, `pkg-config`, `gnome-keyring`; no Tauri
packages); the cache; `cargo xtask plugins --locked` (the guides use the examples); `cargo test
-p runtime --test docs_reference --locked`; `cargo test -p runtime --test docs_walk --locked --
--ignored`, on Linux inside `dbus-run-session` with gnome-keyring unlocked, as the build job runs
its tests. Timeout 40 minutes. The job's time on each system is measured on the pull request's
first run and recorded in `walks.md`; the 25-minute goal counts as met only then. The `changes` job counts `docs/authors/` and `wit/` as code, the
files the checks read, so a change to them alone runs CI in full (FR-019); the comment that cites
the constitution is updated. The `audit` and `deny` jobs also check the walkthrough's crate
(R8): `cargo audit --file docs/authors/getting-started/Cargo.lock` and `cargo deny
--manifest-path docs/authors/getting-started/Cargo.toml check`, as they do for `plugins/`.

**Rationale**: the clarification put the runner on its own machines so the timed hostile cases
keep theirs; running the drift test here too covers changes the build job's filter would
otherwise skip.

**Alternatives considered**: the runner inside the build job (adds minutes to every run and
shares the machine with timed cases); Linux only on pull requests (rejected in clarification).

## R12. The review of the contract

**Decision**: the first implementation task. A reviewer with fresh context (an independent
agent that did not write the contract) gets `wit/v1/widget.wit`, the runtime's sources and
tests, and the current docs, and checks every item as FR-004 lists. Each finding is reproduced
at the source (code, a test, or a small run) before it is acted on, as for the plans of 003 to
005, and goes into `review.md` with its outcome. Findings that change the host get their own
task, test and review; findings only a breaking change fixes go to the maintainer with their cost
before anything else proceeds, since choosing 2.0.0 changes what the docs describe.

**Rationale**: the docs and the freeze depend on what the review finds, so it comes first; an
outside reviewer found real conflicts in every plan of this project that its author had passed.

**Alternatives considered**: reviewing after writing the docs (rework if the contract changes);
the maintainer reviewing alone (the agent review plus the maintainer's decisions is what worked
in 003 to 005).

## R13. Freezing

**Decision**: after every finding has an outcome, copy `wit/v1/widget.wit` to
`wit/history/1.3.0/widget.wit`, add its SHA-256 to `frozen_contracts()` in `contract_v1.rs`, and
add a check that while `wit/v1` declares `dashboard:widget@1.3.0` its bytes equal the frozen
copy, line endings read as LF as `frozen_contracts` reads them, so a CRLF checkout passes. Doc comments of 1.3.0 may be corrected before this step; the only-adds tests compare
definitions, not comments, so they keep passing. `private-notes` keeps building from `wit/v1`
until a new minor.

**Rationale**: the same mechanism as 1.0.0 to 1.2.0 (FR-002); the equality check stops a
"comment fix" to `wit/v1` after the freeze from changing what authors bind without a new minor.

**Alternatives considered**: freezing first and reviewing after (spec forbids it).

## R14. `login_providers` in the manifests

**Decision**: remove it from the 16 manifests under `plugins/` that carry it. The tests that
check the obsolete field (`manifest_schema.rs`, `cli.rs`, the manifests built in
`tests/common`) keep it; one of them already checks that it grants nothing.

**Rationale**: the examples are what authors copy (FR-021); the hostile and fixture manifests
are rebuilt from their files, so their digests change but no test pins them.

**Alternatives considered**: keeping it in the hostile manifests (nothing depends on it there).

## R15. TypeScript mentions and the other pages

**Decision**: replace "TypeScript examples arrive in feature 006" in `plugins/examples/README.md`
and any other mention found by search with "TypeScript plugins arrive in Phase 5", naming no
toolchain or engine: the constitution's fixed decision still names jco and componentize-js, and
Phase 5 amends it before building anything (FR-022); `wit/README.md`, the example READMEs and `plugins/hostile/README.md`
point to the guides and the reference instead of repeating them (FR-023).

## R16. The agent's walk (SC-001)

**Decision**: after the docs pass in CI, an agent with no context of this work runs the
walkthrough in a disposable `rust:1.99.1` Docker container, with the committed tree mounted and
git's `insteadOf` sending the walkthrough's clone to it (R3), told to follow only `docs/authors/`
and to read no code or spec. The agent runs `runtime approve` on its own package in that
container's development record: no person's trust decision is made for them (Principle VII), and
the container and its record are thrown away after the walk.
It records each point where it got stuck; each is fixed in the docs and the walk is repeated
until one has none. Results go in `walks.md`.

**Rationale**: the clarification's second check; Docker is available on the maintainer's
machine, and a container gives a clean data directory and toolchain.

## R17. The maintainer's walk

**Decision**: last, the maintainer walks the walkthrough by hand on Linux, timed, and the time
goes in `walks.md` (SC-001, under 30 minutes not counting tool downloads).
