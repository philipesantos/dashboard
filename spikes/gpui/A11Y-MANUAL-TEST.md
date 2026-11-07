# Screen reader test script (VoiceOver, Narrator, Orca)

About 20 minutes per system. It checks what the automated tests cannot: that a real screen reader
announces the spike's grid, frames and approval dialog, and that the whole flow works without a
mouse. Record each step as **pass**, **fail** (what was heard instead) or **n/a**, and paste the
table into REPORT.md, "Accessibility".

## Before you start

1. Get the binary for the system: the `gpui-spike-<system>-gpui-pre` artifact of the latest
   **Spike GPUI** run, or build it (README.md).
2. Start from a clean data directory, so Settings and the approval dialog appear:
   - macOS / Linux: `SPIKE_DATA_DIR=$(mktemp -d) ./dashboard-gpui-spike`
   - Windows (PowerShell): `$env:SPIKE_DATA_DIR = "$env:TEMP\spike-a11y-$(Get-Random)"; .\dashboard-gpui-spike.exe`
3. Turn the screen reader on:
   - **macOS VoiceOver**: Cmd-F5. VO = Control-Option.
   - **Windows Narrator**: Win-Ctrl-Enter. Narrator key = Caps Lock (or Insert).
   - **Linux Orca**: Super-Alt-S (GNOME), or run `orca`. Orca key = Insert (desktop layout).
4. Do not touch the mouse from here on.

## Steps

| # | Do | Expect to hear (wording varies by reader) | macOS | Windows | Linux |
|---|---|---|---|---|---|
| 1 | Start the app. | The window, "Dashboard (GPUI spike)"; then the **Settings dialog**: "Settings, dialog". | | | |
| 2 | Tab through Settings. | "Your color" group; each color as a radio button with its name ("Gold", "Teal" …), then System, Light, Dark, Done. Focus never leaves the dialog. | | | |
| 3 | Press Space on "Teal", then Tab to Done and press Enter. | "Teal, selected" (or checked); the dialog closes and focus returns to the window. | | | |
| 4 | Tab from the start of the toolbar. | "Dashboard, toolbar"; "Add Hello, button"; … "Settings, button". | | | |
| 5 | On "Add Release status", press Enter. | The **approval dialog**: "Add Release status?, dialog"; reading it (VO-A, Narrator Caps-M, Orca Insert-Space then reading) gives "Your color, Teal: this dialog is from Dashboard", the plugin's id, version and author, "From: …", "Permissions, list", "log (required)". | | | |
| 6 | Tab, Shift-Tab several times. | Only "Cancel" and "Approve and add". Focus never reaches the toolbar behind the dialog. | | | |
| 7 | Press Escape. | The dialog closes; nothing was added. | | | |
| 8 | Open it again (Enter on "Add Release status"), Tab to "Approve and add", press Enter. | The dialog closes; focus lands on the new widget: "Release status, plugin local/ui-sampler. Column 1, row 1, 5 by 5 cells", then its hint "Arrow keys move the widget …". | | | |
| 9 | Explore the widget's content (VO-Right; Narrator Caps-Right; Orca Down in browse mode, or Insert-Down). | "Release status, heading"; the sentence below it; "list, 3 items" with each item; "Table: Platform, Build, Tests" with headers and cells by row; "Button pressed 0 times"; "Press, button". | | | |
| 10 | Tab to "Press" and press Space. | The text becomes "Button pressed 1 times" (the reader may or may not announce the change on its own: note which). | | | |
| 11 | Shift-Tab back to the frame; press Down, then Shift-Right. | The frame's name changes to "Column 1, row 2, 5 by 5 cells", then "6 by 5 cells". Note whether the reader announces the change without being asked. | | | |
| 12 | Add "Hello" and place it next to Release status, then press Left on Hello until it hits Release status. | "That place is taken or off the grid." (a status message). Note whether it is announced. | | | |
| 13 | Add "Spoof test" and read it. | Its heading "Dashboard — Approve permissions" and its "Approve" button are read **inside** "Spoof test, plugin local/ui-sampler". Pressing "Approve" only shows the status "A widget asked for "approve"; the host ignores actions it does not know." | | | |
| 14 | Add "Password test" and "Host color test". | Each frame reads an alert: "This widget could not be shown: widget content: unknown variant `password-field` …" (or `host-secret`). | | | |
| 15 | On a frame, press Delete. | The widget goes away; focus moves to the next control. | | | |
| 16 | Open Settings, switch Light / Dark. | The app's colors change; the reader announces the selected button. | | | |

## Also check with the inspectors

- **macOS, Accessibility Inspector** (Xcode → Open Developer Tool): point it at a frame, a table
  cell and the dialog. Roles: AXGroup (frame), AXTable/AXRow/AXCell, AXButton, AXHeading; the
  dialog should show as AXSheet or a dialog subrole. Record the role and whether it is modal.
- **Windows, Accessibility Insights for Windows** (FastPass, then Inspect): run FastPass on the
  window with the dialog open, and record the failures. Check the dialog's control type
  (Window or Pane) and `IsDialog`/`IsModal`.
- **Linux, Accerciser** (`sudo apt install accerciser`): compare with
  `evidence/atspi-linux-wayland-dashboard.txt`, produced by `scripts/atspi-dump.py`.

## Known gaps already found (do not count as new)

- GPUI has no API to mark a dialog **modal** in the accessibility tree (there is no
  `aria_modal`). Keyboard focus is trapped, but a screen reader's virtual cursor may still read
  the dashboard behind the dialog. Step 6 checks focus; please also note whether the virtual
  cursor (VO-arrows, Narrator scan mode, Orca browse mode) can leave the dialog.
- Changes to a frame's position are only reflected in its name; there is no live announcement.
