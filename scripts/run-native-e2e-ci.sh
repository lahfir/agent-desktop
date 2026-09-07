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

AGENT_DESKTOP_E2E_EXCLUSIVE=1 \
    bash tests/e2e/run.sh
