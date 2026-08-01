---
title: A tri-state boolean predicate fails open the moment it is negated
date: 2026-08-01
category: logic-errors
module: crates/windows/src/tree (element_properties.rs, actions.rs, roles.rs)
problem_type: logic_error
component: tooling
symptoms:
  - "A Value-enabled element whose ValueIsReadOnly read failed is advertised with SetValue and resolves to textfield instead of document."
  - "Negating a boolean-flavoured helper (`!is_true(X)`) is true both when X is really false and when the read behind X failed."
  - "Code review passed on this arm through a full round; only a fresh reviewer, working from the fails-open cost asymmetry, found it."
resolution_type: code_fix
severity: high
tags: [tri-state, evidence, negation, fails-open, windows, uia, actionability]
---

# A tri-state boolean predicate fails open the moment it is negated

## Problem

`ElementProperties` reads Windows UI Automation properties into a three-value
`PropertyOutcome`: `Known(value)`, `Absent` ("the provider answered and does
not implement this"), and `Unknown` ("the read failed, or its answer cannot be
trusted"). `is_true(property)` is the convenience predicate for "does this
gated flag say yes": `gated_flag(property) == Some(true)`.

`resolve_actions` (`crates/windows/src/tree/actions.rs`) and `document_role`
(`crates/windows/src/tree/roles.rs`) both decided whether a `Value`-pattern
control was editable with:

```rust
properties.is_true(TreeProperty::ValueAvailable)
    && !properties.is_true(TreeProperty::ValueIsReadOnly)
```

`is_true` returns `false` for a flag that read `false`, for one that read
`Absent`, and for one that failed and read `Unknown` — all three collapse to
the same `bool`. Negating it collapses them again, so `!is_true(ValueIsReadOnly)`
is `true` in exactly those same three cases. An element whose `ValueAvailable`
read succeeded but whose `ValueIsReadOnly` read *failed* was therefore
classified as **not** read-only: `resolve_actions` advertised `SetValue`, and
`document_role` resolved `Role::TextField` for a control whose writability was
never established. This was the only fails-open arm in the whole vocabulary,
and it directly contradicted `resolve_actions`'s own documented rule that "a
single failed property simply contributes no affordance."

It shipped past code review and every unit test in the change, because both
suites exercised `Known(true)`/`Known(false)`/`Absent` but never an `Unknown`
read on this specific property. It was caught by review reading the fix, not
by a failing test: `git log` shows it landing in `7823f2f` ("fix: close the
code-review findings, including one real fails-open bug"), observed red first
at `left: "textfield", right: "document"`.

## Root cause

`is_true` is safe in positive position — `if is_true(X) { emit X }` — because
`Some(true)` is the only case that fires, and every other case (false, absent,
unknown) correctly withholds the affordance. It becomes unsafe the instant it
is negated, because Boolean negation cannot distinguish "the provider said no"
from "nothing was learned." `!is_true(X)` silently converts "I could not tell"
into "definitely not," which is the exact inversion `PropertyOutcome::Unknown`
exists to prevent. The predicate is asymmetric — correct in one position,
fails-open in the other — which is precisely the shape that survives review
until someone traces the negated call site back through `gated_flag` to
`PropertyOutcome`.

## Solution

Both sites now require an explicit positive statement of the negative:

```rust
properties.is_true(TreeProperty::ValueAvailable)
    && properties.gated_flag(TreeProperty::ValueIsReadOnly) == Some(false)
```

`gated_flag` returns `Option<bool>`, which keeps `Unknown`/`Absent`/gate-closed
as `None` all the way to the comparison; only a genuine `Known(false)` read
satisfies `== Some(false)`. Regression tests pin both directions on both call
sites: `a_failed_read_only_read_produces_no_value_setting_action` in
`crates/windows/src/tree/actions_tests.rs` and
`a_failed_read_only_read_resolves_document_not_textfield` in
`crates/windows/src/tree/roles_tests.rs` each assert that an `Unknown`
`ValueIsReadOnly` read withholds the affordance/role and that a `Known(false)`
read grants it — so a regression can't silently pass by disabling only the
failing half.

## Prevention

- Never negate a tri-state-derived boolean predicate. If a caller needs "the
  provider said no," write `== Some(false)` against the `Option<bool>` (or
  the underlying tri-state) directly — never `!is_true(...)`.
- When a helper flattens a richer type to `bool`, document which position it
  is safe in. `is_true`'s own doc comment now states it is a read "through its
  gate" for a positive check; this doc is the record of why the negated form
  was removed rather than merely discouraged.
- Grep for the failure shape before trusting a fix is complete: a search of
  `crates/windows/src/` for `!properties.is_true` and `!self.is_true` turns up
  zero matches as of this writing — every remaining boolean-gate check is
  either a bare `is_true(X)` in positive position or an explicit
  `gated_flag(X) == Some(true|false)`. If that search ever finds a hit again,
  treat it as the same defect until proven otherwise.
- Cost is asymmetric here exactly as it is for the secure-field gate
  documented alongside `is_true` in `element_properties.rs`: a failed read
  must land on the side that withholds the affordance, not the side that
  grants it. When adding a new gated property, write the `Unknown`-read case
  as its own test before writing the `Known(false)` case, so the fails-open
  arm cannot go unexercised.

## Related

- [Emit state on a positive claim, never on a default](emit-state-on-a-positive-claim-never-on-a-default.md) —
  the sibling defect from the same change: a property whose *default*
  value looks like a claim, found by running against real applications
  rather than by negating a helper.
