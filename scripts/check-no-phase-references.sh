#!/usr/bin/env bash
#
# Fails when shipped source code references the project's delivery plan.
#
# Phase numbers, sub-phase numbers and plan decision ids are project
# bookkeeping. They answer "when was this written", which stops being true the
# moment the roadmap moves, and they mean nothing to anyone reading the code
# without the plan open beside them. A comment should say what is true about
# the code and why, in terms that survive the plan being rewritten.
#
#   banned:  "Sub-phase 2.2 ships the seam, not the predicate"
#   fine:    "The predicate is deliberately unfilled: filling it needs the
#             Chromium detection this module does not do"
#
# Scope is shipped source only - crates/ and src/. It deliberately does NOT
# cover docs/ or probes/: docs/phases.md IS the delivery plan, and the probe
# corpus is evidence organised by the areas that produced it.
#
# Ledger row ids such as A15-7 are NOT banned. They are evidence citations - a
# pointer to the measurement that forced a decision, the way a comment may cite
# a CVE or an RFC - and they stay true regardless of what happens to the
# roadmap. Add a row to the table below to change that.

set -euo pipefail

run_check() {
    local pattern="$1"
    local description="$2"
    local matches
    if matches="$(grep -rnE --include='*.rs' "$pattern" crates src 2>/dev/null)"; then
        printf '%s\n' "$matches" >&2
        printf '  ^ %s - describe what is true, not when it was built\n\n' "$description" >&2
        return 1
    fi
    return 0
}

failed=0

run_check 'sub-?phase' 'sub-phase reference' || failed=1
run_check '[Pp]hase[[:space:]]+[0-9]' 'phase number reference' || failed=1
run_check 'KTD[0-9]' 'plan decision id' || failed=1
run_check '[Uu]nit[[:space:]]+U?[0-9]' 'plan implementation-unit id' || failed=1

# A bare "2.4" is the same reference with the word filed off, and the checks
# above cannot see it. It cannot simply be banned: doc comments legitimately
# carry version numbers (`v0.5.0`, `pre-1.0`, a `"version":"2.1"` envelope
# literal) and measured ratios (`1.35x`, `sub-1.0`). Those shapes are stripped
# from the line first, and whatever bare `N.N` survives is a plan reference.
#
# Scoped to doc comments, because outside them `0.0` and `1.0` are float
# literals and there are hundreds of them.
# Implemented in awk and by tokenising rather than with word-boundary regexes.
# `\b` is a GNU extension that BSD sed and grep do not support, so a regex
# using it silently stops stripping on the macOS runner and reports every
# version number as a violation. Splitting the line into tokens needs no
# boundary support and is exact on both.
bare_reference_check() {
    local matches
    matches="$(
        grep -rnE --include='*.rs' '^[[:space:]]*(///|//!)' crates src 2>/dev/null |
            awk '
                {
                    text = $0
                    gsub(/v[0-9]+\.[0-9]+(\.[0-9]+)?/, " ", text)
                    gsub(/[0-9]+\.[0-9]+\.[0-9]+/, " ", text)
                    gsub(/-[0-9]+\.[0-9]+/, " ", text)
                    gsub(/[0-9]+\.[0-9]+x/, " ", text)
                    gsub(/"[0-9]+\.[0-9]+"/, " ", text)
                    gsub(/[^0-9.]/, " ", text)
                    n = split(text, tokens, " ")
                    for (i = 1; i <= n; i++) {
                        if (tokens[i] ~ /^[0-9]\.[0-9][0-9]?$/) {
                            print
                            next
                        }
                    }
                }
            ' || true
    )"
    if [ -n "$matches" ]; then
        printf '%s\n' "$matches" >&2
        printf '  ^ bare plan reference (a sub-phase number) - name the thing, not the slice of roadmap that shipped it\n\n' >&2
        return 1
    fi
    return 0
}

bare_reference_check || failed=1

if [ "$failed" -ne 0 ]; then
    printf 'Shipped source must not reference the delivery plan.\n' >&2
    printf 'Rewrite the comment so it explains the code, not the roadmap.\n' >&2
    exit 1
fi
