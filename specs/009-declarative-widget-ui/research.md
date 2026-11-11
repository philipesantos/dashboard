# Research: Declarative Widget UI and the Host Frame

Each decision below was checked against the code it relies on, cited as `file:line`. Paths
without a prefix are under `runtime/src/`.

## R1. How the app runs the bundled plugins

**Decision**: the app crate (`app/src-tauri`) owns one `runtime::Runtime`, built at start-up with
`RuntimeOptions::new(<app data dir>/host)` and development mode off. It loads the bundled
packages, embedded in the binary at build time. A host-owned
`Widgets` state holds each loaded `Plugin` behind its own lock, so calls to one widget never run
at the same time, and calls to different widgets can.

- `Runtime::load(bytes, origin, grants)` (`lib.rs:948-987`) checks, links and returns a
  `Plugin`. `render` and `update` are synchronous and blocking (`lib.rs:1380`, `:1435`; Wasmtime
  is built without async, `lib.rs:598-601`), so the app calls them on Tauri's blocking pool,
  never on the main thread (FR-017).
- Load compiles the component: 22 to 68 ms; render takes 49 to 255 µs
  (`specs/001-plugin-runtime/performance.md:26-31`). SC-001's 1 second holds with room.
- The origin of a bundled package is `bundled:<file name>`, never a path, so no file outside the
  binary is read.
- The packages come from `target/plugins/` (`cargo xtask plugins`, `xtask/src/main.rs:85-147`).
  `app/src-tauri/build.rs` fails with "run `cargo xtask plugins` first" when one is missing, and
  prints a `rerun-if-changed` line for each, since its explicit lines turn off Cargo's default
  rerun (`build.rs:21-22`). A digest computed from the very file it embeds would protect nothing,
  so there is no pinned digest; instead a test checks **which** packages a build embeds: exactly
  `showcase` without `shell-probe`, and the hostile `ui-*` fixtures only with it.
- **CI order**: `cargo xtask plugins --locked` moves above the probe build (today `ci.yml:157`,
  before it at `:179`), so the app can be built at all; `CONTRIBUTING.md`'s commands and the
  quickstart list it first too.
- **Build cost**: the app crate now links Wasmtime and Cranelift, built under the release profile's
  `lto = true` and `codegen-units = 1` (`Cargo.toml:18-22`). The probe's release build time and peak memory are
  measured locally under the memory cap before the first push, and the 40-minute job limit is
  checked against the Windows run.
  **Measured** on 2026-11-11 (Linux, 8 threads, under `MemoryMax=8G`, from an empty
  `target/release`, beside another build): `cargo build -p dashboard --release --features
  shell-probe,tauri/custom-protocol` took 8 min 53 s of wall time (1,323 s user, 281% CPU), and
  its largest process peaked at 2.04 GiB resident, well inside the cap. The binary grew from 6.3
  to 21.6 MiB. The 40-minute limit stays: the probe build is a fraction of it, and the Windows
  run of the first push confirms it.
- **macOS signing, later**: Cranelift writes executable memory, so a signed, hardened macOS build
  will need the `com.apple.security.cs.allow-jit` entitlement; the surface test reads
  entitlements, and must allow that one when the updater feature signs builds (Phase 6).

**Alternatives considered**: loading packages from a folder next to the app. Rejected: that is
installation, with its own dialogs and decision 0001's trigger.

## R2. The bundled plugins' approval, without a dialog

**Decision** (maintainer, 2026-11-10; constitution v2.6.3, Principle VII): the app approves each
bundled package **in memory**, at start-up, only when its manifest declares **no permission at
all**: no `permissions`, no `connectors`, and so no `storage`. `GrantStore::approve` grants every
required permission it is given (`grants.rs:586-633`), so this check is the only gate; a test
refuses a bundled package declaring each kind. The
approval lives in a `GrantStore::in_memory()` (`grants.rs:449`) and is never written to disk.

- `load` refuses any plugin without an approval entry, even with no permission
  (`permissions.rs:476-483`: "`{id}` has no approval; approve it first"). The entry identifies the
  package's origin; with no permission it grants nothing.
- Principle VII (v2.6.3): "A plugin built into the app and declaring no permission is part of the
  app, not an installed plugin: the host MAY approve it at start-up, and the approval grants it
  nothing." The amendment followed the plan's independent review, which held that a reading
  recorded only in a plan would be the case-by-case exception governance forbids.

**Alternatives considered**: deferring the real widget until the installation dialog. Declined.

## R3. The app's records stay apart from development (FR-024)

**Decision**:
- **Data**: `<app data dir>/host/` holds kept data (`kept.rs:4-5`), stores
  (`storage/mod.rs:35-40`) and connectors. The development tool uses
  `<data dir>/dev.philsantos.dashboard/development` (`lib.rs:198`), a different path.
- **Grants**: the in-memory record of R2. The app never opens
  `development-grants.json` (`grants.rs:466`).
- **Mode**: `RuntimeOptions::development` stays off (`lib.rs:164-172`), so `local/…` connectors
  are refused with `destination-not-verified` (`lib.rs:1011-1020`).
- **Vault**: the vault's keychain service is the constant `DEVELOPMENT_SERVICE`
  (`vault/mod.rs:26-28`), whose comment says installed plugins get their own from Phase 3.
  `RuntimeOptions` gains `vault_service(name)`; the app passes `dev.philsantos.dashboard.app`.
  The bundled plugins use no account, so the app's vault stays empty in this feature.

`Vault.service` is `&'static str` (`vault/mod.rs:168`), so the option takes `&'static str`.

Tests: one in the app crate builds the app's `RuntimeOptions` and fails if any path or the
service equals the development one, or if development mode is on. Since the runtime's CLI module
is public (`lib.rs:19-20`) and `default_development_path()` and `default_development_data_dir()`
are callable from the app, that test alone could not fail on a stray call; so the runtime's CLI
moves behind a `cli` feature that the `runtime` binary enables and the app does not, and a second
test scans `app/src-tauri/src` for the development functions' names.

## R4. The widget content format and where it is checked

**Decision**: a new workspace crate, `widget-content/`, parses and checks widget content. It
depends only on `serde` and `serde_json`. The app uses it on every render; the runtime's CLI uses
it in `runtime run` and the new `runtime act`, so authors see content problems before they run
the app (US2). Its schema is published as `wit/content.schema.json`, beside the manifest's.

- `render` returns parsed JSON already (`RenderReport.result: Result<Value, _>`,
  `lib.rs:120-131`), capped at 64 KiB before parsing (`max_output_bytes`, `limits.rs`).
- The crate returns either `Content`, a normalised tree with only known fields and tones, plus
  the host's log entries for what it dropped, or a `ContentError` with a short reason.
- The page receives only the normalised tree, never the plugin's JSON.

**Format** (detail in [contracts/content-format.md](contracts/content-format.md)):
`{"format": 1, "body": [<component>…]}`. Components: `text`, `list`, `table`, `chart`, `button`,
`form`, and `stack` for grouping (a vertical group, so a widget can lay out sections without
nesting rules in each component). Styling only by `tone` and `variant` names from the widget map.

**Strictness**: the schema refuses unknown fields, so an author's editor catches a typo; the host
is lenient where HS-09 requires it: a style field or other unknown field is dropped and logged
once, an unknown tone draws as `default` and is logged, an unknown component type draws as a host
placeholder and is logged. Anything else the schema refuses (wrong types, missing fields, over a
bound) is invalid content. An agreement test pins both sides: every schema-valid document is
accepted with no log entry; every document the host refuses is schema-invalid, **except the
host-only rules a schema cannot express**, listed apart as the manifest test does
(`manifest_schema.rs:319`): the 500-component total, the 64 KiB size, rows as long as the
columns, and as many chart labels as values; the documents that differ only by unknown fields or
tones are listed as the lenient cases.

**Alternatives considered**: validating with a JSON Schema library at run time. Rejected: `boon`
(`runtime/Cargo.toml:116`) is a test dependency, and a typed parser gives the normalised tree and
the lenient cases in one place, as the manifest parser does (`manifest.rs:203-222`).

## R5. Bounds on widget content (FR-004)

**Decision**: published in the schema and in `docs/authors/reference/limits.md`, defined as
constants in `widget-content`:

| Bound | Value |
| --- | --- |
| Document size | 64 KiB (the contract's, checked by the runtime) |
| Nesting depth | 8 |
| Components | 500 |
| String length | 2,000 characters |
| List items | 200 |
| Table | 20 columns, 200 rows |
| Chart | 5 series (the five `chart-*` roles), 200 points each |
| Form | 20 fields; select 50 options; input values up to 500 characters |

A document over a bound is invalid content (FR-005), never truncated, so the author learns the
limit.

## R6. Name and icon in the manifest (FR-008)

**Decision**: the manifest gains optional `name` and `icon`:

- `name`: 1 to 60 characters, no control characters, as `author` is checked
  (`manifest.rs:348-360`). Bidirectional overrides are accepted by the parser, as for `author`,
  and removed where the name is drawn.
- `icon`: a PNG, base64, at most 8 KiB decoded, square, 16 to 64 pixels a side. The host decodes
  it with the `png` crate (0.18.1, already in the lock through Tauri) and refuses anything else:
  an SVG, another format, a size or dimension over the bound, an animated PNG. Maintainer's
  decision of 2026-11-10: **PNG only**, since rasterising SVG would bring a large parser of a
  rich format into the host.
- The manifest cap stays 16 KiB (`manifest.rs:19`): 8 KiB of PNG is about 11 KiB of base64.
- **Drawing**: the runtime decodes the icon to RGBA pixels when it parses the manifest
  (`Manifest::icon_rgba()`, where the PNG checks already run), the app hands those pixels to the
  page, and the page draws them on a `<canvas>` with `putImageData`. No URL is created, so `img-src 'self'` stays
  (`tauri.conf.json:15-29`), and the probe's `load-image-data` case is untouched. Without an
  icon, or with one the host refused at load, the frame draws a monogram from the name.
- **Name drawn**: bidirectional controls and other format characters removed, whitespace
  collapsed, cut at 40 characters with an ellipsis, drawn as text; when nothing is left, the last
  part of the id (`local/showcase` → `showcase`).

**HS-02 changes first**: its check reads "An SVG icon holding a script and an external reference
is drawn as an image". It becomes "An SVG icon, or any image that is not a PNG within the bounds,
is refused when the plugin loads, and the frame draws the host's monogram; nothing of it runs or
is fetched." Feature 007's rule allows this: a criterion changes before the feature that runs it.

## R7. Actions: contract 1.4.0 (FR-022, FR-023)

**Decision**:

- **WIT**: a new interface `actor` with `act: func(action: string, input: string) ->
  result<string, string>`, exported by six new worlds, one per existing world plus `actor`:
  `interactive-widget`, `interactive-connected-widget`, `interactive-stored-widget`,
  `interactive-connected-stored-widget`, `interactive-account-widget`,
  `interactive-account-stored-widget`. Existing worlds are untouched: the only-adds test compares
  each world's imports and exports (`runtime/tests/contract_v1.rs:259-275`).
- `input` is `{"config": …, "data": <kept data or null>, "values": {<field>: <value>}}`;
  the output, as for `update`, is the widget's data, up to 256 KiB, which the host keeps **in
  memory only** and passes to the next render in the shape the contract already defines for
  `updater` (`wit/v1/widget.wit:249-254`). A plugin that exports `actor` without `updater`
  receives that same shape; the new interface's doc comment says so, since frozen comments stay.
- **Runtime**: `Phase` gains `Action` (`calls.rs:86-91`). An action runs with the update's time
  limit and render's fuel and memory caps, as an update does (`lib.rs:1462-1465`). Storage opens
  in update mode, all or nothing (`storage/connection.rs:83-105`). Connectors are reachable,
  as in update, but `change-needs-user-action` still refuses any operation that changes data
  (`calls.rs:487-498`; maintainer's decision). Every place that matches on `Phase`
  (`lib.rs:1415,1462,1549,1562,1581`; `calls.rs:464,563`; `storage/mod.rs:517,597,703,716,775`)
  gets the third arm.
- **Action data stays in memory** (maintainer's decision, 2026-11-11): the plugin's `Plugin`
  holds the last data an action returned, with its time, behind its lock; nothing of it reaches
  `kept.rs`'s files, so a widget without `storage` keeps nothing across a restart. Render's choice
  of document (`lib.rs:1392`) gates on `updater()` or `actor()`, so an actor-only plugin sees its
  data; `data` is the newer of the update's kept data on disk and the action's data in memory,
  `fetched_at_ms` its time, and `last_update` the outcome of whichever produced it. Durable state
  is the `storage` permission's, with its approved quota; the threat model's B1 says so.
- **Phase checks** today are `==` comparisons (`calls.rs:464`, `lib.rs:1549,1562`,
  `storage/mod.rs:517,597,716,775`), which a new variant would not trip; they become methods,
  `Phase::may_call_connectors()` and `Phase::writes_storage()`, so each site states the rule.
- **Version**: `V1_VERSION` becomes 1.4.0 and `ACTIONS_SINCE` is added
  (`permissions.rs:27-35`); `Contract::renderer()` names 1.4.0 (`:61`). Host bindings move to
  `interactive-account-stored-widget` (`lib.rs:78-85`).
- **Doc comments** of 1.3.0 that 1.4.0 makes untrue ("Calls work only while the host runs
  `update`", `widget.wit:36`; storage writes "`update` only"; render's document "for a plugin that
  also exports `updater`") are corrected in `wit/v1` before 1.4.0 is frozen, which `wit/README.md`
  allows; `wit/README.md` itself is updated (six more worlds, 1.4.0, the host's world).
- **Freezing**: 1.3.0 is already frozen (`wit/history/1.3.0`). `wit/v1` becomes 1.4.0 and is
  frozen as `history/1.4.0` in the same change, pinned in `frozen_contracts`, because
  `v1_equals_its_frozen_copy` requires it (`contract_v1.rs:348-370`). The not-granted test skips
  `actor` like `renderer` and `updater` (`:11-12,53`). A new only-adds test checks 1.3.0 against
  1.4.0, and the 1.3.0 test moves to history against history.
- **Repointing**: plugins on `wit/v1` move to `history/1.3.0` first: `private-notes`,
  `account-probe`, `token-hunt`. Places that use 1.4.0 as "newer than the host"
  (`permissions.rs:695,976,1011,1102-1109,1453`; `runtime/tests/contract.rs:252-258`;
  `docs/authors/getting-started.md:254-274`) use 1.5.0.

## R8. Declared actions in the manifest

**Decision**: `actions` is an optional manifest array of `{"name": …}`, at most 16, names
`^[a-z][a-z0-9-]{0,31}$`, unique. A plugin that exports `actor` must declare at least one; one
that declares actions must export `actor`, checked at load like `updater` is for connectors
(`permissions.rs:358-364`). The host refuses a name the manifest does not declare **before**
instantiating the plugin, with `undeclared-action`, a new `ErrorKind`.

- Approval: the approval digest hashes the whole package (`package.rs:78-85`), so a new action
  changes it, but load-time re-approval follows grants only (`lib.rs:1166-1218`). An action can
  reach nothing the plugin was not granted, so declaring one needs no new approval.

## R9. The window's commands (FR-019)

**Decision**: three commands, each in `window-surface.toml` with its reason, its permission in
`capabilities/default.json`, and registered through `commands.rs`:

| Command | Arguments | Returns |
| --- | --- | --- |
| `widgets_list` | none | each widget's handle, drawn name, icon pixels or none |
| `widget_render` | `handle` | the normalised content, or a content error |
| `widget_act` | `handle`, `action`, `values` | the new content, or an error |

- **Handles** are short strings the host assigns (`w0`, `w1`…) for the widgets it is showing;
  anything else is refused with `unknown-widget` before any plugin is touched.
- `widget_act` checks, in the order of `contracts/window-commands.md`: the handle; whether the
  widget is busy (a render or an action running); the action name against the manifest (R8); the
  values against the form's bounds (R5); the secret check (R11). Then it runs the action, keeps
  its data, renders and returns the content. A press while the widget is busy is refused with
  `widget-busy`, and the button shows it is working.
- The window's probe of feature 008 gains the cases of R14 and keeps every case it has.

## R10. The page side: frame and components

**Decision**: new host components under `app/src/widgets/`: `Frame.tsx`, `content/*.tsx` (one
per component) and `Widgets.tsx`, the stack of frames shown above the demo's tabs.

- **Frame**: a header and a body as siblings. The header uses the host-only `frame-header` roles
  (`DESIGN.md` 337-346) and draws the name, the icon canvas and the host's actions (a reload
  button). The body uses `frame` roles, with `contain: layout paint`, `overflow: auto`, a fixed maximum
  height (scrolling inside the body past it) and `isolation: isolate`, so nothing of the content
  paints outside it or above the header (HS-03, HS-04, HS-16). Content never uses
  `position: fixed` or `z-index`.
- **Strings**: rendered as React text children only, each wrapped in `<bdi>` so
  bidirectional characters cannot reorder around it (HS-24). No component sets `title`, `href`,
  `src` or any URL attribute from content (HS-07, HS-25).
- **Status tones** draw the lucide icon the widget map names (`circle-check`, `triangle-alert`,
  `circle-x`) beside the text (HS-08).
- **Select** in a form: a host listbox drawn inside the body, not Radix Select or a native
  `<select>`, both of which draw outside the page's layout (HS-25). The form has `noValidate` and
  no `required`, `pattern` or `type` that triggers engine validation; the host shows errors
  inline. Text and number inputs are `type="text"` (numbers use `inputMode="decimal"`); a checkbox is a
  native `<input type="checkbox">`, not Radix's, which renders a hidden extra input. Every input
  has `autocomplete="off"`, no `name` taken from content, no credential hints (HS-06).
- **Chart**: drawn by the host as inline SVG elements (line or bar), series in `chart-1` to
  `chart-5` order, with a text table of the values for assistive technology. No chart library:
  none is installed, and SVG elements load nothing.
- **Design check**: the components use only role utilities at declared opacities
  (`design/src/checks/usage.ts:112-168`). The map from tones and variants to classes lives in a
  `.tsx` module, so the usage scan, which reads only `.tsx` (`usage.ts:120-124`), checks it.
- **Theme**: the frame and components use the same role utilities as the rest of the app, so
  they follow `.dark` with no extra code.

## R11. Form values carrying a held secret (FR-025)

**Decision**: the runtime gains `Runtime::holds_secret_in(&self, text: &str) ->
Result<bool, RuntimeError>`. It reads the app's connected accounts (`accounts.rs:130-151`), takes
each one's secrets from the app's vault (the access token, and the refresh token or key where the
credential has one), and checks the text with the encodings the network layer already uses to withhold a
credential echoed back (`network/mod.rs:245-286`, made a shared function). The app calls it on
each form value before `widget_act` runs the plugin; on a match it refuses with
`secret-in-form`, logs the field's name but never the value, and the plugin is not called.

A vault that cannot be read (`keychain-unavailable`, `vault/mod.rs:205-238`) makes the check an
error, and the form is refused: it fails closed. With no account connected, as in this feature,
the check finds nothing and never touches the keychain. Its tests connect an account in a mock
vault (`lib.rs:515,534`), submit each secret in each encoding, and make the vault fail with the
existing `test_vault_fail_next` hook.

## R12. The plugin's log in the app

**Decision**: the runtime keeps no persistent plugin log; logs come with each report
(`lib.rs:120-146`), and the host can build `LogMessage`s (`lib.rs:113-117`). The app keeps, per
widget, the last 100 entries in memory: the plugin's own, the content checker's (unknown tone,
dropped field, placeholder), and the host's refusals. They are printed to the console of a
development build. Showing them in the UI comes with the dashboard's widget settings.

## R13. Breaking tests below the page

| Rule | Test, failing when the rule is removed |
| --- | --- |
| Content bounds, invalid kinds, lenient cases (FR-003 to FR-005) | `widget-content` unit tests + `tests/schema.rs` (agreement with `wit/content.schema.json` via `boon`) |
| Unknown tone and style fields (HS-09) | `widget-content` test: draws as without them, one log entry |
| Status icons (HS-08) | `widget-content` test: every status tone in the widget map carries its icon in the normalised tree |
| Form: no password, no credential hints (HS-06) | `widget-content` test: `password`, `masked`, credential names and hints refused or dropped |
| Name and icon bounds (HS-02, FR-008) | `runtime/tests/manifest_schema.rs`: agreement cases for `name`, and host-only cases for icons (SVG, JPEG, over 8 KiB, not square, too small or large, animated, malformed); an app test of the drawn name (markup, overrides, format characters, the longest accepted name, nothing left) |
| Actions: declared names, limits, grants, no change operations, kept data, older minors (FR-022, FR-023) | `runtime/tests/actions.rs` with fixtures `action-probe` and `hostile/action-escape` |
| The bundled approval refuses a package with permissions (R2) | app test |
| App records apart, development off (FR-024) | app test |
| Unknown handle, busy widget, bad values, secret in form (FR-019, FR-025) | app tests on the command functions |
| Contract 1.4.0 only adds; 1.3.0 frozen | `runtime/tests/contract_v1.rs` |
| Every author doc example is valid content | `widget-content/tests/docs.rs` reads the examples of `docs/authors/reference/content.md` |
| A release build embeds only `showcase`; hostile fixtures only with `shell-probe` | app test on the embedded list |
| A bundled package declaring permissions, connectors or storage is refused (R2) | app test, one case each |
| The app never calls the development paths (FR-024) | app test of the options, and a source scan of `app/src-tauri/src` |
| The secret check fails closed (FR-025) | runtime test with `test_vault_fail_next` |
| Each `Phase` rule holds for `Action` | `runtime/tests/actions.rs`: store written all or nothing, read operations allowed, change operations refused, connectors refused in render |

## R14. Breaking tests in the page: the probe's widget cases

**Decision**: the probe build (feature 008) also embeds seven hostile widget fixtures under
`plugins/hostile/ui-*`, shown only in the probe build, and `app/src/probe/widgets.ts` adds cases
that check, in the real page on the three systems:

| Case | Criterion | Holds when |
| --- | --- | --- |
| `frame-header-kept` | HS-01, HS-02 | content shaped like a header with another name sits below the real header; the header's text equals the `ui-header` manifest's name as the host draws it (markup and overrides as text, no child element from it) and its only button is the host's reload |
| `frame-clips` | HS-03 | hit-testing points just outside the oversized widget's body, on each side, never returns an element of its content, and every content element's box, intersected with its clipping ancestors, lies inside the body (bounding boxes alone ignore clipping) |
| `frame-header-hittable` | HS-04 | `elementFromPoint` at points inset from the header's corners, and at its centre, returns the header |
| `content-is-text` | HS-24, HS-07 | strings with HTML, script, image tags, bidi overrides, URLs, file paths and custom schemes appear as literal text; no element they name exists; clicking each string changes neither the navigation nor the new-window counter, and no violation fires |
| `strings-isolated` | HS-24 | each content string sits in an element with computed `unicode-bidi: isolate`, and a sentinel after a right-to-left override string keeps its position |
| `dialog-shaped-widget` | HS-16 | beside the dialog-shaped widget, `elementFromPoint` at another widget's button returns that button, which then takes keyboard focus; nothing covers the chrome |
| `nothing-outside-frame` | HS-25 | with the long select open and an invalid form submitted, nothing in the document lies outside the widget's body, no `invalid` event fires, every form has `noValidate`, and no `title` attribute exists in content |
| `status-icons` | HS-08 | each status text has its icon element |
| `form-no-credentials` | HS-06 | every text-entry input is `type="text"`; every input has `autocomplete="off"` and no credential name or hint |
| `foreign-actions-refused` | HS-05 | the probe calls `widget_act` with host command names, another widget's action and undeclared names; each is refused, and a new `actions` counter in the probe (incremented by `widgets.rs` like `probe::refused`) shows no action ran |
| `content-error-shown` | FR-005 | `ui-invalid` shows the host's error state for content over a bound and, after its `wrong-type` action, for a wrong type; `ui-crash` shows it for a render that traps; `ui-badmanifest`, whose name is over the manifest's limit, shows the not-loaded state; the showcase beside them still draws |
| `window-responsive` | FR-017 | while `ui-invalid`'s `spin` action loops until the time limit, the showcase's reload button is hit by `elementFromPoint` and its `widget_render` answers within a second; then the spin ends as `action-failed` |
| `showcase-on-time` | SC-001 | the showcase's content is drawn; the detail gives the time from the page's navigation start, with the DOM-ready and load times, which T035 records per system. The case does not judge the time: SC-001's second holds on the reference machine, while a CI runner or a loaded virtual display took from 0.6 to 2 s, so a bound here would fail by the machine, not the host |

They join `PAGE_CASES` (`app/src-tauri/src/probe.rs:26-81`). Which fixture carries which case:
`ui-header` (HS-01, HS-02), `ui-overflow` (HS-03, HS-04), `ui-strings` (HS-07, HS-24, HS-25,
HS-06, HS-08), `ui-dialog` (HS-16, HS-05: its buttons name host commands and other widgets'
actions, which the content checker lets through as plain names), `ui-invalid` (FR-005, FR-017:
content over a bound; actions `wrong-type` and `spin`), `ui-crash` (a render that traps),
`ui-badmanifest` (a name over 60 characters, refused at load).

## R15. Decision 0001: recorded review

**Decision**: unchanged. Its trigger is "before the app can install a plugin it did not build
itself" (`docs/decisions/0001-plugin-process-isolation.md:55-57`). The bundled plugins are built
with the app and pinned by digest, and compiling them takes 22 to 68 ms. Its accepted risk said
"only the maintainer loads plugins, from the CLI" (`:51`); the decision gains a dated note that
widens its "During Phase 1" scope to the app's own bundled plugins, which does not change that
risk. The
installation item reviews it.

## R16. The author docs and the threat model

**Decision**:
- **New page** `docs/authors/reference/content.md`: every component, tone, variant and bound,
  with `<!-- ref: content <path> -->` markers; `docs_reference.rs` gains the kind `content`,
  sourced from `wit/content.schema.json` (`docs_reference.rs:16`, `:98-180`).
- **Updated**: `wit/README.md` (1.4.0, the six new worlds, the host's world), `manifest.md`
  (`name`, `icon`, `actions`), `contract.md` (1.4.0, `actor`, the six
  worlds), `errors.md` (`undeclared-action`), `limits.md` (content bounds), `cli.md`
  (`runtime act`), and the getting-started walkthrough's contract version.
- **Threat model**: B4 rows cite the probe cases of R14 and the tests of R13 (the design check
  requires a test or hostile case per cited criterion, `design/src/checks/criteria.ts:89-99`).
  New rows on B1 (an action reaching beyond its grants; an undeclared action), and B7 (a window
  command given a forged handle). B1's text "all or nothing in update" and B2's "calls only in
  `update`" become "in update or an action". B1 says an action's data stays in memory and durable
state needs `storage`. A residual row
  records typed text in a form (Principle IV as amended in v2.6.2).
