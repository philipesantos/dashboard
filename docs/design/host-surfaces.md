# Host surfaces: the chrome, the widget frame and host dialogs

The dashboard draws content from plugins it does not trust. A plugin returns declarative UI, and
the host draws it inside a frame. If widget content could draw something that passes for the
host, a plugin could ask the person to sign in, to grant a permission or to connect an account
in a place where only the host may ask (constitution, Principles V and VII). This document is
the design every Phase 3 feature that draws the chrome, a frame or a dialog builds against.

## What only the host draws

- **The chrome**: the app's header and sidebar. The window title bar belongs to the operating
  system.
- **Each widget's frame**: its header, with the plugin's name and icon and the frame's actions,
  and its edge.
- **Host dialogs**: install, permissions, connecting an account, confirming an action, each over
  its scrim.
- **Anything raised above a frame**: the host's menus, selects and popovers.
- **The focus style**, which the host draws on every control, the controls of widget content
  included.

Their colors and shadows are host-only roles in `DESIGN.md`; no tone or variant widget content
can select reaches them, and `pnpm --filter ./design check` fails if one does. That split is by
role, not by color: widget content may use the same accent. What tells a host dialog apart is
its form, which widget content cannot produce: it covers the chrome and every frame, the rest of
the window goes inert, and it holds keyboard focus and is all that assistive technology sees.
A picture of a dialog inside a frame leaves the chrome and the other widgets visible and
working, and its buttons can only trigger that widget's own declared actions.

## How to read the criteria

Each criterion has an id, one checkable statement, how it is checked and the feature that runs
the check:

- `Check:` starts with `test` (an automated test), `hostile case` (a hostile plugin in the
  security suite that must fail in CI) or `review` (a step in the feature's review).
- `Feature:` names the item of the development guide's Phase 3 that runs the check.

A feature that draws the frame or a dialog passes every criterion that names it, or the criterion
changes first. Every row of `docs/threat-model.md` on the plugin UI boundary (B4) in Phase 3
cites the criteria that cover it, and a cited criterion is checked by a test or a hostile case,
never by review alone (Principle VIII). `pnpm --filter ./design check` enforces both.

## The chrome and the frame

### HS-01 The frame's header and its actions belong to the host

The host draws each frame's header, with the plugin's name, its icon and the frame's actions, in
the host-only `frame-header` roles. Widget content is never drawn in the header, and no part of
widget content can add to, replace or restyle it.

Check: hostile case: a widget that asks for every tone and variant and returns content shaped
like a header; the frame's header is unchanged, and its actions are the host's.
Feature: Moldura do host em cada widget com nome e ícone do plugin

### HS-02 The plugin's name and icon come from its manifest, escaped and bounded

The header shows the name and icon from the plugin's manifest, never from widget content. The
name is drawn as escaped text, with control and bidirectional override characters removed and a
length limit, and a host-drawn placeholder stands in when it is empty. The icon is drawn as an
image in a fixed box, never as markup.

Check: test: names holding markup, bidirectional overrides, 500 characters and nothing; each is
drawn escaped, cut at the limit, or replaced by the placeholder.
Feature: Moldura do host em cada widget com nome e ícone do plugin

### HS-03 Widget content is clipped to the frame's body

The frame's body is a containing block for everything in it, with paint containment and
clipping, so no element of widget content, positioned or not, is drawn outside the body.

Check: hostile case: a widget whose content is wider and taller than its frame; nothing of it is
drawn outside the frame's body.
Feature: Moldura do host em cada widget com nome e ícone do plugin

### HS-04 Widget content cannot cover the frame's header

The header sits outside the body, so widget content can neither hide it nor take its clicks.

Check: hostile case: a widget whose content overflows toward the header in every direction; the
header stays fully visible and hit-testable.
Feature: Moldura do host em cada widget com nome e ícone do plugin

## Widget content's controls

### HS-05 A control in widget content triggers only its widget's declared actions

A button or form in widget content can trigger only an action that its widget declared, run by
the host as that widget. It can never grant a permission, install a plugin, connect an account
or answer a host dialog.

Check: hostile case: a widget whose buttons name host commands and other widgets' actions; the
host refuses each, and nothing is granted, installed or connected.
Feature: Biblioteca de UI declarativa

### HS-06 The form has no password field and is never offered for autofill

The simple form has no password or masked field. Its inputs set `autocomplete="off"` and are
never marked as credentials, so no webview offers to fill or save what they hold.

Check: test: the form component accepts no password or masked field, and every input it draws
has `autocomplete="off"` and no credential type or hint.
Feature: Biblioteca de UI declarativa

### HS-07 No plugin string opens a link, a file or another app

No string from a plugin reaches the opener or any call that opens a link, a file or another
app. The declarative UI has no link component.

Check: hostile case: a widget whose every string is a URL, a file path or a custom scheme; nothing
is opened, and the opener is never called.
Feature: Shell Tauri com capabilities mínimas e CSP restritiva

### HS-08 Status is never shown by color alone

The ok, warning and error tones always carry the icon the host draws for them, from the widget
map, so the status reads without color (WCAG 1.4.1).

Check: test: every status tone in the widget map draws its icon beside the text.
Feature: Biblioteca de UI declarativa

### HS-09 Widget content selects styling only by tone and variant

Widget content has no field for a color, a font, a size, a class or a style. A tone the host does
not know draws as the default tone, and the host records the name in the plugin's log.

Check: test: widget content carrying style fields draws exactly as without them, and an unknown
tone draws as the default and adds one entry to the plugin's log.
Feature: Biblioteca de UI declarativa
