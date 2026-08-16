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
#   banned:  "the split the plan pre-committed to"
#   fine:    "The predicate is deliberately unfilled: filling it needs the
#             Chromium detection this module does not do"
#
# Scope is shipped source only - crates/, src/, and the skill markdown under
# skills/. The skill docs are `include_str!`d into the binary
# (crates/core/src/commands/skills.rs), so a plan id written there is served
# to an agent by `agent-desktop skills` exactly as if it had been typed into
# a .rs file. It deliberately does NOT cover docs/ or probes/: docs/phases.md
# IS the delivery plan, and the probe corpus is evidence organised by the
# areas that produced it.
#
# Ledger row ids such as A15-7 are NOT banned. They are evidence citations - a
# pointer to the measurement that forced a decision, the way a comment may cite
# a CVE or an RFC - and they stay true regardless of what happens to the
# roadmap. Add a row to the tables below to change that.

set -euo pipefail

cd "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

failed=0

# The token rules: ids and numbered slices, which are unambiguous anywhere in a
# file and so are matched against whole files rather than doc comments only.
#
# `token_rules` is the single declaration of the set. The tree scan and
# `self_test` both call it and it applies each pattern according to
# `TOKEN_SCAN_MODE`, so the fixture drives the shipped patterns rather than a
# copy of them that can drift.
#
# The sub-phase pattern matches a capital `Sub-phase` as well: an unnumbered
# "This Sub-phase owns it" escaped both the lowercase-only sub-phase rule and
# the phase-number rule below, which needs a digit.
TOKEN_SCAN_MODE=""
TOKEN_SCAN_LINE=""
TOKEN_SCAN_FAILED=0
# The roots the tree scan walks, overridable so the self-test can drive the real
# scan against a planted fixture tree. Without that override the tree branch has
# no coverage at all and only the line branch is ever exercised, which is how a
# tree scan that could not fail reported clean over real violations.
TOKEN_SCAN_RS_ROOTS="crates src"
TOKEN_SCAN_MD_ROOTS="skills"

token_rule() {
    local pattern="$1"
    local description="$2"
    local matches
    if [ "$TOKEN_SCAN_MODE" = "line" ]; then
        if printf '%s\n' "$TOKEN_SCAN_LINE" | grep -qE "$pattern"; then
            printf '%s\n' "$description"
        fi
        return 0
    fi
    # Test the collected text, never the substitution's exit status. A grouped
    # substitution exits with the status of its LAST command, so testing it made
    # a hit in crates/ or src/ invisible whenever the skills/ grep found nothing
    # - which is the usual case, and is how three plan-decision ids reached
    # shipped source with this gate reporting clean.
    # shellcheck disable=SC2086
    matches="$( { grep -rnE --include='*.rs' "$pattern" $TOKEN_SCAN_RS_ROOTS 2>/dev/null; grep -rnE --include='*.md' "$pattern" $TOKEN_SCAN_MD_ROOTS 2>/dev/null; } )"
    if [ -n "$matches" ]; then
        printf '%s\n' "$matches" >&2
        printf '  ^ %s - describe what is true, not when it was built\n\n' "$description" >&2
        TOKEN_SCAN_FAILED=1
    fi
    return 0
}

token_rules() {
    token_rule '[Ss]ub-?[Pp]hase' 'sub-phase reference'
    token_rule '[Pp]hase[[:space:]]+[0-9]' 'phase number reference'
    token_rule 'KTD[0-9]' 'plan decision id'
    # A bare `R4`/`(R11)` requirement id is the same reference as the ids above:
    # it points into a plan document that renumbers, and means nothing to a reader
    # without that plan open. Anchored to a word boundary so a register name or a
    # hex literal never trips it.
    # A requirement id is unambiguous in only two shapes: parenthesised, or
    # introduced by a citation verb. A bare `R2` is NOT bannable - it is a CPU
    # register name and the tail of "Windows Server 2012 R2", both of which a
    # Windows adapter crate writes legitimately. Matching bare ids failed in
    # both directions at once: it rejected that OS version string while missing
    # every real citation that ended in a period or a comma.
    token_rule '\(R[0-9]+\)|(per|See|see|governs|Governs|required by|forces|satisfies) R[0-9]+([^A-Za-z0-9]|$)' 'plan requirement id'
    token_rule '[Uu]nit[[:space:]]+U?[0-9]' 'plan implementation-unit id'
}

token_check() {
    TOKEN_SCAN_MODE="tree"
    TOKEN_SCAN_FAILED=0
    token_rules
    TOKEN_SCAN_MODE=""
    [ "$TOKEN_SCAN_FAILED" -eq 0 ]
}

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
# timeout" still fired. Each strip now names the idiom it permits -
# including an operating-system version qualified by its OS name (`Windows 8.1`),
# which is a fact about the platform floor, not a slice of this project's roadmap.
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
    gsub(/(Windows|macOS|iOS|Android|Ubuntu) [0-9]+\.[0-9]+(\.[0-9]+)?/, " ", text)
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
# the word filed off, and the `[Uu]nit[[:space:]]+U?[0-9]` pattern above cannot
# see it: it requires the literal word "unit" immediately before, so prose like
# "the filter U4 encodes" or "a U1 item-11 branch" passes it clean.
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

# The prose form. An id or a number is not needed to point at the roadmap:
# "the plan's file-size note names this seam" and "the dogfood report's own
# residual" are the same unreadable reference written out in words, and every
# check above exits 0 on them.
#
# The danger here is the opposite one. "plan" is ordinary English, and a gate
# that fires on it becomes noise everyone learns to bypass. So the rules match
# the *authority construction* rather than the word: the plan possessing
# something, the plan as the subject of a prescriptive verb, or a plan-document
# section named as a source. "Both are planned fast-follows", "this function
# plans the descent" and "the execution plan is rebuilt" are all left alone,
# because none of them appeals to the plan for a decision.
#
# Two things are deliberately NOT banned, and the distinction is the same one
# that exempts ledger row ids:
#   - A filesystem path to committed evidence - `docs/dogfood-reports/`,
#     `docs/plans/<id>-captures/` - is a location a reader can open, and it
#     stays true. Only the spaced prose form "the dogfood report" is a
#     reference to the document as an authority; the hyphenated slug in a path
#     is not matched.
#   - "the dogfood run", "the dogfood census". A run is a measurement, like a
#     probe. It is the *report* that is process bookkeeping.
#   - A bare "origin's". The originating sub-phase is caught through its
#     vocabulary ("exit criteria") instead, because a rectangle's origin is
#     ordinary geometry in this repo and "the origin's x" must stay legal.
#
# Scoped to doc comments, where prose lives; non-doc comments are already
# banned outright by scripts/check_rust_comments.py.
#
# The two-line window matters because doc comments wrap: "the\n/// plan's
# file-size note" is one phrase split across two records, and a per-line rule
# would miss it. A pair is only joined when it is literally adjacent in the
# same file, which for doc-comment lines always means the same paragraph.
#
# `sprintf("%c", 39)` builds the apostrophe: the program text is single-quoted
# so the shell does not expand awk's `$0`, which means a literal `'` cannot
# appear in it. Possessives are normalised to `@` and the rules match that.
# shellcheck disable=SC2016
PLAN_PROSE_AWK='
function squeeze(raw,   n, parts, i, out) {
    n = split(raw, parts, " ")
    out = ""
    for (i = 1; i <= n; i++)
        out = out " " parts[i]
    return out " "
}

function normalise(raw,   text, apostrophe) {
    apostrophe = sprintf("%c", 39)
    text = squeeze(tolower(raw))
    sub(/^ (\/\/\/|\/\/!)/, " ", text)
    gsub(apostrophe, "@", text)
    gsub(/[`*]/, "", text)
    return text
}

function offence(raw,   text) {
    text = squeeze(raw)
    if (text ~ /[^a-z]plan@s/)
        return "the delivery plan cited as an authority (possessive)"
    if (text ~ /[^a-z]phases\.md/)
        return "a citation of the delivery plan document"
    if (text ~ /[^a-z]dogfood reports?[^a-z]/)
        return "a delivery report cited as an authority"
    if (text ~ /[^a-z]roadmap/)
        return "a reference to the roadmap"
    if (text ~ /[^a-z]the (delivery )?plan,? (says|said|names|named|notes|noted|forbids|forbade|requires|required|calls for|called for|specifies|specified|mandates|mandated|allows|allowed|permits|permitted|pre-commits|pre-committed|precommitted|commits to|committed to|decides|decided|assigns|assigned|owns|owned|scopes|scoped|defers|deferred|expects|expected|anticipates|anticipated|promises|promised|underestimates|underestimated|wants|wanted)[^a-z]/)
        return "the delivery plan cited as an authority"
    if (text ~ /[^a-z](per|according to|pursuant to|against|in|from|under|by|beyond|outside) the (delivery )?plan[^a-z]/)
        return "the delivery plan cited as an authority"
    if (text ~ /[^a-z](exit criteri(a|on)|acceptance criteri(a|on)|definition of done|verification contract|implementation unit|plan document|plan doc|residual list)[^a-z]/)
        return "delivery-plan section vocabulary"
    return ""
}

{
    colon1 = index($0, ":")
    tail = substr($0, colon1 + 1)
    colon2 = index(tail, ":")
    file = substr($0, 1, colon1 - 1)
    lineno = substr(tail, 1, colon2 - 1) + 0
    body = normalise(substr(tail, colon2 + 1))

    reason = offence(body)
    if (reason != "") {
        print $0
        print "  ^ " reason
    } else if (file == prev_file && lineno == prev_lineno + 1 && prev_reason == "") {
        wrapped = offence(prev_body " " body)
        if (wrapped != "") {
            print prev_record
            print "  ^ " wrapped ", wrapped onto the following line"
        }
    }
    prev_file = file
    prev_lineno = lineno
    prev_body = body
    prev_record = $0
    prev_reason = reason
}'

plan_prose_check() {
    local matches
    matches="$(
        grep -rnE --include='*.rs' '^[[:space:]]*(///|//!)' crates src 2>/dev/null |
            awk "$PLAN_PROSE_AWK" || true
    )"
    if [ -n "$matches" ]; then
        printf '%s\n' "$matches" >&2
        printf '  ^ the plan is not readable from the code - state the fact, not what the plan said about it\n\n' >&2
        return 1
    fi
    return 0
}

# Every rule in this file, applied to one line, exactly as the tree scan
# applies them collectively. Every fixture line below therefore runs against
# every rule, so a new rule that false-fires on an old fixture line is caught
# by the old fixture.
all_offences() {
    local line="$1"
    TOKEN_SCAN_MODE="line"
    TOKEN_SCAN_LINE="$line"
    token_rules
    TOKEN_SCAN_MODE=""
    printf '%s\n' "$line" | awk "$BARE_REFERENCE_AWK"
    printf '%s\n' "$line" | awk "$BARE_UNIT_AWK"
    printf '%s\n' "fixture.rs:1:$line" | awk "$PLAN_PROSE_AWK"
}

# A gate whose own allow/deny set is untested is a gate nobody can trust. The
# numbered cases below were live defects: `phase-2.4` escaped and `2.5 ms`
# false-fired. The prose cases are the shapes that were sitting in shipped
# source while this gate exited 0.
self_test() {
    local must_catch must_pass caught passed failures apostrophe
    local wrapped_catch wrapped_pass
    # Single quotes are load-bearing in both fixtures: the backticks are
    # fixture text a real doc comment carries, and the rules must see them
    # rather than a command substitution the shell already ran.
    # shellcheck disable=SC2016
    must_catch='/// see phase-2.4 for details
/// on (R5): any other code aborts the wait
/// this behavior is required by R7.
/// see R7, the requirement that forces this
/// Governs R11 and R12
/// 2.2 ships the seam
/// the 2.4 evidence field
/// as of 2.10 this is owned elsewhere
/// A U1 item-11 branch decided this
/// The filter U4 encodes
/// this sub-phase owns the seam
/// a later Sub-phase fills it
/// unit U7 built the fallback
/// see KTD5 for the decision
/// (the plan@s file-size note names this seam)
/// The plan@s own note is what the second half pins
/// the conditional split the plan pre-committed to
/// a real app is off limits (the plan forbids naming one in CI)
/// The plan says the resolver owns this
/// the plan requires a bounded walk here
/// what the plan calls for is a seam
/// The dogfood report@s own residual is what this pins
/// per the plan, this arm stays unfilled
/// see `docs/phases.md`@s Hardening and Integration Review scope
/// the three-run clause of the Verification Contract@s live row
/// the origin@s exit criteria are already met
/// the roadmap moves this to a later owner'
    # shellcheck disable=SC2016
    must_pass='/// pre-1.0 and sub-1.0 readings
/// the R2 register holds the return value
/// Windows Server 2012 R2 is the floor
/// R2 and R3 are scratch registers
/// see A23-1 for the measurement that forced this
/// v0.5.0 deleted the layer
/// the envelope is "2.1" on the wire
/// collapsing a failed read to `1.0` is a claim
/// measured 1.35x against 0.80x
/// a 2.5 ms timeout and a 1.5 s deadline
/// uiautomation 0.25.0
/// stored as u32 and read back as u8
/// the character U+0041 renders as A
/// see A16-11 for the measured margins
/// Both are planned fast-follows to this release
/// this function plans the descent before it walks
/// the execution plan is rebuilt when the cache misses
/// planning the walk ahead of time avoids a second pass
/// the captures under `docs/plans/2026-07-27-002-captures/` hold the evidence
/// recorded as a residual in `docs/dogfood-reports/`
/// the dogfood run is why `invalid` is unproduced
/// regenerated on demand by `probes/windows/scratch/run-dogfood.ps1`
/// the original zeroes it and makes a second release a no-op
/// a rectangle whose origin is its top-left corner
/// the flag exists on Windows 8.1 and later
/// requires macOS 14.2 or newer'
    failures=0
    # The fixture is authored with `@` where a real comment carries an
    # apostrophe, because a literal apostrophe cannot appear inside the
    # single-quoted blocks above. It is converted back before the rules see it,
    # so the fixture exercises the real character the tree contains rather than
    # the placeholder the rules normalise it to.
    apostrophe="'"
    must_catch="${must_catch//@/$apostrophe}"
    must_pass="${must_pass//@/$apostrophe}"
    while IFS= read -r line; do
        caught="$(all_offences "$line")"
        if [ -z "$caught" ]; then
            printf 'self-test FAIL (missed): %s\n' "$line" >&2
            failures=1
        fi
    done <<<"$must_catch"
    while IFS= read -r line; do
        passed="$(all_offences "$line")"
        if [ -n "$passed" ]; then
            printf 'self-test FAIL (false positive): %s\n' "$line" >&2
            printf '                              %s\n' "$passed" >&2
            failures=1
        fi
    done <<<"$must_pass"
    # The wrap fixtures are record streams rather than single lines: neither
    # half carries the phrase on its own, so they fail if the two-line window
    # stops joining, and the negative fails if it joins anything adjacent.
    wrapped_catch='wrap.rs:1:/// the conditional split the plan
wrap.rs:2:/// pre-committed to was measured head to head'
    wrapped_pass='wrap.rs:1:/// what this returns is the
wrap.rs:2:/// planned fast-follow to the current read'
    if [ -z "$(printf '%s\n' "$wrapped_catch" | awk "$PLAN_PROSE_AWK")" ]; then
        printf 'self-test FAIL (missed): a plan reference wrapped across two doc-comment lines\n' >&2
        failures=1
    fi
    if [ -n "$(printf '%s\n' "$wrapped_pass" | awk "$PLAN_PROSE_AWK")" ]; then
        printf 'self-test FAIL (false positive): two adjacent doc-comment lines joined into a reference\n' >&2
        failures=1
    fi
    # A fixed fixture path, reused and overwritten every run rather than a fresh
    # mktemp per run. The harness contract forbids `rm` in these scripts, so the
    # fixture cannot clean up after itself; a stable path bounds what it leaves
    # behind at one directory instead of one per invocation.
    local tree_root planted_rs
    tree_root="${TMPDIR:-/tmp}/agent-desktop-phase-reference-selftest"
    mkdir -p "$tree_root/crates/x/src" "$tree_root/skills"
    planted_rs="$tree_root/crates/x/src/planted.rs"
    printf '/// see KTD7 for the decision
pub fn f() {}
' > "$planted_rs"
    TOKEN_SCAN_RS_ROOTS="$tree_root/crates $tree_root/src"
    TOKEN_SCAN_MD_ROOTS="$tree_root/skills"
    if token_check 2>/dev/null; then
        printf 'self-test FAIL (missed): the tree scan passed a planted plan-decision id
' >&2
        failures=1
    fi
    printf '/// describes what the code does
pub fn f() {}
' > "$planted_rs"
    if ! token_check 2>/dev/null; then
        printf 'self-test FAIL (false positive): the tree scan failed a clean fixture tree
' >&2
        failures=1
    fi
    TOKEN_SCAN_RS_ROOTS="crates src"
    TOKEN_SCAN_MD_ROOTS="skills"

    if [ "$failures" -ne 0 ]; then
        printf 'The delivery-plan reference rules do not behave as documented.\n' >&2
        return 1
    fi
    return 0
}

self_test || failed=1

token_check || failed=1

bare_reference_check || failed=1

bare_unit_check || failed=1

plan_prose_check || failed=1

if [ "$failed" -ne 0 ]; then
    printf 'Shipped source must not reference the delivery plan.\n' >&2
    printf 'Rewrite the comment so it explains the code, not the roadmap.\n' >&2
    exit 1
fi
