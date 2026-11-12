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
Press "Contar": the number goes up within a second. Restart the app: it starts over, since an
action's data is kept in memory only.

## 2. Content checked before the app (US2)

```sh
cargo run -p runtime --locked -- approve target/plugins/showcase.wasm
cargo run -p runtime --locked -- run target/plugins/showcase.wasm --check-content
cargo run -p runtime --locked -- act target/plugins/showcase.wasm increment
```

**Expected**: `run` prints the content, then `content: drawn` (no log entries). `act` prints
`acted local/showcase 0.1.0: increment`, the new data (`"count": 1`), the next render's content
and `content: drawn`. Then approve `target/plugins/ui-invalid.wasm` and run `run --check-content`
on it.

**Expected**: `content: not drawn: over-bound at $.body[0].text: longer than 2000 characters`,
exit status 1; in the app, the same error in the frame and one log entry. With a fresh clone,
`cargo xtask plugins` comes before any build of the app: the app embeds what it builds.

## 3. The format and the docs agree (FR-006, SC-003)

```sh
cargo test -p widget-content --locked
cargo test -p runtime --test it --locked docs_reference:: manifest_schema:: contract_v1::
```

**Expected**: all pass. Add `"color": "red"` to a text in the first example of
`docs/authors/reference/content.md`: `widget-content`'s docs test fails, naming the example's
line, because the host draws it as before but with one log entry (`ignored fields this host does
not know`). Revert.

## 4. The hostile widgets stay in their frames (US3, US4, US5)

Run the probe of feature 008 (`specs/008-tauri-shell/contracts/probe.md`).

**Expected**: every case holds, including the widget cases of research R14: `frame-header-kept`,
`frame-clips`, `frame-header-hittable`, `content-is-text`, `strings-isolated`, `dialog-shaped-widget`,
`nothing-outside-frame`, `status-icons`, `form-no-credentials`, `foreign-actions-refused`, `form-reshaped`,
`content-error-shown`, `window-responsive`, `showcase-on-time`.

Then, in `app/src/widgets/Frame.tsx`, remove `contain-layout contain-paint` and `overflow-auto`
from the body and `overflow-hidden` from the frame (any one of them alone still clips the content
to the body) and run it again.

**Expected**: `frame-clips` and `nothing-outside-frame` break. Revert.

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

**Expected**: an undeclared name is refused before the plugin runs; a connector or operation the
approval does not grant answers `not-granted`; a change operation is refused; the store is
written all or nothing; a 1.3.0 plugin has no actions.

## 7. Review: keyboard, screen reader, narrow frames, forced colors

On each system, tab through the showcase: every button, input and the listbox are reachable,
with the focus ring; the listbox opens and closes with the keyboard inside the frame; a screen
reader reads the chart's values from its table. Narrow the window until the frame is at its
narrowest: tables and charts stay inside the body, scrolling inside it. Turn on forced colors
(Windows high contrast) and reduced motion: the frame stays recognisable and the status icons
visible.
