// Copyright 2024 - 2026 Longbridge <https://longbridge.com>
// Licensed under the Apache License, Version 2.0; see LICENSE-APACHE beside this file.
//
// Adapted from gpui-base 0.9.0, src/root.rs (advance_focus, the Tab and TabPrev actions and their
// key bindings), for gpui-unofficial 1.25.3 (see PROVENANCE.md): the copy binding and the rest of
// gpui-base's root are left out, and the key context is the host's.

use gpui::{App, KeyBinding, Window, actions};

actions!(host_root, [Tab, TabPrev]);

/// The key context the host's root view declares, where Tab and Shift+Tab are bound.
pub const CONTEXT: &str = "HostRoot";

/// Scan at most one cycle of the rendered tab order, preserving containment
/// regardless of how many controls exist behind a modal. GPUI exposes traversal
/// but not candidate enumeration. This scan is synchronous: only the final
/// focus path is published at the next draw, not the background candidates.
pub fn advance_focus(window: &mut Window, cx: &mut App, advance: fn(&mut Window, &mut App)) {
    let Some(container) = super::focus_trap::active_focus_trap(window, cx) else {
        advance(window, cx);
        return;
    };

    advance(window, cx);
    // GPUI's tab order can cycle short of the whole window (a stop whose
    // previous stop is itself), so the walk ends at the first repeated stop,
    // not only on a return to the one it began with.
    let mut visited = Vec::new();
    while !container.contains_focused(window, cx) {
        let focused = window.focused(cx);
        if visited.contains(&focused) {
            // No tab stop in the trap is reachable. Its own handle remains a
            // safe focus target even when excluded from ordinary tab order.
            container.focus(window, cx);
            break;
        }
        visited.push(focused);
        advance(window, cx);
    }
}

pub(crate) fn init(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("tab", Tab, Some(CONTEXT)),
        KeyBinding::new("shift-tab", TabPrev, Some(CONTEXT)),
    ]);
}
