#!/usr/bin/env bash
set -euo pipefail

limit=400
failed=0

while IFS= read -r file; do
    [ -f "$file" ] || continue
    if head -n 5 "$file" | grep -q '@generated'; then
        continue
    fi
    lines="$(wc -l < "$file" | tr -d ' ')"
    if [ "$lines" -gt "$limit" ]; then
        printf '%s: %s lines (limit %s)\n' "$file" "$lines" "$limit" >&2
        failed=1
    fi
done < <(git ls-files --cached --others --exclude-standard -- '*.rs')

# The comment half of this gate is only a gate if it can run. Hardcoding
# `python3` makes it unrunnable on a Windows developer box, where the
# interpreter is `python` or `py`: the run then fails for the wrong reason and
# reads as environment noise rather than a real violation, which is how a
# comment rule stops being enforced anywhere but CI.
interpreter=""
for candidate in python3 python py; do
    if command -v "$candidate" >/dev/null 2>&1 && "$candidate" -c '' >/dev/null 2>&1; then
        interpreter="$candidate"
        break
    fi
done

if [ -z "$interpreter" ]; then
    printf 'no working Python 3 interpreter on PATH (tried python3, python, py)\n' >&2
    exit 1
fi

if ! git ls-files -z --cached --others --exclude-standard -- '*.rs' \
    | "$interpreter" scripts/check_rust_comments.py; then
    failed=1
fi

if [ "$failed" -ne 0 ]; then
    exit 1
fi
