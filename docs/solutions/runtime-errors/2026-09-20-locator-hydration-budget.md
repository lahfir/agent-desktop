---
module: agent-desktop-core
date: 2026-09-20
problem_type: runtime_error
tags: [locator, deadline, hydration, sf-symbols]
---

# Discovery consumed the selected-ref hydration budget

Public `find --app 'SF Symbols' --role button --first` failed after approximately
five seconds with `Selected locator hydration exceeded its deadline`. This also
failed all three rounds on both revisions in the canonical-find performance run.

`resolve_query_attempt` gave discovery the entire operation deadline. The macOS
observer reads the whole bounded tree before core evaluates selection. A valid
first match could therefore exist before an incomplete later branch, yet the
selected match could no longer be re-resolved and hydrated into a usable ref.

Selected-ref materialization now reserves up to one second, limited to half the
remaining duration, within the existing deadline. It uses Deadline::capped;
hydration retains the original deadline. Count and non-materializing queries
keep their full discovery budget. Selection completeness and identity checks
remain unchanged. No extra timeout, mutation retry, native special case or
alternate dispatcher is added.

This deliberately gives discovery less time when hydration is required. Dense
queries whose match appears only near the old deadline may still fail. Native
early termination on a proven selected prefix remains a potential future repair;
this change does not implement it or promise arbitrary dense-tree coverage.

The deadline regression uses structural capped-deadline assertions, without
sleeping or tight timing tolerances. Eighty-three locator tests, full workspace
tests, formatting, diff checks and Clippy passed. The completed performance
comparison is `/tmp/ad-native-eval/hydration-budget-performance/report.html`.
SF Symbols first-button find passed all three candidate rounds (p50 4079.8 ms)
and failed all three baseline rounds. All snapshot modes succeeded on both;
depth-30 shapes differ, so their latency is not content-equivalent. The script's
SF Symbols failure marker includes baseline failures; it is not a candidate
find failure. Three trials do not establish the repeated acceptance suite.

Immutable candidate `hydration-budget-candidate`, SHA-256
`5199dd002b17c9c56e67e22cbf9cd713aaf4dd832a0a6bfdbe7decd67bf105e8`,
returned a first-match ref twice in 4.26 and 3.96 seconds. The first ref,
`@spfk598h1ofka:e1`, resolved with public get in 29 ms. No click was attempted.
Find reported the scrollbar button offscreen while get-states returned an empty
array; record this state-evidence discrepancy separately, not as proven parity.

The original goal remains incomplete: Numbers text writes and commits, repeated
cross-app acceptance, and fully headless menu/drag completion are not established.
