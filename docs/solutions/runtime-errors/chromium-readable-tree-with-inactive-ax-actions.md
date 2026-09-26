---
title: A readable Chromium AX tree does not prove its actions are active
date: 2026-09-19
category: runtime-errors
module: macos renderer accessibility activation
problem_type: integration_issue
component: platform_adapter
severity: high
tags: [macos, chromium, accessibility, headless, renderer-activation]
---

# A readable Chromium AX tree does not prove its actions are active

## Reproduction

Comet 152 exposed a complete web accessibility tree and advertised `AXPress`,
but clicks on links, a plain HTML button, and a checkbox returned native success
without changing the page. Native browser tabs worked. An independent Swift
probe reproduced the web failures using `AXUIElementPerformAction` directly.

The same local HTML controls worked through raw AX in an isolated Chrome 153
profile. Comet reported `AXEnhancedUserInterface = false`; Chrome reported true.
Both reported `AXManualAccessibility` unsupported.

Enabling enhanced accessibility made the same Comet controls work. Agent
Desktop's unchanged semantic click then opened the original Hacker News article.
Disabling enhanced accessibility reproduced the no-op button and checkbox.
This was an activation problem, not a website-specific targeting problem.

## Root cause

The observation path considered an observed `webarea` sufficient readiness.
It only requested activation when the web tree was absent, and the macOS
activation implementation supported only `AXManualAccessibility`.

Chromium's application implementation initially enables a limited accessibility
mode on an AX role read. Its `AXEnhancedUserInterface` setter requests complete
mode after a two-second debounce. Reading the flag back as true does not mean
that debounce has finished. The live probe measured usable checkbox state
appearing approximately 2.02 seconds after the write.

The setter also returned `kAXErrorNotImplemented` after applying the flag.
Chromium handles the attribute before forwarding the setter to its superclass;
the observed readback is necessary to recognize the applied change.

Primary sources:

- [Chromium application accessibility activation and two-second debounce](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/chrome/browser/chrome_browser_application_mac.mm)
- [Chromium activation regression tests](https://chromium.googlesource.com/chromium/src/+/refs/tags/133.0.6917.1/chrome/browser/chrome_browser_application_mac_browsertest.mm)
- [Chromium asynchronous accessibility action semantics](https://chromium.googlesource.com/chromium/src/+/main/docs/accessibility/browser/how_a11y_works_3.md)

## Repair boundary

- Reuse the existing renderer activation signal, interaction lease, deadline,
  and observation retry. Keep the policy and click implementation unchanged.
- Preserve manual accessibility preference for renderers that support it.
- Permit enhanced accessibility fallback only after observing a web surface.
  Finder also advertises this attribute, so support alone is not evidence of
  a renderer and must not activate native apps.
- Check the enhanced mode even when web content is already readable. Do not
  treat a false `AXManualAccessibility` readback as disabled: Electron's getter
  compares for exactly the complete mode while its setter adds platform and
  screen-reader flags, so an applied write can still read back false. In
  v0.9.4, a readable manual-accessibility renderer (reproduced with VS Code and
  ClickUp on macOS 26) was activated because the manual flag read back false,
  and the activation was then rejected because the post-write readback was
  still false.
- Read the flag back after a write. A matching value can establish activation
  despite the setter's unsupported response; contradictory readback cannot.
- Wait out Chromium's two-second enhanced-mode debounce within the existing
  deadline. Reject insufficient budgets before writing. Already-enabled modes
  do not pay this startup delay.
- Preserve older manual-mode implementations whose readable web tree exists
  but whose manual activation flag has no readable value.

This does not add browser-name checks, DOM automation, physical click fallbacks,
or a new command/API. The general click result can still be
`delivered_unverified`; callers must inspect the requested effect. The overlay
ripple also remains a delivery visualization, not proof of page navigation.

## Verification

Deterministic unit tests cover attribute selection, native-app exclusion,
already-enabled modes, legacy missing readback, missing renderer retries,
capability/read failures, contradictory setter readback, applied changes despite
unsupported return codes, and insufficient-budget rejection before native I/O.
These cases use fake capability/readback results or null handles and do not
depend on browser startup, global UI state, or exact wall-clock timing.

Live tests are separate: begin with enhanced mode disabled, snapshot with the
release binary, perform one semantic click, and read the actual page state.
Compare a button, checkbox, and link on an isolated local page, plus a native
browser control. Observe foreground PID and physical pointer separately from
the cursor overlay. Do not claim pointer stability when concurrent user input
changes it during the measurement.

### Results on the final release build

- `cargo test --workspace`: 2,231 passed, zero failed, four ignored.
- Formatting, Clippy with warnings denied, core dependency isolation, and
  whitespace checks passed. The release executable is 3,072,144 bytes.
- Cold Comet activation followed by semantic checkbox press changed `0` to `1`;
  a subsequent link press changed the URL to `#destination`. Earlier cold button
  activation changed its output text. Raw AX and Agent Desktop both reproduced
  the disabled-mode failure; enabling the mode also opened the Hacker News article.
- The overlay was enabled. Foreground PID and physical pointer were unchanged
  across the final checkbox/link actions. Finder's enhanced mode remained false.
- The required performance script ran with the activating native fixture skipped
  to preserve strict headless operation. Its application-name probes were unusable:
  Finder had no window, and Comet had multiple windows. A supplemental alternating
  five-round comparison used the same explicit Comet window and immutable binaries.
  All 30 snapshots succeeded with matching ref counts. Warm median milliseconds
  were baseline/fixed: default 627/563, skeleton 582/614, depth-30 588/615.
  These small samples do not establish a universal performance bound; they show
  no repeated two-second warm penalty. Cold enhanced activation intentionally pays
  Chromium's two-second debounce plus the existing observation retry.

Local probe artifacts are under `/tmp/comet-ax-probe`; raw trees may contain
private browser content and should not be published.
