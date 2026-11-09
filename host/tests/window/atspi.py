"""The accessibility tree of a host window through AT-SPI, for host/tests/window.rs.

`dump TITLE` prints one line per node of the window titled TITLE: `role<TAB>name`.
`find TITLE NAME` prints `x y width height` in screen pixels of the first node named NAME.
"""
import sys

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi  # noqa: E402


def windows(title):
    desktop = Atspi.get_desktop(0)
    for a in range(desktop.get_child_count()):
        app = desktop.get_child_at_index(a)
        if app is None:
            continue
        for w in range(app.get_child_count()):
            window = app.get_child_at_index(w)
            if window is not None and window.get_name() == title:
                yield window


def walk(node, depth=0):
    if node is None or depth > 40:
        return
    yield node
    for i in range(node.get_child_count()):
        yield from walk(node.get_child_at_index(i), depth + 1)


def main():
    command, title = sys.argv[1], sys.argv[2]
    for window in windows(title):
        for node in walk(window):
            name = node.get_name() or ""
            if command == "dump":
                print(f"{node.get_role_name()}\t{name}")
            elif command == "find" and name == sys.argv[3]:
                box = node.get_extents(Atspi.CoordType.SCREEN)
                print(f"{box.x} {box.y} {box.width} {box.height}")
                return


main()
