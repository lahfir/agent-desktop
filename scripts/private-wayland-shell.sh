#!/usr/bin/env bash
# Runs a command against a private headless gnome-shell Wayland session.
# Usage: scripts/private-session.sh -- scripts/private-wayland-shell.sh [--] command [args...]
# The shell is started inside the private session's runtime dir and bus with --no-x11: the
# Xwayland start-up is what hangs the shell two starts in three, with its socket present and
# its services unanswering (ledger L28-10, L28-28), and without it the first RemoteDesktop
# answer comes within a second. A shell is kept only once five RemoteDesktop sessions in a row
# are created, started and stopped in time, and the Activities Overview, which a fresh shell opens
# and which keeps every window from taking focus, is closed with an Escape key; one that fails the gate is killed by its process
# group and started again.
# WAYLAND_DISPLAY names the shell's socket for the command; the command is not run otherwise.
set -euo pipefail

[ -n "${AGENT_DESKTOP_PRIVATE_SESSION:-}" ] || {
  echo "private-wayland-shell: run it through scripts/private-session.sh" >&2
  exit 2
}
no_input=0
[ "${1:-}" != "--no-input" ] || { no_input=1; shift; }
[ "${1:-}" = "--" ] && shift
[ $# -gt 0 ] || { echo "usage: private-wayland-shell.sh [--] command [args...]" >&2; exit 2; }

rt=$XDG_RUNTIME_DIR
attempts=8
health_reads=5
shell_pid=

sockets() { find "$rt" -maxdepth 1 -name 'wayland-*' ! -name '*.lock' -printf '%f\n' | sort; }

stop_shell() {
  if [ -n "$shell_pid" ]; then
    kill -KILL -- "-$shell_pid" 2>/dev/null || true
    wait "$shell_pid" 2>/dev/null || true
    shell_pid=
    sleep 2
  fi
}
trap stop_shell EXIT

healthy() {
  python3 - "$health_reads" <<'PY'
import sys, time
import gi
from gi.repository import Gio, GLib

reads = int(sys.argv[1])
service = "org.gnome.Mutter.RemoteDesktop"
root = "/org/gnome/Mutter/RemoteDesktop"
timeout_ms = 3000
bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)

def call(path, interface, method):
    return bus.call_sync(service, path, interface, method, None, None,
                         Gio.DBusCallFlags.NONE, timeout_ms, None)

try:
    for _ in range(reads):
        session = call(root, service, "CreateSession").unpack()[0]
        call(session, service + ".Session", "Start")
        call(session, service + ".Session", "Stop")
        time.sleep(0.4)
except GLib.Error:
    sys.exit(1)
PY
}

start_portals() {
  python3 - "$WAYLAND_DISPLAY" <<'PY'
import os
import subprocess
import sys
import time
import gi
from gi.repository import Gio, GLib

sock = sys.argv[1]
bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)

def call(path, method, params=None):
    return bus.call_sync("org.freedesktop.DBus", path, "org.freedesktop.DBus", method,
                         params, None, Gio.DBusCallFlags.NONE, 3000, None)

def owner_pid(name):
    reply = call("/org/freedesktop/DBus", "GetConnectionUnixProcessID",
                 GLib.Variant("(s)", (name,)))
    return reply.unpack()[0]

def name_owned(name):
    reply = call("/org/freedesktop/DBus", "NameHasOwner", GLib.Variant("(s)", (name,)))
    return reply.unpack()[0]

def stop(name):
    try:
        pid = owner_pid(name)
    except GLib.Error:
        return
    try:
        os.kill(pid, 9)
    except OSError:
        pass

def wait_owned(name):
    for _ in range(150):
        if name_owned(name):
            return True
        time.sleep(0.2)
    return False

for name in ("org.freedesktop.portal.Desktop", "org.freedesktop.impl.portal.desktop.gnome"):
    stop(name)
time.sleep(1)
activation = {"WAYLAND_DISPLAY": sock, "GDK_BACKEND": "wayland", "XDG_SESSION_TYPE": "wayland"}
call("/org/freedesktop/DBus", "UpdateActivationEnvironment", GLib.Variant("(a{ss})", (activation,)))
env = dict(os.environ, WAYLAND_DISPLAY=sock, GDK_BACKEND="wayland", XDG_SESSION_TYPE="wayland")
subprocess.Popen(["/usr/libexec/xdg-desktop-portal-gnome"], env=env,
                 stdin=subprocess.DEVNULL, start_new_session=True)
if not wait_owned("org.freedesktop.impl.portal.desktop.gnome"):
    sys.exit(1)
subprocess.Popen(["/usr/libexec/xdg-desktop-portal", "-r"], env=env,
                 stdin=subprocess.DEVNULL, start_new_session=True)
if not wait_owned("org.freedesktop.portal.Desktop"):
    sys.exit(1)
time.sleep(2)
reply = bus.call_sync("org.freedesktop.impl.portal.desktop.gnome",
                      "/org/freedesktop/portal/desktop",
                      "org.freedesktop.DBus.Introspectable", "Introspect",
                      None, None, Gio.DBusCallFlags.NONE, 3000, None)
text = reply.unpack()[0]
sys.exit(0 if "org.freedesktop.impl.portal.Screenshot" in text else 1)
PY
}

close_overview() {
  python3 - <<'PY'
import sys, time
import gi
from gi.repository import Gio, GLib

service = "org.gnome.Mutter.RemoteDesktop"
bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)

def call(name, path, interface, method, params=None):
    return bus.call_sync(name, path, interface, method, params, None,
                         Gio.DBusCallFlags.NONE, 3000, None)

def overview_open():
    reply = call("org.gnome.Shell", "/org/gnome/Shell", "org.freedesktop.DBus.Properties", "Get",
                 GLib.Variant("(ss)", ("org.gnome.Shell", "OverviewActive")))
    return reply.unpack()[0]

try:
    if not overview_open():
        sys.exit(0)
    session = call(service, "/org/gnome/Mutter/RemoteDesktop", service, "CreateSession").unpack()[0]
    call(service, session, service + ".Session", "Start")
    for down in (True, False):
        call(service, session, service + ".Session", "NotifyKeyboardKeysym", GLib.Variant("(ub)", (0xff1b, down)))
    time.sleep(0.3)
    call(service, session, service + ".Session", "Stop")
    for _ in range(40):
        if not overview_open():
            sys.exit(0)
        time.sleep(0.25)
    sys.exit(1)
except GLib.Error:
    sys.exit(1)
PY
}

start_shell() {
  find "$rt" -maxdepth 1 -name 'wayland-*' -exec rm -f -- {} +
  local before
  socket=
  before=$(sockets)
  setsid env -u DISPLAY gnome-shell --headless --wayland --no-x11 --virtual-monitor 1280x800 \
    > "$AGENT_DESKTOP_PRIVATE_SESSION/gnome-shell-wayland.log" 2>&1 < /dev/null &
  shell_pid=$!
  for _ in $(seq 1 100); do
    socket=$(comm -13 <(printf '%s\n' "$before") <(sockets) | head -n 1)
    [ -n "$socket" ] && break
    sleep 0.2
  done
  [ -n "${socket:-}" ] || return 1
  sleep 3
}

for _ in $(seq 1 "$attempts"); do
  if start_shell && healthy && { [ "$no_input" = 1 ] || close_overview; }; then
    export WAYLAND_DISPLAY=$socket
    start_portals || { stop_shell; continue; }
    "$@"
    exit $?
  fi
  stop_shell
done
echo "private-wayland-shell: no healthy gnome-shell after $attempts starts" >&2
exit 1
