---
module: agent-desktop
date: 2026-09-20
problem_type: runtime_error
tags: [native, scrolling, refs, identity, headless, reliability]
---

# Native command reliability: scrolling, selection and menu discovery

Trials in this area are bounded to a few apps and runs. They do not establish universal reliability.

## Scroll controls are not identified by their value

A scrollbar's value changes when it scrolls, so the value cannot be stable identity. Core's mutable-value role list omitted `scrollbar` and `handle`. The old value was treated as identity, and a legitimate scroll made the live scrollbar fail to resolve. The fix adds both roles to `is_mutable_value_role` in `crates/core/src/tree/roles.rs`. No timeout, fuzzy match, path trust, or ambiguity rule was relaxed.

Matching values alone remain insufficient identity, and distinct named scroll controls remain distinct. Changed geometry without stable identity still needs a safe refusal. The fix does not promise to recover an arbitrary thumb.

Evidence: `crates/core/src/refs/ref_identity_scroll_tests.rs`, test `scroll_positions_neither_prove_nor_reject_element_identity`.

## Select matches canonical collection labels

`select` searched only AXTitle and AXDescription. Snapshots derive a row's name from its child content, so `select` returned ELEMENT_NOT_FOUND for a row the snapshot had named. The search now reads the same bounded name evidence as snapshots. A candidate must advertise Click, because inert text cannot become an action target just because its text matches. Unknown evidence fails closed, and a missing item reports `not_delivered`.

Menu selection shares this search and has no live regression test yet.

Evidence: `crates/macos/src/actions/select/select_name.rs`, test `selection_requires_a_known_canonical_label_and_an_actionable_target`.

## Incomplete branches cannot prove a menu is absent

The app-wide menu search aborted at the first incomplete branch. In a 6,984-item grid, a branch that loaded only part of its children hid a menu open in a sibling. The search now goes breadth-first and defers `AppUnresponsive` branch errors while it inspects siblings. A menu it observes is returned. Otherwise the incomplete error is kept. Permission errors and deadlines stay terminal, and the 2,048-node limit still applies.

Evidence: `crates/macos/src/tree/surfaces.rs`, test `an_incomplete_branch_cannot_prove_menu_absence`.

## Disclosure controls without readable expansion state

A Weight popup button in SF Symbols published AXShowMenu and AXPress but returned `kAXErrorAttributeUnsupported` for AXExpanded and AXDisclosing. Its expand action refused without delivery. When the expansion state is unknown, test the native menu-opening semantics rather than toggling AXPress blindly.

## Monitored trials need quiet foreground

The headless probe counts session-wide input and observes activation notifications. Activity from the user or other apps appears in those counts, so the events cannot be attributed to the agent. A monitored trial with unrelated foreground activity is not proof of a headless success, and unattributed events earn no credit.

Evidence: `tests/real-apps/headless_probe.swift`, a read-only diagnostic and not a driver.
