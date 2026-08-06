#!/usr/bin/env bash
#
# Fails when a script that promises macOS Bash 3.2 support uses a construct
# only Bash 4 or later understands.
#
# The `bash -n` pass below cannot find these. Every developer box and CI runner
# here parses with Bash 4 or 5, where the constructs are valid, so a clean
# parse proves the file parses on the machine running the check - never that it
# parses on the 3.2 that macOS ships. The pattern list is therefore the entire
# gate, and a construct missing from it is a construct that reaches macOS
# unchecked. The self-test drives every entry in both directions, so a pattern
# that has stopped matching fails the gate rather than passing it in silence.

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

files=(.githooks/pre-commit scripts/*.sh tests/e2e/*.sh)
for file in "${files[@]}"; do
    /bin/bash -n "$file"
done

# The single owner of the rule. The tree scan and `self_test` both call
# `bash4_hits`, so the fixtures exercise the shipped pattern rather than a copy
# of it that can drift away.
#
# Associative arrays are matched through every keyword that can declare one.
# Banning `declare -A` and `typeset -A` alone left `local -A` and `readonly -A`
# - the same Bash 4 construct spelled differently - passing clean. The flag run
# is matched on both sides of the `A` so a combined `-gA` or `-Ag` is caught,
# and a lowercase `-a` indexed array, which Bash 3.2 supports, is not.
#
# `globstar` and `wait -n` are matched as bare words because neither has any
# meaning in Bash 3.2: `shopt -s globstar` errors there, and `wait -n` waits on
# a job literally named `-n`.
BASH4_PATTERN='(^|[^[:alnum:]_])(declare|typeset|local|readonly)[[:space:]]+-[a-zA-Z]*A[a-zA-Z]*([^[:alnum:]_]|$)'
BASH4_PATTERN="$BASH4_PATTERN"'|(^|[^[:alnum:]_])(mapfile|readarray|coproc)([^[:alnum:]_]|$)'
BASH4_PATTERN="$BASH4_PATTERN"'|\$\{[^}]+(,,|\^\^)'
BASH4_PATTERN="$BASH4_PATTERN"'|\$\{[^}]*@[QEPAKLUua]\}'
BASH4_PATTERN="$BASH4_PATTERN"'|(^|[^[:alnum:]_])globstar([^[:alnum:]_]|$)'
BASH4_PATTERN="$BASH4_PATTERN"'|(^|[^[:alnum:]_])wait[[:space:]]+-n([^[:alnum:]_]|$)'

bash4_hits() {
    grep -En "$BASH4_PATTERN" "$@"
}

# Both directions are driven, because a pattern that matches everything is as
# useless as one that matches nothing, and only one of the two announces
# itself. The must-pass block is drawn from constructs these scripts really
# use: `declare -a failures=()` in tests/e2e/lib.sh and the `"${files[@]}"`
# expansion three lines above are the near-misses a careless widening breaks.
#
# The fixtures are streams rather than files. `bash4_hits` only ever reads its
# arguments forward, so a process substitution hands it a working path with
# nothing written to disk and nothing to clean up.
self_test() {
    local line failures
    failures=0

    # shellcheck disable=SC2016  # fixture text: the expansions must reach grep unexpanded
    local must_catch=(
        'declare -A registry'
        'typeset -A registry'
        'local -A registry'
        'readonly -A registry'
        'declare -gA registry'
        'declare -Ag registry'
        'mapfile -t lines < input'
        'readarray -t lines < input'
        'coproc worker { cat; }'
        'printf "%s" "${name,,}"'
        'printf "%s" "${name^^}"'
        'printf "%s" "${name@Q}"'
        'shopt -s globstar'
        'wait -n'
    )
    # shellcheck disable=SC2016  # fixture text: the expansions must reach grep unexpanded
    local must_pass=(
        'declare -a failures=()'
        'local -a candidates'
        'for file in "${files[@]}"; do :; done'
        'printf "%s" "${#files[@]}"'
        'printf "%s" "${!options[@]}"'
        'wait "$fixture_pid"'
        'wait'
        'shopt -s nullglob'
        'printf "%s" "${path%%.*}"'
        'printf "%s" "${text//@/$apostrophe}"'
        'echo "maintainer@example"'
        'awk -v target="$package" "{ print }" Cargo.lock'
    )

    for line in "${must_catch[@]}"; do
        if ! bash4_hits <(printf '%s\n' "$line") >/dev/null 2>&1; then
            printf 'self-test FAIL (missed): Bash 4 construct not detected: %s\n' "$line" >&2
            failures=1
        fi
    done
    for line in "${must_pass[@]}"; do
        if bash4_hits <(printf '%s\n' "$line") >/dev/null 2>&1; then
            printf 'self-test FAIL (false positive): Bash 3.2 construct rejected: %s\n' "$line" >&2
            failures=1
        fi
    done

    if [ "$failures" -ne 0 ]; then
        printf 'The Bash 3.2 compatibility rules do not behave as documented.\n' >&2
        return 1
    fi
    return 0
}

self_test

# This file is excluded from its own scan: the pattern string and the fixtures
# above are the constructs written out verbatim, and matching them would fail
# the gate on the gate.
if bash4_hits "${files[@]}" | grep -v '^scripts/check-bash3-compat\.sh:'; then
    echo "Bash 4+ syntax is not allowed in scripts that promise macOS Bash 3.2 support" >&2
    exit 1
fi
