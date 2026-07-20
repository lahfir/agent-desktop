---
title: Make permissioned fixture and real-app checks the adapter gate
date: 2026-07-11
category: best-practices
module: platform adapter verification
problem_type: best_practice
component: testing_framework
severity: high
applies_when:
  - "Changing a platform adapter's tree, resolution, window, input, or accessibility code"
  - "Changing release-binary integration behavior"
  - "Fixing a bug that a mock adapter could not expose"
tags: [e2e, fixture-app, accessibility, macos, regression, adapters]
---

# Make permissioned fixture and real-app checks the adapter gate

## Context

Core unit tests prove shared contracts, but they cannot prove that a native
accessibility API exposes the same tree, identity, or input behavior on a real
desktop. A mock cannot reproduce a bad AX-to-window bridge or disagreement
between independently implemented native readers.

## Guidance

The macOS adapter has two complementary native gates:

- `tests/e2e/run.sh` builds a temporary SwiftUI fixture, drives the release
  binary, and independently observes each harmless effect. It requires macOS,
  a release binary, and Accessibility permission; it exits with a clear
  prerequisite failure when one is unavailable.
- `src/tests/snapshot_test.rs` contains three `#[ignore]` Finder probes:
  snapshot window identity must agree with `list-windows`; a reported button
  name must be findable by that same name; and a fresh find ref must re-resolve
  through `get`. The registration test prevents these guards from silently
  disappearing.

Run normal tests first, then run the native gate on a permissioned machine for
any adapter change. Use the fixture for mutation coverage. Treat real user apps
as observation-only unless a narrowly scoped, reversible interaction is
explicitly authorized.

## Why This Matters

The fixture provides deterministic, safe interaction coverage. Finder probes
cover native seams that the fixture cannot emulate: AX window identity,
accessible-name derivation, and strict re-resolution against a real system
application. Together they catch both behavior regressions and accidental test
deletion without treating a mock as a platform oracle.

## Prevention

- For every native regression, add a deterministic unit test where possible
  and a fixture or safe real-app assertion at the affected seam.
- Verify effects independently of an `ok: true` response.
- Keep native tests opt-in and prerequisite-aware; never make them operate on
  user data or depend on an arbitrary foreground application.
- Record a skipped native gate as skipped, not green.

## Related

- [Build desktop actions as an observe-resolve-preflight-dispatch contract](playwright-grade-desktop-reliability-2026-06-02.md)
- [Guard OS-reordered resources with an identity fingerprint](identity-fingerprint-against-os-reorder-2026-04-16.md)
