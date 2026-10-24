---
title: A cited measurement must match its capture
date: 2026-08-09
category: best-practices
module: probes/windows/FINDINGS.md, crates/windows/src/system/window/window_activate.rs
problem_type: process_gap
component: tooling
symptoms:
  - "A ledger row's observed cell states the opposite of the JSON capture it names."
  - "A shipped doc comment cites a probe row for a fact the probe never measured."
  - "The design is defensible, the evidence sentence under it is false, and every reader downstream inherits the false sentence."
applies_when:
  - "Writing a probe ledger row from a capture"
  - "Citing a probe row in shipped source or in a design document"
  - "Reviewing a change whose rationale rests on a measurement"
root_cause: process_gap
resolution_type: reread_the_artifact_not_the_expectation
severity: high
tags: [evidence, probes, ledger, citations, review, windows]
---

# A cited measurement must match its capture

## Problem

Probe row `A21-6` recorded that an uncontended `SetForegroundWindow` lands on the first attempt every time. The capture it names, committed beside the row, recorded the opposite: the first attempt did not take ownership, and a second attempt was needed.

The false sentence propagated. A design document that sized the retry budget repeated it, and a doc comment on the shipped constant cited the row id. The product's own source claimed a measurement its evidence contradicted. No gate failed, because no gate compared a row's prose to the JSON it names.

## Root cause

The row was written from the expectation that motivated the probe, not from the artifact. The design did not depend on the answer, since a retry budget of two is correct either way, so the wrong number changed no behaviour and drew no attention. Downstream readers cite the row rather than the capture. That makes the row a single point of failure that nobody re-derives.

## Solution

Correct the row against the capture, and correct every site that repeated it, in the same change. Keep the design when the evidence still supports it: a first-attempt rate of zero supports a bounded retry more strongly than a perfect rate would.

Check a row by reading the fields it names, not the fields you expected the capture to have:

```bash
python -c "import json,io; d=json.load(io.open('<capture>.json',encoding='utf-8-sig')); print(d)"
```

Two properties make a row auditable:

- Quote the field names and values the capture uses. A row that says `first_attempt_success_rate: 0` can be diffed against the JSON. A row that says "the first attempt is reliable" cannot.
- Record the verdict the probe wrote (`branch`, `measurable`) rather than paraphrasing it.

## Prevention

- Write the row from the capture open beside you, never from the question the probe was written to answer. A row written without re-reading its capture is unverified.
- When a finding is the answer the author hoped for, re-read the capture once more. Confirming results get less scrutiny than contradicting ones, which is backwards.
- Open the capture before citing a row in shipped source or a design document. Repeating a row is not evidence that it is true.
- When correcting a row, correct every downstream repetition in the same change.
- A design that is right for a reason the evidence does not support is still undefended. Fix the sentence even when the code stays.
- The binary a probe measured must be the binary the product runs. A probe that measures an absolute path does not cover a call made by bare name, and a test helper must use the same resolution as the product. On Windows, a bare `tar` resolved to GNU tar from Git for Windows, which read drive-letter archive paths as network hosts.
- Repetition fixes noise, not environment. Min-of-N gives a stable number for the machine it ran on. A single unrepeated timed sample is not evidence. A timing claim also has to hold on a second environment, such as a CI runner, before it is written down as fact.
- Report the distribution, not the best case. A minimum answers "how fast can this go", not "what will this cost". Report the median beside it, and the max when the spread is wide.
- The ledger gate `probes/windows/13-ledger-check.ps1` (helpers in `probes/windows/13-ledger-content.ps1`, run by `.github/workflows/windows-capability-probe.yml`) fails a row whose quoted `field: value` pair or counted number disagrees with its capture. A qualitative claim is outside its reach, so the rules above still bind the author.

## Related

- [A test that cannot fail is not coverage](a-test-that-cannot-fail-is-not-coverage.md)
