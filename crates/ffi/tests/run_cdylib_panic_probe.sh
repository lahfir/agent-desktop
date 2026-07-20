#!/usr/bin/env bash
set -euo pipefail

ROOT=$(git rev-parse --show-toplevel)
cd "$ROOT"

TARGET_ROOT=${CARGO_TARGET_DIR:-target}
PROBE=${TMPDIR:-/tmp}/agent-desktop-cdylib-panic-probe

cargo build --locked --profile release-ffi -p agent-desktop-ffi --features panic-injection
cc crates/ffi/tests/cdylib_panic_probe.c -o "$PROBE"
DYLIB=$(find "$TARGET_ROOT/release-ffi" -name 'libagent_desktop_ffi.dylib' -print -quit)
if [[ -z "$DYLIB" ]]; then
  echo "FAIL: release-ffi dylib was not produced" >&2
  exit 1
fi
"$PROBE" "$DYLIB"
