---
title: Xcode search clears successfully but exposes no AXValue
module: macos adapter
problem_type: runtime_error
tags: [xcode, value, verification, headless]
date: 2026-09-20
---

# Empty search-field evidence gap

Candidate `/tmp/ad-native-eval/descendant-stop-candidate`, SHA-256
`5f2bf71761548cd6cbb58b8161b0fbc2dc4bfee033f9d30fec603f93613a2115`.
Xcode PID 24741, window w-19740. Public CLI with raw responses, namespace
`/tmp/ad-native-eval/xcode-state`; no response parser supplied action refs.

## Observed behavior

Debug filter `@s287cvyqznmcqt:e75` initially returned empty string. Set-value to
ad_probe_93c217 verified, get returned that exact string, and a viewed screenshot
showed it in the debug filter. Clear on the same ref verified and get returned
empty string again.

Project navigator filter `@s287cvyqznmcqt:e3` accepted CursorCamAppDelegate.
The scoped navigator snapshot then contained just its parent groups and the
matching file. Viewed screenshot confirmed the actual list changed. Clear on the
same ref returned delivered_unverified with insufficient_evidence. No write was
repeated. A subsequent get returned null, while a scoped snapshot showed the full
original navigator tree and original selected row restored. The final screenshot
was also viewed and showed both filters empty and the original editor retained.

Artifacts:
- `/tmp/ad-native-eval/xcode-debug-filter-filled.png`
- `/tmp/ad-native-eval/xcode-navigator-filter-filled.png`
- `/tmp/ad-native-eval/xcode-filter-restored.png`

The 60-second monitor recorded zero activation, zero input-event deltas and no
pointer change; foreground PID 35000 remained unchanged. Its interval covered the
debug-filter round trip and navigator filtering. The subsequent restoration
observations occurred after completion; do not extend the monitor claim to them.

## Raw AX diagnosis

Read-only bounded helper `/tmp/ad-native-eval/xcode-filter-read.swift` reported:

```
AXRole: success, AXTextField
AXSubrole: success, AXSearchField
AXDescription: success, Project navigator filter
AXValue: -25212, nil
AXNumberOfCharacters: success, 0
AXPlaceholderValue: success, Filter
```

This is not a demonstrated dropped write. Xcode exposes no AXValue for the empty
search control; Agent Desktop currently lacks a shared rule for using the native
zero-character evidence. Any repair must preserve unknown versus empty: restrict
it to nonsecure text roles, require a successful numeric zero character count,
and never infer empty from timeout, unsupported count, positive count or secure
content. Snapshot, get and post-action readback must share the rule. Trace both
node_attribute_fetch's batch-value path and text_attributes' direct-value path
before implementing; changing only verification would leave contradictory reads.

## File activation lead ruled out

Raw AX row actions remained AXShowDefaultUI and AXShowAlternateUI. Apple's
[showDefaultUI documentation](https://developer.apple.com/documentation/appkit/nsaccessibility-swift.struct/action/showdefaultui)
describes restoring the original/default UI, not opening a file. No new action
mapping was added and no raw mutation was attempted. File opening remains open.

Corrected the preceding repair's blast-radius note: activate_descendant is used
by click and semantic toggle/check paths. Double-click is currently physical and
does not use that helper. No source files in the Xcode project were edited.

## Repair

Added a shared macOS empty_text reader used by batch node hydration and direct
typed value reads. Only missing values on nonsecure AXTextField/AXTextArea
elements can trigger it. A successful numeric zero AXNumberOfCharacters yields
empty string; positive, negative, fractional, boolean, string, absent and failed
counts do not prove empty. Real read errors propagate. The existing deadline
still governs the extra read; ordinary nonempty values add no IPC. Batch metrics
record the fallback read.

The negative test caught the initial reuse of the broad ax_absence classifier,
which treats generic kAXErrorFailure as absence. The new count reader instead
permits only NoValue and AttributeUnsupported as absent. The failed test run is
retained in `/tmp/ad-native-eval/empty-text-workspace.log`.

The first candidate (4e34fdbe5b1e760d9db2f41da95dc4b477ca7f18be660bed1dc55211e3bf12ff)
passed a live set/filter/clear/get round trip on the original filter ref. Clear
changed from delivered_unverified to delivered_verified, and get returned empty
string. Scoped snapshot still omits empty values through the existing core
observed_tree projection; it does not expose a contradictory nonempty value.
Viewed `/tmp/ad-native-eval/empty-text-restored.png` showed the full original
navigator and empty filters. The 60-second monitor recorded zero input events,
zero activation and unchanged pointer/foreground. Final candidate checks follow
the stricter native-error classification change above.

Final candidate `/tmp/ad-native-eval/empty-text-final-candidate`, SHA-256
`6e9626edf059424869caba2519b8bbb2d749c7443512e5c8045ca9c8809789c1`.
Full workspace, clippy, formatting and diff whitespace checks passed after the
classifier correction. Final-binary get on replacement filter ref e246 returned
empty string. The final three-round merge-base performance check succeeded for
all commands with matching reported shapes. Candidate/base p50 milliseconds:
interactive snapshot 300.5/315.2, skeleton 201.2/205.2, depth-30 309.5/283.9,
find 236.6/227.9. The fallback adds a character-count read only for eligible
missing text values; depth-30 median was 25.6 ms higher in this small sample,
with candidate p95 314.4 vs baseline 325.4 ms. This is not a statistical claim
of equal performance. Artifacts are under
`/tmp/ad-native-eval/empty-text-final-performance/`.

The exact final binary passed all seven background semantic fixture assertions
(`/tmp/ad-native-eval/empty-text-final-semantic.log`). No commit or release was
performed. This closes the diagnosed empty-search-value verification gap; it
does not qualify file opening or the full repeated application acceptance suite.
