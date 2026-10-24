#!/usr/bin/env bash
#
# Enforces the architecture boundaries no compiler checks: core never reaches a
# platform crate or OS-specific code outside its allow-list, no platform crate
# reaches another, every crate keeps its shared top-level layout, and each
# capability lives in its own layer. The rules live in check_architecture.py,
# which plants one violation per rule in a scratch tree before it scans the
# repository, so a rule that has stopped detecting anything fails the gate.

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

INTERPRETER_CANDIDATES=(python3 python py)

resolve_interpreter() {
    local candidate
    for candidate in "${INTERPRETER_CANDIDATES[@]}"; do
        if command -v "$candidate" >/dev/null 2>&1 &&
            "$candidate" -c 'import sys; raise SystemExit(0 if sys.version_info >= (3, 8) else 1)' \
                >/dev/null 2>&1; then
            printf '%s\n' "$candidate"
            return 0
        fi
    done
    return 1
}

if ! interpreter="$(resolve_interpreter)"; then
    printf 'no working Python 3.8+ interpreter on PATH (tried %s)\n' "${INTERPRETER_CANDIDATES[*]}" >&2
    exit 1
fi

"$interpreter" -I scripts/check_architecture.py "$PWD"
