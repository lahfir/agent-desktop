---
title: Stop container-selection fallback after unverified delivery
module: macos adapter
problem_type: runtime_error
tags: [selection, delivery, headless, retries]
date: 2026-09-20
---

# Selection delivery guard

Tracing the unresolved Xcode select-by-name failure exposed a prerequisite
reliability defect in `actions/container_select.rs`. Direct AXSelected writes
returned false when readback was not true, even after accepted delivery. The
caller could then try container attributes. Container array writes also ignored
native error classification and tried another attribute after an accepted but
unverified write. This could duplicate effects or act on broader ancestors.

The existing shared native mutation classifier now handles container writes.
Both direct and container writes use one verification guard: only explicit
non-delivery permits fallback; accepted but unverifiable delivery returns
ACTION_FAILED with delivered_unverified / retry unsafe. No focus, cursor,
clipboard or keyboard behavior changed. Successful verified selections and
explicit unsupported paths retain their existing behavior.

Immutable candidate: `/tmp/ad-native-eval/selection-delivery-candidate`, SHA-256
`061572c91835ba8e9cf224c6e64352075b857d3674d9eb6164ec9ea16db505a2`.

## Evidence

Three container-selection tests passed, including a counted attempt loop that
must stop after the first accepted write with failed readback. Unsupported
delivery must not invoke readback; verified delivery succeeds. Full workspace,
clippy with warnings denied, format and diff checks passed. Initial test runs
exposed an explicit Result type omission and an assertion against the unrelated
default-retry override flag; both test errors were corrected before final gates.
Final workspace log: `/tmp/ad-native-eval/selection-delivery-workspace-verified.log`.

The native diagnostic `tests/real-apps/selection_delivery_fixture.swift` exposes
a row whose selected setter logs a write but whose getter remains false, plus a
container setter that independently logs fallback writes. Public snapshot
returned row `@s3dc99lneq3yhr:e2` in owned window w-19929, PID 35151.
One public click returned ACTION_FAILED, delivered_unverified, retry unsafe.
The native process logged exactly one `MUTATION direct-selection true` and no
container-selection mutation. It was then closed after verifying its executable.

The diagnostic fixture was already frontmost at monitor start. Its 30-second
monitor recorded no additional activation/input/pointer movement, but this trial
is not evidence of background operation. Its startup must not be reused as a
headless acceptance fixture without correcting and verifying that behavior.

The first standard safe-semantic suite attempt failed its initial process-token
checkpoint before test actions; cleanup also refused the changed token. That
attempt remains in `/tmp/ad-native-eval/selection-delivery-semantic.log`. A fresh
trial began only after pgrep confirmed that no AgentDeskFixture process remained.
The precise cause of the token change is not established.
The fresh trial passed all seven semantic assertions on the exact candidate
hash, including never bringing the fixture frontmost. Log:
`/tmp/ad-native-eval/selection-delivery-semantic-run2.log`.

Read-only Xcode performance completed three rounds per case, 12/12 candidate
and baseline successes, with matching corresponding tree shapes. Candidate/base
p50 ms: interactive 306.0/296.3, skeleton 203.9/200.9, depth-30 295.3/291.3,
first-button find 256.1/222.5. Find p50 increased 33.6 ms; this small cumulative
branch comparison cannot attribute the difference to this mutation-only guard.
Report: `/tmp/ad-native-eval/selection-delivery-performance/report.html`.

Xcode label matching and row-selection-versus-file-opening remain unresolved.
This repair is a prerequisite for safely reusing the shared selection path,
not proof of completing those tasks. Provisional rating remains 8/10; the goal
is active and repeated acceptance coverage is still required. Nothing pushed.
