---
title: Pin repr(C) struct sizes at every layer of the FFI boundary
date: 2026-06-10
category: best-practices
module: crates/ffi
problem_type: best_practice
component: ffi
severity: high
applies_when:
  - Adding or changing a public repr(C) struct in crates/ffi
  - A nested repr(C) struct is embedded by value inside another (size change propagates silently)
  - Writing or updating the committed C ABI header (include/agent_desktop.h)
  - Adding a new FFI integration test in crates/ffi/tests/
  - A C, Swift, Python, Go, or Node consumer allocates the struct on the stack or in a fixed buffer
tags:
  - ffi
  - abi
  - repr-c
  - struct-layout
  - static-assert
  - compile-time-pin
  - c-header
  - memory-safety
---

# Pin repr(C) struct sizes at every layer of the FFI boundary

## Context

`AdDragParams` grew from 40 to 48 bytes when `drop_delay_ms: u64` was added. Because `AdDragParams` is embedded **by value** inside `AdAction`, `AdAction` silently grew 88 → 96 bytes with no guard of any kind — the growth was declared nowhere near the `AdAction` definition. A C caller holding the old layout would under-allocate; the Rust side then reads 8 bytes past the caller's buffer, and that stack garbage becomes a live `drop_delay_ms` value. The growth was caught only because a reviewer noticed it manually — it passed CI because there were no pins.

## Guidance

Three synchronized layers guard every `repr(C)` struct that crosses the ABI. Each layer catches drift at a different consumer point.

**Layer 1 — Rust: published const + compile-time assert + extern size fn.**

```rust
// crates/ffi/src/types/action.rs
pub const AD_ACTION_SIZE: usize = 96;

const _: () = assert!(std::mem::size_of::<AdAction>() == AD_ACTION_SIZE);

#[unsafe(no_mangle)]
pub extern "C" fn ad_action_size() -> usize {
    std::mem::size_of::<AdAction>()
}
```

The anonymous const assert fails the Rust build the moment the layout drifts. The extern function lets any binding language query the true size at startup and compare it against its own layout computation.

**Layer 2 — C header: macro + C11-gated `_Static_assert` + layout history.**

```c
/* crates/ffi/include/agent_desktop.h */
#define AD_ACTION_SIZE (sizeof(AdAction))
#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(sizeof(AdAction) == 96, "AdAction ABI size changed");
#endif
```

C11 consumers fail at their own compile time when the header and the pinned literal diverge; pre-C11 consumers verify at runtime by comparing their own layout against `ad_action_size()` (the macro is a sizing shorthand, not a pin — `sizeof` always tracks the current struct). A layout-history comment in the header records past size changes (40→48, the AdAction propagation, renames) so fresh reviewers and upgrading callers stop re-discovering adjudicated breaks.

**Layer 3 — integration test: size, alignment, offset ordering, zeroed-read, const-vs-extern agreement.**

```rust
// crates/ffi/tests/c_abi_layout.rs
assert_eq!(agent_desktop_ffi::types::action::AD_ACTION_SIZE, 96);
assert_eq!(unsafe { common::ad_action_size() }, AD_ACTION_SIZE);
assert_eq!(size_of::<AdAction>(), 96);
assert_eq!(align_of::<AdAction>(), align_of::<usize>());

let offsets = [
    offset_of!(AdAction, kind),
    offset_of!(AdAction, text),
    offset_of!(AdAction, scroll),
    offset_of!(AdAction, key),
    offset_of!(AdAction, drag),
];
assert!(offsets.windows(2).all(|pair| pair[0] < pair[1]));

let copied = unsafe {
    let action = MaybeUninit::<AdAction>::zeroed().assume_init();
    std::ptr::read(&action as *const AdAction)
};
assert_eq!(copied.drag.drop_delay_ms, 0);
```

The zeroed-read assertion doubles as a sentinel check: every field must read as a safe default from zero-initialized memory, because the header tells callers to zero-initialize.

## Why This Matters

Embedded-by-value fields create a *transitive* size dependency: growing the inner struct grows every outer struct that embeds it, with no declaration at the outer definition. Without pins, that propagation is invisible until a caller under-allocates — undefined behavior in the best case, stack garbage promoted to live field values in the worst. The motivating incident proved this is not theoretical: the field addition passed CI cleanly.

Three layers because each guards a different party: the Rust assert guards this repo's own builds, the `_Static_assert` guards C consumers compiling against the committed header, and the integration test guards the cross-language agreement (const, extern fn, and real layout all matching).

## When to Apply

- Every `#[repr(C)]` struct passed by pointer or embedded by value across the FFI boundary
- Double-apply to the **outer** struct whenever a pinned struct is embedded by value in another
- The extern size fn is mandatory when consumers include runtime-layout languages (Python ctypes, Go cgo, Swift unsafe pointers)

## Examples

Adding a field to a pinned struct forces this sequence, and any step done wrong fails loudly:

1. Add the field to the Rust struct → the `const _` assert fails
2. Update the Rust const to the new size → build green
3. Update the header `_Static_assert` literal and the layout-history comment
4. Update the integration test size assertion and extend the zeroed-read check to the new field
5. The header-compile test and `c_abi_layout` test confirm both sides agree

The process is self-documenting: the failing assert names the struct and the expectation at every step.

## Related

- `best-practices/keep-ffi-action-policy-aligned-with-cli-2026-05-12.md` — the behavioral-parity companion; this doc is the structural-parity half of the same FFI review discipline
- `best-practices/playwright-grade-desktop-reliability-2026-06-02.md` — "FFI and CLI divergence makes language bindings less reliable" is the motivation the pins serve
- `best-practices/deterministic-build-artifact-marker-2026-04-16.md` — records that the header is a hand-committed ABI contract, the discipline these pins extend
