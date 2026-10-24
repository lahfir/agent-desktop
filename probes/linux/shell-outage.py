#!/usr/bin/env python3
"""Compare uncached D-Bus shell reads with libatspi across notification changes."""
import json
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
import subprocess
import time

import gi

gi.require_version("Atspi", "2.0")
from gi.repository import Atspi, Gio, GLib

session = Gio.bus_get_sync(Gio.BusType.SESSION, None)


def call(bus, name, path, interface, method, signature=None, args=()):
    body = GLib.Variant(signature, args) if signature else None
    return bus.call_sync(name, path, interface, method, body, None,
                         Gio.DBusCallFlags.NONE, 2000, None).unpack()


address = call(session, "org.a11y.Bus", "/org/a11y/bus", "org.a11y.Bus", "GetAddress")[0]
a11y = Gio.DBusConnection.new_for_address_sync(
    address, Gio.DBusConnectionFlags.AUTHENTICATION_CLIENT |
    Gio.DBusConnectionFlags.MESSAGE_BUS_CONNECTION, None, None)
shell_pid = call(session, "org.freedesktop.DBus", "/org/freedesktop/DBus",
                 "org.freedesktop.DBus", "GetConnectionUnixProcessID",
                 "(s)", ("org.gnome.Shell",))[0]
accessible = "org.a11y.atspi.Accessible"
root_path = "/org/a11y/atspi/accessible/root"
known = None
clear_button = None
readers = ThreadPoolExecutor(max_workers=2)
clock_origin = time.monotonic()


def direct(name, path):
    def read(interface, method, signature=None, args=()):
        started = time.monotonic()
        try:
            value = call(a11y, name, path, interface, method, signature, args)[0]
            return {"value": value, "ms": round((time.monotonic() - started) * 1000),
                    "start_ms": round((started - clock_origin) * 1000)}
        except GLib.Error as error:
            return {"error": str(error), "ms": round((time.monotonic() - started) * 1000),
                    "start_ms": round((started - clock_origin) * 1000)}

    title = readers.submit(read, "org.freedesktop.DBus.Properties", "Get",
                           "(ss)", (accessible, "Name"))
    children = readers.submit(read, accessible, "GetChildren")
    title, children = title.result(), children.result()
    windows = []
    role_errors = []
    for child_name, child_path in children.get("value", []):
        try:
            if call(a11y, child_name, child_path, accessible, "GetRole")[0] == 69:
                windows.append((child_name, child_path))
        except GLib.Error as error:
            role_errors.append(str(error))
    return {"name_read": title, "children_read": children, "windows": windows,
            "role_errors": role_errors, "bus": name, "pid": shell_pid}


def raw():
    global known
    apps = call(a11y, "org.a11y.atspi.Registry", root_path, accessible, "GetChildren")[0]
    found = []
    for name, path in apps:
        try:
            pid = call(a11y, "org.freedesktop.DBus", "/org/freedesktop/DBus",
                       "org.freedesktop.DBus", "GetConnectionUnixProcessID", "(s)", (name,))[0]
            if pid == shell_pid:
                read = direct(name, path)
                if read.get("windows"):
                    known = (name, path)
                found.append(read)
        except GLib.Error:
            continue
    return {"apps": found, "registry_entries": len(apps)}


def cached():
    desktop = Atspi.get_desktop(0)
    found = []
    for index in range(desktop.get_child_count()):
        app = desktop.get_child_at_index(index)
        if app.get_process_id() == shell_pid:
            before = (app.get_name(), app.get_child_count())
            app.clear_cache()
            found.append({"before": before, "after_clear": (app.get_name(), app.get_child_count())})
    return found


def message_list_showing():
    global clear_button
    if clear_button is None:
        desktop = Atspi.get_desktop(0)
        stack = [desktop.get_child_at_index(i) for i in range(desktop.get_child_count())]
        stack = [app for app in stack if app.get_process_id() == shell_pid]
        for _ in range(4000):
            if not stack:
                break
            node = stack.pop()
            if node.get_name() == "Clear all notifications":
                clear_button = node
                break
            stack.extend(node.get_child_at_index(i) for i in range(max(0, node.get_child_count())))
    if clear_button is None:
        return None
    clear_button.clear_cache()
    return clear_button.get_state_set().contains(Atspi.StateType.SHOWING)


def notify(index):
    call(session, "org.freedesktop.Notifications", "/org/freedesktop/Notifications",
         "org.freedesktop.Notifications", "Notify", "(susssasa{sv}i)",
         ("shell-outage-probe", 0, "", f"outage {index}", "independent sender", [], {}, 0))


def toggle():
    import os
    if os.environ.get("WAYLAND_DISPLAY"):
        service = "org.gnome.Mutter.RemoteDesktop"
        path = call(session, service, "/org/gnome/Mutter/RemoteDesktop", service, "CreateSession")[0]
        interface = service + ".Session"
        call(session, service, path, interface, "Start")
        for key, down in [(0xffe1, True), (0xffe1, False), (0xffeb, True),
                          (ord("v"), True), (ord("v"), False), (0xffeb, False)]:
            call(session, service, path, interface, "NotifyKeyboardKeysym", "(ub)", (key, down))
        return path
    subprocess.run(["xdotool", "key", "super+v"], check=True)
    return None


ready_by = time.monotonic() + 30
while True:
    cached()
    if any(app.get("windows") for app in raw()["apps"]):
        break
    assert time.monotonic() < ready_by, "shell window readiness by raw D-Bus"
    time.sleep(0.2)
started = time.monotonic()
remote = None
for index in range(200):
    next_sample = time.monotonic() + 0.2
    if index in (5, 25, 60):
        notify(index)
    if index in (10, 100):
        remote = toggle()
    result = raw()
    stat = Path(f"/proc/{shell_pid}/stat").read_text().rsplit(")", 1)[1].split()
    print(json.dumps({"ms": round((time.monotonic() - started) * 1000),
                      "session_pid": call(session, "org.freedesktop.DBus", "/org/freedesktop/DBus",
                                          "org.freedesktop.DBus", "GetConnectionUnixProcessID",
                                          "(s)", ("org.gnome.Shell",))[0],
                      "process_state": stat[0], "process_start": stat[19],
                      "raw": result, "direct": direct(*known) if known else None,
                      "libatspi": cached(), "message_list_showing": message_list_showing()}), flush=True)
    if remote:
        call(session, "org.gnome.Mutter.RemoteDesktop", remote,
             "org.gnome.Mutter.RemoteDesktop.Session", "Stop")
        remote = None
    time.sleep(max(0, next_sample - time.monotonic()))
