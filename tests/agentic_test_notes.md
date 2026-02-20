# Agent-Desktop Agentic Test Run
## Date: 2026-02-19

## Task: Multi-step desktop automation (TextEdit + Calculator)

---

## Command Results

### Phase 1: Environment Assessment

#### 1. `status`
- Result: OK — returns permissions, platform, ref_count, version
- Issues: NONE

#### 2. `permissions`
- Result: OK — `{"granted":true}`
- Issues: NONE

#### 3. `list-apps`
- Result: OK — returned 8 apps with name + pid
- Issues:
  - **BUG (minor):** `data` is a bare array, not `{"apps":[...]}`. Inconsistent with other commands that nest under a named key. An agent parsing `d["data"]["apps"]` gets a TypeError. Should be `{"apps": [...]}`.
  - **MISSING:** `bundle_id` is always `null`. Should be populated (e.g. `com.apple.TextEdit`) — agents need this for reliable app identification since display names can vary by locale.

### Phase 2: App Launch & Window Management

#### 4. `launch Calculator`
- Result: FAILED — `"App 'Calculator' launched but no window found"`
- Issues:
  - **BUG:** The app did launch successfully and had a window 2 seconds later. `launch` doesn't wait long enough for the window to appear. Should poll for window with a configurable timeout (default ~5s) before declaring failure.
  - The app DID get launched — the error is misleading. Should differentiate "failed to launch" from "launched but window not yet visible."

#### 5. `list-windows --app TextEdit`
- Result: OK — correct window info with id, title, pid, is_focused
- Issues: NONE

#### 6. `focus-window --app TextEdit`
- Result: OK — focused and returned window info
- Issues: NONE

### Phase 3: Observation — Understanding the UI

#### 7. `snapshot --app TextEdit --interactive-only --compact`
- Result: OK — 12 refs, clean tree with roles and names
- Issues: NONE — interactive-only and compact flags work well together

#### 8. `find --app TextEdit --role button`
- Result: OK — found 3 buttons with ref, role, path
- Issues:
  - **IMPROVEMENT:** Buttons @e9, @e10, @e11 have `"name": null`. These are the Bold/Italic/Underline toolbar buttons. The `find` output gives no way to distinguish them without snapshotting. Should fall back to showing `description` or `title` when `name` is null.

#### 9. `find --app TextEdit --role textfield`
- Result: OK — found @e1 with path `["window:Untitled.rtf","scrollarea"]`
- Issues: NONE

### Phase 4: Interaction — Writing Content

#### 10. `click @e1`
- Result: OK
- Issues: NONE

#### 11. `type @e1 "multi-line text"`
- Result: OK — multi-line text with newlines typed correctly
- Issues: NONE — handles newlines in quoted strings well

#### 12. `get @e1 --property text`
- Result: OK — returned full typed text with preserved newlines
- Issues: NONE

### Phase 5: Text Selection & Clipboard

#### 13. `press --app TextEdit "cmd+a"`
- Result: OK
- Issues: NONE

#### 14. `press --app TextEdit "cmd+c"`
- Result: OK
- Issues: NONE

#### 15. `clipboard-get`
- Result: OK — clipboard had the full document text
- Issues: NONE

#### 16. `clipboard-set "new text"`
- Result: OK
- Issues: NONE

#### 17. `clipboard-get` (after set)
- Result: OK — verified clipboard-set worked
- Issues: NONE

### Phase 6: Advanced Interactions

#### 18. `set-value @e1 "replacement"`
- Result: OK — text replaced directly
- Issues: NONE

#### 19. `get @e1 --property value`
- Result: OK — confirmed set-value text
- Issues: NONE — `text` and `value` both return the textfield content (consistent)

#### 20. `double-click @e1`
- Result: OK
- Issues: NONE (visually selected a word)

#### 21. `right-click @e1`
- Result: OK — context menu opened
- Issues: NONE

#### 22. `wait --menu --app TextEdit`
- Result: OK — `{"elapsed_ms":25,"found":true}`
- Issues: NONE

#### 23. `list-surfaces --app TextEdit` (with menu open)
- Result: OK — detected `context_menu` with 31 items
- Issues: NONE

#### 24. `snapshot --surface menu --app TextEdit`
- Result: OK — full context menu tree with 31 menu items
- Issues: NONE

#### 25. `press "escape"`
- Result: OK — dismissed context menu
- Issues:
  - **BUG (moderate):** After pressing escape, the NEXT `snapshot --app TextEdit` (default surface=window) still included the 31 context menu refs mixed in with the window refs (e.g. @e2-@e32 were menuitems). The menu refs only disappeared on a subsequent snapshot. Likely a timing issue — the menu hasn't fully closed in the AX tree when the snapshot happens immediately. Should either: (a) poll until the menu is gone, or (b) filter out menu items from window-surface snapshots.

### Phase 7: Scroll

#### 26. `scroll @e1 --direction down --amount 3`
- Result: OK
- Issues: NONE — no way to verify scroll position changed (no scroll offset in get properties)

#### 27. `scroll @e1 --direction up --amount 3`
- Result: OK
- Issues: NONE

### Phase 8: Screenshot & State Queries

#### 28. `screenshot --app TextEdit /tmp/agentic_test.png`
- Result: OK — 96KB PNG captured correctly
- Issues: NONE

#### 29. `is @e1 --property visible`
- Result: OK — `true`
- Issues: NONE

#### 30. `is @e1 --property enabled`
- Result: OK — `true`
- Issues: NONE

#### 31. `is @e1 --property focused`
- Result: OK — `true`
- Issues: NONE

#### 32. `is @e1 --property checked`
- Result: OK — `false` (textfield isn't checkable)
- Issues:
  - **IMPROVEMENT:** Returns `false` instead of something like `"not_applicable"`. An agent can't tell if the element is unchecked vs. not-a-checkbox. This could cause logic errors (e.g., agent thinks a checkbox is unchecked when it's actually a button).

#### 33. `is @e1 --property expanded`
- Result: OK — `false`
- Issues: Same as checked — `false` vs "not applicable" ambiguity.

#### 34. `get @e1 --property role`
- Result: OK — `"textfield"`
- Issues: NONE

#### 35. `get @e1 --property bounds`
- Result: OK structurally — `"value": null`
- Issues:
  - **BUG:** Bounds returns `null` for @e1 even though the element clearly has screen position. Need to use `--include-bounds` in snapshot to see bounds. The `get` command should return bounds independently without requiring a snapshot flag.

#### 36. `get @e1 --property states`
- Result: OK — `["focused"]`
- Issues: NONE

#### 37. `get @e1 --property title`
- Result: OK — `null` (textfield has no title attribute)
- Issues: NONE

### Phase 9: Expand / Collapse / Toggle / Select

#### 38. `expand @e4` (combobox)
- Result: FAILED — `ACTION_FAILED` (err=-25205)
- Issues:
  - **BUG:** macOS comboboxes don't support AXExpand. The error message is raw AX error code with no suggestion. Should detect that expand isn't in the element's supported actions and return `ACTION_NOT_SUPPORTED` with suggestion: "This element doesn't support expand. Try 'click' to open it instead."

#### 39. `collapse @e4` (combobox)
- Result: FAILED — same as expand
- Issues: Same as above — should return `ACTION_NOT_SUPPORTED`.

#### 40. `toggle @e1` (textfield)
- Result: OK (returned success)
- Issues:
  - **BUG:** `toggle` returned `ok:true` on a textfield which doesn't support toggling. No visible effect occurred. Should either fail with `ACTION_NOT_SUPPORTED` or check supported actions before attempting.

#### 41. `select @e4 "Courier"` (combobox)
- Result: OK structurally (returned success)
- Issues:
  - **BUG:** `select` returned `ok:true` but the combobox value remained "Helvetica" — the font did NOT change. The command silently succeeded without actually performing the selection. Should verify the value changed, or at minimum, not return success if the AX select action wasn't available.

### Phase 10: Calculator (Cross-App Test)

#### 42. `snapshot --app Calculator --interactive-only --compact`
- Result: OK — 53 refs, all buttons properly named ("7", "8", "Add", "Equals", etc.)
- Issues: NONE — excellent button labeling from macOS AX

#### 43. Clicking buttons: `@e27 @e39 @e20 @e18 @e50` (43 × 8 =)
- Result: OK — all 5 clicks registered, screenshot confirmed display showed 344
- Issues:
  - **MISSING:** Calculator display value is NOT in the accessibility tree. An agent cannot read the calculation result programmatically — must use screenshot + OCR. The display is likely a custom-rendered view with no AX value. This is a macOS/Apple limitation, not an agent-desktop bug, but worth documenting.

### Phase 11: Close App

#### 44. `close-app Calculator`
- Result: Reported `ok:true` but app was still running
- Issues:
  - **BUG:** `close-app` reported success but Calculator was still listed in `list-windows` even after 1 second. Only `close-app --force` actually killed it. The non-force variant either isn't sending the right signal or isn't waiting for confirmation. Should either: (a) verify the app actually quit before returning success, or (b) return a different status like `"requested":true` if it can't confirm termination.

### Phase 12: Error Handling

#### 45. `click @e999`
- Result: OK — `STALE_REF` with helpful suggestion
- Issues: NONE — good error UX

#### 46. `snapshot --app NonExistentApp`
- Result: OK — `APP_NOT_FOUND`
- Issues: NONE

#### 47. `wait --element @e999 --timeout 1500`
- Result: OK — `TIMEOUT` with suggestion
- Issues: NONE

#### 48. `wait --window "NonExistent" --app TextEdit --timeout 1500`
- Result: OK — `TIMEOUT`
- Issues: NONE

#### 49. `batch [...] --stop-on-error`
- Result: OK — stopped correctly after failing command, didn't execute remaining
- Issues: NONE

### Phase 13: Help / UX

#### 50. `--help` (top-level)
- Result: OK — shows all commands with categories
- Issues:
  - **IMPROVEMENT:** Subcommand descriptions are empty. Every `Commands:` line just shows the name with no description. Should have one-line descriptions (e.g., `snapshot    Capture the accessibility tree of an application`).

#### 51. `snapshot --help`
- Result: OK — shows all options
- Issues:
  - **IMPROVEMENT:** No option descriptions. `--app`, `--max-depth`, `--compact` etc. have no help text. Should describe what each flag does.

### Phase 14: Global Output Issues

#### 52. Stderr duplication
- Issues:
  - **BUG:** Every error JSON is printed TWICE (both to stdout and stderr, or duplicated on one stream). Makes parsing harder — an agent doing `json.load(sys.stdin)` gets a parse error if stderr bleeds into stdout. Error should appear once on stdout (for machine parsing) and optionally on stderr (for human debugging with `--verbose`).

---

## Summary

### Working Well (22/30 commands flawless)
- `status`, `permissions`, `version` — solid
- `snapshot` (all surface variants) — excellent, the core strength
- `find` — works well for element discovery
- `get` (text, value, role, states) — reliable
- `is` (visible, enabled, focused) — works
- `click`, `double-click`, `right-click` — all work
- `type`, `set-value` — both work correctly including multi-line
- `press` (with `--app` targeting) — keyboard automation solid
- `clipboard-get`, `clipboard-set` — roundtrip perfect
- `scroll` — works (hard to verify but no errors)
- `wait` (all variants: ms, --element, --window, --menu) — timeouts & success both correct
- `list-windows`, `focus-window` — reliable
- `list-surfaces` — correctly detects menus
- `screenshot` — clean captures
- `batch` (with and without --stop-on-error) — correct behavior

### Bugs Found (7)
1. **`list-apps` data shape** — bare array instead of `{"apps":[...]}`
2. **`launch` window detection** — doesn't wait long enough, reports failure when app launches fine
3. **`close-app` (non-force)** — reports success but app keeps running
4. **`expand`/`collapse` error code** — returns `ACTION_FAILED` instead of `ACTION_NOT_SUPPORTED`
5. **`toggle` false success** — returns ok on elements that don't support toggle
6. **`select` false success** — returns ok but value doesn't change
7. **Stderr duplication** — all errors printed twice

### Needs Improvement (5)
1. **`get --property bounds`** returns null — should work independently of snapshot flags
2. **`is` checked/expanded** — returns `false` instead of "not applicable" for irrelevant properties
3. **`find` output** — buttons with null names are indistinguishable without secondary info
4. **`--help` descriptions** — all subcommands and options lack description text
5. **`list-apps` bundle_id** — always null, should be populated

### Missing Features / Gaps (2)
1. **Calculator display** — not accessible via AX tree (macOS limitation, not a bug)
2. **Scroll position** — no way to query current scroll offset

### Severity Ranking
1. **P0 (breaks agent workflows):** stderr duplication (JSON parse failures), `select`/`toggle` false success (agent thinks action worked when it didn't)
2. **P1 (degrades reliability):** `launch` window detection, `close-app` not actually closing, `expand`/`collapse` wrong error code
3. **P2 (minor inconsistencies):** `list-apps` shape, `get bounds` null, `is` false vs N/A, help text missing, bundle_id null
