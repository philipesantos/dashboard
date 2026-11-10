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

## The form of a host dialog

Every host dialog shares one form. Each property below is one that widget content cannot
produce under HS-03 and HS-04, and none depends on color.

### HS-10 A host dialog covers the chrome and every frame

A host dialog is drawn in a layer above the chrome and every frame, in the host-only `dialog`
roles at the `dialog` elevation, over the `scrim`.

Check: test: with a dialog open, the topmost element at points over the chrome and over each
frame belongs to the dialog or its scrim.
Feature: Diálogo de instalação com permissões em linguagem clara

### HS-11 The rest of the window is inert while a dialog is open

Until the dialog closes, everything outside it ignores the pointer and the keyboard and is hidden
from assistive technology.

Check: test: with a dialog open, clicks and key presses aimed at the chrome and at a frame do
nothing, and the accessibility tree holds only the dialog.
Feature: Diálogo de instalação com permissões em linguagem clara

### HS-12 A dialog ignores input for a short moment after it appears

Clicks and keys are ignored for a short moment after the dialog appears, so input aimed at what
was under the pointer or the caret cannot answer it. The dialog feature measures the length.

Check: test: a click on the risky choice and an Enter press sent within the delay after opening
do nothing; the same input after the delay is handled.
Feature: Diálogo de instalação com permissões em linguagem clara

### HS-13 A dialog names who is asking

The dialog names the plugin or connector the decision is about, by its manifest name and its
repository id (`github.com/owner/repo`), drawn by the host as escaped text with a length limit,
as the frame does (HS-02).

Check: test: plugins whose names hold markup, bidirectional overrides and 500 characters; the
dialog shows each escaped and bounded, with the repository id in full.
Feature: Diálogo de instalação com permissões em linguagem clara

### HS-14 The safe choice is the default

The safe choice (deny or cancel) is the dialog's default action, so pressing Enter without
moving focus never approves.

Check: test: opening a dialog and pressing Enter after the input delay takes the safe choice.
Feature: Diálogo de instalação com permissões em linguagem clara

### HS-15 Long content scrolls inside the dialog

When a dialog's content is longer than the window, it scrolls inside the dialog, and who is
asking and the safe choice stay reachable without leaving it.

Check: test: a dialog with more permissions than fit in a small window; its body scrolls, and its
title and safe choice stay in view.
Feature: Diálogo de instalação com permissões em linguagem clara

### HS-16 Widget content cannot produce the form of a host dialog

Widget content cannot produce a layer above the chrome and the frames, a scrim, or an inert
window. A picture of a dialog in a frame leaves the chrome and the other widgets visible and
responding.

Check: hostile case: the declarative UI's hostile widget, which uses every tone and variant to
look like a dialog; the chrome stays uncovered, the other frames keep responding to the pointer
and the keyboard, and its buttons trigger only its own declared actions.
Feature: Biblioteca de UI declarativa

### HS-17 The dialog's form holds in forced colors

None of the properties that mark a host dialog depends on color, so each holds when the system
replaces the theme's colors, as Windows high contrast does. The dialog keeps a visible edge
there.

Check: test: in forced-colors emulation, HS-10 and HS-11 still hold and the dialog has a visible
border.
Feature: Diálogo de instalação com permissões em linguagem clara

## Dialogs with the keyboard and a screen reader

Trust decisions happen only in host dialogs, so a dialog someone cannot operate shuts them out of
installing plugins and connecting accounts. Each check below runs with only a keyboard, and with
the system's screen reader on each of the three systems: Narrator on Windows, VoiceOver on macOS
and Orca on Linux.

### HS-18 Focus moves into the dialog and stays there

When a dialog opens, focus moves to its safe choice. Tab and Shift+Tab move only among the
dialog's controls until it closes.

Check: test: after opening, the focused element is the safe choice, and cycling Tab and
Shift+Tab past both ends never leaves the dialog; the feature's review repeats it with a keyboard
on each of the three systems.
Feature: Diálogo de instalação com permissões em linguagem clara

### HS-19 Escape takes the safe choice

Escape takes the dialog's safe choice (deny or cancel) and closes it. Escape never approves.

Check: test: Escape on every host dialog, with focus on each of its controls, takes the safe
choice.
Feature: Diálogo de instalação com permissões em linguagem clara

### HS-20 Focus returns to where it was

When a dialog closes, focus returns to the control that opened it, or to the window when that
control is gone.

Check: test: closing a dialog returns focus to its opener; when the opener was removed while the
dialog was open, focus lands on the window.
Feature: Diálogo de instalação com permissões em linguagem clara

### HS-21 The dialog is announced with its name and description

The dialog is exposed as a dialog, with an accessible name (its title) and description (who is
asking and what is decided), so a screen reader announces all three on open.

Check: test: the accessibility tree exposes the dialog's role, name and description; the
feature's review hears the announcement with the screen reader on each of the three systems.
Feature: Diálogo de instalação com permissões em linguagem clara

### HS-22 Nothing behind the dialog is reachable by assistive technology

While a dialog is open, a screen reader's virtual cursor and its lists of headings, links and
landmarks reach nothing outside the dialog.

Check: test: with a dialog open, the accessibility tree outside it is empty; the feature's review
tries the screen reader's navigation keys on each of the three systems.
Feature: Diálogo de instalação com permissões em linguagem clara

### HS-23 Transitions follow reduced motion

Dialog and frame transitions follow the system's reduced-motion setting: with it on, they
appear and move without animation.

Check: test: with reduced motion emulated, opening a dialog and moving a frame run no animation.
Feature: Grade do dashboard
