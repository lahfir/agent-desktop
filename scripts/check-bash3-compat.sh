#!/usr/bin/env bash
set -euo pipefail

repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo"

files=(.githooks/pre-commit scripts/*.sh tests/e2e/*.sh)
for file in "${files[@]}"; do
    /bin/bash -n "$file"
done

forbidden='declare[[:space:]]+-[a-zA-Z]*A|typeset[[:space:]]+-[a-zA-Z]*A|(^|[^[:alnum:]_])(mapfile|readarray|coproc)([^[:alnum:]_]|$)|\$\{[^}]+(,,|\^\^)'
if grep -En "$forbidden" "${files[@]}" | grep -v '^scripts/check-bash3-compat\.sh:'; then
    echo "Bash 4+ syntax is not allowed in scripts that promise macOS Bash 3.2 support" >&2
    exit 1
fi
