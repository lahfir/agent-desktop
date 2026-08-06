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

## Standing practice on Windows

Windows carries no fixture harness, so the gate takes the form of a scripted
dogfood run against off-the-shelf software, and every sub-phase does one before
it merges. The committed reports in `docs/dogfood-reports/` are the record: the
macOS enhanced-reliability run that set the pattern, then vocabulary
(sub-phase 2.3), the observation read path (2.4), and resolution and the live
locator (2.5). This is not a rule that recurred; it is how each layer of the
adapter enters the product.

The vocabulary run shows what the shape buys. `probes/windows/scratch/run-dogfood.ps1` drove the
`ControlType`→`Role`, action, and state vocabulary against four real UI stacks nobody in this
repository wrote — classic Notepad (Win32 `EDIT` proxy), Explorer (DirectUI shell), and
WinForms/WPF scratch fixtures. It found one real defect no unit test had: `invalid` was
emitted on every node of every target, because a UIA form-validity flag's `false` default was
read as a positive claim rather than the absence of one. It also recorded two targets —
Chromium/Electron and the modern Settings app — as **skipped with a reason** rather than
silently green, exactly this doc's "record a skipped native gate as skipped" rule. The
resulting report is committed at
`docs/dogfood-reports/2026-07-31-feat-windows-2-3-vocabulary-dogfood.md`; the raw per-node
census JSON it was produced from is deliberately gitignored (see `docs/plans/2026-07-31-001-captures/`
in `.gitignore`), because a census can carry a real application's on-screen text where a
report describing shapes and counts does not — the durable record is the report, not the
capture.

A run substitutes for the fixture app only while it keeps the fixture app's discipline — real
software, effects verified independently, skips recorded honestly, and raw captures kept out of
the repository while the judgement drawn from them is kept in it.

## Related

- [Build desktop actions as an observe-resolve-preflight-dispatch contract](playwright-grade-desktop-reliability-2026-06-02.md)
- [Guard OS-reordered resources with an identity fingerprint](identity-fingerprint-against-os-reorder-2026-04-16.md)
- [Never ship platform code that CI cannot execute](never-ship-platform-code-that-ci-cannot-execute.md) — the complementary gate. This doc covers permissioned native verification of the macOS adapter against real applications; that one covers running platform-conditional core code on real Windows and Linux runners.
