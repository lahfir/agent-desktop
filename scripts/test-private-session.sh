#!/usr/bin/env bash
# Behaviour checks for scripts/private-session.sh and the shell wrappers.
# Run on Linux:
#   bash scripts/test-private-session.sh
set -u
here=$(cd "$(dirname "$0")" && pwd)
helper=$here/private-session.sh
mutter=$here/private-mutter-shell.sh
failed=0
marker=$(mktemp -u)

check() {
  if [ "$2" = ok ]; then echo "ok   $1"; else echo "FAIL $1"; failed=1; fi
}

rt_of() {
  local child
  for child in $(ps -o pid= --ppid "$1" 2>/dev/null); do
    tr '\0' '\n' < "/proc/$child/environ" 2>/dev/null | sed -n 's/^XDG_RUNTIME_DIR=//p' | grep . && return
  done
}

has_member() {
  local environ
  for environ in /proc/[0-9]*/environ; do
    if grep -qzF "XDG_RUNTIME_DIR=$1" "$environ" 2>/dev/null; then
      return 0
    fi
  done
  return 1
}

member_named() {
  local environ pid
  for environ in /proc/[0-9]*/environ; do
    grep -qzF "XDG_RUNTIME_DIR=$1" "$environ" 2>/dev/null || continue
    pid=${environ%/environ}
    [ "$(cat "$pid/comm" 2>/dev/null)" = "$2" ] && return 0
  done
  return 1
}

wait_for_rt() {
  local wrapper=$1 rt="" _
  for _ in $(seq 1 100); do
    rt=$(rt_of "$wrapper")
    [ -n "$rt" ] && break
    sleep 0.1
  done
  printf '%s' "$rt"
}

AGENT_DESKTOP_WM_ROOT=/nonexistent "$helper" --wm -- touch "$marker" 2> /dev/null
status=$?
verdict=ok
{ [ "$status" -ne 0 ] && [ ! -e "$marker" ]; } || verdict=bad
check "a missing openbox fails and never runs the payload" "$verdict"
case "$marker" in
  "${TMPDIR:-/tmp}"/tmp.?*) if [ -e "$marker" ]; then rm -f -- "$marker"; fi ;;
esac

# shellcheck disable=SC2016
seen=$(AT_SPI_BUS_ADDRESS=unix:path=/nonexistent/sentinel NO_AT_BRIDGE=1 GTK_A11Y=none \
  "$helper" -- sh -c 'echo "${AT_SPI_BUS_ADDRESS-unset}/${NO_AT_BRIDGE-unset}/${GTK_A11Y-unset}"')
verdict=ok
[ "$seen" = "unset/unset/unset" ] || verdict=bad
check "inherited accessibility overrides never reach the payload" "$verdict"

"$helper" -- sh -c 'exit 41' 2>/dev/null
status=$?
verdict=ok
[ "$status" -eq 41 ] || verdict=bad
check "the payload's exit status passes through" "$verdict"

spawn_group() {
  python3 - "$@" > "$pid_file" <<'PY' &
import signal, subprocess, sys
signal.signal(signal.SIGINT, signal.SIG_DFL)
proc = subprocess.Popen(sys.argv[1:], start_new_session=True)
print(proc.pid, flush=True)
sys.exit(proc.wait())
PY
  launcher=$!
  member_pid=""
  for _ in $(seq 1 50); do
    [ -s "$pid_file" ] && { member_pid=$(cat "$pid_file"); break; }
    sleep 0.1
  done
}

for sig in INT TERM HUP; do
  pid_file=$(mktemp)
  spawn_group "$helper" -- sh -c 'sleep 300'
  rt=$(wait_for_rt "$member_pid")
  for _ in $(seq 1 100); do
    member_named "$rt" sleep && break
    sleep 0.1
  done
  sleep 0.5
  kill -"$sig" -- "-$member_pid" 2>/dev/null || true
  wait "$launcher" 2>/dev/null
  status=$?
  rm -f "$pid_file" 2>/dev/null || true
  verdict=ok
  [ -n "$rt" ] || verdict=bad
  [ ! -d "$rt" ] || verdict=bad
  has_member "$rt" && verdict=bad
  case "$sig" in
    INT) want=130 ;;
    HUP) want=129 ;;
    *) want=143 ;;
  esac
  [ "$status" -eq "$want" ] || verdict=bad
  check "SIG$sig to the session group during a running payload leaves no member" "$verdict"
done

pid_file=$(mktemp)
spawn_group env AGENT_DESKTOP_SESSION_GRACE_S=2 "$helper" -- sh -c 'trap "" TERM; while :; do sleep 1; done'
rt=$(wait_for_rt "$member_pid")
for _ in $(seq 1 100); do
  member_named "$rt" sh && break
  sleep 0.1
done
sleep 0.5
kill -TERM -- "-$member_pid" 2>/dev/null || true
wait "$launcher" 2>/dev/null
status=$?
rm -f "$pid_file" 2>/dev/null || true
verdict=ok
[ "$status" -eq 143 ] || verdict=bad
[ -n "$rt" ] && [ ! -d "$rt" ] || verdict=bad
has_member "$rt" && verdict=bad
check "a payload that ignores TERM is killed after the grace period" "$verdict"

pid_file=$(mktemp)
spawn_group "$helper" -- "$mutter" -- sh -c 'sleep 300'
rt=$(wait_for_rt "$member_pid")
verdict=ok
[ -n "$rt" ] || verdict=bad
saw_shell=1
for _ in $(seq 1 300); do
  member_named "$rt" gnome-shell && { saw_shell=0; break; }
  sleep 0.5
done
[ "$saw_shell" -eq 0 ] || verdict=bad
kill -TERM -- "-$member_pid" 2>/dev/null || true
wait "$launcher" 2>/dev/null
rm -f "$pid_file" 2>/dev/null || true
[ ! -d "$rt" ] || verdict=bad
has_member "$rt" && verdict=bad
check "the setsid-started shell is gone after the session group is cancelled" "$verdict"

# shellcheck disable=SC2016
"$helper" -- "$mutter" -- sh -c '
  busctl --user call org.gnome.Terminal /org/gnome/Terminal \
    org.freedesktop.Application Activate "a{sv}" 0 >/dev/null 2>&1
  found=1
  for _ in $(seq 1 50); do
    if xdotool search --class erminal >/dev/null 2>&1; then
      found=0
      break
    fi
    sleep 0.4
  done
  exit "$found"
' 2>/dev/null
status=$?
verdict=ok
[ "$status" -eq 0 ] || verdict=bad
check "a bus-activated Terminal under the mutter wrapper maps on its display" "$verdict"

# shellcheck disable=SC2016
"$helper" -- "$mutter" -- sh -c '
  [ "$(busctl --user --timeout=3 get-property org.gnome.Shell /org/gnome/Shell org.gnome.Shell Mode)" = "s \"ad-private\"" ] &&
    ! grep -q AuthenticationAgent "$AGENT_DESKTOP_PRIVATE_SESSION/gnome-shell-mutter.log"
' 2>/dev/null
status=$?
verdict=ok
[ "$status" -eq 0 ] || verdict=bad
check "the mutter shell runs in the session mode without the polkit agent" "$verdict"

exit "$failed"
