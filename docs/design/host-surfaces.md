# Host surfaces

What only the host draws, and how each part is checked. A host dialog is told apart from
everything a widget supplies by its form, not by its colors or by a sign the person must remember
to look for: it covers the host's chrome and blocks every host window, which no widget frame,
panel or declared control can do (constitution v5.0.0, Principles V and VII). The colors and
sizes come from [`DESIGN.md`](../../DESIGN.md); the surfaces' tokens are in the `host-surfaces`
crate, which only the host may depend on.

Each criterion below is something a later Phase 3 feature turns into an automated test that fails
the build (Principle VIII); the accessibility criteria are checked on the platform's
accessibility tree (AX on macOS, UI Automation on Windows, AT-SPI on Linux) through AccessKit, and
walks with VoiceOver, Narrator and Orca are an extra check before a release, not the test. **Observe** is what
the test measures, **Expect** the result that passes, **Answers** the constitution rule and the
threat-model rows (quoted exactly from [the threat model](../threat-model.md)) it answers, and
**Tested by** the feature that writes the test. `design-gen`'s `criteria` test checks that every
Phase 3 row on boundary B4 is answered here or decided elsewhere.

## Chrome

### HS-01 The chrome is never covered by a widget frame or panel
- **Observe**: the bounds of the chrome and of every widget frame and frame panel, at window sizes
  800 × 600, 1280 × 800 and 2560 × 1440, at scale factors 1, 1.5 and 2, and while frames are moved
  or resized to and past the window's edges.
- **Expect**: no frame or panel intersects the chrome's bounds.
- **Answers**: V
  - Password field or drawing over the frame (fake login)
  - Something that is not a host dialog passes for one: the operating system's native prompt, or a widget drawing one
- **Tested by**: host window

### HS-02 Every host dialog covers the chrome
- **Observe**: the bounds of the scrim of each host dialog and of the chrome, in every host window,
  while the dialog is open.
- **Expect**: the scrim covers the whole of every host window, chrome included, and the dialog's
  surface sits above it.
- **Answers**: VII
  - Something that is not a host dialog passes for one: the operating system's native prompt, or a widget drawing one
- **Tested by**: host dialogs

## Widget frame

### HS-03 Widget content draws only inside its frame
- **Observe**: the bounds of everything drawn from a widget's content, including content that
  asks for sizes, offsets or elevation larger than its frame.
- **Expect**: every pixel of the content is inside the frame's content area; what does not fit is
  clipped, never drawn outside.
- **Answers**: V
  - Password field or drawing over the frame (fake login)
- **Tested by**: widget kit

### HS-04 No widget opens a window or a dialog
- **Observe**: the host's windows, the accessibility tree and the calls to the operating system's
  notification and window APIs before and after loading, rendering and updating a hostile widget
  that asks for a window, a popup, a dialog or a notification.
- **Expect**: the set of host windows is unchanged, no node with a dialog or alert-dialog role
  appears and no notification is posted; the request is refused or ignored and logged, escaped.
- **Answers**: V
  - Widget opens a window or dialog (phishing)
- **Tested by**: widget kit

### HS-05 Nothing a plugin supplies takes a dialog's role
- **Observe**: the role of every accessibility node built from a widget's content or from a
  declared action or setting, for content that names roles, titles or modal flags.
- **Expect**: no such node has a dialog, alert-dialog or window role, nor a modal flag, and each is
  a descendant of its frame's node.
- **Answers**: V
  - Something that is not a host dialog passes for one: the operating system's native prompt, or a widget drawing one
- **Tested by**: widget kit

### HS-06 The frame's label is the plugin's name, made safe
- **Observe**: the label drawn on a frame for plugin names holding control, format and separator
  characters (Unicode Cc, Cf, Zl, Zp, which include bidirectional overrides and zero-width
  characters), combining marks only, more than 40 grapheme clusters (UAX #29), nothing left after
  cleaning, and names like "Dashboard" or "System".
- **Expect**: every Cc, Cf, Zl and Zp character is removed; the name is capped at 40 grapheme
  clusters followed by an ellipsis; an empty result is shown as the host's "Unnamed widget"; a name
  imitating the host is drawn like any other, inside its frame and under the chrome (HS-01); the
  label is in the host's font and the frame-label style, and the frame's accessible name is the
  same text.
- **Answers**: V
  - Something that is not a host dialog passes for one: the operating system's native prompt, or a widget drawing one
- **Tested by**: widget kit

### HS-07 The monogram is the host's, from the name
- **Observe**: the monogram drawn for the same names as HS-06, and for names starting with an
  emoji, a digit, a symbol or a letter of a non-Latin script.
- **Expect**: the first letter or digit of the cleaned name, upper-cased where the script has case,
  or a fixed host glyph when there is none; drawn in the neutral roles of the monogram component,
  never in a color the plugin chose; no image or icon from the plugin (decision 0001).
- **Answers**: V
  - Something that is not a host dialog passes for one: the operating system's native prompt, or a widget drawing one
- **Tested by**: widget kit

## Frame panel

### HS-08 A panel stays attached to its frame
- **Observe**: the bounds of a frame and of the panel it opens, as the frame moves, resizes and
  nears the window's edges.
- **Expect**: the panel touches its frame's edge along at least one side, moves with it, never
  covers the chrome, and closes when its frame is removed.
- **Answers**: V
  - A plugin's settings panel passes for the host's or collects a secret: a label like "GitHub password", a masked field, a number field for a code, a panel that dims or blocks the window
- **Tested by**: declared actions

### HS-09 A panel never dims or blocks the window
- **Observe**: the pixels outside an open panel and its shadow's extent, compared with the same
  dashboard of static frames before the panel opened; pointer input sent to other frames and to
  the chrome; and Tab pressed from the panel's last control.
- **Expect**: no pixel outside the panel and its shadow changes; the other frames and the chrome
  still receive their input; Tab moves focus out of the panel.
- **Answers**: V
  - A plugin's settings panel passes for the host's or collects a secret: a label like "GitHub password", a masked field, a number field for a code, a panel that dims or blocks the window
- **Tested by**: declared actions

## Host dialogs

### HS-10 A host dialog blocks every host window
- **Observe**: pointer and keyboard input sent, while a dialog is open, to every host window,
  including windows other than the one the dialog belongs to, and to every frame and panel.
- **Expect**: no input outside the dialog reaches anything; a click outside the dialog does not
  close or answer it; closing the window, or quitting, while it is open answers it as a refusal.
- **Answers**: VII
  - Something that is not a host dialog passes for one: the operating system's native prompt, or a widget drawing one
- **Tested by**: host dialogs

### HS-11 A new dialog ignores input for its first second
- **Observe**: with an injected clock counted from the first frame that shows the dialog, clicks on
  its buttons and presses of Enter, Space and Escape at 0, 500, 999 and 1000 ms; a key and a pointer
  button held down from before it appeared; and the same after the window regains focus and after
  a dialog's content changes or a second dialog replaces it.
- **Expect**: nothing before 1000 ms answers the dialog and input at 1000 ms does; a key or button
  already down answers nothing until released and pressed again; the 1000 ms start again when the
  window regains focus and when the dialog's content changes or another replaces it.
- **Answers**: VII
  - A dialog appears under the user's pointer or keystroke, so input meant for something else answers it
- **Tested by**: host dialogs

### HS-12 Trust decisions never use a native prompt
- **Observe**: the host's code paths that grant a permission, install a plugin or connector,
  connect an account and choose a destination, and the crates the host depends on.
- **Expect**: none of them calls GPUI's `Window::prompt` or any native dialog API, and `deny.toml`
  bans the native dialog crates (`rfd`, `native-dialog`); each decision is asked in a host dialog.
  The provider's sign-in page in the browser and the OS keychain's own prompts are not counted.
- **Answers**: VII
  - Something that is not a host dialog passes for one: the operating system's native prompt, or a widget drawing one
- **Tested by**: host dialogs

## Fields the host draws for a plugin

### HS-13 Every typed field carries the host's note
- **Observe**: every field the host draws from a plugin's declared settings or action parameters
  that accepts typed text or digits, number fields included, and the accessibility tree around it.
- **Expect**: below each field, in the field-note style, the host's own words name the widget that
  receives what is typed and say the field is not for passwords, tokens or codes; the note is the
  field's accessible description.
- **Answers**: V
  - A plugin's settings panel passes for the host's or collects a secret: a label like "GitHub password", a masked field, a number field for a code, a panel that dims or blocks the window
- **Tested by**: declared actions

### HS-14 No field the host draws for a plugin is masked
- **Observe**: the drawn text and the accessibility tree of fields declared with names like
  "password", "secret", "token" or "pin", or with any flag asking for masking.
- **Expect**: the typed text is drawn as typed, and no node is exposed as a password or secure
  text field.
- **Answers**: V
  - A plugin's settings panel passes for the host's or collects a secret: a label like "GitHub password", a masked field, a number field for a code, a panel that dims or blocks the window
- **Tested by**: declared actions

### HS-15 A plugin's text sits under the widget's label
- **Observe**: where a plugin's names and descriptions for its settings and actions are drawn in a
  panel, and the accessibility tree around them.
- **Expect**: the panel's heading is the frame's label (HS-06), drawn by the host; the plugin's
  text comes after it, in the kit's body and label styles, never in a title or host style; their
  nodes are descendants of the panel, itself attached to that frame.
- **Answers**: V
  - A plugin's settings panel passes for the host's or collects a secret: a label like "GitHub password", a masked field, a number field for a code, a panel that dims or blocks the window
- **Tested by**: declared actions

## Declared controls and focus

### HS-20 Every declared control works from the keyboard
- **Observe**: Tab order, accessible names and activation by Enter and Space of every control the
  host draws from a plugin's declared actions and settings, in frames and panels; opening a panel,
  Escape inside it, and where focus goes after.
- **Expect**: every such control is reachable with Tab, has the declared name as its accessible
  name and works from the keyboard alone; Escape closes the panel and returns focus to the control
  that opened it (Principle V: usable through the host's controls alone).
- **Answers**: V
  - Content draws its own field or reads keystrokes and pointer positions (fake login, keylogging)
- **Tested by**: declared actions

### HS-21 Popups from declared controls stay in their frame or panel
- **Observe**: the bounds and the windows of the host-drawn popups a declared control opens
  (a choice list, a date picker, a tooltip with a plugin's description).
- **Expect**: each is drawn inside the frame or panel it belongs to, or attached to it as HS-08
  says for panels; none covers the chrome, and none is a separate OS window or takes a dialog role.
- **Answers**: V
  - Widget opens a window or dialog (phishing)
- **Tested by**: declared actions

### HS-22 A panel never covers another widget's frame label
- **Observe**: the bounds of an open panel and of every other frame's label, as frames sit next to
  each other at the sizes of HS-01.
- **Expect**: no panel intersects another frame's label; when there is no room, the panel opens on
  the other side or the dashboard scrolls, so whose text is whose stays clear.
- **Answers**: V
  - A plugin's settings panel passes for the host's or collects a secret: a label like "GitHub password", a masked field, a number field for a code, a panel that dims or blocks the window
- **Tested by**: declared actions

### HS-23 The frame label stays visible at the frame's smallest size
- **Observe**: the frame's label and monogram at the smallest size the grid lets a frame take.
- **Expect**: the monogram and at least the first grapheme cluster of the label, with the ellipsis,
  stay drawn; widget content gives way first.
- **Answers**: V
  - Something that is not a host dialog passes for one: the operating system's native prompt, or a widget drawing one
- **Tested by**: dashboard grid

### HS-24 Every host control shows the host's focus ring
- **Observe**: every control the host draws, host and declared alike, when it has keyboard focus.
- **Expect**: a ring in the `focus` role, 2 px thick and 2 px outside the control, that reaches 3:1
  against what surrounds it (DESIGN.md's lint); no control hides it.
- **Answers**: V
  - Content draws its own field or reads keystrokes and pointer positions (fake login, keylogging)
- **Tested by**: host window

## Keyboard and screen readers

GPUI does not mark a dialog as modal in its accessibility tree (there is no `aria_modal`), so a
screen reader's virtual cursor could leave a dialog even while keyboard focus is held. The host
makes up for it by what it exposes: while a host dialog is open, it is the only part of any host
window in the tree (HS-18). How (removing the other nodes, or hiding them) is the host-dialog
feature's choice; both pass HS-18. Each criterion here is tested with real key presses in a real
window, not only headless, and on the accessibility tree of each system in CI; walks with
VoiceOver on macOS, Narrator on Windows and Orca on Linux are an extra check before a release.

### HS-16 Focus starts inside a dialog, never on the confirming button
- **Observe**: the focused element right after each host dialog opens, from the keyboard and from
  the pointer, in a real window.
- **Expect**: focus is inside the dialog, on its first control that is not the one that confirms
  (Cancel, or the first field); the dialog has no default button, so Enter with the initial focus,
  or in a field, never confirms it.
- **Answers**: VII
  - A dialog appears under the user's pointer or keystroke, so input meant for something else answers it
- **Tested by**: host dialogs

### HS-17 Focus never leaves an open dialog, and Escape refuses
- **Observe**: the focused element after pressing Tab and Shift+Tab repeatedly, more times than
  the dialog has controls, and the dialog's answer after Escape, in a real window.
- **Expect**: focus cycles through the dialog's controls only; Escape closes the dialog as a
  refusal and returns focus to where it was before the dialog opened.
- **Answers**: VII
  - Something that is not a host dialog passes for one: the operating system's native prompt, or a widget drawing one
- **Tested by**: host dialogs

### HS-18 While a dialog is open, it is all that assistive technology sees
- **Observe**: the accessibility tree of every host window while a host dialog is open, read
  through AccessKit and the platform's tree (AX, UI Automation, AT-SPI) in CI.
- **Expect**: only the dialog and its contents are exposed; no frame, panel, chrome or widget node
  can be reached until the dialog closes.
- **Answers**: VII
  - Something that is not a host dialog passes for one: the operating system's native prompt, or a widget drawing one
- **Tested by**: host dialogs

### HS-19 A dialog announces itself as the host's
- **Observe**: the AccessKit properties of a host dialog's node and its children when it opens.
- **Expect**: role Dialog, labelled with the host's name for the dialog (such as "Install a
  plugin") and described by its question in the host's words; a plugin's names, if shown, are in a
  child node of their own after the host's words.
- **Answers**: VII
  - Something that is not a host dialog passes for one: the operating system's native prompt, or a widget drawing one
- **Tested by**: host dialogs

## Decided elsewhere

- Widget content imitates a host dialog inside its frame, with placed declared actions as its buttons ("Allow access?")
  Owner: declared actions. HS-01, HS-03 and HS-05 keep such a fake inside its frame, under the
  chrome and out of the dialog role,
  but the buttons are the widget's own actions; whether the host writes the effect and
  destination on every control of an action that changes data, and whether a destructive action
  always asks for a host dialog, is decided in that spec (threat model, open questions).
- Widget content makes the host fetch a remote resource or a local file, or open a URL (an image address, a link)
  Owner: widget kit, which maps content only to host components, takes images only as bytes and
  never passes a plugin string to GPUI's `img()` or `open_url`; and the host window, which keeps
  GPUI's HTTP client null.
- Crafted image, SVG or font exploits a decoder, which now runs in the host process beside the vault
  Owner: the host window's plan, which revisits decision 0001; until then no image block or
  plugin icon exists, and fonts are the host's own and, for glyphs Barlow and IBM Plex Mono lack, the system's.
- Asks for little at install, then presses the user to turn on optional permissions one at a time
  Owner: installation, whose dialogs warn on every grant change.
- Value outside its declared type, or a parameter type the host cannot draw, reaches the plugin
  Owner: declared actions.
- Content draws its own field or reads keystrokes and pointer positions (fake login, keylogging)
  Owner: widget kit and declared actions: content has no input controls or events. HS-03 keeps
  what it draws inside its frame, and HS-13 and HS-14 cover the fields the host draws instead.
- A setting makes a connector reach an address the user did not choose in a host dialog
  Owner: declared actions, with the destination chosen in a host dialog (installation).
