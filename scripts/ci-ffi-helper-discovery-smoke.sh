#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

STAGE=$(mktemp -d /tmp/agent-desktop-ffi-helper.XXXXXX)
cp target/release-ffi/libagent_desktop_ffi.dylib "$STAGE/"
cp target/release/agent-desktop-macos-helper "$STAGE/"
chmod +x "$STAGE/agent-desktop-macos-helper"
DYLIB_SHA=$(shasum -a 256 "$STAGE/libagent_desktop_ffi.dylib" | awk '{print $1}')
HELPER_SHA=$(shasum -a 256 "$STAGE/agent-desktop-macos-helper" | awk '{print $1}')
AD_DYLIB_PATH="$STAGE/libagent_desktop_ffi.dylib" \
  AD_HEADER_PATH=crates/ffi/include/agent_desktop.h \
  AD_EXPECT_MACOS_HELPER=1 \
  python3 tests/ffi-python/smoke.py
test "$DYLIB_SHA" = "$(shasum -a 256 "$STAGE/libagent_desktop_ffi.dylib" | awk '{print $1}')"
test "$HELPER_SHA" = "$(shasum -a 256 "$STAGE/agent-desktop-macos-helper" | awk '{print $1}')"
