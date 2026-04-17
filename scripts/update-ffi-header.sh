#!/usr/bin/env bash
set -euo pipefail

# Regenerate the committed FFI header from source.
#
# Run this after changing any `#[no_mangle] pub extern "C"` signatures in
# crates/ffi/src/ and commit the result. build.rs stamps the absolute path
# to the generated header at target/ffi-header-path.txt so we never have to
# guess which of several cached `agent-desktop-ffi-<hash>/` build dirs is
# current — `find target | head -1` would pick arbitrarily.

ROOT=$(git rev-parse --show-toplevel)
cd "$ROOT"

cargo build -p agent-desktop-ffi >/dev/null 2>&1

STAMP=target/ffi-header-path.txt
if [ ! -f "$STAMP" ]; then
  echo "ERROR: $STAMP was not produced by build.rs. Check the build output." >&2
  exit 1
fi

GENERATED=$(cat "$STAMP")
if [ ! -f "$GENERATED" ]; then
  echo "ERROR: stamped header path does not exist: $GENERATED" >&2
  exit 1
fi

cp "$GENERATED" crates/ffi/include/agent_desktop.h
echo "Updated crates/ffi/include/agent_desktop.h"
