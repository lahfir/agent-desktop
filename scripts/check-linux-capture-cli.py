#!/usr/bin/env python3
"""Drive the release `screenshot` command in a private X session and compare each
image with an independent xwd capture. Run through scripts/private-session.sh.
Usage: check-linux-capture-cli.py PATH_TO_AGENT_DESKTOP"""
import json
import os
import socket
import subprocess
import sys
import tempfile
import time

PAINTER = """
import sys, gi
gi.require_version("Gtk", "3.0")
from gi.repository import Gtk, Gdk, GLib
title, color, x, y, w, h, popup = sys.argv[1:8]
kind = Gtk.WindowType.POPUP if popup == "1" else Gtk.WindowType.TOPLEVEL
win = Gtk.Window(type=kind)
win.set_title(title)
win.set_default_size(int(w), int(h))
css = Gtk.CssProvider()
css.load_from_data(("window, window.background { background-color: %s; background-image: none; }" % color).encode())
Gtk.StyleContext.add_provider_for_screen(Gdk.Screen.get_default(), css, Gtk.STYLE_PROVIDER_PRIORITY_USER)
win.move(int(x), int(y))
win.connect("destroy", Gtk.main_quit)
win.show_all()
GLib.timeout_add(500, lambda: (win.move(int(x), int(y)), False)[1])
Gtk.main()
"""

CHILDREN = []
FAILURES = []


def check(name, ok, detail=""):
    print(("PASS " if ok else "FAIL ") + name + (" " + detail if detail else ""))
    if not ok:
        FAILURES.append(name)


def ad(*args, env=None):
    out = subprocess.run([AD, *args], capture_output=True, text=True, timeout=60, env=env)
    reply = json.loads(out.stdout)
    assert reply.get("ok") is True or reply.get("ok") is False, out.stdout
    expected = 0 if reply["ok"] else 1
    assert out.returncode == expected, (out.returncode, out.stdout, out.stderr)
    return reply


def paint(title, color, x, y, w, h, popup=False):
    child = subprocess.Popen(
        ["python3", SCRIPT, title, color, str(x), str(y), str(w), str(h), "1" if popup else "0"],
        stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        start_new_session=True)
    CHILDREN.append(child)
    deadline = time.time() + 40
    while time.time() < deadline:
        if subprocess.run(["xwininfo", "-name", title], capture_output=True).returncode == 0:
            time.sleep(1.5)
            return child
        time.sleep(0.2)
    raise SystemExit("no window " + title)


def listed(title):
    deadline = time.time() + 40
    while time.time() < deadline:
        found = [w for w in ad("list-windows").get("data", []) if w["title"] == title]
        if found:
            time.sleep(1.2)
            return [w for w in ad("list-windows")["data"] if w["title"] == title][0]
        time.sleep(0.3)
    raise SystemExit("not listed " + title)


def xwd_root():
    from PIL import Image
    path = os.path.join(WORK, "root.xwd")
    subprocess.run(["xwd", "-silent", "-root", "-out", path], check=True)
    data = open(path, "rb").read()
    field = lambda i: int.from_bytes(data[i * 4:i * 4 + 4], "big")
    header, colours = field(0), field(19)
    width, height, line, bits = field(4), field(5), field(12), field(11)
    assert field(7) == 0 and field(14) == 0xFF0000 and bits in (24, 32), "unexpected xwd layout"
    start = len(data) - line * height
    return Image.frombuffer("RGB", (width, height), data[start:], "raw", "BGR" if bits == 24 else "BGRX", line, 1)


def shot(*args):
    from PIL import Image
    path = os.path.join(WORK, "shot.png")
    if os.path.exists(path):
        os.remove(path)
    reply = ad("screenshot", *args, path)
    image = Image.open(path).convert("RGB") if reply.get("ok") else None
    return reply, image


def same(image, oracle):
    from PIL import ImageChops
    if image.size != oracle.size:
        print("  size", image.size, oracle.size)
        return False
    box = ImageChops.difference(image, oracle).getbbox()
    if box is not None:
        print("  differs in", box, "of", image.size)
        for point in [(box[0], box[1]), ((box[0] + box[2]) // 2, (box[1] + box[3]) // 2), (box[2] - 1, box[3] - 1)]:
            print("   at", point, image.getpixel(point), oracle.getpixel(point))
    return box is None


_COMPOSITING = []


def compositing():
    if not _COMPOSITING:
        reply, _ = shot()
        _COMPOSITING.append(reply.get("error", {}).get("details", {}).get("reason") == "compositing")
    return _COMPOSITING[0]


def window_leg():
    paint("adcli-red", "#ff0000", 200, 150, 320, 200)
    window = listed("adcli-red")
    reply, image = shot("--window-id", window["id"])
    check("window screenshot ok", reply["ok"], str(reply.get("error")))
    b = window["bounds"]
    if compositing():
        check("window image is at least the reported bounds", image.size[0] >= round(b["width"]) and image.size[1] >= round(b["height"]), str((image.size, b)))
    else:
        check("window size equals the reported bounds", image.size == (round(b["width"]), round(b["height"])), str((image.size, b)))
    if not compositing():
        crop = xwd_root().crop((round(b["x"]), round(b["y"]), round(b["x"] + b["width"]), round(b["y"] + b["height"])))
        check("window image equals the xwd root crop", same(image, crop))
    cx, cy = image.size[0] // 2, image.size[1] // 2
    check("window centre is red", image.getpixel((cx, cy)) == (255, 0, 0))
    return window, image


def covered_leg(window, before):
    paint("adcli-cover", "#0000ff", 100, 100, 600, 450, popup=True)
    if not compositing():
        root = xwd_root()
        check("oracle: the cover hides the window on the screen", root.getpixel((360, 250)) == (0, 0, 255))
    reply, image = shot("--window-id", window["id"])
    check("covered window screenshot ok", reply["ok"], str(reply.get("error")))
    check("covered window equals its uncovered image", image is not None and same(image, before))
    check("covered image holds no cover colour", image is not None and (0, 0, 255) not in {c for _, c in image.getcolors(10 ** 6)})


def full_screen_leg():
    reply, image = shot()
    if compositing():
        err = reply.get("error", {})
        check("compositing full screen refused", err.get("code") == "PLATFORM_NOT_SUPPORTED" and err.get("details", {}).get("reason") == "compositing", str(err))
        reply0, _ = shot("--screen", "0")
        check("compositing --screen 0 refused", reply0.get("error", {}).get("details", {}).get("reason") == "compositing")
        return
    check("full screen ok", reply["ok"], str(reply.get("error")))
    check("full screen equals the xwd root", same(image, xwd_root()))
    check("full screen shows the cover", image.getpixel((360, 250)) == (0, 0, 255))
    reply0, image0 = shot("--screen", "0")
    check("--screen 0 equals the xwd root", reply0["ok"] and same(image0, xwd_root()))
    reply9, _ = shot("--screen", "9")
    check("--screen 9 is INVALID_ARGS", reply9.get("error", {}).get("code") == "INVALID_ARGS")


def off_screen_leg():
    paint("adcli-edge", "#ff0000", 1200, 700, 320, 200)
    window = listed("adcli-edge")
    b = window["bounds"]
    if b["x"] + b["width"] <= 1280 and b["y"] + b["height"] <= 800:
        print("SKIP off_screen: the manager kept the window on the screen", b)
        return
    reply, _ = shot("--window-id", window["id"])
    err = reply.get("error", {})
    check("off-screen window refused with off_screen", err.get("details", {}).get("reason") == "off_screen", str(err))


def permissions_leg():
    reply = ad("permissions")
    state = reply.get("data", {}).get("screen_recording", {}).get("state")
    check("X11 screen_recording is not_required", state == "not_required", str(reply))
    path = os.path.join(WORK, "wayland-socket")
    listener = socket.socket(socket.AF_UNIX)
    listener.bind(path)
    listener.listen(4)
    env = dict(os.environ, WAYLAND_DISPLAY=path)
    reply = ad("permissions", env=env)
    state = reply.get("data", {}).get("screen_recording", {}).get("state")
    check("Wayland screen_recording is unknown", state == "unknown", str(reply))
    listener.close()


def main():
    global AD, WORK, SCRIPT
    AD = sys.argv[1]
    WORK = tempfile.mkdtemp(prefix="adcap-cli-", dir=os.environ["AGENT_DESKTOP_PRIVATE_SESSION"])
    SCRIPT = os.path.join(WORK, "painter.py")
    with open(SCRIPT, "w") as painter:
        painter.write(PAINTER)
    try:
        print("compositing =", compositing())
        window, before = window_leg()
        covered_leg(window, before)
        full_screen_leg()
        off_screen_leg()
        permissions_leg()
    finally:
        for child in CHILDREN:
            try:
                os.killpg(child.pid, 9)
            except ProcessLookupError:
                pass
    print("FAILURES:", FAILURES)
    sys.exit(1 if FAILURES else 0)


if __name__ == "__main__":
    main()
