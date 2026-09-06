#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

if [ "$(uname -s)" != "Darwin" ]; then
    echo "native E2E runner must be macOS" >&2
    exit 2
fi
if [ "${AGENT_DESKTOP_NATIVE_E2E_RUNNER:-}" != "1" ]; then
    echo "refusing native desktop control without AGENT_DESKTOP_NATIVE_E2E_RUNNER=1" >&2
    exit 2
fi

cargo build --locked --release -p agent-desktop
cargo build --locked --release -p agent-desktop-macos --bin agent-desktop-macos-helper
cargo build --locked --profile release-ffi -p agent-desktop-ffi

AGENT_DESKTOP_E2E_EXCLUSIVE=1 \
AGENT_DESKTOP_E2E_RELEASE_BIN="$ROOT/target/release/agent-desktop" \
AGENT_DESKTOP_E2E_RELEASE_FFI="$ROOT/target/release-ffi/libagent_desktop_ffi.dylib" \
AGENT_DESKTOP_E2E_RELEASE_FFI_HELPER="$ROOT/target/release/agent-desktop-macos-helper" \
    bash tests/e2e/run.sh
