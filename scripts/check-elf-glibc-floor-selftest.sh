#!/usr/bin/env bash
#
# Proves scripts/check-elf-glibc-floor.sh in both directions. It builds tiny C
# programs on this host: one that calls pidfd_getpid (bound to GLIBC_2.39 by the
# glibc of a current host), one that calls only puts. The gate must fail the
# first, pass the second, and fail a file with no version information.
#
# A host whose libc is older than 2.39 cannot produce the failing program.
# That is an error and not a pass, because a self-test that skips proves
# nothing. AGENT_DESKTOP_GLIBC_SELFTEST_SKIP=1 turns that case into a loud skip
# for a local run on an old host; CI never sets it.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
GATE="$ROOT/scripts/check-elf-glibc-floor.sh"
WORK=$(mktemp -d "${TMPDIR:-/tmp}/glibc-floor-selftest.XXXXXX")
trap 'rm -rf -- "$WORK"' EXIT

fail() {
  echo "self-test FAIL: $1" >&2
  exit 1
}

expect_exit() {
  local want=$1 name=$2
  shift 2
  local out got=0
  out=$("$@" 2>&1) || got=$?
  if [ "$got" -ne "$want" ]; then
    printf '%s\n' "$out" >&2
    fail "$name: exit $got, expected $want"
  fi
  LAST_OUTPUT=$out
}

cat > "$WORK/ok.c" <<'SRC'
#include <stdio.h>
int main(void) { puts("ok"); return 0; }
SRC
cat > "$WORK/high.c" <<'SRC'
#define _GNU_SOURCE
#include <stdio.h>
#include <sys/types.h>
#include <sys/syscall.h>
#include <unistd.h>
#include <sys/pidfd.h>
int main(int argc, char **argv) { (void)argv; return argc > 5 ? pidfd_getpid(argc) : 0; }
SRC

gcc -O1 -o "$WORK/ok" "$WORK/ok.c" || fail "cannot compile the passing program"
if ! gcc -O1 -o "$WORK/high" "$WORK/high.c" 2>/dev/null; then
  if [ "${AGENT_DESKTOP_GLIBC_SELFTEST_SKIP:-0}" = "1" ]; then
    echo "SKIPPED: this host's libc cannot produce a GLIBC_2.39 program, so the failing direction is unproven" >&2
    exit 0
  fi
  fail "this host's libc cannot produce a GLIBC_2.39 program; run the self-test on a host with glibc 2.39 or newer"
fi

expect_exit 1 "a GLIBC_2.39 file fails the 2.35 floor" "$GATE" "$WORK/high" 2.35
case "$LAST_OUTPUT" in
  *pidfd_getpid*) ;;
  *) fail "the failure message does not name pidfd_getpid: $LAST_OUTPUT" ;;
esac
case "$LAST_OUTPUT" in
  *GLIBC_2.39*) ;;
  *) fail "the failure message does not name GLIBC_2.39: $LAST_OUTPUT" ;;
esac
expect_exit 1 "versions compare numerically, so 2.39 is above 2.4" "$GATE" "$WORK/high" 2.4
expect_exit 0 "a GLIBC_2.39 file passes a 2.39 floor" "$GATE" "$WORK/high" 2.39
expect_exit 0 "a file that needs at most 2.34 passes the 2.35 floor" "$GATE" "$WORK/ok" 2.35
expect_exit 1 "a file above a lower floor fails" "$GATE" "$WORK/ok" 2.0

gcc -c -o "$WORK/ok.o" "$WORK/ok.c"
expect_exit 1 "an object with no version information fails" "$GATE" "$WORK/ok.o" 2.35
case "$LAST_OUTPUT" in
  *"no symbol version information"*) ;;
  *) fail "the no-version-information message is missing: $LAST_OUTPUT" ;;
esac
printf 'not an elf file\n' > "$WORK/text"
expect_exit 1 "a non-ELF file fails" "$GATE" "$WORK/text" 2.35
expect_exit 2 "a missing file is a usage error" "$GATE" "$WORK/absent" 2.35
expect_exit 2 "a malformed floor is a usage error" "$GATE" "$WORK/ok" latest

echo "OK: the glibc floor gate fails above the floor, passes at it, and fails without version information"
