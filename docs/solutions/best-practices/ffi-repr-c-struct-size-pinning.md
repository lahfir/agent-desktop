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

A struct embedded by value inside another struct makes the outer struct's size
depend on the inner one. Growing the inner struct silently grows the outer one
too. A C caller compiled against the old layout then under-allocates, and Rust
reads past the caller's buffer. Without pins, CI passes such a change.

## Guidance

Pin each `repr(C)` struct that crosses the ABI at four layers. Each layer
catches drift for a different consumer.

1. **Rust.** A `pub const` size, a `const _` assert on `size_of`, and an
   `extern "C"` size getter (`crates/ffi/src/types/action.rs`).
2. **C header.** The size macro is emitted from the Rust const. The
   `_Static_assert` lines come from the `trailer` in `crates/ffi/cbindgen.toml`.
3. **Integration test.** Size, alignment, field offsets, and a zeroed-read check
   in `crates/ffi/tests/c_abi_layout.rs`.
4. **Consumer smoke test.** `tests/ffi-python/smoke.py` loads the dylib through
   ctypes and checks each `ad_*_size()` getter against its header macro.

The trailer is the one layer with no generator behind it. Adding a pinned struct
without its `_Static_assert` lines in the trailer is silent. `cbindgen --verify`
regenerates from the same trailer (`.github/workflows/ci.yml:740-743`), so CI
stays green and the C-side pin does not exist. Add the assert lines to
`crates/ffi/cbindgen.toml` before regenerating the header with
`scripts/update-ffi-header.sh`.

When a pinned struct is embedded by value in another struct, pin the outer
struct the same way. The extern size getter is required when consumers load the
library at runtime, such as Python ctypes, Go cgo, or Swift unsafe pointers.
