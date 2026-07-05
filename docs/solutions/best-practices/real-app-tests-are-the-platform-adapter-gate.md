---
title: "Green mock/stub unit tests are not sufficient verification for platform-adapter changes"
date: 2026-07-03
category: best-practices
module: "crates/macos, crates/core, src, tests"
problem_type: best_practice
component: testing
severity: high
applies_when:
  - "A platform adapter (crates/macos, and later crates/windows / crates/linux) changes how it reads windows, builds trees, computes element names or identity, resolves refs, or dispatches actions"
  - "Deciding whether a change is ready to merge based on `cargo test` results"
  - "Adding or auditing tests for observation or interaction behavior"
  - "Adding a new consumer of an element's accessible name or identity"
tags:
  - testing
  - verification
  - mock-adapter
  - real-app-tests
  - e2e
  - platform-adapter
  - accessible-name
  - regression
---

# Green mock/stub unit tests are not sufficient verification for platform-adapter changes

## Context

The foundation-contract branch passed the full unit CI green, then broke
observation and interaction against every real macOS app. The gap: the core
unit suite runs against small in-memory stub adapters — per-test doubles like
`RetryAdapter` / `AmbiguousThenOkAdapter` in `ref_action_wait_tests.rs`; there is
no central `MockAdapter`. Those stubs model the core-logic contracts (the
auto-wait poll loop, `identity_matches`, `node_matches`, error classification)
but they have no accessibility tree, no CGWindow layer, and no AX attributes, so
they structurally cannot exercise the platform adapter's native plumbing. A bug
that lives in that plumbing passes `cargo test` green.

Three such regressions shipped green and were only surfaced by running the live
e2e (`tests/e2e/run.sh`, which drives the release binary against a real SwiftUI
fixture and verifies every effect by independent observation):

1. Window resolution matched AX windows by a nonexistent `AXWindowNumber`
   attribute instead of the real AX-to-CGWindowID bridge `_AXUIElementGetWindow`,
   so `snapshot`/`find` returned `WINDOW_NOT_FOUND` for every real app.
2. Accessible-name computation diverged across three sites in the macOS crate —
   the snapshot builder (`title || description`), the strict ref resolver, and
   the live query matcher each computed a name differently — so `find --name`
   returned null and freshly-created refs returned `STALE_REF`.
3. The `ref.resolve.ok` trace event was dropped in the auto-wait refactor.

## Guidance

### Treat green stub-adapter tests as necessary but not sufficient for adapter changes

A green `cargo test --workspace` proves the platform-agnostic core logic is
sound. It proves nothing about whether the platform adapter reads the OS
correctly, because the stub adapters never touch the OS. For any change under
`crates/macos` (or, later, `crates/windows` / `crates/linux`) that affects how
the adapter reads windows, builds trees, computes names or identity, resolves
refs, or dispatches actions, green units are a precondition to merge — not a
verdict.

### The real-app test layer is the platform-adapter gate

Two test surfaces exercise the real adapter, and they are the actual merge gate
for adapter changes:

- `tests/e2e/run.sh` — drives the release binary against the real SwiftUI
  fixture, asserting every effect by independent observation (never the
  command's own `ok:true`).
- The `#[ignore]` real-app tests in `src/tests/snapshot_test.rs` — run with
  `cargo test -- --ignored` against real apps (Finder/TextEdit).

Both require macOS with Accessibility permission and cannot run in permissionless
CI, so they must be run on a permissioned machine before merging an adapter
change. `#[ignore]` real-app tests that never run provide zero protection — the
guards for all three regressions above already existed as `#[ignore]` snapshot
tests and would have failed instantly, but nothing ran them.

### A faithful mock would not have caught these — do not build mock theater

The instinct after a mock-hid-a-bug incident is to make the mock "faithful to
real." For this bug class that is theater: a correct-by-construction in-memory
adapter resolves windows from its own map (no bridge to get wrong) and computes
names one way (nothing to diverge), so it cannot reproduce a bridge error or a
per-site divergence. The bugs live *below* the trait contract, in platform
mechanics a mock does not have. The only thing that covers platform mechanics is
a test that drives the real platform. Invest there, not in a more elaborate mock.

### Give element accessible-name/identity computation exactly one owner

The `STALE_REF` regression was caused by three copies of "what is this element's
name" drifting apart. An element's accessible name must be computed by a single
canonical reducer, `crates/macos/src/tree/builder.rs::accessible_name`
(precedence: title → description → static-text value → aggregated child
label), which the snapshot builder calls directly when it stores a ref's name.
`crates/macos/src/tree/element.rs::resolve_element_name` is a thin
AXElement-only wrapper (`accessible_name(el, &fetch_node_attrs(el))`) called by
the strict ref resolver, the live matcher, and hit-test occluder naming. A
ref's stored name and the resolver's recomputed name must come from the same
reducer, or freshly-created refs go stale. Any new consumer of an element's
name or identity must call the canonical reducer or its wrapper — never
re-read `AXTitle` (or any single attribute) itself.

## Why This Matters

The user-visible failures were total, not marginal: `snapshot` and `find`
returned `WINDOW_NOT_FOUND` for every app, `find --name` returned null, and every
ref-addressed action failed to resolve — while CI stayed green and the change
looked shippable. An automation tool that cannot observe or act on any real app
is broken regardless of its unit-test count. The cost of the gap is paid at the
worst time (after merge, against real users' apps), and the fix is cheap
(running the real-app gate takes minutes).

## When to Apply

Run the real-app gate before merging any change that touches:

- window enumeration or window-id resolution
- tree building, element attribute reads, or accessible-name/identity computation
- ref allocation or strict ref re-resolution
- action dispatch, actionability, or live-state reads
- anything else under a platform adapter crate

The gate:

```bash
cargo build --release                 # e2e drives target/release/agent-desktop
bash tests/e2e/run.sh                 # real fixture, verify by observation
cargo test -- --ignored               # #[ignore] real-app snapshot tests
```

Green `cargo test --workspace` alone does not clear an adapter change.

## Examples

The real-app guards that fail closed on each regression class live in
`src/tests/snapshot_test.rs` as `#[ignore]` tests — they must be *run*, not just
present:

```rust
// Window bridge: a list-windows id must resolve back through snapshot.
let ids = list_windows("Finder");          // -> ["w-66616", ...]
let snap = snapshot("Finder");             // must be ok:true
assert!(ids.contains(snap["data"]["window"]["id"]));

// Name consistency: an element found by role must be findable by the
// accessible name it reports (guards name-computation divergence).
let by_role = find("Finder", role = "button", first = true);
let name = by_role["name"];                // the name the element reports
let by_name = find("Finder", role = "button", name = name, first = true);
assert_eq!(by_name["ref_id"], by_role["ref_id"]);   // same element

// Strict resolution: a ref just produced by find must re-resolve through get
// (guards ref identity vs stored-name drift -> STALE_REF).
let ref_id = find("Finder", role = "button", first = true)["ref_id"];
assert!(get(ref_id, property = "role")["ok"]);
```

And the single-owner name computation the resolver, matcher, and builder share:

```rust
// crates/macos/src/tree/builder.rs — the one accessible-name reducer.
pub(crate) fn accessible_name(el: &AXElement, attrs: &NodeAttrs) -> Option<String> {
    // title, else description, else (static text only) value, else a label
    // aggregated from descendant text.
}

// crates/macos/src/tree/element.rs — thin wrapper the strict resolver, live
// matcher, and hit-test occluder naming call; the snapshot builder calls
// accessible_name directly.
pub fn resolve_element_name(el: &AXElement) -> Option<String> {
    accessible_name(el, &fetch_node_attrs(el))
}
```

## Related

- [Playwright-grade desktop reliability contract](playwright-grade-desktop-reliability-2026-06-02.md) — the reliability contract these tests verify; its "When to Apply" lists *what* to test, this doc adds that those tests must run against real apps because stub-adapter units cannot cover platform mechanics.
- [Guard OS-reordered resources with an identity fingerprint, not a raw index](identity-fingerprint-against-os-reorder-2026-04-16.md)
- [Keep raw arguments out of trace-reachable error messages](../conventions/keep-raw-arguments-out-of-trace-reachable-error-messages.md)
