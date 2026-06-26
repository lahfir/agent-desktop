#!/usr/bin/env bash
set -euo pipefail

# Regenerate the committed FFI header from source. This script is deliberately
# outside the normal Cargo build graph so cbindgen never executes during
# ordinary builds, tests, CI, or release packaging.
#
# Required cbindgen version: 0.29.4  (verify with `cbindgen --version`)
# Install: cargo install cbindgen --version 0.29.4 --locked
# The [const] allow_static_const = false key requires cbindgen >= 0.26;
# the trailer key requires cbindgen >= 0.25.

ROOT=$(git rev-parse --show-toplevel)
cd "$ROOT"

REQUIRED_CBINDGEN="0.29.4"

if ! command -v cbindgen >/dev/null 2>&1; then
  echo "ERROR: cbindgen is not installed. Install and audit it explicitly before regenerating the FFI header." >&2
  echo "  cargo install cbindgen --version ${REQUIRED_CBINDGEN} --locked" >&2
  exit 1
fi

ACTUAL_CBINDGEN=$(cbindgen --version 2>&1 | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -1)
if [ "${ACTUAL_CBINDGEN}" != "${REQUIRED_CBINDGEN}" ]; then
  echo "ERROR: cbindgen ${REQUIRED_CBINDGEN} required, found ${ACTUAL_CBINDGEN}." >&2
  echo "  cargo install cbindgen --version ${REQUIRED_CBINDGEN} --locked" >&2
  exit 1
fi

cbindgen crates/ffi \
  --config crates/ffi/cbindgen.toml \
  --output crates/ffi/include/agent_desktop.h
echo "Updated crates/ffi/include/agent_desktop.h"
