# Host surfaces

What only the host draws, and how each part is checked. A host dialog is told apart from
everything a widget supplies by its form, not by its colors or by a sign the person must remember
to look for: it covers the host's chrome and blocks every host window, which no widget frame,
panel or declared control can do (constitution v5.0.0, Principles V and VII). The colors and
sizes come from [`DESIGN.md`](../../DESIGN.md); the surfaces' tokens are in the `host-surfaces`
crate, which only the host may depend on.

Each criterion below is something a later Phase 3 feature turns into a test. **Observe** is what
the test measures, **Expect** the result that passes, **Answers** the constitution rule and the
threat-model rows (quoted exactly from [the threat model](../threat-model.md)) it answers, and
**Tested by** the feature that writes the test. `design-gen`'s `criteria` test checks that every
Phase 3 row on boundary B4 is answered here or decided elsewhere.

## Chrome

### HS-01 The chrome is never covered by a widget frame or panel
- **Observe**: the bounds of the chrome and of every widget frame and frame panel, at every window
  size the grid allows and while frames are moved or resized.
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
- **Observe**: the host's windows and the accessibility tree before and after loading, rendering
  and updating a hostile widget that asks for a window, a popup, a dialog or a notification.
- **Expect**: the set of host windows is unchanged and no node with a dialog or alert-dialog role
  appears; the request is refused or ignored and logged, escaped.
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
- **Observe**: the label drawn on a frame for plugin names holding control characters,
  bidirectional overrides and isolates, combining marks only, more than 40 visible characters, and
  names like "Dashboard" or "System".
- **Expect**: control and bidirectional formatting characters are removed, the name is capped at
  40 visible characters followed by an ellipsis, it is drawn in the host's own font and the
  frame-label style, and the frame's accessible name is the same text.
- **Answers**: V
  - Password field or drawing over the frame (fake login)
- **Tested by**: widget kit

### HS-07 The monogram is the host's, from the name
- **Observe**: the monogram drawn for the same names as HS-06, and for names starting with an
  emoji, a digit, a symbol or a letter of a non-Latin script.
- **Expect**: the first letter or digit of the cleaned name, upper-cased where the script has case,
  or a fixed host glyph when there is none; drawn in the neutral roles of the monogram component,
  never in a color the plugin chose; no image or icon from the plugin (decision 0001).
- **Answers**: V
  - Password field or drawing over the frame (fake login)
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
- **Observe**: the pixels outside an open panel, and pointer and keyboard input sent to other
  frames and to the chrome while it is open.
- **Expect**: nothing outside the panel changes color, and the other frames and the chrome still
  receive their input.
- **Answers**: V
  - A plugin's settings panel passes for the host's or collects a secret: a label like "GitHub password", a masked field, a number field for a code, a panel that dims or blocks the window
- **Tested by**: declared actions

## Host dialogs

### HS-10 A host dialog blocks every host window
- **Observe**: pointer and keyboard input sent, while a dialog is open, to every host window,
  including windows other than the one the dialog belongs to, and to every frame and panel.
- **Expect**: no input outside the dialog reaches anything; a click outside the dialog does not
  close or answer it.
- **Answers**: VII
  - Something that is not a host dialog passes for one: the operating system's native prompt, or a widget drawing one
- **Tested by**: host dialogs

### HS-11 A new dialog ignores input for its first second
- **Observe**: clicks on a dialog's buttons and presses of Enter, Space and Escape sent 0 ms, 500 ms,
  999 ms and 1000 ms after the dialog appears, and a key held down from before it appeared.
- **Expect**: nothing sent before 1000 ms answers the dialog, and a key already down when it
  appeared answers nothing until it is released and pressed again.
- **Answers**: VII
  - A dialog appears under the user's pointer or keystroke, so input meant for something else answers it
- **Tested by**: host dialogs

### HS-12 Trust decisions never use a native prompt
- **Observe**: the operating system's dialog APIs called by the host process while granting a
  permission, installing a plugin or connector, connecting an account and choosing a destination.
- **Expect**: none; each decision is asked in a host dialog. The provider's sign-in page in the
  browser and the OS keychain's own prompts are not counted.
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
- **Expect**: they appear only below the frame-label of the widget that declared them, in the
  kit's styles, and their nodes are descendants of the panel, itself attached to that frame.
- **Answers**: V
  - A plugin's settings panel passes for the host's or collects a secret: a label like "GitHub password", a masked field, a number field for a code, a panel that dims or blocks the window
- **Tested by**: declared actions

## Decided elsewhere

- Widget content imitates a host dialog inside its frame, with placed declared actions as its buttons ("Allow access?")
  Owner: declared actions. HS-01 to HS-03 keep such a fake inside its frame and under the chrome,
  but the buttons are the widget's own actions; whether the host writes the effect and
  destination on every control of an action that changes data, and whether a destructive action
  always asks for a host dialog, is decided in that spec (threat model, open questions).
- Widget content makes the host fetch a remote resource or a local file, or open a URL (an image address, a link)
  Owner: widget kit, which maps content only to host components, takes images only as bytes and
  never passes a plugin string to GPUI's `img()` or `open_url`.
- Crafted image, SVG or font exploits a decoder, which now runs in the host process beside the vault
  Owner: the host window's plan, which revisits decision 0001; until then no image block or
  plugin icon exists, and the only fonts are the host's own.
- Asks for little at install, then presses the user to turn on optional permissions one at a time
  Owner: installation, whose dialogs warn on every grant change.
- Value outside its declared type, or a parameter type the host cannot draw, reaches the plugin
  Owner: declared actions.
- Content draws its own field or reads keystrokes and pointer positions (fake login, keylogging)
  Owner: widget kit and declared actions: content has no input controls or events; HS-03 to HS-05
  keep what it draws inside its frame.
- A setting makes a connector reach an address the user did not choose in a host dialog
  Owner: declared actions, with the destination chosen in a host dialog (installation).
