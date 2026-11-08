# Contract: `docs/design/host-surfaces.md`

The frame and dialog criteria (FR-018–FR-021, R10). The coverage test in `design-gen` reads this
file and `docs/threat-model.md`.

## Shape

```markdown
# Host surfaces

<lead: what only the host draws, and why it is told apart by form, not by color>

## Chrome
### HS-01 The chrome is never covered by a widget frame or panel
- **Observe**: the bounds of the chrome and of every frame and panel, at every window size the
  grid allows.
- **Expect**: no frame or panel intersects the chrome.
- **Answers**: V; "Password field or drawing over the frame (fake login)"; "Something that is not
  a host dialog passes for one: the operating system's native prompt, or a widget drawing one"
- **Tested by**: host window

## Widget frame
## Frame panel
## Host dialogs
## Fields the host draws for a plugin
## Keyboard and screen readers

## Decided elsewhere
- "Widget content imitates a host dialog inside its frame, with placed declared actions as its
  buttons (\"Allow access?\")": the control of an action that changes data, and whether a
  destructive action asks for a host dialog, are decided in the Phase 3 spec for declared actions
  and settings (threat model, open questions).
```

## Rules the coverage test enforces

1. Every `### HS-NN` heading is unique and followed by the four fields, each non-empty.
2. Every threat row quoted in **Answers** or under "Decided elsewhere" matches the first cell of
   a row of the threat model's table, exactly.
3. Every row of that table whose boundary includes B4 and whose "Test from" names Phase 3 is
   quoted by at least one criterion or under "Decided elsewhere". Rows that are not about the
   host's surfaces go under "Decided elsewhere" with the feature that owns them: the decoder row
   (decision 0001, revisited in a later Phase 3 plan), content that makes the host fetch or open
   something (widget kit), escalation one optional permission at a time (installation), values
   outside their declared type, content drawing its own field and settings that become
   destinations (declared actions).
4. **Tested by** names one of: `host window`, `widget kit`, `host dialogs`, `declared actions`,
   `installation`, `dashboard grid` (the Phase 3 items of the guide).

## Criteria the file must hold (FR-019, FR-020)

At least one each for: the chrome never covered by a frame or panel; the chrome covered by every
host dialog; every host window blocked while a dialog is open; focus kept inside a dialog;
pointer input outside it ignored; only the dialog exposed to assistive technology; no
plugin-supplied node with a dialog role; nothing drawn outside a frame; the panel attached to its
frame, never dimming or blocking; the field note on every typed field, bound for assistive
technology, and no masked field; the 1000 ms input delay and held keys (R11); the frame's label
(control and bidirectional characters removed, length cap, host font) and monogram (first letter
or digit, upper-cased, fixed host glyph otherwise, neutral colors); native prompts never used;
initial focus on the first control that is not the one that confirms; Escape as refusal; what a
screen reader announces; how the host makes up for GPUI's missing modal state in the
accessibility tree; tests with VoiceOver, Narrator and Orca and real key presses in a real
window.
