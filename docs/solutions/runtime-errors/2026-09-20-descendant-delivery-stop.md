---
title: Stop descendant activation after unverified delivery
module: macos adapter
problem_type: runtime_error
tags: [activation, delivery, reliability, headless]
date: 2026-09-20
---

# Descendant activation delivery stop

The click and semantic toggle activation chains share activate_descendant. Its
child/action loop returned only verified outcomes. An accepted native action
with no observed effect returned DeliveredUnverified, but the loop discarded
that result and tried another action or sibling before eventually reporting
NotDelivered. This contradicted the outer chain's no-retry delivery contract.

The loop now uses the existing DeliveryOutcome::terminates_chain predicate.
Unverified delivery is retained and stops further mutation; unsupported actions
still allow discovery of a supported sibling. No new fallback or native action
is introduced. This can expose an unverified result where the previous code
silently attempted additional mutations; it does not solve Xcode file opening.

The production loop is exercised directly by two deterministic tests: an
unsupported action followed by unverified delivery makes exactly two calls on
the first child, and unsupported first-child actions can reach a verified second
child without touching a third. The first test fails under the previous
was_verified predicate.

## Validation

Focused regression test and clippy passed. Formatting passed. The first workspace
run inside the sandbox failed two CLI event-wait process tests while reading
missing response data; that failure is retained in
`/tmp/ad-native-eval/descendant-stop-workspace.log`. The native-access full
workspace run passed, including both new regression tests, with its log at
`/tmp/ad-native-eval/descendant-stop-workspace-native.log`.

Immutable candidate `/tmp/ad-native-eval/descendant-stop-candidate`, SHA-256
`5f2bf71761548cd6cbb58b8161b0fbc2dc4bfee033f9d30fec603f93613a2115`.
The owned background semantic fixture passed all seven assertions on this binary.
This fixture covers general delivery regressions, not the descendant fault path;
that path is covered by the deterministic loop tests.

The ten-round merge-base comparison completed with all 80 commands successful.
Candidate/base p50 milliseconds: interactive snapshot 301.9/306.6, skeleton
211.1/203.9, depth-30 296.6/298.5, find 230.5/231.1. Reported first-success shapes
matched. The deep-snapshot slowdown did not reproduce; no performance-specific
code change was justified by these results. Skeleton median increased 7.2 ms in
this sample. The runs show substantial between-run latency variation and do not
establish its cause. Report and JSON are under
`/tmp/ad-native-eval/descendant-stop-performance/`.

Before this repair, a ten-round read-only A/B isolated capability recovery from
the preceding collection-label candidate. Deep-snapshot p50 was 693.1 vs 716.7 ms
and p95 958.5 vs 817.2 ms. The earlier 1094.5 ms median did not recur in that
comparison. Find p50 was 449.0 vs 397.4 ms. All 80 commands succeeded. This does
not prove absence of a regression against merge-base or explain the long tails.
Raw aggregate: `/tmp/ad-native-eval/capability-isolated-perf.json`.

Rating remains provisional 8/10; broad repeatability and file activation remain
unqualified.
