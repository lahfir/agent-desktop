# FFI Python Smoke Harness

Proves the C ABI works from a non-Rust host (Python ctypes) and gates it in CI.

## What it tests

| Leg | Check |
|-----|-------|
| 1 | `ad_abi_version()` equals `AD_ABI_VERSION_MAJOR` from the header (catches header/dylib mismatch at import) |
| 2 | Every `ad_*_size()` getter equals the corresponding `AD_*_SIZE` macro (catches struct layout drift) |
| 3 | `ad_version()` returns `ok:true` JSON with `data.version` (basic command pipeline) |
| 4 | `ad_adapter_create` → `ad_snapshot` → `ad_free_string` → `ad_adapter_destroy` without crash or leak; stub adapter yields a clean `PLATFORM_NOT_SUPPORTED` envelope |

## Building the dylib

```bash
# From the repo root:
cargo build --locked --profile release-ffi -p agent-desktop-ffi --features stub-adapter
```

The dylib lands at `target/release-ffi/libagent_desktop_ffi.dylib` (macOS) or
`target/release-ffi/libagent_desktop_ffi.so` (Linux).

`--profile release-ffi` keeps `panic = "unwind"`, which is required for the
`catch_unwind` traps inside the FFI layer. Using the default `release` profile
(which has `panic = "abort"`) would silently defeat those traps.

`--features stub-adapter` replaces the real platform adapter with a no-op that
returns `PLATFORM_NOT_SUPPORTED` for every adapter call. This lets CI run the
harness without requiring macOS Accessibility permissions.

## Running locally

```bash
# macOS (arm64 or x86_64):
python3 tests/ffi-python/smoke.py \
  target/release-ffi/libagent_desktop_ffi.dylib \
  crates/ffi/include/agent_desktop.h
```

Alternatively, use environment variables:

```bash
export AD_DYLIB_PATH=target/release-ffi/libagent_desktop_ffi.dylib
export AD_HEADER_PATH=crates/ffi/include/agent_desktop.h
python3 tests/ffi-python/smoke.py
```

## Stub enforcement (`AD_EXPECT_STUB=1`)

Set `AD_EXPECT_STUB=1` when running the harness against a stub-adapter build.
With this variable set, the harness fails if `ad_snapshot()` returns `ok:true`,
which would indicate the dylib was **not** built with `--features stub-adapter`.
CI always sets this variable so a real-adapter dylib cannot accidentally pass the
stub-only gate.

## Dependencies

`smoke.py` uses only the Python standard library (`ctypes`, `json`, `re`,
`pathlib`, `sys`). No `pip install` required.

## Real-adapter happy path

The smoke harness gates the AX-independent surface (ABI version, struct sizes,
`ad_version`) and the `PLATFORM_NOT_SUPPORTED` passthrough path. The real-adapter
happy path (a successful `ad_snapshot` with `ok:true`) is covered by the E2E
harness (`tests/e2e/run.sh`), which requires AX permission and a release build
without the stub feature.
