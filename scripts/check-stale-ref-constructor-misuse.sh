#!/usr/bin/env bash
#
# Fails when a new non-test caller reaches for `AdapterError::stale_ref` or
# `AppError::stale_ref` outside their one legitimate shape: a **ref id**,
# interpolated into "{ref_id} not found in current RefMap". Handing either
# constructor a whole sentence instead produces a message that is both
# ungrammatical and wrong about the cause - the ref was found and read, and
# it was the live evidence that refused it, not a missing RefMap entry.
#
# A caller with that shape must build its error directly -
# `AdapterError::new(ErrorCode::StaleRef, message)` with the snapshot-refresh
# suggestion and the not-delivered disposition - exactly as
# `crates/windows/src/tree/resolve_match.rs`'s `stale_evidence_error` already
# does.
#
# This is a call-site count, not a `debug_assert`: a runtime assertion would
# panic the two test fixtures that legitimately hand these constructors a
# non-ref-id placeholder to exercise unrelated behaviour, and would be
# compiled out of release builds anyway.

set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

EXPECTED_ADAPTER_CALLS=2
EXPECTED_APP_CALLS=2

# Counts a constructor's non-test call sites under crates/ and src/.
#
# "Non-test" excludes two shapes, because neither is production code:
#   - a `*_tests.rs` file. This repo's convention is a parent module
#     declaring `#[cfg(test)] #[path = "x_tests.rs"] mod tests;` - the
#     `#[cfg(test)]` attribute lives in the PARENT file, so the test file
#     itself carries no marker a single-file scan could see. Filename is the
#     only signal available for a call site inside one.
#   - an inline `#[cfg(test)]`-gated item within an otherwise production
#     file: a whole `mod tests { ... }` block (`crates/ffi/src/commands/mod.rs`,
#     `crates/ffi/src/commands/envelope_out.rs`) or a single gated function
#     mixed into production code (`RefStore::for_tests` in
#     `crates/core/src/refs_store.rs`, ahead of that file's own real
#     `stale_ref` caller). Brace-depth tracking skips exactly the item the
#     attribute gates and resumes counting once it closes, rather than
#     truncating the rest of the file at the first `#[cfg(test)]` seen -
#     which would have silently dropped `refs_store.rs`'s real caller from
#     the count. This depth count is a line-by-line character scan with no
#     awareness of string or comment context, so a `{`/`}` pair from a
#     format-string placeholder on the SAME line as the item's own braces
#     could in principle desync it; none of this corpus's `#[cfg(test)]`
#     items contain one.
count_calls() {
    local constructor="$1"
    local total=0
    local file matches

    while IFS= read -r -d '' file; do
        case "$file" in
        *_tests.rs) continue ;;
        esac
        matches="$(awk -v ctor="${constructor}(" '
            BEGIN { depth = 0; pending = 0; skipping = 0; skip_depth = 0; count = 0 }
            {
                line = $0
                if (!skipping) {
                    if (line ~ /#\[cfg\(test\)\]/) { pending = 1 }
                    if (index(line, ctor) > 0) { count++ }
                }
                n = length(line)
                for (i = 1; i <= n; i++) {
                    c = substr(line, i, 1)
                    if (c == "{") {
                        if (pending && !skipping) {
                            skipping = 1
                            skip_depth = depth
                            pending = 0
                        }
                        depth++
                    } else if (c == "}") {
                        depth--
                        if (skipping && depth == skip_depth) { skipping = 0 }
                    }
                }
                if (pending && line ~ /;[ \t]*$/) { pending = 0 }
            }
            END { print count + 0 }
        ' "$file")"
        total=$((total + matches))
    done < <(find crates src -name '*.rs' -print0)

    echo "$total"
}

adapter_calls="$(count_calls "AdapterError::stale_ref")"
app_calls="$(count_calls "AppError::stale_ref")"

failed=0

if [ "$adapter_calls" -ne "$EXPECTED_ADAPTER_CALLS" ]; then
    printf 'FAIL: AdapterError::stale_ref has %s non-test caller(s), expected %s.\n' \
        "$adapter_calls" "$EXPECTED_ADAPTER_CALLS" >&2
    failed=1
fi

if [ "$app_calls" -ne "$EXPECTED_APP_CALLS" ]; then
    printf 'FAIL: AppError::stale_ref has %s non-test caller(s), expected %s.\n' \
        "$app_calls" "$EXPECTED_APP_CALLS" >&2
    failed=1
fi

if [ "$failed" -ne 0 ]; then
    printf '\nA changed count almost always means a whole sentence reached a ref-id\n' >&2
    printf 'constructor. Build the error directly instead:\n' >&2
    printf '  AdapterError::new(ErrorCode::StaleRef, message)\n' >&2
    printf '      .with_suggestion("Run '"'"'snapshot'"'"' to refresh, then retry with the updated ref.")\n' >&2
    printf '      .with_disposition(DeliverySemantics::not_delivered())\n' >&2
    printf 'as crates/windows/src/tree/resolve_match.rs'"'"'s stale_evidence_error already does.\n' >&2
    printf 'If the new caller genuinely passes a ref id, update EXPECTED_*_CALLS above.\n' >&2
    exit 1
fi

printf 'OK: AdapterError::stale_ref %s non-test caller(s), AppError::stale_ref %s.\n' \
    "$adapter_calls" "$app_calls"
