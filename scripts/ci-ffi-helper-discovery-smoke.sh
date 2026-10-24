#!/usr/bin/env bash
# Prove a foreign ctypes host finds the packaged clipboard helper beside the
# library image, never through PATH.
#
# Usage: scripts/ci-ffi-helper-discovery-smoke.sh
#
# Copies the built library and its helper into a scratch directory, runs the
# smoke harness there, and checks that neither file changed. The macOS leg uses
# the dylib and the macOS helper; the Linux leg uses the .so and the Linux
# clipboard owner. The Linux leg needs a display because the helper owns an X11
# selection, so it fails closed when DISPLAY is unset rather than skipping.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

case "$(uname -s)" in
  Darwin)
    EXPECT_HELPER=AD_EXPECT_MACOS_HELPER
    LIB="target/release-ffi/libagent_desktop_ffi.dylib"
    HELPER="target/release/agent-desktop-macos-helper"
    hash_file() { shasum -a 256 "$1" | awk '{print $1}'; }
    ;;
  Linux)
    EXPECT_HELPER=AD_EXPECT_LINUX_HELPER
    if [ -f target/release-ffi/libagent_desktop_ffi.so ]; then
      LIB="target/release-ffi/libagent_desktop_ffi.so"
    else
      LIB="target/release/libagent_desktop_ffi.so"
    fi
    HELPER="target/release/agent-desktop-linux-clipboard-owner"
    hash_file() { sha256sum "$1" | awk '{print $1}'; }
    [ -n "${DISPLAY:-}" ] || {
      echo "ci-ffi-helper-discovery-smoke: Linux needs DISPLAY for the selection owner" >&2
      exit 1
    }
    ;;
  *)
    echo "ci-ffi-helper-discovery-smoke: unsupported host $(uname -s)" >&2
    exit 2
    ;;
esac

for file in "$LIB" "$HELPER"; do
  [ -f "$file" ] || { echo "ci-ffi-helper-discovery-smoke: missing $file" >&2; exit 1; }
done

STAGE=$(mktemp -d "${TMPDIR:-/tmp}/agent-desktop-ffi-helper.XXXXXX")
trap 'rm -rf -- "$STAGE"' EXIT
mkdir "$STAGE/nopath"
cp "$LIB" "$STAGE/"
cp "$HELPER" "$STAGE/"
chmod +x "$STAGE/$(basename "$HELPER")"
STAGED_LIB="$STAGE/$(basename "$LIB")"
STAGED_HELPER="$STAGE/$(basename "$HELPER")"
LIB_SHA=$(hash_file "$STAGED_LIB")
HELPER_SHA=$(hash_file "$STAGED_HELPER")

env "$EXPECT_HELPER=1" AD_HELPER_EMPTY_PATH="$STAGE/nopath" AD_DYLIB_PATH="$STAGED_LIB" \
  AD_HEADER_PATH=crates/ffi/include/agent_desktop.h \
  python3 tests/ffi-python/smoke.py

test "$LIB_SHA" = "$(hash_file "$STAGED_LIB")"
test "$HELPER_SHA" = "$(hash_file "$STAGED_HELPER")"
echo "ci-ffi-helper-discovery-smoke: packaged helper discovered from the library image"
