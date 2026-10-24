#!/usr/bin/env bash
# The independent sender and observer the live notification tests check against. It talks to
# whatever session bus the environment names, so run it only inside a private session.
# Usage: notification-oracle.sh notify APP SUMMARY BODY [KEY=LABEL,...]   prints the daemon id
#        notification-oracle.sh close ID
#        notification-oracle.sh monitor FILE                              prints the monitor pid
set -euo pipefail

dest=org.freedesktop.Notifications
path=/org/freedesktop/Notifications

actions_literal() {
  local list=$1 out="" pair
  [ -n "$list" ] || { echo "[]"; return; }
  IFS=',' read -r -a pairs <<< "$list"
  for pair in "${pairs[@]}"; do
    out="${out:+$out, }'${pair%%=*}', '${pair#*=}'"
  done
  echo "[$out]"
}

case "${1:?command}" in
  notify)
    app=${2:?app}; summary=${3:?summary}; body=${4:-}; actions=$(actions_literal "${5:-}")
    gdbus call --session --dest "$dest" --object-path "$path" --method "$dest.Notify" \
      "$app" 0 "" "$summary" "$body" "$actions" "{}" 0 | sed -n 's/^(uint32 \([0-9]*\),)$/\1/p'
    ;;
  close)
    gdbus call --session --dest "$dest" --object-path "$path" \
      --method "$dest.Close${CLOSE_SUFFIX:-Notification}" "${2:?id}" >/dev/null
    ;;
  monitor)
    dbus-monitor --session "type='signal',interface='$dest'" > "${2:?file}" 2>&1 &
    echo $!
    ;;
  *) echo "usage: notification-oracle.sh notify|close|monitor ..." >&2; exit 2 ;;
esac
