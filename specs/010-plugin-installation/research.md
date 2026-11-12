# Research: Plugin Installation and the Installation Dialog

Each decision below was checked against the code it relies on, cited as `file:line`. Paths
without a prefix are under `runtime/src/`. The plan's independent review (2026-11-12: a security
review and a fact-check, each by an agent with none of this feature's context) is folded in; the
last section maps each finding to its outcome.

## R1. Decision 0001: compile out of process now, run out of process next

**Decision**: installed packages are compiled in a separate, confined operating-system process
(R2, R13). The app loads the compiled form and, in this feature, runs it in its own process, as
it runs the bundled plugins today. A new record,
`docs/decisions/0003-compile-installed-plugins-out-of-process.md`, records the review; 0001 gains
a dated note pointing to it. Moving plugin execution out of the app's process is the next feature,
before the alpha (maintainer's decision, 2026-11-12).

**Rationale**:
- 0001's trigger is reached: "before the app can install a plugin it did not build itself"
  (`docs/decisions/0001-plugin-process-isolation.md:55-58`).
- Compiling is the only phase no limit covers: `Component::from_binary` (`lib.rs:934`) runs before
  any fuel, epoch or memory limiter (`limits.rs:22-241`) and uses every core. This feature closes
  that row.
- What changed for running is not its cost but who wrote the code: third-party code now runs in
  the process that holds the keychain service and OAuth tokens. A sandbox escape in Wasmtime would
  reach them. 0003 states that this risk is accepted only until the next feature, with 0001's
  mitigations (pinned versions, `cargo audit` on every change and weekly, minimal Wasmtime
  features); the alpha's gate does not open before it lands. The size of that work, every host
  interface (render, update, act, connectors, storage, accounts, logs) behind a process boundary,
  is why it is a feature of its own.

**Alternatives considered**: running out of process in this feature (0001's option 3): it doubles
the feature, all of it security core. Accepting in-process running without a follow-up: rejected
by the maintainer.

## R2. The compile process: limits, attribution and lifecycle

**Decision**: `runtime::compile` gains two halves.

`child_main()`, entered when the binary's only argument is `--compile-package`:

1. prints the engine's compatibility fingerprint (`Engine::precompile_compatibility_hash`) on
   standard output, so the parent can tell an app updated on disk from a compile failure;
2. builds the `Engine` from the same `engine_config()` as `Runtime::start` (`lib.rs:606-665`,
   factored out; Wasmtime refuses a compiled form whose tunables, features or version differ,
   `wasmtime-50.0.1/src/engine/serialization.rs:49-124`), and builds rayon's global pool;
3. confines itself (R13);
4. reads at most `max_package_bytes` from standard input, checks the component preamble as
   `inspect` does (`lib.rs:922-934`), so no text format is accepted even in test builds where
   `wat` is on (`runtime/Cargo.toml:145-152`), and calls `Engine::precompile_component`;
5. writes the compiled bytes to standard output and exits 0.

`Compiler::compile(package) -> Result<Vec<u8>, CompileError>` in the parent:

- starts its own executable (`/proc/self/exe` on Linux, `current_exe` elsewhere) with a cleared
  environment except `RAYON_NUM_THREADS`, `SystemRoot` on Windows, and `LD_LIBRARY_PATH` when set
  (AppImage; checked from the AppImage in the tasks);
- applies the limits before the child reads anything: on Unix in `pre_exec` (`setrlimit`,
  `PR_SET_PDEATHSIG` on Linux, `nice`); on Windows the child is assigned to its job object before
  the parent writes the first byte, and it blocks on standard input until then;
- compares the fingerprint with its own: a mismatch is `compile-failed` ("the app was updated
  while it ran; restart it"), never a retry;
- reads standard error up to 64 KiB into the app's log, never to the page;
- always waits for the child after killing it, so none is left behind.

| Limit | Value | Enforced by |
| --- | --- | --- |
| Wall-clock time | 20 s | The parent kills the child, every system |
| CPU time | 60 s | `RLIMIT_CPU` (Unix, per process; `SIGXCPU`); the job's process time limit (Windows) |
| Memory | 1.5 GiB | `RLIMIT_DATA` (Linux; counts glibc arenas, checked on kernel 7.0); the job's process memory limit, committed memory (Windows); on macOS, which enforces neither, a watchdog thread in the child samples its own physical footprint every 50 ms and exits |
| Threads | half the cores, at least 1 | `RAYON_NUM_THREADS` (rayon's global pool, `wasmtime-50.0.1/src/engine.rs:213-226`) |
| Priority | below normal | `nice(10)` (Unix); `BELOW_NORMAL_PRIORITY_CLASS` (Windows) |
| Output | 64 MiB | The parent stops reading and kills the child |
| Parent gone | — | `PR_SET_PDEATHSIG(SIGKILL)` (Linux); kill-on-job-close (Windows); the macOS watchdog also exits when `getppid()` changes |

**Attribution**: a limit refuses the installation naming it. The child's global allocator wraps
the system one and `_exit`s with code 86 when an allocation fails; a spawn failure in rayon is
caught in the child and exits 86 too. The parent maps: wall-clock kill or `SIGXCPU` or the job's
end-of-time message → `compile-time-limit`; exit 86, the macOS watchdog's exit, or the job's peak
memory at its limit → `compile-memory-limit`; anything else → `compile-failed`.

**Numbers**: the legitimate near-cap package peaks at 593 MB resident
(`specs/001-plugin-runtime/performance.md:48,59`) and 592 MB of `VmData` (the review's repro,
Wasmtime 50.0.1); 1.5 GiB keeps it under half the limit, the plan's own rule. Its compile took
0.22 s with 12 threads and 1.46 s with one; 20 s keeps a wide margin even on two-vCPU runners at
low priority. A CI test compiles the `compile-heavy` fixture (R11) through the real child under
the default limits on the three systems and prints its time and peak, which the tasks record.

`libc` is already a direct Unix dependency of the runtime (`runtime/Cargo.toml:156-157`);
`windows-sys` 0.61, already in the lock through Wasmtime, becomes a direct Windows dependency.

## R3. Trusting a compiled form (FR-008)

**Decision**: the compiled bytes are stored as `compiled.cwasm` beside the package, and the
installed record keeps their SHA-256. At load the host reads the file into memory, checks that
digest on those bytes, and passes the same bytes to `unsafe { Component::deserialize(...) }`
(never `deserialize_file`, which would read the file again after the check). The package's digest
is checked too: a mismatch is `package-changed`, never recompiled. A compiled form whose digest
does not match, or that Wasmtime refuses as built by another engine (an app update), is discarded
and compiled again through R2, once.

New runtime entry: `Runtime::load_compiled(package, compiled, origin, grants)`, which runs the
same `inspect` steps as `load` (`lib.rs:910-958`) with the deserialized component in place of
`from_binary`, so the manifest, contract and permission checks stay one code path.

**Rationale**: `deserialize` runs machine code as given. Only bytes the host produced, in its
confined child, and whose digest it recorded may reach it. The compiler's output is still trusted
as code, as it was when the app compiled in its own process; that residual is a threat-model row
(R12). A process of the same user that rewrites both file and record is outside the threat model
(`specs/005-plugin-credentials/spec.md:653-654`).

## R4. Talking to GitHub (FR-001, FR-003, FR-004)

**Decision**: a new public module `runtime::install::github`, built on a new host GET in
`network/` (the only place `ureq` may be used, `clippy.toml:16-60`), with the existing TLS setup
(rustls, Mozilla roots, `network/mod.rs:107-136`) and the public-only resolver. The redirect rules
are the installer's own: at most 3 hops, HTTPS only, to the two asset hosts below.

| Step | Request | Checked |
| --- | --- | --- |
| Identity | `GET https://api.github.com/repos/{owner}/{repo}` | `200`; `full_name`, lowercased, equals the typed id; a `301` (renamed, `Location: /repositories/{id}`) is refused naming that the plugin moved; keep `id` |
| Release | `GET /repositories/{id}/releases/latest` | By permanent identifier, so a name taken over between calls cannot serve another repository's release. Latest non-draft, non-prerelease; `tag_name` is `v<version>` or `<version>`; exactly one asset ending in `.wasm`, its `size` within the package cap |
| Package | `GET /repositories/{id}/releases/assets/{asset_id}`, `Accept: application/octet-stream` | Redirect to `release-assets.githubusercontent.com` or `objects.githubusercontent.com` only; body capped at the package cap; the asset's `digest` (`sha256:…`) must equal the computed one when present |
| Update | `GET /repositories/{id}` | `404` means deleted; a `full_name` other than the installed id means renamed; both refuse the update, naming why. Then the release step as above |

`/repositories/{id}/…` paths are served (checked live: `200`) but not documented. If GitHub stops
serving them, the installer falls back to `/repos/{owner}/{repo}/…` and checks, after the
download, that `/repositories/{id}` still names the same repository; a test covers both paths.

Then the manifest id must equal the repository id (`identity-mismatch`), the manifest version must
equal the tag's (`tag-mismatch`), and an update's version must be greater by Semantic Versioning
(`not-newer`; `semver` is a dependency, `runtime/Cargo.toml:16`).

Requests carry `Accept: application/vnd.github+json`, `X-GitHub-Api-Version: 2022-11-28`, the
`dashboard/<version>` user agent, and no credential. Each API answer is capped at 256 KiB and 10 s;
the package download has 60 s (10 MiB at about 1.5 Mbit/s). A `403` or `429` with
`x-ratelimit-remaining: 0`, or a `retry-after`, is `rate-limited`, naming when to try again;
anything else is `source-unreachable`. Costs: 3 requests per installation, 2 per update check, 3
when it finds a newer release; the unauthenticated limit is 60 an hour per address.

A `ReleaseSource` trait sits between the installer and GitHub; R16 says where the test sources
live.

## R5. Where installed plugins live (FR-006)

**Decision**: under the app's host directory, `<app data dir>/host/` (`app_options`,
`app/src-tauri/src/widgets.rs:62-63`):

```text
host/
├── grants.json, grants.json.lock     # the app's grant record, now on disk (GrantStore::open)
├── installed.json, installed.json.lock
└── packages/<package digest hex>/
    ├── package.wasm
    └── compiled.cwasm
```

Installing writes in this order, each with the existing atomic write and lock
(`records.rs:14-46`): the package and compiled form (in a temporary directory, renamed into place),
then the grant, then the installed record. At start-up, only when both records were read without
error, the host removes package directories no installed entry names, installed entries with no
grant, and `github:` grants with no installed entry. If either record cannot be read, nothing is
removed and the installed widgets show the error.

The installation grant is built fresh: the required items and the optionals turned on in the
dialog, never optionals kept from an earlier entry of the same origin, which `approve_connected`
does today (`grants.rs:696-707`); the app calls a new `GrantStore::install` that starts from an
empty entry. Updates keep feature 002's rules (`grants.rs:582-638`).

The grant's origin is `github:<permanent identifier>` (origin is opaque text, `grants.rs:58-60`),
so a package claiming the id from another repository is `not-granted` (`permissions.rs:541-547`).
The bundled showcase keeps `bundled:<file>`, approved at every start as before
(`widgets.rs:364-434`). The app never reads the development record, nor the tool the app's.

**Alternatives considered**: one directory per id: an update would overwrite the running version's
files before approval. Content addressing keeps both until the new one is approved.

## R6. The dialog's model is built by the host (FR-010 to FR-014)

**Decision**: the runtime builds a typed `DialogModel` from a `CheckedPackage` and the current
grant entry; the page only draws it:

- who is asking: the name and the author, both through `drawn_name` (`app/src-tauri/src/name.rs`;
  today `check_author` refuses only control characters, `manifest.rs:534-543`, so bidirectional
  controls pass), the author labelled "declarado pelo plugin"; the id in full; the version and, for
  an update, the installed one;
- every permission, as a host sentence (below) with its parameters, required or optional, and for
  an update `added`, `changed` or `unchanged`; an update lists everything the new version holds,
  not only the changes (Principle III);
- every connector as the CLI shows it today (`cli/mod.rs:1013-1193`), with the same change marks;
- the warning (R7) and the two digests (R7).

The CLI's `describe` prints the same model, so they cannot disagree; a test compares them for
every fixture.

**Plain language (FR-011)**: one Rust function with an exhaustive `match` on the permission enum
that `permissions.rs:132-166` gains in place of its string registry; a permission without a
sentence does not compile. Portuguese, as the app's strings (`app/src/widgets/Frame.tsx:28-36`);
the CLI keeps English names.

| Permission | Sentence |
| --- | --- |
| `log` | "Escrever mensagens no registro do app, que só você vê." |
| `storage` | "Guardar dados neste computador, até {quota}." |
| connector (read) | "Consultar {service}: {operations}." |
| connector (change) | "Alterar dados em {service}: {operations}." (shown; callable only once a later feature brings user-confirmed changes, `change-needs-user-action`) |
| account | "Ler os dados da sua conta em {service}, com o acesso: {access}. Nenhuma conta está conectada no app ainda." |

## R7. The dangerous combination rule and what an approval covers (FR-014, FR-016 to FR-018)

**Decision**: `runtime::flows::judge(entry_after, remembered) -> Vec<Flow>`, a flow being
`{ source, destinations }`:

- a **source** is a connector granted with an account (`grants.rs:98-107`), or a source in
  `remembered`;
- **remembered** sources are kept in the grant entry (`account_sources`, data-model): every
  account connector the plugin held while it also held `storage`. They stay until its store is
  cleared, which nothing in this feature does (uninstalling is the guide's later item), so data
  kept from an account cannot leave through a destination a later version adds. The host's kept
  data, the update cache, is discarded whenever a source goes away, as the CLI already does when a
  connector is turned off (`cli/mod.rs:1531-1546`);
- **destinations** are every destination host of every other granted connector, plus a source's
  own hosts outside its credential's origins;
- a local-network source (none can be granted in the app, R9) is judged the same way.

`judge` runs on the grants the plugin would hold after the change: at installation on the required
items plus the optionals turned on (`request_judge`, on every toggle); at an update, on the new
version's; in the development tool's `optional … on` too (`cli/mod.rs:1525-1550` runs no check
today), whose output prints the flows. The CLI's `data flow:` lines (`cli/mod.rs:1214-1248`,
`:1329-1352`) use `judge`, so both share one rule.

**What an approval covers**: besides the approval digest (`grants.rs:270-325`: package,
permissions, connectors, storage), every model carries a **choice digest**: the SHA-256 of the
canonical `{approval digest, optional_on sorted, warning}`. `install_start`, `update_start` and
`request_judge` return it; `request_approve` must echo it, and the host recomputes it from its own
state: any difference is `approval-changed` and nothing is granted. So the person approves exactly
the optionals and the warning they saw.

**Rationale**: the store and the widget's memory join everything a widget reads, so any account
source plus any other destination is a possible flow; the rule does not try to prove a flow,
which no declaration can.

## R8. The window's host dialog (FR-021 to FR-023, HS-10 to HS-23)

**Decision**: one `HostDialog` component in `app/src/host/`, the only place a host dialog is drawn:

- built on the Radix dialog (`app/src/components/ui/dialog.tsx`), which gives the portal on
  `body`, a sibling of `#root` (`app/index.html:14`), the focus trap, `aria-hidden` outside,
  `onEscapeKeyDown` and preventable outside dismissal, with the style nonce feature 008 set up
  (`main.tsx:7-12`);
- the content in the host-only `dialog` roles and elevation (`DESIGN.md:82-87`, `:545-548`), not
  `popover` as the demo wrapper does today;
- while open, `#root` gets `inert` (HS-11), and outside pointer events are prevented, not taken as
  a dismiss; `inert` is removed in the same step that closes it;
- **focus return (HS-20)**: Radix returns focus only to a `Dialog.Trigger`
  (`@radix-ui/react-dialog` 1.3.1, `dist/index.mjs:174-177`, per the fact-check), and these dialogs
  open after an asynchronous call, without one. `HostDialog` records the opener (the focused
  element when the request started) and its own `onCloseAutoFocus` focuses it, or the window when
  it is gone;
- **keys**: Enter and Escape are handled by the dialog's own `keydown` handler, which does not
  filter on `isTrusted`: Escape and Enter both take the safe choice unless focus is on another
  control, where Enter activates that control (HS-14, HS-19). The safe choice is first in tab
  order and focused on open (HS-18);
- `aria-labelledby` the title and `aria-describedby` the who-is-asking line and the decision
  (HS-21); a fixed header and footer, only the body scrolling (HS-15); a `forced-colors:` border in
  `CanvasText` (HS-17); animation only under `motion-safe:` (HS-23);
- **input delay (HS-12)**: pointer and key events on the dialog are swallowed for **1000 ms** after
  it opens, Firefox's default for its security dialogs (`security.dialog_enable_delay`). The delay
  starts again when the window regains focus, and when input aimed at the dialog arrives during it
  (FR-022); the approve button stays disabled while it runs, so the wait shows.

Closing the window or quitting with a dialog open takes the safe choice: the window's close event
sends `request_cancel`, and the host drops pending requests and kills a running compile child at
exit (FR-023). One request at a time: a second `install_start` while one is pending or running is
`request-pending`.

## R9. Connectors at installation (FR-012, FR-015, FR-024)

**Decision**: installation runs the runtime's resolution with development mode off
(`lib.rs:1062-1160`): catalog connectors resolve; `local/` ones are refused as
`destination-not-verified` (`lib.rs:1073-1083`), and repository ones too (`lib.rs:1084-1091`),
whose message the installer replaces by one naming the later item. A refused connector refuses the
installation even when optional: `inspect` keeps refused connectors apart (`lib.rs:910-958`) and
`check` refuses them (`lib.rs:899-905`).

An account connector installs; with no account in the app's vault its calls answer
`account-not-connected` (`calls.rs:756-765`), and the dialog says so (R6).

**Found while implementing (T006, 2026-11-12)**: the built-in catalog holds only
`catalog/github`, which declares no credential (`connectors/catalog/github.json`). So no widget
installed in the app can use an account until a catalog connector with a credential lands, and in
the app the dangerous combination warning has no source to find. The rule, the dialog's account
line and FR-024 are still built and tested, with a connector the tests' catalog holds
(`catalog/account-echo`, `Catalog::with_extra` under `test-network`); the `install-account`
fixture is refused as `connector-unavailable` outside the tests.

## R10. The window's new commands (FR-025)

**Decision**: five commands, each added to `window-surface.toml` with its reason, granted in
`capabilities/default.json` and registered in `commands.rs` (feature 008's three steps); see
[contracts/window-commands.md](contracts/window-commands.md). `widgets_list` returns installed
widgets too, with their owner (R14); `Widgets.shown` becomes a lock-guarded list so a widget can be
added after start (`widgets.rs:135-138`). The page refreshes the list after an approval. The
probe's `foreign-actions-refused` case gains the five commands: widget content cannot reach them.

## R11. Breaking tests (FR-026, Principle VIII)

**Below the page** (runtime and app crate tests, in the `it` executable, `runtime/tests/it.rs`):

| Case | Rule | Expected |
| --- | --- | --- |
| `install-from-file`, `bad-address` | FR-001 | No command or path takes a file; `file:`, URLs, other hosts refused |
| `install-not-from-chrome` | FR-002 | The probe's `foreign-actions-refused` case: widget content cannot call the five commands |
| `identity-mismatch`, `tag-mismatch` | FR-003 | Refused, nothing kept |
| `changed-release` | FR-004 | Same tag, other bytes: refused |
| `repo-jacked` | FR-004 | Name now served by another permanent id (deleted and recreated): refused |
| `name-swapped-mid-install` | FR-004 | The name's id changes between the identity and release steps: the release is still the recorded id's |
| `renamed-at-install`, `renamed-at-update` | FR-004 | 301 at installation, `full_name` change at update: refused, naming it |
| `no-asset`, `two-assets`, `asset-digest-mismatch` | FR-004, FR-005 | Refused |
| `oversized-asset` | FR-005 | Declared size, or a body that grows past the cap: refused before compiling |
| `crash-mid-install` | FR-006 | Kill after each write step: the next start keeps nothing usable; an unreadable record removes nothing |
| `cancel-keeps-nothing` | FR-006 | After `request_cancel`: no package, compiled form, grant or entry; the temporary directory gone |
| `window-closed-mid-dialog`, `quit-mid-compile` | FR-023 | The window's close drops the request (its token then `unknown-request`, nothing granted); stopping the host mid-compile kills the child and keeps nothing |
| `connector-major-unavailable` | spec edge case | A catalog major the host does not ship: `connector-unavailable` before any dialog |
| `account-not-connected-in-app` | FR-024 | An account connector installs; its call answers `account-not-connected`; nothing of the development record or vault is used |
| `compile-slow`, `compile-cpu`, `compile-memory`, `compile-output` | FR-007 | The `compile-heavy` fixture against lowered limits, one per limit: refused naming that limit; the test process's own memory unchanged; no child left |
| `compile-confined` | FR-029 | A child built with a test hook that, after confining itself, tries to open a file and a socket: both fail, on the three systems |
| `compiled-tampered`, `package-tampered` | FR-008 | Recompiled once; refused as `package-changed` |
| `stale-optional` | FR-013, FR-014 | An orphan grant with an optional on: reinstalling grants only what the dialog showed |
| `approval-changed`, `choice-changed` | FR-014 | Stale approval digest, or a choice digest for another optional set: refused |
| `repository-connector`, `local-connector` | FR-015 | Refused, even optional |
| `ask-little-then-more` | FR-016, FR-017 | Every subset of a fixture's optionals: the warning exactly when the set holds a flow |
| `account-then-elsewhere` | FR-016 | v1 account + storage, v2 drops the account and adds a destination: warning |
| `cli-optional-flow` | FR-016 | The development tool's `optional on` prints the flow it completes |
| `no-false-warning` | FR-018 | No flow, no warning |
| `flow-not-hidden` | FR-018 | Another case or a trailing dot on the other destination, or the account connector required beside an optional destination: the flow is still found |
| `downgrade` | FR-019 | Equal or lower version: refused |
| `update-lists-everything` | FR-020 | The update model holds every destination, with change marks |
| `one-request`, `token-replayed`, `optional-bounds` | contract | A second start is `request-pending`; an approved token is `unknown-request`; out-of-bound choices refused |
| `plain-language` | FR-011 | Compile-time exhaustiveness, plus every permission and connector property of every fixture present in the model |
| `redirect-elsewhere` | R4 | An asset redirect to another host: refused |

The `compile-heavy` fixture is a real widget (manifest id `github.com/dashboard-fixtures/compile-heavy`,
a `renderer` export) padded with generated functions to just under the package cap. The generator
moves from `bench/src/bin/compile_cost.rs` into a function `cargo xtask plugins` calls; the bench
keeps using it.

**In the page** (the shell probe, against the probe release source, with the showcase and a hostile
widget on screen). Synthetic events are untrusted: the browser gives them no default action and
does not hit-test `.click()`. So each case tests the host's own code, and says where a criterion
needs more:

| Criterion | Probe case | What only the review step covers |
| --- | --- | --- |
| HS-10 | `elementFromPoint` over the chrome and each frame returns the dialog or its scrim | — |
| HS-11 | `#root` is `inert`; hit tests as HS-10; nothing outside the dialog is focusable | The accessibility tree (§6), and on Windows the probe's `Accessibility.getFullAXTree` through WebView2's DevTools protocol where the probe build can call it |
| HS-12 | The dialog's handler drops synthetic Enter and pointer events before 1000 ms and takes them after; the delay restarts on a window `focus` event | A real click and key press (§6) |
| HS-13 | The model's hostile names and authors are drawn as text, bounded; the id in full | — |
| HS-14 | Synthetic Enter on open, after the delay, takes the safe choice through the dialog's handler | — |
| HS-15 | In a small window, the body scrolls and the header and safe choice stay in view | — |
| HS-17 | The stylesheet holds the `forced-colors` border rule (read through the CSSOM); WebKit never matches `forced-colors` on macOS and Linux | Windows high contrast (§6) |
| HS-18 | Focus starts on the safe choice; synthetic Tab and Shift+Tab at both ends wrap (Radix's handler) | Tab through every control (§6) |
| HS-19 | Synthetic Escape from each control takes the safe choice | — |
| HS-20 | After close, focus is on the opener; with the opener removed, on the window | Screen reader (§6) |
| HS-21 | Role, `aria-labelledby` and `aria-describedby` resolve to the title and description | The announcement (§6) |
| HS-22 | Everything outside the portal is `aria-hidden` or `inert` | The screen reader's lists (§6) |
| HS-23 | Every animation rule on the dialog is inside `prefers-reduced-motion: no-preference` (CSSOM) | Reduced motion on (§6) |

Plus `frame-owner-shown` (FR-030, HS-02 as changed), the extended `foreign-actions-refused`,
`install-on-time` (SC-001 without the network: under 10 s from `install_start` to first render on
the probe source) and `window-responsive-while-compiling` (SC-003: no gap above 100 ms between
animation frames or 10 ms timers while `compile-heavy` compiles). On Windows, the HS-11 and HS-22
cases also read `Accessibility.getFullAXTree` through `CallDevToolsProtocolMethod` if the probe
build can call it with developer tools off; the first Windows CI run settles it (tasks T045).

## R12. The threat model, decision records and author docs (FR-027, FR-028)

**Decision**:
- Rows `docs/threat-model.md:113`, `:152`, `:153`, `:155`, `:157`, `:158` and `:175` name their
  tests from R11; `:152` and `:153` move from "planned" to covered; `:158`'s residual changes to
  "a name trusted at first installation", with the owner now in every installed frame (R14).
- B6 (`:90`) and the diagram line (`:51`) state what this feature provides (pinned release,
  permanent identifier, digests, confined compile) and what waits for Phase 6 (signatures,
  revocation, a signed updater).
- New rows, each with its residual: a name trusted at first installation (typo-squatting, a
  look-alike owner); an author account taken over that ships an update asking for nothing new
  (confirmed in one step); no revocation yet; the compile process's output trusted as code; the
  host's own requests to GitHub (what they reveal, what a hostile answer can do).
- The open questions at `:192-194` and `:205-213` are closed with links to 0003 and this feature.
- `docs/decisions/0003-…md` and a dated note in 0001.
- A new author guide, `docs/authors/guides/publishing.md`, from
  [contracts/release.md](contracts/release.md); the manifest reference's `id` row says it is now
  checked against the repository.

## R13. Confining the compile process (FR-029)

**Decision**: the child confines itself after building its engine and thread pool (R2 step 3), so
nothing it does later can open a file, a socket or reach another process. On Linux, Landlock comes
before the pool, since it binds only the calling thread and those it starts afterwards, and the
seccomp filters after, on every thread (the implementation's review, T053):

| System | Mechanism |
| --- | --- |
| Linux | `PR_SET_NO_NEW_PRIVS`; a Landlock ruleset that handles every file-system access right and grants none (when the kernel supports it); a seccomp filter, applied to every thread (`SECCOMP_FILTER_FLAG_TSYNC`), that allows only `read` on fd 0, `write` on fds 1 and 2, memory calls (`mmap`, `munmap`, `mprotect`, `madvise`, `mremap`, `brk`), `futex`, thread exit and signals, `sched_yield`, `getrandom`, `clock_gettime`, and `exit_group`, and kills the process on anything else, stacked on a second filter that answers `open`, `openat` and `openat2` with `EACCES` (glibc opens files of its own, and handles the refusal). Crates `seccompiler` (rust-vmm) and `landlock`, under `cargo deny` |
| macOS | `sandbox_init` with the system's pure-computation profile (`kSBXProfilePureComputation`): no file, network or Mach services, the keychain's included; the watchdog thread runs inside it |
| Windows | The child starts in an AppContainer with no capabilities (no network, no user files, no keychain) and in a job object that also forbids child processes and desktop access. The app grants the AppContainer read and execute on its executable file alone, not inheritable, when it runs from a per-user location; its folder's other files stay out of reach (T053). If AppContainer cannot start the binary on a supported Windows version, the fallback is a restricted, low-integrity token, and the residual (reading the user's files) is recorded |

The breaking test `compile-confined` (R11) runs on the three systems in CI: one run per attempt
(a file from the main thread, a file from a compile thread that existed before confinement, a
loopback port), each ending as soon as it worked, so no later refusal can hide one. The new crates and
the per-system code are security core.

**Alternatives considered**: no confinement, limits only: a bug in Cranelift's handling of input
would run with the user's rights, beside their files and keychain (the review's finding, the
maintainer chose confinement).

## R14. The owner in every installed frame (FR-030)

**Decision**: `widgets_list`'s `FrameInfo` gains `source: "github.com/<owner>"` for installed
widgets, absent for bundled ones; `Frame.tsx` draws it after the name, smaller, as text, in the
frame header's host-only roles. Criterion HS-02 in `docs/design/host-surfaces.md` changes first
to say so, as feature 007's rule allows, and `pnpm --filter ./design check` keeps its cited
checks. The id beside a shared name (feature 009) stays.

## R15. Constitution v2.7.0

**Decision** (maintainer, 2026-11-12): MINOR amendment. Principle VII keeps the development tool
granting to local packages, in its own record, with no end date, since authors need it after the
app has its dialogs; Principle IX lists the plugin installer with its compile process and the host
dialogs in the security core. Committed apart, before this plan, with its rationale.

## R16. Where the test release sources live

**Decision**: the in-memory `ReleaseSource` used by the runtime's tests is `#[cfg(test)]` or behind
the runtime's existing `test-network` feature, which CI already proves never builds in release
(`ci.yml:195-206`). The probe's release source, with the hostile releases, exists only under the
app's `shell-probe` feature, like the probe's hostile widgets (`build.rs:27-35`) and commands, and
answers only when `DASHBOARD_PROBE=1` (`probe.rs:179-183`). The test that checks which packages a
build embeds (feature 009, research R1) also checks that the release source of a build without
`shell-probe` is GitHub's alone.

## Review outcomes (2026-11-12)

Security review (S) and fact-check (C); each finding was checked at its source.

| Finding | Outcome |
| --- | --- |
| S1 account data kept in the store, then a new destination | R7: remembered sources; spec FR-016; test `account-then-elsewhere` |
| S2 the compile child runs with the user's rights | R13 (maintainer: confine on the three systems), spec FR-029; residual row (R12) |
| S3 an orphan grant keeps optionals on | R5: fresh installation entry, orphan grants removed; test `stale-optional` |
| S4 the approval does not cover the optionals or the warning | R7: choice digest; spec FR-014 |
| S5 release and asset fetched by name | R4: by permanent identifier; test `name-swapped-mid-install` |
| S6, C4 synthetic events do not test what HS criteria state | R8 explicit key handlers; R11 table says what each case shows and what the review covers |
| S7 Principle VII's end date; the CLI's optional runs no check | R15 (v2.7.0); R7: `judge` in the CLI |
| S8 threat model stale on B6; missing rows; the frame shows no source | R12, R14 (maintainer: owner always), spec FR-027, FR-030 |
| S9 0001's review rests on call cost | R1 rewritten (maintainer: running moves out in the next feature) |
| S10 an update dialog showing only the change | Spec US3, FR-020; R6; test `update-lists-everything` |
| S11 the delay does not restart | R8, spec FR-022 |
| S12, C2, C7, C8, C9 compile process lifecycle and attribution | R2 |
| S13 missing tests | R11 |
| S14 security files not named for review | Plan, Constitution Check IX |
| S15 author not sanitised | R6, spec FR-010 |
| S16 cleanup on an unreadable record | R5 |
| C1 1 GiB within half of a legitimate peak | R2: 1.5 GiB, measured as each system counts |
| C3 Radix returns focus only to a trigger | R8 |
| C5 the bench fixture cannot be installed; quickstart flags | R11 `compile-heavy`; quickstart §5 |
| C6 "not linked into release" cannot be tested that way | R16 |
| C10 `precompile_component` accepts text with `wat` | R2 step 4 |
| C11 timing with fewer threads | R2 numbers |
| C12 download time and JSON caps | R4 |
| C13 undocumented `/repositories/{id}` paths; request count | R4 |
| C14 orphan grants | R5 |
| C15 the page cannot cancel during fetch or compile | Data model, contract |
| Citations (`widgets.rs:25-26`, 005 `:651-652`, `lib.rs:899-905`, `libc`) | Corrected |

## The implementation's independent review (T053, 2026-11-12)

A reviewer who had not written the code read the security core of the branch. Each finding was
checked at its source, fixed with a test that breaks without the fix, and the test was seen to
break:

| Finding | Fix | Breaking test |
| --- | --- | --- |
| An update that is only confirmed judged the warning without the optional connectors it keeps on | The installer passes the kept-on connectors to the model (`ModelInput::kept_on`) | `install::a_confirmed_update_warns_with_the_optionals_it_keeps_on` |
| The compile process's error stream, which a package can steer, reached the page, and its text decided whether a stop was the memory limit | The page gets the kind and the app's sentence; the detail goes to the app's error stream, escaped. The memory limit is read from how the child ended (its own code 86, or an abort, the only one left once its panic hook exits with 70), never from its words | `compile::tests::only_how_the_child_ended_says_it_ran_out_of_memory`, `install::tests::the_page_gets_the_kind_and_the_apps_sentence_never_the_detail` |
| Paths of the person's files in error details reached the page | As above: no detail reaches the page | As above |
| `compile-confined` could pass with the file confinement off: the file opened, then a later call was killed before the test saw it | One run per attempt, each ending as soon as it worked, with the system's own `open` | `compile::the_compile_process_can_open_no_file_and_reach_no_network`, broken by turning off Landlock and the `EACCES` filter |
| Landlock was applied after the compile threads existed, and binds only the calling thread | Landlock before the pool, seccomp after | The same test's attempt from a compile thread, broken by moving Landlock back |
| A fresh compiled form is deserialized before the person decides, and the comment on it said otherwise | The comment says what happens; the threat model and decision 0003 record the residual until the next feature | None: a residual |
| The start-up cleanup of a second instance could take a package written a moment before its record for a crash's leftover | An exclusive `install.lock` held from an installation's first write to its last, and by the cleanup | `install::the_cleanup_waits_for_an_installation_in_progress` |
| On Windows the AppContainer was granted read and execute on the executable's folder, inherited by everything in it | The executable file alone, not inheritable | CI's Windows run of `compile-confined` (the child must still start) |

