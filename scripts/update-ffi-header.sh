#!/usr/bin/env bash
set -euo pipefail

# Regenerate the committed FFI header from source.
# Run this after changing any `#[no_mangle] pub extern "C"` signatures
# in crates/ffi/src/ and commit the result.

cargo build -p agent-desktop-ffi 2>/dev/null

GENERATED=$(find target -path '*/agent-desktop-ffi-*/out/agent_desktop.h' -newer crates/ffi/include/agent_desktop.h | head -1)
if [ -z "$GENERATED" ]; then
  GENERATED=$(find target -path '*/agent-desktop-ffi-*/out/agent_desktop.h' | head -1)
fi

if [ -z "$GENERATED" ]; then
  echo "ERROR: cbindgen did not produce a header. Check build output." >&2
  exit 1
fi

cp "$GENERATED" crates/ffi/include/agent_desktop.h
echo "Updated crates/ffi/include/agent_desktop.h"
