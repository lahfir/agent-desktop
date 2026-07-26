---
title: "refactor: centralize macOS action dispatch via chain executor"
type: refactor
status: active
date: 2026-02-23
origin: docs/brainstorms/2026-02-23-macos-ax-first-robustness-brainstorm.md
---

# refactor: centralize macOS action dispatch via chain executor

> Current contract note (2026-05-12): this historical plan is superseded where it
> implies automatic focus, cursor, keyboard, pasteboard, or CGEvent fallback from
> normal ref commands. The current command path is headless by default. Physical
> or headed behavior is available only through explicit mouse/focus/keyboard
> commands or an explicit FFI policy.

## Overview

Replace per-command fallback functions (`smart_activate`, `smart_right_activate`, etc.) with a centralized chain executor where commands declare AX fallback steps as data. Also add error suggestions to all commands, AX messaging timeout, transient error retry, non-ASCII text support, NSPasteboard FFI clipboard, and post-action state hints.

This is a pure macOS adapter refactoring. Core crate and PlatformAdapter trait are unchanged.

## Problem Statement

The current macOS action dispatch has three problems:

1. **Duplicated patterns**: `activate.rs` (400 LOC), `dispatch.rs` (367 LOC), and `extras.rs` (396 LOC) share ~100-150 LOC of duplicated code: attribute setting, action enumeration, child/parent traversal, sleep timing. All three files are at the 400 LOC ceiling — any additions require splitting.

2. **Inconsistent robustness**: `click` has a 14-step chain, `scroll` has 8 steps, but `set-value`, `type`, `clear`, `focus`, `scroll-to`, and `press` are single-shot with no fallback. This breaks headless/background operation when the single AX call fails.

3. **Missing error guidance**: Several commands (`set-value`, `focus`, `type`, `press`, `clear`) return raw AX error codes with no suggestions for the calling agent.

(see brainstorm: `docs/brainstorms/2026-02-23-macos-ax-first-robustness-brainstorm.md`)

## Proposed Solution

### Architecture: 3-Layer Chain System

**Layer 1 — Shared Utilities (`ax_helpers.rs`)**
Extract all duplicated AX patterns into reusable functions: `try_ax_action()`, `set_ax_bool()`, `set_ax_string()`, `try_each_child()`, `try_each_ancestor()`, `with_retry()`, `ensure_visible()`, `set_messaging_timeout()`.

**Layer 2 — Element Discovery (`discovery.rs`)**
One-time `ElementCaps` struct queried once per action: available actions, settable attributes, role, PID. Eliminates redundant AX IPC calls across chain steps.

**Layer 3 — Chain Executor + Definitions (`chain.rs` + `chain_defs.rs`)**
`ChainStep` enum defines what to try. `execute_chain()` walks steps with pre-action setup (scroll-to-visible, messaging timeout) and post-action enrichment (state hints). Each command is a static chain definition — data, not code.

### Concrete Data Model

```rust
pub enum ChainStep {
    /// Try AXUIElementPerformAction with the given action name.
    Action(&'static str),

    /// Try AXUIElementSetAttributeValue with given attribute and value.
    SetAttr {
        attr: &'static str,
        value: AttrValue,
    },

    /// Focus the element first (AXFocused=true), then retry the inner step.
    WithFocus(Box<ChainStep>),

    /// Try the inner step on up to `limit` child elements.
    OnChildren {
        step: Box<ChainStep>,
        limit: usize,
    },

    /// Try the inner step on up to `limit` ancestor elements.
    OnAncestors {
        step: Box<ChainStep>,
        limit: usize,
    },

    /// Custom logic that cannot be expressed declaratively.
    /// Receives element + caps, returns true if the step succeeded.
    Custom {
        label: &'static str,
        func: fn(&AXElement, &ElementCaps) -> bool,
    },

    /// CGEvent coordinate-based fallback. Automatically focuses the window.
    CGEvent(CGFallback),
}

pub enum AttrValue {
    Bool(bool),
    StaticStr(&'static str),
    Dynamic, // Placeholder — actual value passed via ChainContext at runtime
}

pub enum CGFallback {
    Click(MouseButton, u32),
    None,
}

pub struct ChainDef {
    pub steps: &'static [ChainStep],
    pub suggestion: &'static str,
}
```

**Why `Custom` variant**: Steps like `try_show_alternate_ui` (AXShowAlternateUI → sleep → child AXPress), `try_select_via_parent` (walk parent, check role, set AXSelectedRows), and `try_keyboard_activate` (focus + space key) involve multi-step conditional logic that cannot be expressed as flat declarative steps without over-engineering. `Custom { label, func }` is the escape hatch — the label enables tracing, the func contains the logic.

**Why `AttrValue::Dynamic`**: `set-value` and `type` chains need runtime text values. The executor checks for `Dynamic` and substitutes from a `ChainContext` struct passed alongside.

### File Layout

```
crates/macos/src/actions/
├── mod.rs              # mod declarations + pub use perform_action
├── ax_helpers.rs       # NEW: shared AX utilities (~200 LOC)
├── discovery.rs        # NEW: ElementCaps one-time query (~120 LOC)
├── chain.rs            # NEW: ChainStep enum + execute_chain() (~250 LOC)
├── chain_defs.rs       # NEW: all chain definitions (~300 LOC)
├── dispatch.rs         # SIMPLIFIED: perform_action delegates to chains (~200 LOC)
├── activate.rs         # DELETED: absorbed into chain_defs.rs + ax_helpers.rs
└── extras.rs           # REFACTORED: select_value + ax_scroll use ax_helpers (~300 LOC)
```

LOC budget per file stays under 400.

## Technical Considerations

### Retry Policy

- **Transient errors**: Only `kAXErrorCannotComplete` (-25204). Apple docs classify this as "target app temporarily unable to process request."
- **Terminal errors**: `kAXErrorInvalidUIElement`, `kAXErrorNotImplemented`, `kAXErrorAPIDisabled`, `kAXErrorNoValue`.
- **Max retries**: 1 retry per step (not per chain).
- **Delay**: 100ms fixed between retry attempts.
- **Idempotency guard**: For stateful actions (`Toggle`, `Check`, `Uncheck`), read element value before retry. If value already changed from pre-action state, skip retry (action succeeded despite error code).
- **Retry scope**: Inside the chain executor's step loop. A step that fails after retry simply falls through to the next step.

### Side Effect Safety

```
Before chain:  read element value/state (if action is stateful)
  Step N:      try AX action → returns error
  Retry:       read element value/state again
               if changed → action succeeded, do NOT retry
               if unchanged → retry the step
```

This handles the "retried AXPress toggles checkbox back" problem.

### Window Focus Policy

(from brainstorm: Window Focus Policy section)

| Step Type | Focuses Window | Reason |
|-----------|---------------|--------|
| `Action(name)` | No | AX IPC targets element directly |
| `SetAttr { .. }` | No | AX IPC targets element directly |
| `WithFocus(step)` | No | Sets AXFocused on element, not window |
| `OnChildren/OnAncestors` | No | AX IPC |
| `Custom { .. }` | Depends on impl | Custom decides |
| `CGEvent(..)` | **Yes** | CGEvent is coordinate-based |

The executor only calls `ensure_app_focused()` inside the `CGEvent` step handler.

### Non-ASCII Text Input

Strategy: per-segment splitting.

1. Scan input text, split into runs of ASCII and non-ASCII characters.
2. ASCII runs: keyboard synthesis via `AXUIElementPostKeyboardEvent`.
3. Non-ASCII runs: clipboard paste strategy:
   a. Save current clipboard text (best-effort — non-text clipboard data types are lost on restore).
   b. Set clipboard to the non-ASCII segment.
   c. Synthesize Cmd+V via app-targeted keyboard event.
   d. Restore clipboard (log warning via `tracing::warn!` if restore fails, but report action as success).

**Prerequisite**: NSPasteboard FFI must be implemented first to avoid ~200ms subprocess overhead per clipboard operation.

### NSPasteboard FFI

Replace `pbcopy`/`pbpaste` subprocess calls with direct Cocoa FFI. Use raw `objc_msgSend` from `core-foundation` to avoid adding new dependencies:

```rust
// Pseudocode — actual impl uses core-foundation-sys + manual objc runtime calls
fn clipboard_get() -> Result<String, AdapterError> {
    let pasteboard = msg_send![class!(NSPasteboard), generalPasteboard];
    let string = msg_send![pasteboard, stringForType: NSPasteboardTypeString];
    // Convert NSString to Rust String
}
```

No new crate dependency needed — `core-foundation-sys` already provides the FFI primitives. The `objc_msgSend` calls are ~10 LOC per function.

### Stale Ref Auto-Recovery

Lives inside the macOS adapter's `resolve_element` implementation (not in core). PlatformAdapter trait stays unchanged.

```
resolve_element(entry) called
  → normal resolution: match (pid, role, name, bounds_hash)
  → if ELEMENT_NOT_FOUND:
      → relaxed resolution: match (pid, role, name) only
      → if found: check bounds proximity (within 50px any edge)
      → if bounds acceptable: return handle
      → if multiple matches: return first in DFS order (same as snapshot)
  → if still not found: return STALE_REF with existing suggestion
```

**Not applied during batch**: The `--stop-on-error` flag already provides batch failure handling. Auto-recovery in batch could interact with wrong elements. Only single-command execution triggers auto-recovery.

### Post-Action State Hints

After a successful chain execution, the executor reads element state for these action types:

| Action | What to read | Delay |
|--------|-------------|-------|
| Click, Toggle, Check, Uncheck | value + states | 50ms |
| SetValue, Clear | value | 0ms (AX set is synchronous) |
| TypeText | value | 50ms |
| Expand, Collapse | states (expanded/collapsed) | 0ms |
| All others | nothing | - |

If the state read fails (element disappeared), return `ActionResult` with `post_state: None`. The action itself is still reported as success.

### AX Messaging Timeout

Current: `element_for_pid()` in `element.rs` already sets 2.0s timeout on app-level elements.

Change: In `execute_chain()`, before walking steps, call `AXUIElementSetMessagingTimeout(el, 3.0)` on the target element. This is separate from the app-level timeout — it applies to the specific element being acted upon.

### Global Chain Timeout

10 seconds max for entire chain execution. Tracked via `Instant::now()` at chain start. Before each step, check elapsed time. If exceeded, return `AdapterError::timeout("Chain execution exceeded 10s")`.

### Error Suggestions (Complete List)

| Command | On Failure | Suggestion |
|---------|-----------|------------|
| set-value | Chain exhausted | "Try 'clear' then 'type', or check element is a text field." |
| type | Chain exhausted | "Try 'set-value' for direct text insertion." |
| clear | Chain exhausted | "Try 'press cmd+a' then 'press delete'." |
| focus | Chain exhausted | "Try 'click' to focus the element instead." |
| scroll-to | Chain exhausted | "Element may not be in a scrollable container." (existing) |
| press | Key synth failed | "Ensure the target app is active, or use 'press' with --app flag." |
| click | Chain exhausted | "Element may not be interactable. Try 'mouse-click --xy X,Y'." |
| right-click | Chain exhausted | "Try 'mouse-click --button right --xy X,Y'." |
| expand | Chain exhausted | "Try 'click' to open it instead." (existing) |
| collapse | Chain exhausted | "Try 'click' to close it instead." (existing) |
| toggle | Wrong role | "Toggle works on checkboxes, switches, and radio buttons. Use 'click' for other elements." (existing) |
| hover | Element-level call | "Use 'hover @ref' which resolves ref to coordinates automatically." |
| drag | Element-level call | "Use 'drag --from @ref --to @ref' for ref-based drag." |
| key-down/key-up | Element-level call | "Use 'key-down'/'key-up' commands directly." |
| set-value (AX err) | Specific AX error | Include `platform_detail` with AX error code for debugging. |

## System-Wide Impact

### Interaction Graph

```
CLI command → core command handler → adapter.execute_action()
  → dispatch.rs perform_action() → chain executor
    → discovery.rs (one AX IPC batch)
    → chain.rs execute_chain() (walks ChainStep list)
      → ax_helpers.rs (try_ax_action, set_ax_bool, etc.)
      → explicit physical policy path (only when the caller selected it)
    → post-state read (optional)
  → JSON ActionResult response
```

No callbacks, middleware, or observers. Pure function call chain. No new event system.

### Error Propagation

```
AX API error code → with_retry() classifies as transient/terminal
  → transient: retry once, then fall through to next chain step
  → terminal: fall through immediately
  → all steps exhausted: AdapterError with suggestion
    → wrapped in AppError → JSON error envelope
```

### State Lifecycle Risks

- **Clipboard corruption during non-ASCII**: Save/restore is best-effort. Non-text clipboard data (images, files) cannot be preserved across the save/restore cycle. Documented limitation.
- **Element disappears mid-chain**: Each step independently handles AX errors. If element disappears, remaining steps fail fast (kAXErrorInvalidUIElement is terminal). Final error includes "Run 'snapshot' to refresh" suggestion.
- **Stale ref auto-recovery false positive**: Mitigated by 50px bounds proximity check. In worst case, agent acts on wrong element — same risk as manually re-snapshotting and re-resolving.

### API Surface Parity

- `PlatformAdapter` trait: **No changes**. All refactoring is internal to macOS crate.
- `ActionResult` struct: Already has `post_state: Option<ElementState>`. No schema change.
- JSON output format: `post_state` field may now be populated (was always None). Agents that don't read it are unaffected.
- CLI interface: **No changes**. Same commands, same flags, same output format.

## Acceptance Criteria

### Functional Requirements

- [ ] All 50 existing commands produce identical JSON output for the happy path
- [ ] `click` chain matches current `smart_activate` 14-step behavior
- [ ] `right-click` chain matches current `smart_right_activate` 7-step behavior
- [ ] `set-value`, `type`, `clear`, `focus`, `scroll-to` have multi-step fallback chains
- [ ] Non-ASCII text input works via clipboard paste (e.g., `type @e1 "Hola"`)
- [ ] Clipboard operations use NSPasteboard FFI (no subprocess)
- [ ] All error responses include `suggestion` field
- [ ] `post_state` populated for stateful actions
- [ ] Transient AX errors retried once with 100ms delay
- [ ] Chain execution times out at 10 seconds
- [ ] AX messaging timeout set to 3s on target elements

### Non-Functional Requirements

- [ ] No new crate dependencies added
- [ ] All files under 400 LOC
- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] `cargo test --lib --workspace` passes
- [ ] `cargo tree -p agent-desktop-core` shows zero platform crate names
- [ ] Release binary stays under 15MB

### Quality Gates

- [ ] `activate.rs` fully absorbed — file deleted
- [ ] No duplicated AX patterns across files (attribute setting, action enumeration, tree traversal)
- [ ] Every `ChainStep::Custom` has a descriptive label for tracing
- [ ] Every error path has a suggestion

## Implementation Approach: Two Stages

### Why Two Stages

The existing click chain (14-step `smart_activate`) and scroll chain (8-step `ax_scroll`) were built through **manual trial and error** — testing AX APIs directly against real macOS apps (TextEdit, Finder, System Settings, Safari, Xcode) to discover what works, then coding the proven sequences. The same approach is needed for the new chains.

We cannot guess what AX actions or attributes work for `set-value`, `type`, `clear`, `focus`, or `scroll-to` across different app frameworks (AppKit, SwiftUI, Electron, Chromium). We must test first.

**Stage A (Code Directly):** Build the chain executor infrastructure + migrate existing proven chains. This is pure refactoring of known-working code.

**Stage B (Explore Then Code):** For each command that needs a NEW chain, manually probe real apps using the built binary, record what works, then write the chain definition based on observations.

### Stage B Exploration Protocol

For each command needing a new chain (set-value, type, clear, focus, scroll-to):

1. **Build and run the binary** with the chain executor from Stage A
2. **Open target apps**: TextEdit, System Settings, Safari, Finder, Notes, Calculator
3. **Snapshot and identify elements** of the relevant type:
   ```bash
   agent-desktop snapshot --app "TextEdit" -i
   # Find text fields, buttons, checkboxes, sliders, etc.
   ```
4. **Test AX actions directly** against each element using the existing commands:
   ```bash
   # For set-value: does direct AXValue set work on this element?
   agent-desktop set-value @e3 "test"
   # For clear: does AXValue="" work? Does Cmd+A then Delete work?
   agent-desktop clear @e3
   # For focus: does AXFocused=true work? Does AXPress set focus?
   agent-desktop focus @e3
   ```
5. **Record results** in a discovery log per command:
   - Which elements/roles support the primary AX approach?
   - Which need focus first?
   - Which need an alternative approach entirely?
   - Which apps behave differently?
6. **Write the chain definition** based on the actual observed behavior
7. **Test the chain** against all target apps to verify it works end-to-end

This ensures chains are built on empirical evidence, not assumptions.

## Implementation Phases

### Stage A: Infrastructure + Known Chain Migration (Code Directly)

These phases involve pure refactoring of existing, proven code. No AX exploration needed.

---

### Phase 1: Foundation (`ax_helpers.rs` + `discovery.rs`)

Extract shared utilities from duplicated code across activate.rs, dispatch.rs, extras.rs.

**Files created:**
- `crates/macos/src/actions/ax_helpers.rs` (~200 LOC)
- `crates/macos/src/actions/discovery.rs` (~120 LOC)

**ax_helpers.rs contents:**
```rust
pub fn try_ax_action(el: &AXElement, name: &str) -> bool
pub fn set_ax_bool(el: &AXElement, attr: &str, value: bool) -> bool
pub fn set_ax_string(el: &AXElement, attr: &str, value: &str) -> bool
pub fn try_each_child(el: &AXElement, f: impl Fn(&AXElement) -> bool, limit: usize) -> bool
pub fn try_each_ancestor(el: &AXElement, f: impl Fn(&AXElement) -> bool, limit: usize) -> bool
pub fn list_ax_actions(el: &AXElement) -> Vec<String>
pub fn ensure_visible(el: &AXElement)
pub fn set_messaging_timeout(el: &AXElement, seconds: f32)
pub fn with_retry<F>(f: F, max_retries: u32, delay_ms: u64) -> Result<bool, i32>
```

**discovery.rs contents:**
```rust
pub struct ElementCaps {
    pub actions: Vec<String>,
    pub settable_value: bool,
    pub settable_focus: bool,
    pub settable_selected: bool,
    pub settable_disclosing: bool,
    pub role: Option<String>,
    pub has_children: bool,
    pub pid: Option<i32>,
}

pub fn discover(el: &AXElement) -> ElementCaps
```

**Tasks:**
- [ ] Create `ax_helpers.rs` with all shared functions
- [ ] Create `discovery.rs` with `ElementCaps` and `discover()`
- [ ] Register both modules in `actions/mod.rs`
- [ ] Update `extras.rs` to use `ax_helpers` functions (remove duplicates)
- [ ] Verify `cargo test --lib --workspace && cargo clippy`

### Phase 2: Chain Executor (`chain.rs`)

Build the executor that walks a `ChainDef` against an element.

**Files created:**
- `crates/macos/src/actions/chain.rs` (~250 LOC)

**Contents:**
- `ChainStep` enum (Action, SetAttr, WithFocus, OnChildren, OnAncestors, Custom, CGEvent)
- `AttrValue` enum (Bool, StaticStr, Dynamic)
- `CGFallback` enum (Click, None)
- `ChainDef` struct (steps, suggestion)
- `ChainContext` struct (dynamic_value: Option<String>, pre_state: Option<String>)
- `execute_chain(el: &AXElement, def: &ChainDef, ctx: &ChainContext) -> Result<(), AdapterError>`

**Executor logic:**
1. `ensure_visible(el)` — best-effort AXScrollToVisible
2. `set_messaging_timeout(el, 3.0)` — prevent hangs
3. `discover(el)` — one-time capability query
4. Walk steps: for each step, try with retry wrapper
5. For stateful actions: read pre-state before first step, check before retry
6. Track elapsed time — abort at 10 seconds
7. All steps failed → `AdapterError::new(ActionFailed, "...").with_suggestion(def.suggestion)`
8. `tracing::debug!` for each step attempt

**Tasks:**
- [ ] Create `chain.rs` with all types and executor
- [ ] Add unit test: executor with mock steps (Custom steps that return true/false)
- [ ] Add unit test: timeout behavior
- [ ] Add unit test: retry on transient error
- [ ] Register in `actions/mod.rs`

### Phase 3: Migrate Known Chains (`chain_defs.rs`)

Migrate ONLY the existing, proven chains into the new system. New chains come later in Stage B.

**Files created:**
- `crates/macos/src/actions/chain_defs.rs` (~200 LOC initially, grows in Stage B)

**Files modified:**
- `crates/macos/src/actions/dispatch.rs` — simplified to delegate to chains
- `crates/macos/src/actions/activate.rs` — **deleted**

**Migrated chains (proven, existing logic):**

| Chain | Steps | Origin |
|-------|-------|--------|
| `click_chain` | 14 steps (migrated from `smart_activate`) | activate.rs |
| `double_click_chain` | AXOpen → 2x click → CGEvent double | activate.rs |
| `right_click_chain` | 7 steps (migrated from `smart_right_activate`) | activate.rs |
| `triple_click_chain` | 3x click → CGEvent triple | activate.rs |
| `expand_chain` | Action(AXExpand) → SetAttr(AXDisclosing=true) | dispatch.rs inline |
| `collapse_chain` | Action(AXCollapse) → SetAttr(AXDisclosing=false) | dispatch.rs inline |

**NOT migrated yet** (need AX exploration in Stage B): set-value, focus, clear, scroll-to, type.
These stay as their current single-shot implementation until Stage B discovers proper chains.

**Custom step functions** (migrated from activate.rs):
- `try_show_alternate_ui(el, caps)` — from activate.rs
- `try_keyboard_activate(el, caps)` — from activate.rs
- `try_select_via_parent(el, caps)` — from activate.rs
- `try_select_then_show_menu(el, caps)` — from activate.rs

**dispatch.rs after Phase 3 (~250 LOC):**
```rust
pub fn perform_action(el: &AXElement, action: &Action) -> Result<ActionResult, AdapterError> {
    let label = action_label(action);
    match action {
        // Migrated to chain executor
        Action::Click => execute_chain(el, &CLICK_CHAIN, &ChainContext::default())?,
        Action::DoubleClick => execute_chain(el, &DOUBLE_CLICK_CHAIN, &ChainContext::default())?,
        Action::RightClick => execute_chain(el, &RIGHT_CLICK_CHAIN, &ChainContext::default())?,
        Action::TripleClick => execute_chain(el, &TRIPLE_CLICK_CHAIN, &ChainContext::default())?,
        Action::Expand => execute_chain(el, &EXPAND_CHAIN, &ChainContext::default())?,
        Action::Collapse => execute_chain(el, &COLLAPSE_CHAIN, &ChainContext::default())?,

        // Kept as-is until Stage B exploration
        Action::SetValue(val) => ax_set_value(el, val)?,
        Action::TypeText(text) => { /* current impl */ },
        Action::Clear => ax_set_value(el, "")?,
        Action::SetFocus => { /* current impl */ },
        Action::ScrollTo => { /* current impl */ },

        // Stays custom (not chain-based)
        Action::Toggle => { /* role check + click_chain */ },
        Action::Check => { /* state check + click_chain */ },
        Action::Uncheck => { /* state check + click_chain */ },
        Action::Select(value) => extras::select_value(el, value)?,
        Action::Scroll(d, a) => extras::ax_scroll(el, d, *a)?,
        Action::PressKey(combo) => crate::input::keyboard::synthesize_key(combo)?,

        // Adapter-level (coordinate-based)
        Action::Hover | Action::Drag(_) | Action::KeyDown(_) | Action::KeyUp(_) => {
            return Err(adapter_level_error(action, &label));
        }
        _ => return Err(AdapterError::not_supported(&label)),
    }
    Ok(ActionResult::new(label))
}
```

**Tasks:**
- [ ] Create `chain_defs.rs` with migrated chain definitions and custom step functions
- [ ] Migrate `smart_activate` → `click_chain` (14 steps)
- [ ] Migrate `smart_right_activate` → `right_click_chain` (7 steps)
- [ ] Migrate expand/collapse inline logic → `expand_chain`/`collapse_chain`
- [ ] Simplify `dispatch.rs` to delegate migrated chains, keep others as-is
- [ ] Delete `activate.rs`
- [ ] Verify all existing behavior preserved: `cargo test --lib --workspace`
- [ ] **Manual verification**: test click, right-click, double-click on TextEdit, Finder, System Settings
- [ ] **Manual verification**: test expand/collapse on Finder sidebar, System Settings disclosure

### Phase 4: NSPasteboard FFI

Replace `pbcopy`/`pbpaste` subprocess calls with direct Cocoa FFI. Done early because it's a prerequisite for the non-ASCII type chain in Stage B.

**Files modified:**
- `crates/macos/src/input/clipboard.rs` — rewrite implementation (~80 LOC)

**Implementation approach:**
Use `objc_msgSend` via the `core-foundation-sys` crate (already a dependency). No new crate needed.

```rust
extern "C" {
    fn objc_msgSend(receiver: *const Object, sel: Sel, ...) -> *const Object;
    fn objc_getClass(name: *const c_char) -> *const Object;
    fn sel_registerName(name: *const c_char) -> Sel;
}

pub fn get() -> Result<String, AdapterError> {
    let pasteboard_class = objc_getClass("NSPasteboard");
    let pasteboard = objc_msgSend(pasteboard_class, sel_registerName("generalPasteboard"));
    let nsstring = objc_msgSend(pasteboard, sel_registerName("stringForType:"), ns_pasteboard_type_string());
    // Convert NSString → Rust String via CFString bridge
}

pub fn set(text: &str) -> Result<(), AdapterError> {
    let pasteboard = /* generalPasteboard */;
    objc_msgSend(pasteboard, sel_registerName("clearContents"));
    let ns_string = CFString::new(text);
    objc_msgSend(pasteboard, sel_registerName("setString:forType:"), ns_string, ns_pasteboard_type_string());
}
```

**Tasks:**
- [ ] Rewrite `clipboard.rs` using objc_msgSend FFI
- [ ] Keep same public API: `get()`, `set()`, `clear()`
- [ ] Test clipboard roundtrip: set → get → verify
- [ ] Test with non-ASCII content
- [ ] Test with empty string
- [ ] Benchmark: verify < 1ms per operation (vs ~50ms with subprocess)

### Phase 5: Error Suggestions

Add suggestions to every command that's missing them. Can be done independently of Stage B.

**Files modified:**
- `crates/macos/src/actions/dispatch.rs` — add suggestions to match arms for hover/drag/key-down/key-up and current single-shot commands
- `crates/macos/src/actions/chain_defs.rs` — each chain's `suggestion` field (already there for migrated chains)
- `crates/macos/src/input/keyboard.rs` — suggestion on unknown key error

All suggestions listed in the "Error Suggestions (Complete List)" table above.

**Tasks:**
- [ ] Add suggestions to hover/drag/key-down/key-up element-level errors in dispatch.rs
- [ ] Add suggestions to set-value, focus, clear, type, press, scroll-to errors in dispatch.rs
- [ ] Verify all chain definitions have suggestion fields
- [ ] Add suggestion to keyboard.rs unknown key error: "Valid keys: a-z, 0-9, return, escape, tab, space, delete, arrow keys, f1-f12."
- [ ] Add `platform_detail` with AX error codes to all AX-originated errors
- [ ] Audit: grep for `AdapterError::new` without `.with_suggestion` — all should have one

### Phase 6: Resilience (Timeout + Retry + Stale Recovery)

These are integrated into the chain executor and ax_helpers built in Phases 1-2.

**AX Messaging Timeout:**
- In `chain.rs` `execute_chain()`: call `set_messaging_timeout(el, 3.0)` before step loop
- Already in `ax_helpers.rs` from Phase 1

**Retry Logic:**
- Already in `ax_helpers.rs` `with_retry()` from Phase 1
- In `chain.rs`: wrap each step execution with retry

**Stale Ref Auto-Recovery:**
- In `crates/macos/src/tree/resolve.rs`: add relaxed resolution fallback
- Normal resolve fails → try again without bounds_hash → check bounds proximity (50px)
- Only for single-command execution (add flag to distinguish from batch)

**Global Chain Timeout:**
- In `chain.rs`: `Instant::now()` at start, check before each step

**Tasks:**
- [ ] Verify `set_messaging_timeout()` is called in execute_chain (should be from Phase 2)
- [ ] Verify `with_retry()` is integrated into step execution (should be from Phase 2)
- [ ] Add global 10s timeout to chain executor if not already present
- [ ] Add relaxed resolution to resolve.rs
- [ ] Add 50px bounds proximity check
- [ ] Skip auto-recovery when called from batch dispatch
- [ ] Test: verify retry does not double-toggle a checkbox

**Stage A complete.** At this point the binary is functional with the chain executor, migrated click/right-click/expand/collapse chains, NSPasteboard FFI, error suggestions, and resilience. The remaining single-shot commands still work as before.

---

### Stage B: AX Exploration + New Chain Definitions (Explore Then Code)

These phases require **manual AX testing against real apps** to discover proper fallback sequences. Each phase follows the exploration protocol defined above.

---

### Phase 7: Explore + Define set-value Chain

**Apps to test:** TextEdit (text field), System Settings (text inputs, sliders), Safari (URL bar), Notes (text area), Calculator (display)

**Questions to answer through exploration:**
- Does `AXUIElementSetAttributeValue(kAXValueAttribute, value)` work on all text field roles?
- Which elements need `AXFocused=true` set before `AXSetValue` succeeds?
- Do sliders support `AXSetValue`? What format (string number)?
- Do combo boxes support `AXSetValue` directly?
- What happens when `AXSetValue` is called on a read-only element?

**Expected chain shape** (hypothesis — to be validated):
```
SetAttr(AXValue, value) → WithFocus(SetAttr(AXValue, value)) → error
```

**Tasks:**
- [ ] Run exploration protocol on 5+ apps for set-value
- [ ] Record discovery log: which roles/apps work with which approach
- [ ] Write `set_value_chain` definition in chain_defs.rs based on findings
- [ ] Wire into dispatch.rs
- [ ] Test chain against all explored apps

### Phase 8: Explore + Define clear Chain

**Apps to test:** TextEdit, System Settings, Safari URL bar, Notes, any search fields

**Questions to answer:**
- Does `AXSetValue("")` clear all text field types?
- For elements where `AXSetValue` doesn't work, does focus + Cmd+A + Delete work?
- Are there elements where neither approach works?
- Does app-targeted keyboard (Cmd+A to app PID) work for select-all, or must it be system-wide?

**Expected chain shape** (hypothesis):
```
SetAttr(AXValue, "") → WithFocus(SetAttr(AXValue, "")) → Custom(select_all_then_delete) → error
```

**Tasks:**
- [ ] Run exploration protocol on 5+ apps for clear
- [ ] Record discovery log
- [ ] Write `clear_chain` definition based on findings
- [ ] Implement `select_all_then_delete` custom step if needed
- [ ] Wire into dispatch.rs and test

### Phase 9: Explore + Define focus Chain

**Apps to test:** TextEdit, Finder (sidebar items, file list), Safari (tabs, URL bar), System Settings

**Questions to answer:**
- Does `AXUIElementSetAttributeValue(kAXFocusedAttribute, true)` work universally?
- Which elements don't support `AXFocused` but can be focused via `AXPress`?
- Does `AXSelected=true` function as a focus mechanism for list/table items?
- Are there elements that cannot be focused at all?

**Expected chain shape** (hypothesis):
```
SetAttr(AXFocused, true) → Action(AXPress) → SetAttr(AXSelected, true) → error
```

**Tasks:**
- [ ] Run exploration protocol on 5+ apps for focus
- [ ] Record discovery log
- [ ] Write `focus_chain` definition based on findings
- [ ] Wire into dispatch.rs and test

### Phase 10: Explore + Define scroll-to Chain

**Apps to test:** Finder (long file lists), Safari (long pages), System Settings (scrollable panels), Xcode (code editor)

**Questions to answer:**
- Does `AXScrollToVisible` work reliably across apps?
- When it fails, can we walk parents to find a scroll area and verify visibility without sending physical wheel input?
- How do we verify the element is actually visible after scrolling?
- Does `AXScrollToVisible` work on elements inside nested scroll views?

**Expected chain shape** (hypothesis):
```
Action(AXScrollToVisible) → Custom(visible_in_scroll_context) → error
```

**Tasks:**
- [ ] Run exploration protocol on 5+ apps for scroll-to
- [ ] Record discovery log
- [ ] Write `scroll_to_chain` definition based on findings
- [ ] Implement a visibility-check custom step with no cursor or wheel input
- [ ] Wire into dispatch.rs and test

### Phase 11: Explore + Define type Chain

This is the most complex chain — it mixes AX attribute setting with keyboard synthesis and non-ASCII clipboard paste. Requires NSPasteboard FFI from Phase 4.

**Apps to test:** TextEdit, Safari URL bar, Notes, System Settings search, Spotlight, Terminal

**Questions to answer:**
- Does `AXSetValue(current + new_text)` (append) work on text fields? Or does it replace?
- Does `AXUIElementPostKeyboardEvent` to the app PID (not system-wide) deliver keys to the focused element within that app?
- For non-ASCII: does clipboard paste (Cmd+V) via app-targeted keyboard reliably insert text?
- What is the timing needed between focus + keyboard delivery?
- Do some apps intercept Cmd+V differently (e.g., Terminal)?

**Expected chain shape** (hypothesis):
```
Custom(ax_append_value) → Custom(focus_then_app_keyboard) → Custom(focus_then_system_keyboard_with_non_ascii) → error
```

**Tasks:**
- [ ] Run exploration protocol on 5+ apps for type
- [ ] Test ASCII typing via app-targeted keyboard events
- [ ] Test non-ASCII typing via clipboard paste strategy
- [ ] Test mixed ASCII + non-ASCII text
- [ ] Record discovery log
- [ ] Implement `execute_type()` in dispatch.rs based on findings
- [ ] Implement `try_ax_append_value()`, `type_via_app_keyboard()`, `type_with_non_ascii_support()`
- [ ] Test against all explored apps

### Phase 12: Post-Action State Hints

Can be done after Stage B chains are finalized — it's an enrichment layer on top of the chain executor.

**Files modified:**
- `crates/macos/src/actions/dispatch.rs` — add `read_post_state()` after chain execution

**Implementation:**
```rust
fn read_post_state(el: &AXElement, action: &Action) -> Option<ElementState> {
    let delay = match action {
        Action::Click | Action::Toggle | Action::Check
        | Action::Uncheck | Action::TypeText(_) => 50,
        Action::SetValue(_) | Action::Clear
        | Action::Expand | Action::Collapse => 0,
        _ => return None,
    };
    if delay > 0 {
        std::thread::sleep(std::time::Duration::from_millis(delay));
    }
    let value = copy_string_attr(el, "AXValue");
    let states = read_element_states(el);
    Some(ElementState { role: None, value, states })
}
```

**Tasks:**
- [ ] Implement `read_post_state()` in dispatch.rs
- [ ] Wire into `perform_action()` return path
- [ ] Test: click a checkbox → verify post_state shows checked
- [ ] Test: set-value on text field → verify post_state shows new value
- [ ] Test: action on disappearing element → verify post_state is None (not an error)

## Dependencies & Prerequisites

- No new crate dependencies
- Requires Rust stable (already pinned via `rust-toolchain.toml`)
- NSPasteboard FFI uses `core-foundation-sys` (already in deps)
- `objc_msgSend` FFI declarations need `extern "C"` blocks — no crate needed

## Risk Analysis & Mitigation

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Chain executor doesn't capture all activate.rs nuances | Medium | High | Migrate click_chain first, test manually against TextEdit/Finder/System Settings before proceeding |
| Explicit physical path regression | Low | High | Existing integration tests catch this; add new test for physical-policy path |
| Non-ASCII clipboard restore fails silently | Medium | Low | Log via tracing::warn; agent can clipboard-get to verify |
| Stale ref auto-recovery picks wrong element | Low | Medium | 50px bounds check; only for single commands |
| NSPasteboard FFI crashes on edge cases | Low | High | Test with empty clipboard, non-text content, locked pasteboard |
| Global 10s timeout too aggressive for slow apps | Medium | Medium | Make timeout configurable via env var AGENT_DESKTOP_CHAIN_TIMEOUT_MS |

## Sources & References

### Origin

- **Brainstorm document:** [docs/brainstorms/2026-02-23-macos-ax-first-robustness-brainstorm.md](docs/brainstorms/2026-02-23-macos-ax-first-robustness-brainstorm.md) — Key decisions: AX-first architecture, centralized chain executor, window focus only for CGEvent, per-segment non-ASCII handling.

### Internal References

- Current action dispatch: `crates/macos/src/actions/dispatch.rs`
- Current activation chains: `crates/macos/src/actions/activate.rs`
- Current scroll chain: `crates/macos/src/actions/extras.rs`
- Error types: `crates/core/src/error.rs`
- ActionResult/ElementState: `crates/core/src/action.rs`
- Current clipboard: `crates/macos/src/input/clipboard.rs`
- Current keyboard: `crates/macos/src/input/keyboard.rs`
- Element resolution: `crates/macos/src/tree/resolve.rs`
- Prior plan (smart click chain): `docs/plans/2026-02-20-feat-smart-ax-click-chain-plan.md`
- Prior plan (fallback chains): `docs/plans/2026-02-21-fix-fallback-chains-edge-cases-plan.md`
