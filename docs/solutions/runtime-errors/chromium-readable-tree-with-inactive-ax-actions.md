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

## Mechanism

Chromium starts in a limited accessibility mode when an AX role is read. Its `AXEnhancedUserInterface` setter requests complete mode after a two-second debounce. Reading the flag back as true does not mean the debounce has finished, so usable state can arrive about two seconds after the write.

The setter returns `kAXErrorNotImplemented` after it applies the flag. Chromium handles the attribute before it forwards the setter to its superclass. The readback is therefore the only reliable sign that the change applied.

Electron's `AXManualAccessibility` getter compares for exactly the complete mode, while its setter adds platform and screen-reader flags. An applied write can read back false.

Primary sources:

- [Chromium application accessibility activation and two-second debounce](https://chromium.googlesource.com/chromium/src/+/refs/heads/main/chrome/browser/chrome_browser_application_mac.mm)
- [Chromium activation regression tests](https://chromium.googlesource.com/chromium/src/+/refs/tags/133.0.6917.1/chrome/browser/chrome_browser_application_mac_browsertest.mm)
- [Chromium asynchronous accessibility action semantics](https://chromium.googlesource.com/chromium/src/+/main/docs/accessibility/browser/how_a11y_works_3.md)

## Fix

The fix reuses the existing renderer activation signal, deadline, and observation retry. The policy and click implementation stay unchanged.

- Check enhanced mode even when web content is already readable. A readable tree is not proof that its actions work.
- Read the flag back after a write. For enhanced mode, a matching readback can establish activation despite the unsupported return code. A contradictory readback cannot.
- Wait out the two-second debounce within the existing deadline. Reject a budget that is too small before writing. Already-enabled modes do not pay this delay.
- Keep older manual-mode renderers working when their tree is readable but their manual flag has no readable value.

The fix adds no browser-name checks, DOM automation, or physical click fallback. A click can still return `delivered_unverified`, so callers must check the requested effect.

Evidence: the test `observed_activation_overrides_a_setters_not_implemented_response` in `crates/macos/src/system/renderer_activation.rs`.
