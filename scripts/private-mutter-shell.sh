#!/usr/bin/env bash
# Runs a command against a private X11 session managed by mutter (gnome-shell --x11), the window
# manager of a default GNOME desktop, on its own Xvfb display.
# Usage: scripts/private-session.sh -- scripts/private-mutter-shell.sh [--] command [args...]
# A fresh shell opens Activities Overview when its first window maps. Map an owned primer, wait
# for that event, and close Overview through its D-Bus property before handing the display to tests.
# DISPLAY names the new display for the
# command; :200 up avoids the embedded mutter fixture's :150-199 range.
# The shell runs in a session mode without the polkit agent component. Every private session of
# one login shares that login's logind session, and logind keeps one polkit agent per session, so
# the first shell to start would become the agent for all of them. It would then show the colord
# "Authentication Required" dialog of every other session as a modal that holds keyboard focus,
# queues the focus of each new window and swallows pointer input until it closes.
set -euo pipefail

[ -n "${AGENT_DESKTOP_PRIVATE_SESSION:-}" ] || {
  echo "private-mutter-shell: run it through scripts/private-session.sh" >&2
  exit 2
}
[ "${1:-}" = "--" ] && shift
[ $# -gt 0 ] || { echo "usage: private-mutter-shell.sh [--] command [args...]" >&2; exit 2; }

log=$AGENT_DESKTOP_PRIVATE_SESSION
xvfb_pid=
shell_pid=
primer_pid=

stop_all() {
  if [ -n "$primer_pid" ]; then
    kill "$primer_pid" 2>/dev/null || true
    wait "$primer_pid" 2>/dev/null || true
  fi
  if [ -n "$shell_pid" ]; then
    kill -KILL -- "-$shell_pid" 2>/dev/null || true
    wait "$shell_pid" 2>/dev/null || true
  fi
  if [ -n "$xvfb_pid" ]; then
    kill "$xvfb_pid" 2>/dev/null || true
    wait "$xvfb_pid" 2>/dev/null || true
  fi
}
trap stop_all EXIT

display=
base=${AGENT_DESKTOP_SESSION_BASE:-${XDG_RUNTIME_DIR:+$XDG_RUNTIME_DIR/agent-desktop-sessions}}
base=${base:-/tmp/agent-desktop-sessions-$(id -u)}
exec 9> "$base/display.lock"
flock 9
for candidate in $(seq 200 249); do
  [ -e "/tmp/.X11-unix/X$candidate" ] || [ -e "/tmp/.X$candidate-lock" ] && continue
  display=":$candidate"
  break
done
[ -n "$display" ] || { echo "private-mutter-shell: no free display" >&2; exit 1; }

Xvfb "$display" -screen 0 1280x800x24 -nolisten tcp 9>&- > "$log/xvfb-mutter.log" 2>&1 &
xvfb_pid=$!
for _ in $(seq 1 50); do
  [ -S "/tmp/.X11-unix/X${display#:}" ] && break
  sleep 0.2
done
[ -S "/tmp/.X11-unix/X${display#:}" ] || { echo "private-mutter-shell: Xvfb did not start" >&2; exit 1; }
kill -0 "$xvfb_pid"
flock -u 9
exec 9>&-

# The outer wrapper pushed its own display into the activation environment before this
# display existed, so a bus-activated app (Terminal, Files) would map there instead of
# here (ledger L18-17, L34-3). Push ours before the shell or any app starts.
push_vars="DISPLAY"
[ -n "${XAUTHORITY:-}" ] && push_vars="$push_vars XAUTHORITY"
# shellcheck disable=SC2086  # the list of variable names splits on purpose
DISPLAY=$display dbus-update-activation-environment $push_vars

overview_active() {
  busctl --user --timeout=3 get-property org.gnome.Shell /org/gnome/Shell org.gnome.Shell OverviewActive 2>/dev/null |
    sed -n 's/^b \(.*\)$/\1/p'
}

# Accessibility services activated by the shell or primer must synthesize input on this display.
dbus-update-activation-environment "DISPLAY=$display"

# gnome-shell reads custom session modes from the system data directories only, so the mode file
# sits in a directory prepended to XDG_DATA_DIRS. It lists the user mode's components without
# polkitAgent; every other property comes from the user mode.
mode_root="$log/shell-modes"
mkdir -p "$mode_root/gnome-shell/modes"
cat > "$mode_root/gnome-shell/modes/ad-private.json" <<'JSON'
{
  "parentMode": "user",
  "components": ["keyring", "autorunManager", "automountManager"]
}
JSON

for _ in $(seq 1 5); do
  DISPLAY=$display GNOME_SHELL_SESSION_MODE=ad-private XDG_DATA_DIRS="$mode_root:${XDG_DATA_DIRS:-/usr/local/share:/usr/share}" setsid gnome-shell --x11 --replace > "$log/gnome-shell-mutter.log" 2>&1 < /dev/null &
  shell_pid=$!
  for _ in $(seq 1 60); do
    [ -n "$(overview_active)" ] && break
    sleep 0.5
  done
  if [ -n "$(overview_active)" ]; then
    break
  fi
  kill -KILL -- "-$shell_pid" 2>/dev/null || true
  wait "$shell_pid" 2>/dev/null || true
  shell_pid=
done
[ -n "$shell_pid" ] || { echo "private-mutter-shell: no healthy gnome-shell --x11" >&2; exit 1; }

DISPLAY=$display /usr/bin/python3 - "$log/mutter-mapped" <<'PY' &
import pathlib
import sys
import gi
gi.require_version("Gtk", "3.0")
from gi.repository import Gtk
window = Gtk.Window(title="Private Mutter startup")
window.set_default_size(320, 120)
window.add(Gtk.Label(label="Private desktop ready"))
window.connect("map-event", lambda *_: pathlib.Path(sys.argv[1]).touch())
window.show_all()
Gtk.main()
PY
primer_pid=$!
for _ in $(seq 1 100); do
  [ -f "$log/mutter-mapped" ] && break
  kill -0 "$primer_pid"
  sleep 0.1
done
[ -f "$log/mutter-mapped" ] || { echo "private-mutter-shell: primer never mapped" >&2; exit 1; }
# Shell exports its property before runStartupAnimation sets Overview visible.
# Closing the initial false value can therefore race the later startup show.
# Observe that one startup show before issuing the hide request.
for _ in $(seq 1 40); do
  [ "$(overview_active)" = true ] && break
  sleep 0.25
done
[ "$(overview_active)" = true ] || { echo "private-mutter-shell: startup Overview never appeared" >&2; exit 1; }
for _ in $(seq 1 5); do
  busctl --user --timeout=3 set-property org.gnome.Shell /org/gnome/Shell org.gnome.Shell OverviewActive b false && break
done
for _ in $(seq 1 40); do
  [ "$(overview_active)" = false ] && break
  sleep 0.25
done
[ "$(overview_active)" = false ] || { echo "private-mutter-shell: Overview never closed" >&2; exit 1; }
kill "$primer_pid"
wait "$primer_pid" 2>/dev/null || true
primer_pid=

# The session's displayfd names the display its tests must drive; this display now owns that role.
echo "${display#:}" > "$AGENT_DESKTOP_PRIVATE_SESSION/displayfd"

DISPLAY=$display "$@"
