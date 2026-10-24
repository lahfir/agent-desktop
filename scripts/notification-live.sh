#!/usr/bin/env bash
# Run a command against a private gnome-shell that owns org.freedesktop.Notifications.
# Call it inside scripts/private-session.sh: its private bus, display and runtime dir must
# already be exported. It never signals a process it did not start, and stops its own shell
# by pid on exit.
# Usage: scripts/private-session.sh -- bash scripts/notification-live.sh command [args...]
set -euo pipefail

attempts=${NOTIFICATION_LIVE_ATTEMPTS:-4}
health_seconds=${NOTIFICATION_LIVE_HEALTH_SECONDS:-25}
shell_pid=""

answers() {
  gdbus call --session --dest org.freedesktop.Notifications \
    --object-path /org/freedesktop/Notifications \
    --method org.freedesktop.Notifications.GetServerInformation >/dev/null 2>&1
}

healthy() {
  local streak=0
  for _ in $(seq 1 $((health_seconds * 2))); do
    kill -0 "$shell_pid" 2>/dev/null || return 1
    if answers; then streak=$((streak + 1)); else streak=0; fi
    [ "$streak" -ge 3 ] && return 0
    sleep 0.5
  done
  return 1
}

stop_shell() {
  [ -n "$shell_pid" ] || return 0
  kill "$shell_pid" 2>/dev/null || true
  for _ in $(seq 1 20); do
    kill -0 "$shell_pid" 2>/dev/null || return 0
    sleep 0.25
  done
  kill -KILL "$shell_pid" 2>/dev/null || true
}
trap stop_shell EXIT

: "${AGENT_DESKTOP_PRIVATE_SESSION:?run inside scripts/private-session.sh}"
log="$AGENT_DESKTOP_PRIVATE_SESSION/gnome-shell.log"
for attempt in $(seq 1 "$attempts"); do
  gnome-shell --x11 --replace >> "$log" 2>&1 &
  shell_pid=$!
  if healthy; then
    echo "notification-live: shell $shell_pid answered on attempt $attempt" >&2
    break
  fi
  echo "notification-live: attempt $attempt did not become healthy" >&2
  stop_shell
  shell_pid=""
  if [ "$attempt" = "$attempts" ]; then
    tail -n 20 "$log" >&2 || true
    exit 1
  fi
done

status=0
"$@" || status=$?
exit "$status"
