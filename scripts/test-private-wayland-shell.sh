#!/usr/bin/env bash
# Checks the documented private Wayland shell gives a command a working
# screenshot portal: the GNOME backend must expose the Screenshot interface the
# frontend forwards to, or every request is refused at once with response 2.
set -euo pipefail
cd "$(dirname "$0")/.."

check=$(cat <<'PY'
import sys
import gi
from gi.repository import Gio

bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
reply = bus.call_sync("org.freedesktop.impl.portal.desktop.gnome",
                      "/org/freedesktop/portal/desktop",
                      "org.freedesktop.DBus.Introspectable", "Introspect",
                      None, None, Gio.DBusCallFlags.NONE, 3000, None)
text = reply.unpack()[0]
if "org.freedesktop.impl.portal.Screenshot" not in text:
    print("the portal backend exposes no Screenshot interface", file=sys.stderr)
    sys.exit(1)
print("the portal backend exposes Screenshot")
PY
)

bash scripts/private-session.sh -- bash scripts/private-wayland-shell.sh -- \
  python3 -c "$check"
