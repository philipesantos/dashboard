#!/usr/bin/env python3
"""Measures WebKitGTK 4.1, the engine Tauri 2 uses on Linux, on the GPUI spike's dashboard as a
web page (webkitgtk-bench.html): first frame, the GL renderer WebKit uses, and the same drag as
the spike's SPIKE_DRAG_BENCH. Prints lines like the spike's.

    webkitgtk-bench.py [--drag] [--exit-after SECONDS]

Needs python3-gi with WebKit2 4.1 and Gtk 3.0 (gir1.2-webkit2-4.1), as Tauri does.
"""
import os
import sys
import time

START = time.monotonic()

import gi  # noqa: E402

gi.require_version("Gtk", "3.0")
gi.require_version("WebKit2", "4.1")
from gi.repository import GLib, Gtk, WebKit2  # noqa: E402

drag = "--drag" in sys.argv
exit_after = None
if "--exit-after" in sys.argv:
    exit_after = float(sys.argv[sys.argv.index("--exit-after") + 1])


def cpu_seconds():
    total = 0.0
    for pid in tree(os.getpid()):
        try:
            with open(f"/proc/{pid}/stat") as f:
                fields = f.read().rsplit(")", 1)[1].split()
            total += (int(fields[11]) + int(fields[12])) / os.sysconf("SC_CLK_TCK")
        except OSError:
            pass
    return total


def tree(pid):
    pids = [pid]
    for p in pids:
        try:
            for task in os.listdir(f"/proc/{p}/task"):
                with open(f"/proc/{p}/task/{task}/children") as f:
                    pids.extend(int(c) for c in f.read().split())
        except OSError:
            pass
    return pids


state = {"cpu": None}


def on_message(_manager, result):
    value = result.get_js_value() if hasattr(result, "get_js_value") else result
    message = value.to_json(0)
    import json

    data = json.loads(message)
    if data["kind"] == "first-frame":
        print(f"webkitgtk: first frame presented {int((time.monotonic() - START) * 1000)} ms after start; GL renderer: {data['renderer']}", flush=True)
        if drag:
            GLib.timeout_add(1500, start_drag)
    elif data["kind"] == "drag":
        frames = sorted(data["frames"])
        t = data["seconds"]
        pick = lambda q: frames[int((len(frames) - 1) * q)]
        slow = sum(1 for f in frames if f > 1000 / 30)
        cpu = (cpu_seconds() - state["cpu"]) / t * 100
        print(
            f"webkitgtk: drag bench {len(frames)} frames in {t:.1f}s ({len(frames) / t:.1f} fps); frame ms p50 {pick(.5):.1f} "
            f"p95 {pick(.95):.1f} p99 {pick(.99):.1f} max {frames[-1]:.1f}; over 33 ms: {slow}; CPU {cpu:.0f}% (all processes)",
            flush=True,
        )
        Gtk.main_quit()


def start_drag():
    state["cpu"] = cpu_seconds()
    view.evaluate_javascript("startDrag()", -1, None, None, None, None, None)
    return False


manager = WebKit2.UserContentManager()
manager.connect("script-message-received::bench", on_message)
manager.register_script_message_handler("bench")
view = WebKit2.WebView.new_with_user_content_manager(manager)
settings = view.get_settings()
settings.set_hardware_acceleration_policy(WebKit2.HardwareAccelerationPolicy.ALWAYS)
window = Gtk.Window(title="Dashboard (WebKitGTK bench)")
window.set_default_size(1280, 820)
window.add(view)
window.connect("destroy", Gtk.main_quit)
here = os.path.dirname(os.path.abspath(__file__))
view.load_uri("file://" + os.path.join(here, "webkitgtk-bench.html"))
window.show_all()
if exit_after:
    GLib.timeout_add(int(exit_after * 1000), Gtk.main_quit)
Gtk.main()
