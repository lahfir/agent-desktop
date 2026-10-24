#!/usr/bin/env bash
#
# Fails when a shipped Linux ELF file needs a glibc symbol version above the floor.
# The floor belongs to the host that links the file, not to the source, so the
# gate reads the finished file and not the build recipe.
#
# Usage: scripts/check-elf-glibc-floor.sh <elf-file> <max-version>   (for example 2.35)
# Exit:  0 the file needs no glibc version above the floor
#        1 the file needs a higher version, or has no version information to read
#        2 bad arguments or an unreadable file

set -euo pipefail

if [ "$#" -ne 2 ]; then
  echo "usage: $0 <elf-file> <max-version>" >&2
  exit 2
fi
FILE=$1
MAX=$2
case "$MAX" in
  [0-9]*.[0-9]*) ;;
  *) echo "check-elf-glibc-floor: max version '$MAX' is not of the form 2.35" >&2; exit 2 ;;
esac
if [ ! -r "$FILE" ]; then
  echo "check-elf-glibc-floor: cannot read $FILE" >&2
  exit 2
fi
if ! command -v readelf >/dev/null 2>&1; then
  echo "check-elf-glibc-floor: readelf is not installed" >&2
  exit 2
fi

NEEDS=$(readelf -W --version-info "$FILE" 2>/dev/null || true)
if ! printf '%s\n' "$NEEDS" | grep -q 'Version needs section'; then
  echo "FAIL: $FILE carries no symbol version information, so its glibc floor cannot be read"
  exit 1
fi

VERSIONS=$(printf '%s\n' "$NEEDS" | grep -o 'GLIBC_[0-9][0-9.]*' | sed 's/^GLIBC_//' | sort -u -V || true)
if [ -z "$VERSIONS" ]; then
  echo "FAIL: $FILE names no GLIBC_ version, so it is not a glibc-linked file"
  exit 1
fi

HIGHEST=$(printf '%s\n' "$VERSIONS" | tail -n 1)
ABOVE=$(printf '%s\n%s\n' "$VERSIONS" "$MAX" | sort -u -V | awk -v max="$MAX" 'found { print } $0 == max { found = 1 }')
if [ -n "$ABOVE" ]; then
  echo "FAIL: $FILE needs GLIBC_${HIGHEST}, above the GLIBC_${MAX} floor"
  SYMBOLS=$(readelf -W --dyn-syms "$FILE" 2>/dev/null || true)
  for version in $ABOVE; do
    printf '%s\n' "$SYMBOLS" | grep -F "@GLIBC_${version}" | awk -v v="$version" '{ print "  requires GLIBC_" v ": " $(NF-1) }' | head -n 20 || true
  done
  exit 1
fi

echo "OK: $FILE needs at most GLIBC_${HIGHEST} (floor GLIBC_${MAX})"
