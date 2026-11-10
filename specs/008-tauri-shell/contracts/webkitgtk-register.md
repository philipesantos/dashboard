# Contract: The WebKitGTK Register

`docs/webkitgtk-register.toml` lists every change made only because Linux uses the system
WebKitGTK (spec FR-022, research R15). When Linux moves to the embedded engine, each entry is
removed or replaced as its `removal` says, together with its markers, until the list is empty.

## Format

```toml
# Every change made only because Linux uses the system WebKitGTK (spec 008, FR-022).
# Mark each place with WEBKITGTK-ONLY(<id>) in a comment, and list it here with its files.
# The test in app/src-tauri/tests/webkitgtk_register.rs fails when the two disagree.
# The move to the embedded engine works through this list until it is empty.

[[entry]]
id = "ci-dmabuf"
files = [".github/workflows/ci.yml"]
reason = "WebKitGTK's DMA-BUF renderer fails under xvfb, so the probe step turns it off."
removal = "Delete the variable."
```

| Field | Rule |
| --- | --- |
| `id` | Kebab-case (`[a-z0-9-]+`), unique. |
| `files` | One or more paths from the repository root. Each exists and holds the marker. |
| `reason` | Non-empty: what WebKitGTK does that makes the change necessary. |
| `removal` | Non-empty: what the move to the embedded engine does with it: "Delete", "Replace with …", or, for a tripwire match that stays, "Keep". |

## The marker

`WEBKITGTK-ONLY(<id>)`, inside a comment, on or just above the change. A change spread over
several lines in one place needs one marker. The test reads the files `git ls-files` lists,
apart from `specs/` and the register itself.

## The tripwire

A marker someone forgot cannot be matched against the register, so the test also looks for the
plain signs of a WebKitGTK-only change. In the tracked files under `app/` (apart from
`app/src-tauri/gen/`) and `.github/`, a line that matches one of these, ignoring case, needs a
marker on the same line or within the three lines above it:

| Sign | Pattern |
| --- | --- |
| The engine or its crate | `webkit` (covers `webkit2gtk`, `WebKitGTK`, `libwebkit2gtk`) |
| Its environment variables | `WEBKIT_` |
| A Linux-only build condition in the app | `target_os = "linux"`, `cfg!(target_os` |
| Engine sniffing in the UI | `userAgent`, `navigator.platform` |

A match that is not a WebKitGTK-only change, such as a Linux condition the embedded engine also
needs, still gets an entry, with "Keep" as its removal and the reason it stays. The tripwire
catches what names the engine. It cannot catch a change that does not, such as a CSS value
picked because WebKitGTK draws it differently, so review still asks about those.

## Failure lines

One line per difference, then a non-zero exit:

```text
docs/webkitgtk-register.toml: marker "x" in app/src-tauri/src/probe.rs has no entry
docs/webkitgtk-register.toml: entry "x" has no marker in .github/workflows/ci.yml
docs/webkitgtk-register.toml: entry "x" names app/missing.rs, which does not exist
docs/webkitgtk-register.toml: entry "x" has no reason
docs/webkitgtk-register.toml: entry "x" has no removal
docs/webkitgtk-register.toml: entry "x" is listed twice
docs/webkitgtk-register.toml: entry "X_Y" is not kebab-case
app/src/Widget.tsx:42: names the engine ("userAgent") with no WEBKITGTK-ONLY marker
```
