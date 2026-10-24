#!/usr/bin/env bash
#
# One check over the three Linux channels that must carry the clipboard owner
# helper: the CLI archive, the FFI archive and the npm package table. A channel
# that omits the helper installs a program whose clipboard-set has no owner, and
# nothing else in the pipeline notices.
#
# Usage:
#   check-linux-archive-contents.sh cli <cli.tar.gz>
#   check-linux-archive-contents.sh ffi <ffi.tar.gz>
#   check-linux-archive-contents.sh npm <platform-key>             (linux-x64 or linux-arm64)
#   check-linux-archive-contents.sh all <cli.tar.gz> <ffi.tar.gz> <platform-key>
# Exit: 0 every named channel carries the helper, 1 a channel does not, 2 bad arguments

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HELPER=agent-desktop-linux-clipboard-owner
LIBRARY=libagent_desktop_ffi.so

usage() {
  sed -n '12,16p' "$0" | sed 's/^# \{0,1\}//' >&2
  exit 2
}

problems=0
fail() {
  echo "FAIL: $1" >&2
  problems=$((problems + 1))
}

archive_listing() {
  tar -tzf "$1" | sed 's|^\./||' | sort
}

executable_members() {
  tar -tvzf "$1" | awk '$1 ~ /^-..x/ { print $NF }' | sed 's|^\./||' | sort
}

check_cli() {
  local archive=$1 listing expected executable
  [ -r "$archive" ] || { fail "cannot read CLI archive $archive"; return; }
  listing=$(archive_listing "$archive")
  expected=$(printf 'agent-desktop\n%s\n' "$HELPER" | sort)
  if [ "$listing" != "$expected" ]; then
    fail "CLI archive $archive lists '$(printf '%s' "$listing" | tr '\n' ' ')', expected exactly '$(printf '%s' "$expected" | tr '\n' ' ')'"
    return
  fi
  executable=$(executable_members "$archive")
  if [ "$executable" != "$expected" ]; then
    fail "CLI archive $archive members are not all executable: '$(printf '%s' "$executable" | tr '\n' ' ')'"
    return
  fi
  echo "OK: CLI archive carries agent-desktop and $HELPER, both executable"
}

check_ffi() {
  local archive=$1 listing root member executable
  [ -r "$archive" ] || { fail "cannot read FFI archive $archive"; return; }
  listing=$(archive_listing "$archive")
  root=$(printf '%s\n' "$listing" | awk -F/ 'NF > 1 { print $1; exit }')
  if [ -z "$root" ]; then
    fail "FFI archive $archive has no top-level directory"
    return
  fi
  for member in "lib/$LIBRARY" "lib/$HELPER" include/agent_desktop.h LICENSE README.md SHA256SUMS; do
    if ! printf '%s\n' "$listing" | grep -Fxq "$root/$member"; then
      fail "FFI archive $archive does not list $root/$member"
    fi
  done
  executable=$(executable_members "$archive")
  if ! printf '%s\n' "$executable" | grep -Fxq "$root/lib/$HELPER"; then
    fail "FFI archive $archive: $root/lib/$HELPER is not executable"
  fi
  readme=$(tar -xzOf "$archive" "$root/README.md" 2>/dev/null || true)
  for phrase in "glibc 2.35" "SONAME" "-rpath" "one at a time" "ad_snapshot" "carry text only" "$HELPER" "wl-clipboard"; do
    if ! printf '%s\n' "$readme" | grep -Fq -- "$phrase"; then
      fail "FFI archive README does not state '$phrase'"
    fi
  done
  [ "$problems" -eq 0 ] && echo "OK: FFI archive carries $LIBRARY and $HELPER side by side"
  return 0
}

check_npm() {
  local key=$1 entries expected
  case "$key" in
    linux-x64|linux-arm64) ;;
    *) echo "check-linux-archive-contents: '$key' is not linux-x64 or linux-arm64" >&2; exit 2 ;;
  esac
  entries=$(node -e "
    const entry = require(process.argv[1]).PLATFORMS[process.argv[2]];
    if (!entry) process.exit(3);
    process.stdout.write(entry.entries.join('\n'));
  " "$ROOT/npm/lib/platform.js" "$key" | sort) || { fail "npm/lib/platform.js has no $key entry"; return; }
  expected=$(printf 'agent-desktop\n%s\n' "$HELPER" | sort)
  if [ "$entries" != "$expected" ]; then
    fail "npm table entries for $key are '$(printf '%s' "$entries" | tr '\n' ' ')', expected exactly '$(printf '%s' "$expected" | tr '\n' ' ')'"
    return
  fi
  echo "OK: npm table for $key lists agent-desktop and $HELPER"
}

[ "$#" -ge 1 ] || usage
channel=$1
shift
case "$channel" in
  cli) [ "$#" -eq 1 ] || usage; check_cli "$1" ;;
  ffi) [ "$#" -eq 1 ] || usage; check_ffi "$1" ;;
  npm) [ "$#" -eq 1 ] || usage; check_npm "$1" ;;
  all)
    [ "$#" -eq 3 ] || usage
    check_cli "$1"
    check_ffi "$2"
    check_npm "$3"
    ;;
  *) usage ;;
esac

[ "$problems" -eq 0 ]
