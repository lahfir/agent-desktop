---
module: agent-desktop-macos
date: 2026-09-20
problem_type: runtime_error
tags: [numbers, text, parameterized-accessibility, diagnosis]
---

# Numbers advertises a separate parameterized text operation

The earlier probe enumerated actions and selected ordinary attributes, but omitted
parameterized attributes. That was an investigation gap; setter no-ops did not
exhaust the editor's advertised semantic contract.

On owned Numbers PID 46296, the focused B2 AXTextArea advertises:

- AXStringForRange
- AXAttributedStringForRange
- AXLineForIndex
- AXReplaceRangeWithText
- AXBoundsForRange
- AXRangeForLine

The writable ordinary attributes are AXFocused, AXSelectedText,
AXSelectedTextRange, AXSelectedTextRanges and AXValue. Advertised actions remain
AXShowMenu, TSAccessibilityAddCommentAction and TSAccessibilityDeselectAllAction.
The value remains seed-target. No replacement or commit was attempted in this
inventory probe, and no production dispatch path changed.

The independent `tests/real-apps/ax_probe.swift` now records ordinary and
parameterized attribute names, settable status for each advertised attribute,
and AXIdentifier/AXDOMIdentifier readback. The editor, cell and table exposed
neither identifier in this trial. This improves diagnostic coverage; it is not
an application-specific driver or acceptance success.

Apple's published AppKit parameterized-attribute index fetched through Context7
did not supply this operation's argument format. The local AppKit SDK export
list contains NSAccessibilityReplaceRangeWithTextParameterizedAttribute, while
the public header search did not find its declaration. WebKit's maintained
[test runner](https://github.com/WebKit/WebKit/blob/main/Tools/WebKitTestRunner/InjectedBundle/mac/AccessibilityUIElementMac.mm)
invokes accessibilityReplaceRange:withText: directly inside the target process.
That is evidence of a semantic operation, not the cross-process parameter format
or proof that Numbers implements it correctly.

## Cross-process contract and separate Numbers trial

Disassembling the installed AppKit implementation in an owned disposable helper
identified `_NSAccessibilityResultForReplaceRangeWithText`. It reads a dictionary
using `NSAccessibilityReplacementRangeKey` and `NSAccessibilityReplacementTextKey`,
extracts an NSRange, and invokes `accessibilityReplaceRange:withText:`. Reading
those exported constants in the helper yielded `AXReplacementRange` and
`AXReplacementText`. No debugger attached to Numbers.

`tests/real-apps/replace_contract_fixture.m` supplies an owned background window
with a receiver that logs the exact incoming range/text. The separate
`replace_range_probe.swift` sends a CFDictionary containing an AXValue CFRange
and CFString through AXUIElementCopyParameterizedAttributeValue. In PID 83111,
the receiver logged `{0, 11}` and `contract-verified`; native result was success
and true, and the actual text changed from seed-target to contract-verified.
Foreground endpoints both remained PID 75875. This proves parameter transport,
not the behavior of AppKit's inherited text implementation or Numbers.

In the subsequent separate Numbers trial, a fresh public CLI find returned
`@s19bkizkoef5ta:e1`, focused/selected and value seed-target. The older editor ref
correctly refused after its geometry changed. The raw helper selected the unique
AXTextArea with that seed value within the exact owned window. One replacement
requested parameterized-0920 at range `{0, 11}`. AX returned success and true,
but the editor still read seed-target. Independent document readback remained
`seed-target, neighbor-guard, 8, 5`. The inspected screenshot
`/tmp/ad-native-eval/parameterized-numbers.png` also showed seed-target in B2.

The thirty-second observer recorded no Numbers activation, but recorded other
app activations, pointer changes and session input. This is contaminated for
clean headless qualification; unchanged foreground endpoints do not cure that.
No automatic repeat or alternate setter followed the uncertain mutation.

The default AppKit method observed in the helper obtains the current input
context's client and calls insertText:replacementRange:. In the fixture's
`--standard` mode (plain NSTextView, no receiver override), PID 83938 likewise
returned native success and true but left seed-target unchanged after requesting
standard-background. Foreground endpoints remained 75875. Thus a controlled
background AppKit control reproduces the no-op with the validated payload; the
custom receiver succeeds. Input-context routing is a supported hypothesis, not
proof of Numbers' private implementation or a demonstrated headless remedy.
No production fallback was added: this additional advertised operation has not
demonstrated a working Numbers write, commit or persistence path. N4 remains
failed and N5/N8 remain uncompleted.

The checked diagnostic subsequently passed two writes in a fresh custom fixture:
seed-target to `A😀B`, then to unicode-range-verified. Receiver logs confirmed
UTF-16 ranges `{0, 11}` and `{0, 4}` respectively. A fresh standard fixture's
unchanged readback correctly caused diagnostic exit 1 despite native success
and true. All owned fixture processes were closed after their trials. These
checks validate the diagnostic; they add no Agent Desktop acceptance passes.

## Internal first-responder differential

The fixture's `--standard-focused` trial (PID 84721) called makeFirstResponder
on its own NSTextView. AppKit returned true and the window's firstResponder was
the text view, while app.active and window.keyWindow remained false. The text
view had an NSTextInputContext, but NSTextInputContext.currentInputContext was
nil. The cross-process replacement requested responder-background once and
returned native success and true; readback stayed seed-target, so the checked
probe exited 1. Foreground endpoints both remained PID 860.

This eliminates missing internal first-responder assignment as a sufficient
explanation or remedy for the standard-control result. It supports the observed
default AppKit route through the current input context. It does not establish
Numbers' private implementation, nor authorize app activation, process injection
or a new application-specific scripting backend. No production path changed.
The distinct tested AXValue, AXSelectedText and parameterized replacement paths
have still not supplied a working Numbers background write/commit prerequisite.
