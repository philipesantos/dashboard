# Contract: the host dialog

## Asking

```rust
// in host::dialog
pub struct Request {
    pub name: HostText,            // the host's name for the decision
    pub question: HostText,        // in the host's words
    pub subject: Option<PluginText>,
    pub refuse: HostText,          // first control, initial focus
    pub confirm: HostText,         // last control
}
pub enum Answer { Confirmed, Refused }

impl DialogHost {
    /// Queues the request; at most one dialog is shown (FR-018).
    pub fn ask(cx: &mut App, window: AnyWindowHandle, request: Request) -> Task<Answer>;
}
```

Nothing else in the host may draw a scrim or take every window's input; `ask` is the only way a
host decision is asked (FR-014).

## Form (each line is a test)

| Rule | Criterion |
| --- | --- |
| The scrim covers every host window entirely, chrome included; the dialog sits above it | HS-02 |
| No pointer or key input outside the dialog reaches anything, in any host window | HS-10 |
| A click outside neither closes nor answers it | HS-10 |
| A window's close request or the app's quit answers `Refused` first | HS-10 |
| Input answers nothing before 1000 ms; at 1000 ms it does; held keys and buttons wait for release | HS-11 |
| The 1000 ms restart on window activation, content change, or a new dialog | HS-11 |
| Initial focus on `refuse`; no default button; Enter there or in a field never confirms | HS-16 |
| Tab and Shift+Tab cycle inside; Escape answers `Refused` and restores the previous focus | HS-17 |
| Every control shows the focus ring: `focus` role, 2 px wide, 2 px outside | HS-24 |
| No native prompt: GPUI's prompts and dialogs are banned and its prompt builder refuses | HS-12 |

## For assistive technology

| Node | Properties |
| --- | --- |
| each window's background root (chrome and content area) | role `Pane`; `hidden` while a dialog is open (HS-18) |
| the dialog | role `Dialog`, `modal`, label = `name`, description = `question` (HS-19) |
| the subject | a child node after the question, label = the plugin's name |
| the controls | role `Button`, labels = `refuse` and `confirm` |

While a dialog is open, the platform tree of every host window (AT-SPI, UI Automation, AX) and
`accesskit_consumer`'s `common_filter` over the window's tree reach only the dialog and its
children.

## The first use: "Remove from dashboard"

`name` = "Remove a widget"; `question` = "Remove this widget from the dashboard? Its permissions are
withdrawn and the data the host kept for it is deleted. Its own storage stays."; `subject` = the
plugin's label; `refuse` = "Cancel"; `confirm` = "Remove". Confirmed: `GrantStore::remove` and
`Runtime::discard_kept_data`, as `runtime remove`; the frame leaves. Refused: nothing changes.
