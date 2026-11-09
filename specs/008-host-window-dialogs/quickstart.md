# Quickstart: validating the host window and its dialogs

Run from the repository root. Heavy builds on the maintainer's machine go under
`systemd-run --user --scope -p MemoryMax=8G`.

## 1. Build and open the window

```sh
cargo xtask plugins                         # builds the example plugins
cargo run -p runtime --bin runtime -- approve target/plugins/hello-widget.wasm
cargo run -p host --bin dashboard
```

Expect: one window, the chrome on top with "Appearance" and "Reload", a frame for hello-widget
with its monogram and name and "Rendered" in the ok tone. One Tab puts the focus ring on the first
control, "Appearance".

## 2. A failing plugin

Approve a hostile fixture whose render traps, reload with the chrome's "Reload" or by focusing the
window again. Expect its frame to say "Did not render" with the reason, in the error tone, and the
rest of the window to keep working.

A plugin that declares a `local/` connector does not load in the window (the runtime runs without
development mode): its frame says so in the host's words. It still runs with `runtime run`.

## 3. The panel and the chrome

Open a frame's panel; resize the window down to 800 × 600 and move toward the top. Expect the
panel attached to its frame and nothing over the chrome. Escape closes it and focus returns to the
frame's control.

## 4. The dialog

From a frame's menu, choose "Remove from dashboard". Expect the scrim over the whole window, chrome
included; clicks and keys outside the dialog do nothing; the buttons ignore input for one second;
focus on "Cancel"; Tab cycles inside; Escape refuses and focus goes back to the frame's menu
button. Confirm with
"Remove": the frame leaves, and `runtime permissions` on that package shows it unapproved.

## 5. Appearance

Open the appearance settings from the chrome; pick sand, a host color, a widget color, the radius
and the theme; the window redraws at once. Pick a widget color within 30° of the host color: the
host refuses it and names the colors. Restart: the choice is kept. Pick "Match the system", then
switch the system between light and dark: the window follows without a restart (FR-037; GPUI's
test window cannot switch the system's theme, so this step is checked here, by hand).

## 6. The automated layers

```sh
cargo test -p host                          # logic, checks, headless hostile cases
cargo test -p host --test window            # real window: input, pixels and accessibility tree
```

On Linux, the window target runs as CI runs it:

```sh
dbus-run-session -- bash -c '
  busctl --user call org.a11y.Bus /org/a11y/bus org.a11y.Bus GetAddress >/dev/null
  busctl --user set-property org.a11y.Bus /org/a11y/bus org.a11y.Status IsEnabled b true
  xvfb-run -a cargo test -p host --test window'
```

Expect every test to pass; the window target prints the measured start and dialog times (SC-001,
SC-006).

## 7. The bans

Add a call to `window.prompt(...)`, `cx.open_url(...)` or `img("https://…")` anywhere in `host/src/`
outside `media.rs`: `cargo clippy -p host` fails naming the rule;
`cargo clippy --manifest-path host/ban-fixture/Cargo.toml` fails once for each of the 29 banned
methods. Add a string literal to a `child(...)` in a screen: `cargo test -p host --test checks`
fails. Change a GPUI pin to `^1.25.3`: the pin check fails.

## 8. Before a release

Walk every host dialog with VoiceOver, Narrator and Orca following
`docs/accessibility/screen-reader-walk.md`, and record the results.
