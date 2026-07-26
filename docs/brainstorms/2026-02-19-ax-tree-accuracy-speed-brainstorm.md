---
date: 2026-02-19
topic: ax-tree-accuracy-speed
---

# AX Tree Accuracy & Speed: First-Principles Fix

## What We're Building

A corrected, fast accessibility tree traversal engine that returns complete, readable node data
on every call. The current implementation has two compounding bugs that make the tool nearly
useless in practice: node names are missing for most elements, and traversal is 4–5 seconds
for a medium-sized UI.

---

## Root Cause Diagnosis

### Bug 1 — Empty names

`fetch_node_attrs` tries `kAXTitleAttribute` then `kAXDescriptionAttribute` and calls it done.

The macOS AX tree does **not** guarantee that either attribute carries the visible label text.
For the vast majority of real-world controls:

| Element type | Where the label actually lives |
|---|---|
| `AXOutlineRow` (Finder sidebar) | `kAXTitleAttribute` ← **should work**, but our batch parse drops it |
| `AXRow` (Finder column browser) | child `AXCell` → child `AXStaticText` → `kAXValueAttribute` |
| `AXButton` with image only | `kAXDescriptionAttribute` |
| `AXTextField` | `kAXPlaceholderValueAttribute` if empty, else `kAXValueAttribute` |
| `AXStaticText` | `kAXValueAttribute` (not title!) |
| `AXMenuItem` | `kAXTitleAttribute` |

The batch fetch uses `AXUIElementCopyMultipleAttributeValues`. The result is a `CFArray` of
`CFType` values. Our parsing calls `item.downcast::<CFString>()`. This is correct for string
attrs — but **boolean attrs** (`kAXEnabledAttribute`, `kAXFocusedAttribute`) are `CFBoolean`,
so they downcast to `None`. That's fine because we re-fetch them individually. But the real
problem is more subtle: **some elements return their string attrs as `CFString` subclasses
or as `AXTextMarker` types** that do not downcast to plain `CFString`. In those cases we get
`None` even when data is present.

The fix requires a multi-attribute fallback chain **plus** a child-text fallback.

### Bug 2 — Unknown roles

`roles.rs` maps ~22 role strings but the AX API has ~55. Anything not mapped becomes
`"unknown"`. In Finder's column browser, most elements are `AXBrowser`, `AXColumn`, `AXRow`
(table row variant), `AXLayoutItem` — all currently `"unknown"`. This makes the tree
unreadable and prevents `find --role` from working.

Unmapped roles in the existing codebase:
`AXBrowser`, `AXColumn`, `AXRow` (table row), `AXGrid`, `AXHandle`, `AXPopover`,
`AXDockItem`, `AXRuler`, `AXRulerMarker`, `AXTimeField`, `AXDateField`, `AXHelpTag`,
`AXMatte`, `AXDrawer`, `AXLayoutArea`, `AXLayoutItem`, `AXLevelIndicator`,
`AXRelevanceIndicator`, `AXSearchField` (already covered by textfield but missing as alias),
`AXSwitch`, `AXMenuButton`.

### Bug 3 — Speed

Every node requires 2–4 round-trips across the Mach IPC boundary:
1. `AXUIElementCopyMultipleAttributeValues` — 1 call for 6 attrs
2. `copy_children` tries up to 3 attr fetches (`kAXChildren`, `kAXContents`,
   `AXChildrenInNavigationOrder`) even when the first one succeeds

Each Mach IPC call costs ~1–5 ms. A 150-node tree = 300–600 round-trips = 2–5 seconds.

The fix is: stop on first successful children attribute, and batch-fetch **more** useful attrs
in the single `AXUIElementCopyMultipleAttributeValues` call so we never need per-attr fallback
calls for the common case.

### Non-bug: `-i` flag

Already implemented. `#[arg(long, short = 'i')]` on `interactive_only`. Works as
`agent-desktop snapshot -i`. The long form is `--interactive-only`, not `--interactive`.
We should add an alias `--interactive` to match user expectation.

---

## Approach A: Fix name resolution with fallback chain (Recommended)

**What it is:** Extend `fetch_node_attrs` to try 5 name sources in order, including
reading the first `AXStaticText` child. Expand `roles.rs` to cover all standard AX roles.
Stop `copy_children` on first non-empty result. Add `--interactive` as alias.

**Pros:**
- Targets the exact root cause. Minimal code change.
- No architecture change — same `build_subtree` recursion.
- Risk is low. Every change is additive.

**Cons:**
- The child-text fallback adds 1 extra IPC call per node that has no direct name.
  For nodes that DO have `kAXTitleAttribute`, this extra call is skipped.

**Speed estimate:** ~2x improvement from stopping children fetch early.
Name accuracy: ~95% of real apps will have readable names.

---

## Approach B: Batch all attrs including children in one call

**What it is:** Use `AXUIElementCopyMultipleAttributeValues` to fetch role, subrole, title,
description, value, placeholder, help, enabled, focused, selected, expanded, children — all
in a single call. Parse `CFBoolean` and `CFArray` correctly alongside `CFString`.

**Pros:**
- Reduces per-node IPC calls from 2–4 down to 1.
- Could bring 150-node tree from 4s down to <1s.

**Cons:**
- Requires correctly parsing mixed-type `CFArray` results (CFString, CFBoolean, CFArray).
- `AXUIElementCopyMultipleAttributeValues` doesn't support fetching array attrs
  (kAXChildren) — children still need a separate call.
- Higher implementation risk; the CF type introspection is fragile.

---

## Approach C: AXUIElementCopyAttributeNames + full dynamic attribute dump

**What it is:** For each node, first call `AXUIElementCopyAttributeNames` to get the list of
all attributes the element supports, then batch-fetch only those. This is what macOS
Accessibility Inspector does internally.

**Pros:**
- Most complete and accurate — never misses a custom attribute.
- Would fully expose Electron app data, web areas, custom controls.

**Cons:**
- Doubles the IPC calls per node (one for names, one for values).
- Much slower than Approach A or B for the common case.
- Over-engineered for Phase 1.

---

## Why Approach A, then B incrementally

Start with A because it fixes the accuracy problem with minimal risk and gives us a working,
readable tree. Then layer in B's batch parsing improvements as a separate optimization pass
once the output is verified correct.

The correctness of the tree is more valuable right now than raw speed. A 2-second tree with
readable names beats a 0.5-second tree of `"unknown"` nodes.

---

## Key Decisions

- **Name fallback chain**: `kAXTitleAttribute` → `kAXDescriptionAttribute` →
  `kAXValueAttribute` → first AXStaticText child's `kAXValueAttribute` →
  `kAXPlaceholderValueAttribute`
- **Expand roles.rs**: Add all ~33 missing role mappings from `role_constants.rs`
- **Add subrole field**: Include `kAXSubroleAttribute` in output so agents can distinguish
  `AXOutlineRow` vs `AXTableRow` vs plain `AXRow`
- **children fetch**: Break on first non-empty result (already partially there, needs
  confirmation that `kAXChildrenAttribute` wins 99% of the time and others are rarely needed)
- **`--interactive` alias**: Add alongside existing `--interactive-only` and `-i`
- **Test harness**: Write a Rust integration test that snapshots Finder and asserts:
  - ≥10 nodes have non-empty names
  - Zero nodes have role `"unknown"`
  - Total time < 3 seconds

---

## Open Questions

- Do we want to include `kAXSubrole` in the JSON output, or keep the schema flat?
- Should we emit a `label` field (the human-readable role description) separately from `role`?
- For Finder's column browser specifically: are file rows exposed as `AXRow` children of
  `AXColumn` children of `AXBrowser`? Need to confirm with a debug attr-dump.
- Should `find` command also search `value` field, not just `name`?

---

## Next Steps

→ `/workflows:plan` to break this into implementation tasks
