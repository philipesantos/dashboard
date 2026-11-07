#!/usr/bin/env python3
"""Prints the AT-SPI accessibility tree of a running app on Linux, as a screen reader sees it.

    atspi-dump.py <part of the application or window name> [max depth]

Needs python3-gi with Atspi 2.0, and the accessibility bus enabled (Orca or Accerciser enable it;
`busctl --user set-property org.a11y.Bus /org/a11y/bus org.a11y.Status IsEnabled b true` too).
"""
import sys

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi  # noqa: E402

wanted = sys.argv[1].lower()
max_depth = int(sys.argv[2]) if len(sys.argv) > 2 else 12
INTERESTING = {"focused", "focusable", "selected", "checked", "showing", "modal", "disabled"}


def states(node):
    names = []
    state_set = node.get_state_set()
    for state in state_set.get_states():
        name = state.value_nick
        if name in INTERESTING:
            names.append(name)
    return names


def dump(node, depth):
    if depth > max_depth:
        return
    try:
        role = node.get_role_name()
        name = node.get_name() or ""
        description = node.get_description() or ""
        actions = []
        action = node.get_action_iface()
        if action is not None:
            actions = [Atspi.Action.get_action_name(action, i) for i in range(action.get_n_actions())]
        extra = f" desc={description!r}" if description else ""
        text = node.get_text_iface()
        if text is not None:
            value = Atspi.Text.get_text(text, 0, -1)
            if value and value != name:
                extra += f" text={value!r}"
        extra += f" actions={actions}" if actions else ""
        print(f"{'  ' * depth}[{role}] {name!r}{extra} {states(node)}")
        for i in range(node.get_child_count()):
            dump(node.get_child_at_index(i), depth + 1)
    except Exception as error:  # a node can vanish while we walk
        print(f"{'  ' * depth}<error: {error}>")


desktop = Atspi.get_desktop(0)
apps = [desktop.get_child_at_index(i) for i in range(desktop.get_child_count())]
found = False
for app in apps:
    if app is None:
        continue
    names = [app.get_name() or ""] + [
        (app.get_child_at_index(i).get_name() or "") for i in range(app.get_child_count())
    ]
    if any(wanted in name.lower() for name in names):
        found = True
        dump(app, 0)
if not found:
    print("not found; applications:", [a.get_name() for a in apps if a is not None])
    sys.exit(1)
