---
title: One measurement is not a measurement
date: 2026-08-01
category: best-practices
module: probes/windows/15-vocabulary
problem_type: process_gap
component: tooling
symptoms:
  - "A single timed sample shows a strictly larger property set reading faster than a smaller one."
  - "A repeated identical baseline differs from itself by more than 2x between two consecutive runs."
  - "A performance ratio range measured on one machine falls outside the range when re-measured on CI."
root_cause: process_gap
resolution_type: methodology_change
severity: medium
tags: [performance, measurement, ci, windows, benchmarking, min-of-n]
---

# One measurement is not a measurement

## Problem

The Windows vocabulary work (2026-08-01) needed a real number to decide whether
`WALK_SET` — the properties read on every UI Automation node — should ship
as one flat prefetch or split into a core set plus a conditional
pattern-state fetch. The performance claim behind that decision was wrong
twice, in opposite directions, before it was trustworthy.

## Root cause

**First wrong: noise disguised as a result.** `probes/windows/FINDINGS.md`
row A15-13 records it directly: *"the first form of the A15-11 measurement
took one sample per property set and produced a non-monotonic ordering in
which adding twelve properties measured faster than omitting them, and in
which a repeated identical baseline differed from itself by 2.18x."* A
single timed sample cannot distinguish the property set's real cost from
scheduler jitter and cold-cache effects on the walk that happened to run
first — and scheduler/cache interference is one-sided: it can only make a
run slower, never faster, so its noise is asymmetric rather than
zero-centered. The fix in `probes/windows/15-vocabulary/probe_cost.rs`
(`measure_batch_cost`, lines 49-70) is exactly the standard cure for
one-sided noise: run a discarded warm-up first (so a lazily-built provider's
peer-construction cost isn't charged to whichever set runs first), then take
the **minimum of seven repeats** (`BATCH_REPEATS`) and report it alongside
the median. Row A15-13 records this as its own ledger row rather than
folding it into A15-11 or A15-12: *"recorded as a methodology row rather
than a finding about UI Automation: a timing claim in this corpus needs
repetition and a warm-up."*

**Then wrong again, differently.** With the min-of-seven method in place,
row A15-12 measured the core/conditional split against the flat `WALK_SET`
and an earlier form of that same row read *"0.95x to 1.05x, no recovery in
either direction."* That range was min-only, taken on the dev box alone, and
did not hold: re-measured across four runs the split landed at **0.80x to
1.08x** — cheaper on the out-of-process WPF provider (0.91x on min, 0.80x on
median, 26 round trips over 81 nodes) and at-or-above parity on the
in-process Win32 proxy (1.01x-1.07x, once 0.70x on a single min). The two
environments disagree about the one case that looked like a win: the hosted
Server 2025 CI runner measured the same split at 1.07x-1.08x on the Win32
fixture and **0.99x-1.03x** on the WPF fixture — no recovery on either —
where the dev box's own sub-1.0 WPF readings had suggested the split paid
for itself. The row's own conclusion: *"the sub-1.0 WPF readings are a
dev-box artifact rather than a property of the design."*

## Solution

Two corrections, not one. Repeating the measurement (warm-up discarded,
min-of-seven, median reported alongside it) fixed the non-monotonic result
and made the number reproducible. It did not, by itself, make the number
generalise — the corrected range still needed a second environment before
it could be written down as a fact. `probes/windows/FINDINGS.md`'s Area 15
header records that every row here was measured on both "this ledger's dev
box" and the hosted `windows-latest` runner through
`.github/workflows/windows-capability-probe.yml`, and it was exactly that
second environment that overturned the narrower dev-box-only range. Both
figures — the honest 0.80x-1.08x spread and the flat-set decision it
produced — are what shipped; the split was *"designed, measured and
rejected"* on the honest range, not on the best case.

## Prevention

- **Repetition fixes noise; it does not fix environment.** Min-of-N gives a
  stable number for the box you ran it on. A cost claim needs to hold on at
  least a second environment — a dev box and a CI runner differ in core
  count, power profile, and contention — before it is written down as fact.
- **Report the distribution, not the best case.** A minimum is the right
  statistic for "how fast can this go," and the wrong one for "what will
  this cost." Quoting a min-derived range as the expected range is exactly
  how the second error happened here; report min alongside median (and, for
  a spread this wide, the max) so a reader can see how much the number
  moves.
- When the method used to measure something changes, that is itself a
  finding, not an implementation detail to fold into the row it corrects. A
  later reader comparing numbers across rows needs to know they were
  produced by different methods — hence A15-13 exists as its own row rather
  than as a footnote on A15-11.
- A single unrepeated timed sample is not evidence. Treat it the same as an
  untested assumption: cheap to produce, and capable of pointing the
  decision the wrong way.
