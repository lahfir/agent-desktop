---
title: Emit a state only on a positive claim, never on a property's own default
date: 2026-08-01
category: logic-errors
module: crates/windows/src/tree (states.rs, property_ids.rs)
problem_type: logic_error
component: tooling
symptoms:
  - "`invalid` is emitted on 26 of 26 Notepad nodes and 113 of 113 Explorer nodes — static text, title bars, windows and menus alike."
  - "A pattern-state property (ToggleState, ValueIsReadOnly, ExpandCollapseState, IsSelected, CanSelectMultiple) reads a plausible value on an element that does not implement the pattern at all, and never returns the UIA not-supported sentinel."
  - "Every unit test passes while the defect ships; it is found only by pointing the tool at real applications."
resolution_type: code_fix
severity: high
tags: [windows, uia, vocabulary, states, default-value, dogfood, gating]
---

# Emit a state only on a positive claim, never on a property's own default

## Problem

Two independent measurements on this branch produced the same failure shape:
a property's *default* value was read as if it were a *claim*.

**Pattern-state properties.** UI Automation exposes pattern state — toggle,
expand/collapse, selection — as plain automation properties rather than as a
pattern instance, so `ToggleState`, `ExpandCollapseState`,
`SelectionItemIsSelected`, `ValueIsReadOnly` and `SelectionCanSelectMultiple`
all ride the same batch read as everything else. Probe `A15-7`
(`probes/windows/FINDINGS.md`) measured what a provider returns for these
properties on an element that implements *none* of the corresponding
patterns: a static text control reported `ToggleToggleState = 2`
(`ToggleState_Indeterminate`), `ValueIsReadOnly = true`,
`ExpandCollapseExpandCollapseState = 3` (`LeafNode`), `SelectionItemIsSelected
= false`, `SelectionCanSelectMultiple = false` — every one of the five a
plausible-looking value, and never the not-supported sentinel that would let
the tri-state classifier report `Absent`. Read ungated, the classifier reports
all five `Known`, so every inert node in every tree would have carried
`indeterminate` and `readonly`. `TreeProperty::gate()`
(`crates/windows/src/tree/property_ids.rs`) fixes this by naming the
`Is*PatternAvailable` property that must read `true` before the paired state
property means anything, and `ElementProperties::gated_flag` /
`gated_number` (`crates/windows/src/tree/element_properties.rs`) are the
*only* accessors for a gated property, so a call site cannot forget to gate.

**`IsDataValidForForm`.** This property defaults to `false`, and the first
implementation of `states.rs` read it exactly as Microsoft's ARIA state table
prescribes: `false -> invalid`. The 2026-07-31 dogfood run
(`docs/dogfood-reports/2026-07-31-feat-windows-2-3-vocabulary-dogfood.md`)
pointed the shipped binary at real Notepad and Explorer windows and found
`invalid` on **26 of 26** Notepad nodes and **113 of 113** Explorer nodes —
static text, title bars, windows, scrollbars and menus, not just form fields.
Every unit test passed throughout, because a unit test
asserts what its author already believed about the property.

## Root cause

Both properties have a default value that is indistinguishable, at the wire
level, from a genuine negative answer — and in both cases the code emitted a
state on that default rather than on a value the provider affirmatively
claimed. `IsDataValidForForm = false` means "no form validation rule has
declared this element invalid," which is true of essentially every element in
existence, form field or not; it is not the provider asserting "this is
invalid." The pattern-state properties are the same shape one level down: a
provider that never implemented `TogglePattern` still has to return *some*
`ToggleState` when asked, and the UIA client stack does not distinguish that
default from a real toggle sitting in its default position.

The contrast that makes the rule checkable is the neighbouring property,
`IsRequiredForForm`, which shares the exact same default (`false`) and is
safe. `required` is emitted only when the flag reads `true` — a positive
claim the provider had to affirmatively make. `invalid` was emitted on
`false` — the property's default — so it decorated the whole tree. Same
property shape, opposite polarity of the emission rule, opposite safety.

## Solution

`TreeProperty::gate()` closes the pattern-state half: every pattern-derived
read in `crates/windows/src/tree/states.rs` (`push_toggle_state`,
`push_expand_collapse_state`, and the `SelectionItemIsSelected` /
`ValueIsReadOnly` / `SelectionCanSelectMultiple` / `WindowIsModal` arms in
`resolve_states`) goes through `gated_flag`/`gated_number`, which returns
`None` — contributing no state — unless the paired availability property
reads `true` first.

`IsDataValidForForm` is no longer read at all. It is out of `TreeProperty::WALK_SET`
in `crates/windows/src/tree/property_ids.rs`, and `invalid` is unproduced on
Windows — recorded in `resolve_states`'s doc comment in `states.rs` rather than
worked around. Two regression tests in `crates/windows/src/tree/states_tests.rs`
pin this: `a_default_false_form_validity_flag_produces_no_invalid_token` feeds
`IsDataValidForForm = false` across four roles and asserts no `invalid` token
appears, *and* separately asserts the sibling `required` arm still fires on
`true` — so the test cannot pass by disabling both arms.
`invalid_is_unproduced_whatever_the_read_set_says` sweeps both `true` and
`false` and asserts `invalid` never appears, whatever the input. Both were
observed failing against the reintroduced defect with the message `a
statictext was reported invalid because a form flag defaulted to false`.
After the fix, nodes carrying any state fell from 100% (every node, on every
target) to 14/26 (Notepad), 49/113 (Explorer), 1/43 (WinForms) and 26/82
(WPF) — the vocabulary now discriminates instead of decorating.

## Prevention

- **Emit a state or affordance only on a positive claim from the provider,
  never on a value that is also the property's documented or observed
  default.** Before wiring a new boolean property to a token, check what it
  reads on an element that plainly does not have the semantic in question —
  if that's the same value that would trigger emission, the property is
  unusable for this purpose, the way `IsDataValidForForm` turned out to be.
- **Gate every pattern-derived property on its own `Is*PatternAvailable`
  flag**, through `TreeProperty::gate()` and `gated_flag`/`gated_number` —
  never read a pattern-state property straight. A15-7 is the standing
  evidence that "not implemented" and "implemented, at its default" are the
  same bytes on the wire.
- **A signal that is true of everything carries no information.** The same
  reasoning that keeps `IsLegacyIAccessiblePatternAvailable` — measured
  `true` on 141 of 141 elements in probe `A2-2` — from being treated as an
  affordance by itself (`resolve_actions` gates it on
  `LegacyDefaultAction` being non-empty instead) is what makes a
  universally-true state token worthless even when every individual read
  succeeded.
- **Unit tests that restate the mapping cannot catch this class of defect.**
  The dogfood run — pointing the built binary at Notepad and Explorer and
  reading the actual output — is what found `IsDataValidForForm`'s failure;
  every test written against the author's own model of the property passed
  throughout. Any new state or role source derived from a UIA property this
  vocabulary has not yet measured needs a dogfood pass against real software
  before it ships, not just unit coverage.

## Related

- [A tri-state boolean predicate fails open the moment it is negated](tri-state-evidence-collapses-under-negation.md) —
  the sibling defect from the same change: the same read set, a different
  way of turning "I don't know" into a false affirmative.
