---
title: A cited measurement must match its capture
date: 2026-08-09
category: best-practices
module: probes/windows/FINDINGS.md, docs/phases.md, crates/windows/src/system/window_activate.rs
problem_type: process_gap
component: tooling
symptoms:
  - "A ledger row's observed cell states the opposite of the JSON capture it names."
  - "A shipped doc comment cites a probe row for a fact the probe never measured."
  - "The design is defensible, the evidence sentence under it is false, and every reader downstream inherits the false sentence."
applies_when:
  - "Writing a probe ledger row from a capture"
  - "Citing a probe row in shipped source or in the phase document"
  - "Reviewing a change whose rationale rests on a measurement"
root_cause: process_gap
resolution_type: reread_the_artifact_not_the_expectation
severity: high
tags: [evidence, probes, ledger, citations, review, windows]
---

# A cited measurement must match its capture

## Problem

Sub-phase 2.9 measured whether an uncontended `SetForegroundWindow` lands on
the first attempt, to justify the size of the activation retry budget. The
ledger row recorded:

> 5/5 trials `owned_after_first: true`, `first_attempt_success_rate: 1`,
> `first_attempt_always_lands: true`

The capture it names, committed beside it and byte-identical across two
environments, records the exact opposite on every trial:

```json
{ "api_returned_true": false, "owned_after_first": false,
  "second_attempt_needed": true, "owned_after_second": false }
```

with top-level `first_attempt_success_rate: 0` and
`first_attempt_always_lands: false`.

The false sentence then propagated the way cited facts do. `docs/phases.md`
repeated it. A doc comment on the shipped constant repeated it, citing the row
id — so the product's own source claimed a measurement that its evidence
contradicted.

Nothing failed. Every gate was green, because no gate compares a row's prose
to the JSON it names.

## Root cause

The row was written from the **expectation that motivated the probe**, not
from the artifact the probe produced. "Does the first attempt always land?" is
a yes/no question with an obvious hoped-for answer, and the row recorded the
hope. That is easiest to do precisely when the design does not depend on the
answer: a retry budget of two is correct whether the first attempt lands 5/5
or 0/5, so the wrong number changed no behaviour and drew no attention.

The propagation is the second half. Once a row exists, downstream readers cite
the row, not the capture — that is the point of having a ledger. A row is
therefore a single point of failure for every statement derived from it, and
it is the one artifact nobody re-derives.

## Solution

Correct the row against the capture, and correct every site that repeated it,
in place. Keep the design: a 0/5 first-attempt rate supports a bounded retry
*more* strongly than 5/5 would.

The check that catches this costs one command — open the capture and read the
fields the row names, rather than the fields you expected it to have:

```bash
python -c "import json,io; d=json.load(io.open('<capture>.json',encoding='utf-8-sig')); print(d)"
```

Two properties make a row auditable, and both are cheap:

- **Quote the field names and values the capture actually uses.** A row that
  says `first_attempt_success_rate: 0` can be diffed against the JSON by
  anyone. A row that says "the first attempt is reliable" cannot.
- **Record the branch the capture recorded.** When a probe writes its own
  verdict field (`branch`, `measurable`), the row repeats that verdict rather
  than paraphrasing it.

## Prevention

- Write the row **from the capture open beside you**, never from the question
  the probe was written to answer. If the row and the capture were authored in
  the same sitting, the row is unverified.
- When a row's finding is the answer the author was hoping for, re-read the
  capture once more. A confirming result gets less scrutiny than a
  contradicting one, which is exactly backwards.
- Before citing a row in shipped source or in the phase document, open the
  capture. A citation is a claim about an artifact, and repeating a row is not
  evidence that the row is true.
- When correcting a row, correct every downstream repetition in the same
  change. The row is the source, but the copies are what readers hit.
- A design that is right for a reason the evidence does not support is still
  undefended. Fix the sentence even when the code stays.

## Related

- [A test that cannot fail is not coverage](a-test-that-cannot-fail-is-not-coverage.md)
- [A verification gate is code and needs its own test](a-verification-gate-is-code-and-needs-its-own-test.md)
- [An enforcement gate must cover everything the binary embeds](an-enforcement-gate-must-cover-everything-the-binary-embeds.md)
