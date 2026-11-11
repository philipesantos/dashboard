# Quickstart: Validating the Declarative Widget UI and the Host Frame

Commands run from the repository root, with the prerequisites of `CONTRIBUTING.md`.

## 1. The bundled widget appears (US1)

```sh
cargo xtask plugins --locked
pnpm --filter ./app tauri dev
```

**Expected**: above the demo's tabs, the "Vitrine" widget in its frame: its icon and name in the
header, then every component (heading, status texts with icons, list, table, chart, buttons, a
form). It appears within a second of the window. Switch to dark: the frame and content follow.
Press "Contar": the number goes up within a second, and survives a restart.

## 2. Content checked before the app (US2)

```sh
cargo run -p runtime --locked -- run target/plugins/showcase.wasm
cargo run -p runtime --locked -- act target/plugins/showcase.wasm increment
```

**Expected**: `run` prints the content and "drawn, no log entries". `act` prints the new data and
the next render's content. Then run `run` on `target/plugins/ui-invalid.wasm`.

**Expected**: the content error with its path, and one log entry.

## 3. The format and the docs agree (FR-006, SC-003)

```sh
cargo test -p widget-content --locked
cargo test -p runtime --test it --locked docs_reference:: manifest_schema:: contract_v1::
```

**Expected**: all pass. Add `"color": "red"` to a text in the docs' example: the schema test
fails on it, and the host draws it as before with one log entry.

## 4. The hostile widgets stay in their frames (US3, US4, US5)

Run the probe of feature 008 (`specs/008-tauri-shell/contracts/probe.md`).

**Expected**: every case holds, including the widget cases of research R14: `frame-header-kept`,
`frame-clips`, `frame-header-hittable`, `content-is-text`, `dialog-shaped-widget`,
`nothing-outside-frame`, `status-icons`, `form-no-credentials`, `foreign-actions-refused`,
`content-error-shown`.

Then, in `app/src/widgets/Frame.tsx`, remove `overflow: hidden` from the body and run it again.

**Expected**: `frame-clips` breaks. Revert.

## 5. The window's surface (US6)

```sh
cargo test -p dashboard --locked
```

**Expected**: the surface test passes with the three commands listed, granted and registered.
Remove `widget_act`'s entry from `window-surface.toml`: the test fails naming it. Revert.

## 6. Actions only as declared and granted

```sh
cargo test -p runtime --test it --locked actions::
```

**Expected**: an undeclared name is refused before the plugin runs; a change operation is
refused; the store is written all or nothing; a 1.3.0 plugin has no actions.

## 7. Review: keyboard and screen reader

On each system, tab through the showcase: every button, input and the listbox are reachable,
with the focus ring; the listbox opens and closes with the keyboard inside the frame; a screen
reader reads the chart's values from its table.
