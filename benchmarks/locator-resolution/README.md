# Locator resolution benchmark

This deterministic harness compares the legacy full-snapshot locator path with
the handle-free observed-tree evaluator over synthetic Chromium/Electron accessibility
trees. It exercises deep anonymous wrapper chains, duplicate role/name
candidates, moving bounds, simultaneous `AXIdentifier` and `AXDOMIdentifier`
values, containment predicates, and large trees.

Run it without network access:

```bash
rtk cargo run -p agent-desktop-core --release --example locator_benchmark \
  > /private/tmp/agent-desktop-locator-synthetic.json
```

The JSON report includes 31-run p50/p95 wall-clock latency, candidate nodes,
predicate work, requested synthetic attribute values, cardinality, and
correctness. `live_find_selected_refs` uses the same selected-match-only
materialization contract as the default CLI `find` path. `live_arena_direct` is
reported separately as a lower-overhead direct-target metric, and
`live_count_no_refmap` verifies that count-only requests perform no action or
settable evidence reads. The harness does not claim absolute native AX calls:
role-dependent action and settable probes make that model inaccurate. Native
Slack/Electron IPC measurements require Accessibility permission and belong in
the privileged macOS integration suite.

This synthetic benchmark JSON is consumed by the repo-wide performance report:
run `bash scripts/perf-baseline-compare.sh` (optionally with `--apps "Slack,Google
Chrome"`) to produce `report.html`, covering the HEAD-vs-main fixture A/B,
optional real-app read-only probes, and this synthetic benchmark.

For privileged Electron/Chromium runs, the macOS adapter first asks whether the
application root exposes `AXManualAccessibility` as settable, reads its current
value, and sets it at most once when supported. The adapter then waits within
the request's existing absolute deadline until the attribute reports ready,
recording attempted/succeeded/ready activation in locator stats. Unsupported
native applications are left unchanged. This follows
[Electron's official third-party accessibility guidance](https://github.com/electron/electron/blob/main/docs/tutorial/accessibility.md),
which documents setting `AXManualAccessibility` from native assistive software
to expose Chromium's accessibility tree before automatic assistive-technology
detection has enabled it. The
[current Electron macOS implementation](https://github.com/electron/electron/blob/main/shell/browser/mac/electron_application.mm)
advertises the attribute as settable, reports `true` only in complete mode, and
handles `AXEnhancedUserInterface` separately; that source behavior is why the
benchmark methodology requires a post-set readiness read instead of treating a
successful setter as immediate readiness.
