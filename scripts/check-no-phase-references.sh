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
# The awk program both the real scan and the self-test run, so the fixture
# below is testing the shipped rule rather than a copy of it.
#
# The strips are deliberately narrow. An earlier version stripped any
# `-<digit>.<digit>`, which silently swallowed `phase-2.4` - the exact thing
# the check exists to catch - while innocent prose decimals like "a 2.5 ms
# timeout" still fired. Each strip now names the idiom it permits.
# Single quotes are load-bearing: `$0` is awk's whole-line variable and must
# reach awk unexpanded. Double quotes would have the shell substitute its own
# `$0` - the script's name - into the program text.
# shellcheck disable=SC2016
BARE_REFERENCE_AWK='
{
    text = $0
    gsub(/v[0-9]+\.[0-9]+(\.[0-9]+)?/, " ", text)
    gsub(/[0-9]+\.[0-9]+\.[0-9]+/, " ", text)
    gsub(/(pre|post|sub|over|under)-[0-9]+\.[0-9]+/, " ", text)
    gsub(/[0-9]+\.[0-9]+ ?(x|ms|s|us|%|MiB|MB|KB|GB)([^A-Za-z]|$)/, " ", text)
    gsub(/"[0-9]+\.[0-9]+"/, " ", text)
    gsub(/`[0-9]+\.[0-9]+`/, " ", text)
    gsub(/[^0-9.]/, " ", text)
    n = split(text, tokens, " ")
    for (i = 1; i <= n; i++) {
        if (tokens[i] ~ /^[0-9]\.[0-9][0-9]?$/) {
            print
            next
        }
    }
}'

bare_reference_check() {
    local matches
    matches="$(
        grep -rnE --include='*.rs' '^[[:space:]]*(///|//!)' crates src 2>/dev/null |
            awk "$BARE_REFERENCE_AWK" || true
    )"
    if [ -n "$matches" ]; then
        printf '%s\n' "$matches" >&2
        printf '  ^ bare plan reference (a sub-phase number) - name the thing, not the slice of roadmap that shipped it\n\n' >&2
        return 1
    fi
    return 0
}

# A bare "U4" implementation-unit id is the same reference as "unit U4" with
# the word filed off, and `run_check`'s '[Uu]nit[[:space:]]+U?[0-9]' pattern
# above cannot see it: it requires the literal word "unit" immediately
# before, so prose like "the filter U4 encodes" or "a U1 item-11 branch"
# passes it clean.
#
# Tokenized on the same principle as BARE_REFERENCE_AWK above, so a real Rust
# type name is never mistaken for a plan reference: `u32`/`u8` are lowercase
# and the pattern requires an uppercase `U`; `U+0041` unicode notation is
# split by the `+` into separate `U` and `0041` tokens, so the bare `U` token
# has no trailing digits to match; and an uppercase ledger citation like
# `A16-11` never produces a standalone `U<digit>` token because the `A16` and
# `11` stay glued to their own alphanumeric runs.
# shellcheck disable=SC2016
BARE_UNIT_AWK='
{
    n = split($0, tokens, /[^A-Za-z0-9]+/)
    for (i = 1; i <= n; i++) {
        if (tokens[i] ~ /^U[0-9]{1,2}$/) {
            print
            next
        }
    }
}'

bare_unit_check() {
    local matches
    matches="$(
        grep -rnE --include='*.rs' '^[[:space:]]*(///|//!)' crates src 2>/dev/null |
            awk "$BARE_UNIT_AWK" || true
    )"
    if [ -n "$matches" ]; then
        printf '%s\n' "$matches" >&2
        printf '  ^ bare plan reference (an implementation-unit id) - name the thing, not the unit that shipped it\n\n' >&2
        return 1
    fi
    return 0
}

# A gate whose own allow/deny set is untested is a gate nobody can trust. Both
# of the cases marked MUST-CATCH below were live defects: `phase-2.4` escaped
# and `2.5 ms` false-fired. Every fixture line runs against both awk programs
# together, exactly as the real scan combines `bare_reference_check` and
# `bare_unit_check` into one `failed` outcome, so this exercises the same
# rule text the scan runs rather than a copy of it.
self_test() {
    local must_catch must_pass caught passed failures
    must_catch='/// see phase-2.4 for details
/// 2.2 ships the seam
/// the 2.4 evidence field
/// as of 2.10 this is owned elsewhere
/// A U1 item-11 branch decided this
/// The filter U4 encodes'
    # shellcheck disable=SC2016
    must_pass='/// pre-1.0 and sub-1.0 readings
/// v0.5.0 deleted the layer
/// the envelope is "2.1" on the wire
/// collapsing a failed read to `1.0` is a claim
/// measured 1.35x against 0.80x
/// a 2.5 ms timeout and a 1.5 s deadline
/// uiautomation 0.25.0
/// stored as u32 and read back as u8
/// the character U+0041 renders as A
/// see A16-11 for the measured margins'
    failures=0
    while IFS= read -r line; do
        caught="$(
            printf '%s\n' "$line" | awk "$BARE_REFERENCE_AWK"
            printf '%s\n' "$line" | awk "$BARE_UNIT_AWK"
        )"
        if [ -z "$caught" ]; then
            printf 'self-test FAIL (missed): %s\n' "$line" >&2
            failures=1
        fi
    done <<< "$must_catch"
    while IFS= read -r line; do
        passed="$(
            printf '%s\n' "$line" | awk "$BARE_REFERENCE_AWK"
            printf '%s\n' "$line" | awk "$BARE_UNIT_AWK"
        )"
        if [ -n "$passed" ]; then
            printf 'self-test FAIL (false positive): %s\n' "$line" >&2
            failures=1
        fi
    done <<< "$must_pass"
    if [ "$failures" -ne 0 ]; then
        printf 'The bare-reference rule does not behave as documented.\n' >&2
        return 1
    fi
    return 0
}

self_test || failed=1

bare_reference_check || failed=1

bare_unit_check || failed=1

if [ "$failed" -ne 0 ]; then
    printf 'Shipped source must not reference the delivery plan.\n' >&2
    printf 'Rewrite the comment so it explains the code, not the roadmap.\n' >&2
    exit 1
fi
