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

Four synchronized layers guard every `repr(C)` struct that crosses the ABI. Each layer catches drift at a different consumer point.

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

**Layer 2 — C header: generated macro + C11-gated `_Static_assert` from the cbindgen trailer.**

```c
/* crates/ffi/include/agent_desktop.h */
#define AD_ACTION_SIZE 96
#if defined(__STDC_VERSION__) && __STDC_VERSION__ >= 201112L
_Static_assert(sizeof(AdAction) == AD_ACTION_SIZE, "AdAction ABI size changed");
#endif
```

C11 consumers fail at their own compile time when the generated macro and the
actual C layout diverge. Pre-C11 and runtime-layout consumers compare their own
layout against `ad_action_size()`.

Only the alignment and offset guards are hand-written literals; the **size**
literal is not. `crates/ffi/cbindgen.toml` sets `[const] allow_static_const =
false`, so `AD_ACTION_SIZE` is emitted into the header from the Rust `pub
const` — the size lives in exactly one place, the Rust source, and updating it
there updates the header on the next regeneration.

The `_Static_assert` lines themselves are a different matter: they live in the
hand-maintained `trailer` key of `crates/ffi/cbindgen.toml`, not in any Rust
source. **Adding a pinned struct without adding its trailer asserts is silent** —
`cbindgen --verify` regenerates using that same trailer and compares the result
against the committed header, so a missing assert matches on both sides and CI
stays green while the C-side pin does not exist. The trailer is the one place in
this scheme with no generator behind it.

**Layer 3 — integration test: size, alignment, offset ordering, zeroed-read, const-vs-extern agreement.**

```rust
// crates/ffi/tests/c_abi_layout.rs
assert_eq!(agent_desktop_ffi::AD_ACTION_SIZE, 96);
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

**Layer 4 — consumer-side smoke test: extern getter vs. the shipped header.**

`tests/ffi-python/smoke.py`, run by the `ffi-python-smoke` CI lane, loads the
built dylib through ctypes, parses the numeric `#define`s out of the committed
header, and asserts each `ad_*_size()` getter returns what its `AD_*_SIZE` macro
says. This is the layer that actually exercises the extern getter from Layer 1
the way a runtime-layout binding does — the Rust layers never call it across a
real dylib boundary, and the C `_Static_assert` never runs it at all.

## Why This Matters

Embedded-by-value fields create a *transitive* size dependency: growing the inner struct grows every outer struct that embeds it, with no declaration at the outer definition. Without pins, that propagation is invisible until a caller under-allocates — undefined behavior in the best case, stack garbage promoted to live field values in the worst. The motivating incident proved this is not theoretical: the field addition passed CI cleanly.

Four layers because each guards a different party: the Rust assert guards this repo's own builds, the `_Static_assert` guards C consumers compiling against the committed header, the integration test guards the cross-language agreement (const, extern fn, and real layout all matching), and the Python smoke test guards the runtime-layout consumer that queries the dylib instead of compiling against the header.

## When to Apply

- Every `#[repr(C)]` struct passed by pointer or embedded by value across the FFI boundary
- Double-apply to the **outer** struct whenever a pinned struct is embedded by value in another
- The extern size fn is mandatory when consumers include runtime-layout languages (Python ctypes, Go cgo, Swift unsafe pointers)

## Examples

Adding a field to a pinned struct forces this sequence, and every step but one fails loudly when done wrong:

1. Add the field to the Rust struct → the `const _` assert fails
2. Update the Rust const to the new size → build green
3. Regenerate the committed header (`scripts/update-ffi-header.sh`) so the size macro emitted from the Rust const is updated
4. Update the integration test size assertion and extend the zeroed-read check to the new field
5. Extend `tests/ffi-python/smoke.py`'s getter/macro pair list if the struct is newly pinned
6. The header-compile test, the `c_abi_layout` test, and the `ffi-python-smoke` lane confirm every side agrees

Pinning a **new** struct takes one more step, and it is the step nothing will
remind you about: add its `_Static_assert` lines to the `trailer` in
`crates/ffi/cbindgen.toml` **before** regenerating. Regeneration alone will not
create them, and because `cbindgen --verify` regenerates from that same trailer,
their absence is invisible to CI. Every other step is self-documenting — the
failing assert names the struct and the expectation — so treat the trailer as the
one place to check by hand.

## Related

- [Keep FFI action policy aligned with CLI action policy](keep-ffi-action-policy-aligned-with-cli-2026-05-12.md) — the behavioral-parity companion; this document is the structural-parity half of the same FFI review discipline.
- [Playwright-grade desktop reliability contract](playwright-grade-desktop-reliability-2026-06-02.md) — ABI pins prevent bindings from silently diverging from the reliability contract.
- The committed `crates/ffi/include/agent_desktop.h`, its generated header checks, and the Rust layout tests are the current source of truth; this repository intentionally does not use a Cargo build-artifact marker for header generation.
