---
date: 2026-02-23
topic: macos-ax-first-robustness
---

# macOS AX-First Robustness

> Current contract note (2026-05-12): this brainstorm is historical. The current
> implementation keeps CLI ref commands headless by default; focus, cursor,
> keyboard, pasteboard, and CGEvent paths require explicit physical/headed
> commands or explicit FFI policy selection.

## What We're Building

Harden the macOS adapter so every command optimizes for AX-first execution (headless-capable, background-safe), with multi-step fallback chains, actionable error suggestions, and resilience against transient IPC failures.

## Why This Approach

agent-desktop's core value proposition is headless/background desktop automation. AX API actions work on unfocused windows; CGEvent coordinate-based input requires the window to be in front. Every command that currently does a single AX call and gives up is a missed opportunity — and a broken experience when running in the background.

`smart_activate()` (click) already has a 14-step chain. `ax_scroll()` has an 8-step chain. The other commands (`set-value`, `type`, `focus`, `press`, `clear`, `hover`, `drag`) are single-shot with no fallback. This brainstorm covers bringing them all up to parity.

## Current State Audit

### Commands WITH Robust Fallback Chains
| Command | Chain | Steps | CGEvent Fallback |
|---------|-------|-------|------------------|
| click | `smart_activate()` | 14 | Yes (last resort) |
| double-click | `smart_double_activate()` | AXOpen -> 2x smart_activate -> CGEvent | Yes |
| right-click | `smart_right_activate()` | AXShowMenu -> focus+retry -> select+menu -> focus+menu -> parent -> child -> CGEvent | Yes |
| triple-click | `smart_triple_activate()` | 3x smart_activate -> CGEvent | Yes |
| scroll | `ax_scroll()` | 8 | Yes (mouse wheel, last) |
| select | `select_value()` | Role-specific (combobox/popup/list) | No |
| expand/collapse | AXExpand -> AXDisclosing | 2 steps | No |
| check/uncheck | State check -> smart_activate if needed | 2 | Yes (via smart_activate) |
| toggle | Role check -> smart_activate | 2 | Yes (via smart_activate) |

### Commands WITHOUT Fallback Chains (Single-Shot)
| Command | Current Behavior | Needs Chain |
|---------|-----------------|-------------|
| set-value | Single `AXUIElementSetAttributeValue` | Yes |
| type | Focus element + `synthesize_text()` (system-wide CGEvent keys) | Yes |
| clear | Delegates to `ax_set_value(el, "")` | Yes |
| focus | Single `AXUIElementSetAttributeValue(kAXFocusedAttribute)` | Yes |
| scroll-to | Single `AXScrollToVisible` | Yes |
| press | System-wide `AXUIElementPostKeyboardEvent` | Partial |
| hover | Returns `ActionNotSupported` at element level | Needs rethink |
| drag | Returns `ActionNotSupported` at element level | Needs rethink |
| key-down/key-up | Returns `ActionNotSupported` at element level | Needs rethink |

### Commands Missing Error Suggestions
| Command | Current Error | Missing Suggestion |
|---------|--------------|-------------------|
| set-value | "SetValue failed (err=N)" | "Try 'clear' then 'type', or check element is a text field." |
| focus | "SetFocus failed (err=N)" | "Try 'click' to focus the element instead." |
| type | (no error path for AX focus fail) | "If typing fails, try 'set-value' for direct text insertion." |
| press | "AXUIElementPostKeyboardEvent failed" | "Ensure the target app is focused, or use 'press' with --app flag." |
| clear | Same as set-value | "Try selecting all text (press cmd+a) then press delete." |
| scroll-to | Has suggestion already | OK |
| hover/drag/key-down/key-up | "requires adapter-level handling" | "Use 'mouse-move --xy X,Y' for hover" / "Use 'drag --from-xy --to-xy'" |

## Window Focus Policy

**Principle:** Only focus the window when falling back to CGEvent coordinate-based input.

| Action Type | Needs Window Focus | Why |
|-------------|-------------------|-----|
| AXUIElementPerformAction | No | AX IPC targets the element directly |
| AXUIElementSetAttributeValue | No | AX IPC targets the element directly |
| AXUIElementPostKeyboardEvent (to app) | No | Targets app AXElement by PID |
| CGEventPost (mouse/keyboard) | **Yes** | System-wide events go to frontmost window |
| `synthesize_text()` (system-wide) | **Yes** | Posts to system-wide AX element |
| `synthesize_mouse()` | **Yes** | CGEvent coordinate-based |

**Current right-click**: `smart_right_activate()` tries AXShowMenu first (no focus needed). Falls back to CGEvent right-click only as last resort (focuses window at that point). This is correct — AXShowMenu works on unfocused windows.

**Type command issue**: Currently uses system-wide `AXUIElementPostKeyboardEvent` via `synthesize_text()`. This means keys go to whatever is focused system-wide, NOT specifically to the target app. The chain should try `AXSetValue` first, then app-targeted key posting, then system-wide as last resort.

## Centralized Chain Architecture

### Problem with Per-Command Chains

The current codebase has 3 hand-written chains (`smart_activate` 14 steps, `smart_right_activate` 7 steps, `ax_scroll` 8 steps) with ~100-150 LOC of duplicated patterns:

- **AXScrollToVisible preamble** — 3 places, 3 different error strategies
- **list_ax_actions / has_ax_action** — near-duplicate functions across files
- **Attribute set boilerplate** — 6+ instances of CFString + AXUIElementSetAttributeValue + error check
- **Child/parent walk loops** — 5 near-identical traversals with different predicates
- **Sleep timing** — 7 instances, 4 different durations, no policy

Writing per-command chains would add 6 more instances of this same boilerplate. Instead: centralize.

### Design: Centralized Element Discovery + Chain Executor

Two layers: (1) discover what an element supports once, (2) execute a declarative chain.

#### Layer 1: Element Capabilities (one-time discovery)

```rust
// New file: crates/macos/src/actions/discovery.rs

pub struct ElementCaps {
    pub actions: Vec<String>,        // AXPress, AXConfirm, AXOpen, AXShowMenu, etc.
    pub settable_value: bool,        // kAXValueAttribute is settable
    pub settable_focus: bool,        // kAXFocusedAttribute is settable
    pub settable_selected: bool,     // AXSelected is settable
    pub settable_disclosing: bool,   // AXDisclosing is settable
    pub role: Option<String>,        // normalized role string
    pub has_children: bool,
    pub pid: Option<i32>,
}

pub fn discover(el: &AXElement) -> ElementCaps {
    // Single batch call: list actions + check settable attrs + read role
    // Replaces scattered is_attr_settable + list_ax_actions + element_role calls
}
```

Every chain step gets `&ElementCaps` instead of re-querying the element. This avoids redundant IPC calls to the accessibility server.

#### Layer 2: Chain Executor (declarative steps)

```rust
// New file: crates/macos/src/actions/chain.rs

pub enum ChainStep {
    /// Try AXUIElementPerformAction with given action name
    PerformAction(&'static str),
    /// Try AXUIElementSetAttributeValue
    SetAttr { attr: &'static str, value: AttrValue },
    /// Try the step on the element, if fails try with focus first
    WithFocus(Box<ChainStep>),
    /// Try the step on up to N children
    OnChildren { step: Box<ChainStep>, limit: usize },
    /// Try the step on up to N ancestors
    OnAncestors { step: Box<ChainStep>, limit: usize },
    /// Custom logic (for command-specific steps that don't fit the pattern)
    Custom(fn(&AXElement, &ElementCaps) -> bool),
    /// Explicit physical policy step only
    CGEvent(CGFallback),
}

pub enum AttrValue {
    Bool(bool),
    Str(String),
}

pub enum CGFallback {
    Click(MouseButton, u32),
    MouseWheel { dy: i32, dx: i32 },
    None,
}

pub struct AXChain {
    pub steps: Vec<ChainStep>,
    pub suggestion: &'static str,
}

pub fn execute_chain(
    el: &AXElement,
    caps: &ElementCaps,
    chain: &AXChain,
) -> Result<(), AdapterError> {
    // 1. Pre-action: AXScrollToVisible (best-effort)
    // 2. Pre-action: AXUIElementSetMessagingTimeout(el, 3.0)
    // 3. Walk steps, try each with retry on kAXErrorCannotComplete
    // 4. If all fail: return error with chain.suggestion
}
```

#### Layer 3: Command Definitions (data, not code)

Each command declares its chain as a static definition. No per-command function bodies needed.

```rust
// In dispatch.rs or a new chain_defs.rs

fn click_chain() -> AXChain {
    AXChain {
        steps: vec![
            PerformAction("AXPress"),
            PerformAction("AXConfirm"),
            PerformAction("AXOpen"),
            PerformAction("AXPick"),
            Custom(try_show_alternate_ui),
            OnChildren { step: Box::new(PerformAction("AXPress")), limit: 3 },
            SetAttr { attr: "AXSelected", value: AttrValue::Bool(true) },
            OnAncestors { step: Box::new(SetAttr { attr: "AXSelectedRows", .. }), limit: 2 },
            Custom(try_custom_actions),
            WithFocus(Box::new(PerformAction("AXPress"))),
            Custom(try_keyboard_activate),
            OnAncestors { step: Box::new(PerformAction("AXPress")), limit: 2 },
            CGEvent(CGFallback::Click(MouseButton::Left, 1)),
        ],
        suggestion: "Element may not be interactable. Try 'mouse-click --xy X,Y'.",
    }
}

fn set_value_chain(value: &str) -> AXChain {
    AXChain {
        steps: vec![
            SetAttr { attr: "AXValue", value: AttrValue::Str(value.into()) },
            WithFocus(Box::new(SetAttr { attr: "AXValue", value: AttrValue::Str(value.into()) })),
        ],
        suggestion: "Try 'clear' then 'type', or check element is a text field.",
    }
}

fn focus_chain() -> AXChain {
    AXChain {
        steps: vec![
            SetAttr { attr: "AXFocused", value: AttrValue::Bool(true) },
            PerformAction("AXPress"), // clicking often sets focus as side effect
        ],
        suggestion: "Try 'click' to focus the element instead.",
    }
}

fn clear_chain() -> AXChain {
    AXChain {
        steps: vec![
            SetAttr { attr: "AXValue", value: AttrValue::Str(String::new()) },
            WithFocus(Box::new(SetAttr { attr: "AXValue", value: AttrValue::Str(String::new()) })),
            Custom(select_all_then_delete), // Cmd+A, Delete via app-targeted keys
        ],
        suggestion: "Try 'press cmd+a' then 'press delete'.",
    }
}

fn right_click_chain() -> AXChain {
    AXChain {
        steps: vec![
            PerformAction("AXShowMenu"),
            WithFocus(Box::new(PerformAction("AXShowMenu"))),
            Custom(try_select_then_show_menu),
            OnAncestors { step: Box::new(PerformAction("AXShowMenu")), limit: 3 },
            OnChildren { step: Box::new(PerformAction("AXShowMenu")), limit: 5 },
            CGEvent(CGFallback::Click(MouseButton::Right, 1)),
        ],
        suggestion: "Try 'mouse-click --button right --xy X,Y'.",
    }
}

fn scroll_to_chain() -> AXChain {
    AXChain {
        steps: vec![
            PerformAction("AXScrollToVisible"),
            Custom(visible_in_scroll_context),
        ],
        suggestion: "Element may not be in a scrollable container.",
    }
}

fn expand_chain() -> AXChain {
    AXChain {
        steps: vec![
            PerformAction("AXExpand"),
            SetAttr { attr: "AXDisclosing", value: AttrValue::Bool(true) },
        ],
        suggestion: "Try 'click' to open it instead.",
    }
}

fn collapse_chain() -> AXChain {
    AXChain {
        steps: vec![
            PerformAction("AXCollapse"),
            SetAttr { attr: "AXDisclosing", value: AttrValue::Bool(false) },
        ],
        suggestion: "Try 'click' to close it instead.",
    }
}
```

### Shared Utilities (extracted from current duplicated code)

```rust
// New file: crates/macos/src/actions/ax_helpers.rs

/// Set a boolean attribute on an element. Returns true on success.
pub fn set_ax_bool(el: &AXElement, attr: &str, value: bool) -> bool

/// Set a string attribute on an element. Returns true on success.
pub fn set_ax_string(el: &AXElement, attr: &str, value: &str) -> bool

/// Perform a named AX action. Returns true on success.
pub fn try_ax_action(el: &AXElement, action: &str) -> bool

/// List all available AX actions on an element.
pub fn list_ax_actions(el: &AXElement) -> Vec<String>
// (replaces both list_ax_actions in activate.rs AND has_ax_action in dispatch.rs)

/// Try a predicate on each child element, up to limit.
pub fn try_each_child(el: &AXElement, f: impl Fn(&AXElement) -> bool, limit: usize) -> bool

/// Try a predicate on ancestors, up to limit.
pub fn try_each_ancestor(el: &AXElement, f: impl Fn(&AXElement) -> bool, limit: usize) -> bool

/// Retry an AX operation up to N times on kAXErrorCannotComplete.
pub fn with_retry<F, T>(f: F, max_retries: u32) -> Result<T, i32>
    where F: Fn() -> Result<T, i32>

/// Set messaging timeout to prevent 6s hangs on unresponsive apps.
pub fn set_messaging_timeout(el: &AXElement, seconds: f32)

/// Best-effort AXScrollToVisible before acting.
pub fn ensure_visible(el: &AXElement)
```

### What This Replaces

| Current Code | Replaced By |
|-------------|-------------|
| `smart_activate()` 64 LOC | `click_chain()` 15 LOC + chain executor |
| `smart_right_activate()` 30 LOC | `right_click_chain()` 10 LOC + chain executor |
| `smart_double_activate()` 12 LOC | Composing click_chain with count=2 |
| `smart_triple_activate()` 7 LOC | Composing click_chain with count=3 |
| `try_child_activation()` 10 LOC | `OnChildren` step in chain |
| `try_parent_activation()` 13 LOC | `OnAncestors` step in chain |
| `try_child_show_menu()` 8 LOC | `OnChildren` step in right_click_chain |
| `try_parent_show_menu()` 13 LOC | `OnAncestors` step in right_click_chain |
| `is_attr_settable` + set pattern (6 instances) | `set_ax_bool()` / `set_ax_string()` |
| `has_ax_action()` + `list_ax_actions()` (2 files) | Single `list_ax_actions()` in ax_helpers |
| Individual perform_action match arms | Chain definitions (data, not code) |

### What Stays Custom

Some commands have truly unique logic that doesn't fit the chain pattern:

- **scroll** (`ax_scroll`): Its 8-step chain is scroll-specific (scroll bars, page actions, value shifts). Stays as custom function, but uses shared helpers.
- **select** (`select_value`): Role-specific branching (combobox vs popup vs list). Stays custom, uses shared helpers.
- **type**: Mixed AX + keyboard synthesis + non-ASCII clipboard paste. Uses chain for the AX steps, but the keyboard/clipboard fallback is a `Custom` step.
- **hover/drag/key-down/key-up**: Adapter-level (coordinate-based), not element actions. Just need better error suggestions at the element-action level.

### New File Layout

```
crates/macos/src/actions/
├── mod.rs              # re-exports
├── dispatch.rs         # perform_action match arms (slimmed down, delegates to chains)
├── chain.rs            # ChainStep enum + execute_chain() executor
├── chain_defs.rs       # All chain definitions (click, set-value, focus, clear, etc.)
├── discovery.rs        # ElementCaps discovery (one-time per action)
├── ax_helpers.rs       # Shared AX utilities (set_ax_bool, try_each_child, retry, etc.)
├── activate.rs         # REMOVED (absorbed into chain_defs.rs + ax_helpers.rs)
└── extras.rs           # select_value + ax_scroll (kept, refactored to use ax_helpers)
```

### Benefits

1. **No redundancy**: Each AX pattern (set attr, perform action, walk children) implemented once
2. **Adding new commands is trivial**: Define a chain (5-15 LOC), no new functions needed
3. **Easy to tune**: Reorder chain steps, add/remove steps without touching executor
4. **Testable**: Can unit-test the executor with mock elements and verify step ordering
5. **Self-documenting**: Chain definitions read like a recipe — "try AXPress, try AXConfirm, try children, fall back to CGEvent"

### Type Command Special Case

The `type` command is the most complex because it mixes AX actions with keyboard synthesis:

```rust
fn type_chain(text: &str) -> AXChain {
    AXChain {
        steps: vec![
            // Step 1: Append via AX (handles non-ASCII natively)
            Custom(|el, caps| ax_append_value(el, text)),
            // Step 2: Focus + app-targeted keyboard (ASCII only, or clipboard paste for non-ASCII)
            Custom(|el, caps| focus_then_type_to_app(el, caps, text)),
            // Step 3: Focus + system-wide keyboard (last resort, needs element focused)
            Custom(|el, caps| {
                set_ax_bool(el, "AXFocused", true);
                synthesize_text(text).is_ok()
            }),
        ],
        suggestion: "Try 'set-value' for direct text insertion.",
    }
}
```

### hover / drag / key-down / key-up

These are inherently coordinate-based (adapter-level, not element actions). They don't need AX chains. Just improve error messages in the element-level `perform_action` match:

```rust
Action::Hover | Action::Drag(_) | Action::KeyDown(_) | Action::KeyUp(_) => {
    return Err(AdapterError::new(
        ErrorCode::ActionNotSupported,
        format!("{} is handled at the adapter level, not as an element action", label),
    )
    .with_suggestion(match action {
        Action::Hover => "Use 'hover @ref' which resolves ref to coordinates automatically.",
        Action::Drag(_) => "Use 'drag --from @ref --to @ref' for ref-based drag.",
        Action::KeyDown(_) | Action::KeyUp(_) => "Use 'key-down'/'key-up' commands directly.",
        _ => unreachable!(),
    }));
}

## Additional Gaps to Fix

### 1. AX Messaging Timeout
**Problem:** Unresponsive apps cause 6-second hangs per AX call (Apple default).
**Fix:** Call `AXUIElementSetMessagingTimeout(element, 3.0)` on every resolved element before performing actions. 3 seconds is generous enough for slow apps but prevents indefinite hangs.
**Where:** In `resolve_element_impl()` or at the start of `perform_action()`.

### 2. Retry on kAXErrorCannotComplete (-25204)
**Problem:** Transient IPC failure treated as terminal. Apple docs say this is a temporary condition.
**Fix:** Wrap AX calls in a retry helper: up to 2 retries with 100ms backoff for `kAXErrorCannotComplete`. Only retry idempotent operations (AXPress, AXSetValue, etc.).
**Where:** New utility function in `actions/` module, used by `perform_action()` and chain steps.

### 3. Non-ASCII Text Input
**Problem:** `synthesize_text()` silently drops any character without a keycode mapping (all non-ASCII).
**Fix:** For the `type` command chain, step 1 (AXSetValue append) handles non-ASCII natively. If falling back to key synthesis, detect non-ASCII characters and use clipboard-paste: save current clipboard, set clipboard to the non-ASCII segment, Cmd+V, restore clipboard.
**Where:** `input/keyboard.rs` `synthesize_text()` or the type chain in `dispatch.rs`.

### 4. NSPasteboard FFI for Clipboard
**Problem:** Current clipboard uses `pbpaste`/`pbcopy` shell subprocesses — slow (~50ms per call), fragile, and creates visible process spawns.
**Fix:** Use `NSPasteboard.generalPasteboard` via objc2/Cocoa FFI directly. This is ~10x faster and avoids subprocess overhead.
**Where:** Replace `input/clipboard.rs` implementation.

### 5. Stale Element Auto-Recovery
**Problem:** When an element ref becomes stale (`STALE_REF`), the agent must manually re-snapshot. Many actions could auto-recover by re-resolving the element.
**Fix:** In `execute_action_impl()`, if `resolve_element` returns `STALE_REF`, attempt one automatic re-resolution using the stored `(pid, role, name)` tuple. If the re-resolved element's bounds are close (within 50px), proceed with the action. Otherwise, return `STALE_REF` as before.
**Caution:** Only do this for single-element actions. Don't auto-recover during batch operations (could lead to wrong-element interactions).
**Where:** `adapter.rs` `execute_action_impl()` wrapper.

### 6. Post-Action State Hints
**Problem:** After a successful action, the agent has no feedback about what changed. It must re-snapshot to verify.
**Fix:** For certain actions, include a `hint` field in `ActionResult`:
- click/toggle/check/uncheck: read back the element's value/state after action
- set-value: read back value to confirm it was set
- type: read back value to confirm text was appended

This is optional enrichment — the `data` field can include `{ "post_state": { "value": "new text" } }`.
**Where:** After the action succeeds in `perform_action()`, do one quick attribute read.

## Key Decisions

- **Centralized chain architecture**: One executor + declarative step definitions, not per-command functions. Adding a command = defining a chain (5-15 LOC).
- **One-time element discovery**: `ElementCaps` struct queried once per action, shared across all chain steps. Eliminates redundant AX IPC calls.
- **AX-first, explicit physical paths only**: All default CLI ref commands try pure AX approaches and fail clearly rather than silently moving focus or the cursor.
- **Window focus only for explicit physical policy**: Never call `ensure_app_focused()` from the default ref command path.
- **App-targeted keys over system-wide**: `AXUIElementPostKeyboardEvent(app_element, ...)` targets a specific app. System-wide posting is last resort.
- **Non-ASCII via clipboard**: The most reliable cross-app method for typing non-ASCII characters on macOS.
- **3-second AX timeout**: Balances responsiveness with tolerance for slow apps.
- **Single retry for transient errors**: kAXErrorCannotComplete gets one retry with 100ms backoff, handled inside the executor.
- **Stale ref auto-recovery is opt-in**: Only for single-element actions, with bounds proximity check.
- **activate.rs absorbed**: All 400 LOC of activate.rs becomes ~50 LOC of chain definitions + shared helpers. No more per-command fallback functions.

## Open Questions

- Should the retry helper also handle `kAXErrorNotImplemented` (-25208) by skipping to next chain step instead of retrying?
- Should post-action state hints be opt-in via a `--verify` flag, or always-on?
- For non-ASCII clipboard paste: should we always restore the previous clipboard, or is that too slow for bulk typing?
- The NSPasteboard FFI change — should we add `objc2` as a dependency, or use raw `objc_msgSend` to keep deps minimal?
- `ChainStep::Custom` closures — use `fn` pointers (no captures, simple) or `Box<dyn Fn>` (can capture values like text string)?

## Next Steps

1. Build the foundation: `ax_helpers.rs` (shared utilities) + `discovery.rs` (ElementCaps)
2. Build `chain.rs` (executor)
3. Migrate `smart_activate` → `click_chain()` definition in `chain_defs.rs` (proves the pattern)
4. Migrate `smart_right_activate` → `right_click_chain()`
5. Add new chains: set-value, focus, clear, scroll-to, expand, collapse
6. Build type chain (complex: AX + keyboard + clipboard)
7. Refactor `ax_scroll` to use shared helpers (stays custom, but less duplicated)
8. Add error suggestions to hover/drag/key-down/key-up
9. Additional gaps: AX timeout, retry, non-ASCII, NSPasteboard, stale recovery, post-action hints
