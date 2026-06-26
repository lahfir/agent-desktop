# Build and link

## Building the cdylib

```sh
cargo build --profile release-ffi -p agent-desktop-ffi
```

Output:

- macOS: `target/release-ffi/libagent_desktop_ffi.dylib`
- Linux: `target/release-ffi/libagent_desktop_ffi.so`
- Windows: `target/release-ffi/agent_desktop_ffi.dll`

The generated header is at `crates/ffi/include/agent_desktop.h`. CI
validates that the committed header matches what `cargo build`
regenerates — if you change a type in `crates/ffi/src/`, rebuild
locally and commit the updated header.

`--profile release-ffi` keeps `panic = "unwind"`, which is required for
the `catch_unwind` traps inside every `extern "C"` entrypoint. The default
`release` profile uses `panic = "abort"` (for CLI binary-size reasons) and
silently defeats those traps.

## Prebuilt archives

Every GitHub release ships prebuilt archives for:

- macOS arm64 and x86_64
- Linux x64 and arm64 (glibc)
- Windows x64 MSVC

Each archive contains the dylib/so/dll, `include/agent_desktop.h`, and
`LICENSE`. Integrity: compare against `checksums.txt` in the release
assets. Supply-chain verification: each release is signed via Sigstore
attestation — verify with `cosign verify-blob` before deploying.

## ABI handshake (do this first)

After `dlopen` / `LoadLibrary`, compare the dylib major to the header you
compiled against before calling anything else:

```c
AdResult rc = ad_init(AD_ABI_VERSION_MAJOR);
if (rc != AD_RESULT_OK) {
    // header and dylib have incompatible major versions
    fprintf(stderr, "ABI mismatch: %s\n", ad_last_error_message());
    return -1;
}
```

Alternatively, read the raw dylib major and compare yourself:

```c
uint32_t dylib_major = ad_abi_version();
if (dylib_major != AD_ABI_VERSION_MAJOR) {
    fprintf(stderr, "ABI major: header=%u dylib=%u\n",
            AD_ABI_VERSION_MAJOR, dylib_major);
    abort();
}
```

`ad_init` returns `AD_RESULT_ERR_INVALID_ARGS` on mismatch (with a
diagnostic in `ad_last_error_message`). A mismatch means the header you
compiled against and the loaded dylib are incompatible — do not call
anything further.

## Struct size validation

Languages whose struct layout may diverge from C (Python ctypes, Go cgo,
JNI, etc.) must validate every size-pinned struct before passing it to
the library. The FFI exposes three validation layers:

1. **Header macros**: `AD_ACTION_SIZE`, `AD_WAIT_ARGS_SIZE`,
   `AD_REF_ENTRY_SIZE`, `AD_DRAG_PARAMS_SIZE`, `AD_ACTION_RESULT_SIZE`,
   `AD_ACTION_STEP_SIZE`, `AD_ELEMENT_STATE_SIZE`.
2. **Runtime getters**: `ad_action_size()`, `ad_wait_args_size()`,
   `ad_ref_entry_size()`, `ad_drag_params_size()`, `ad_action_result_size()`,
   `ad_action_step_size()`, `ad_element_state_size()` — each returns the
   same value the macro encodes, compiled from the Rust side.
3. **C11 static asserts** in the header (`#ifndef AGENT_DESKTOP_ABI_ASSERTS`)
   catch mismatches at C compile time.

Compare your binding's `sizeof` equivalent against the getter at load
time, before building or passing any of these structs. The Python smoke
harness (`tests/ffi-python/smoke.py` Leg 2) demonstrates this check for
all size-pinned structs in a single loop.

## Worked example: Python ctypes smoke harness

`tests/ffi-python/smoke.py` is the canonical reference for Python
consumers. It covers:

- Leg 1: `ad_abi_version()` vs `AD_ABI_VERSION_MAJOR` (header/dylib match)
- Leg 2: every `ad_*_size()` getter vs its `AD_*_SIZE` macro (struct layout)
- Leg 3: `ad_version()` → parse JSON → `ad_free_string` (basic pipeline)
- Leg 4: `ad_adapter_create` → `ad_snapshot` → `ad_free_string` →
  `ad_adapter_destroy` (full adapter lifecycle, stub passes through
  `PLATFORM_NOT_SUPPORTED`)

Run it:

```bash
python3 tests/ffi-python/smoke.py \
  target/release-ffi/libagent_desktop_ffi.dylib \
  crates/ffi/include/agent_desktop.h
```

Or with environment variables:

```bash
AD_DYLIB_PATH=target/release-ffi/libagent_desktop_ffi.dylib \
AD_HEADER_PATH=crates/ffi/include/agent_desktop.h \
python3 tests/ffi-python/smoke.py
```

No `pip install` required — `smoke.py` uses the Python standard library only.

## Minimal C example

```c
#include <stdio.h>
#include "agent_desktop.h"

int main(void) {
    // 1. ABI handshake
    if (ad_init(AD_ABI_VERSION_MAJOR) != AD_RESULT_OK) {
        fprintf(stderr, "ABI mismatch: %s\n", ad_last_error_message());
        return 1;
    }

    // 2. Create adapter
    AdAdapter *adapter = ad_adapter_create();
    if (!adapter) {
        fprintf(stderr, "adapter_create failed: %s\n", ad_last_error_message());
        return 1;
    }

    // 3. Check permissions
    AdResult rc = ad_check_permissions(adapter);
    if (rc != AD_RESULT_OK) {
        fprintf(stderr, "permission denied: %s\n", ad_last_error_message());
        ad_adapter_destroy(adapter);
        return 1;
    }

    // 4. Snapshot the focused window
    char *json_out = NULL;
    rc = ad_snapshot(adapter,
                     NULL,   // app (null = focused window)
                     0,      // surface = Window
                     10,     // max_depth
                     false,  // interactive_only
                     false,  // compact
                     &json_out);
    if (rc == AD_RESULT_OK && json_out) {
        printf("%s\n", json_out);
        // parse JSON to find @e refs in data.tree
        ad_free_string(json_out);
    } else if (json_out) {
        // command-level error — ok:false envelope, still must free
        fprintf(stderr, "snapshot error: %s\n", json_out);
        ad_free_string(json_out);
    } else {
        // infrastructure error — *out is null
        fprintf(stderr, "snapshot failed: %s\n", ad_last_error_message());
    }

    ad_adapter_destroy(adapter);
    return 0;
}
```

Compile:

```sh
clang -I./crates/ffi/include main.c \
      -L./target/release-ffi -lagent_desktop_ffi \
      -o snapshot_demo
install_name_tool -change \
    libagent_desktop_ffi.dylib \
    @executable_path/target/release-ffi/libagent_desktop_ffi.dylib \
    snapshot_demo
```

## Observe-act workflow in C

After parsing the snapshot JSON and extracting a ref ID (e.g. `"@e5"`):

```c
AdAction act = {0};              // zero-init before setting any field
act.kind = AD_ACTION_KIND_CLICK;

char *result = NULL;
AdResult rc = ad_execute_by_ref(
    adapter,
    "@e5",        // ref ID from snapshot data.tree
    NULL,         // snapshot_id — NULL = latest for this session
    &act,
    0,            // policy = Headless
    &result
);
if (result) {
    // parse JSON — ok:true on success, ok:false on STALE_REF etc.
    printf("%s\n", result);
    ad_free_string(result);
}
if (rc != AD_RESULT_OK) {
    const char *det = ad_last_error_details();  // may be NULL; treat as sensitive
    fprintf(stderr, "execute_by_ref failed (%d): %s\n",
            (int)rc, ad_last_error_message());
}
```

To type text, set the action kind and text field:

```c
AdAction type_act = {0};
type_act.kind = AD_ACTION_KIND_TYPE_TEXT;
type_act.text = "hello";
// TypeText defaults to focus_fallback via ad_execute_by_ref; explicit policy:
rc = ad_execute_by_ref(adapter, "@e3", NULL, &type_act,
                       AD_POLICY_KIND_FOCUS_FALLBACK, &result);
```

## Call graph reminder

All adapter-touching FFI calls must run on the **main thread** on macOS.
For Python that typically means the script's entry point, not a worker
spawned via `threading`. See [threading.md](threading.md).

## Minimal Python ctypes example

```python
import ctypes, json
from ctypes import c_int, c_int32, c_uint8, c_bool, c_char_p, POINTER, c_void_p

lib = ctypes.CDLL("./target/release-ffi/libagent_desktop_ffi.dylib")

# ABI handshake
lib.ad_init.restype = c_int32
lib.ad_init.argtypes = [ctypes.c_uint32]
AD_ABI_VERSION_MAJOR = 1  # sync with header macro
rc = lib.ad_init(AD_ABI_VERSION_MAJOR)
assert rc == 0, f"ABI mismatch: rc={rc}"

# Adapter lifecycle
lib.ad_adapter_create.restype = c_void_p
lib.ad_adapter_create.argtypes = []
lib.ad_adapter_destroy.restype = None
lib.ad_adapter_destroy.argtypes = [c_void_p]

# Snapshot
lib.ad_snapshot.restype = c_int32
lib.ad_snapshot.argtypes = [c_void_p, c_char_p, c_int32, c_uint8, c_bool, c_bool,
                             POINTER(c_char_p)]
lib.ad_free_string.restype = None
lib.ad_free_string.argtypes = [c_char_p]

lib.ad_last_error_message.restype = c_char_p
lib.ad_last_error_message.argtypes = []

adapter = lib.ad_adapter_create()
assert adapter, "ad_adapter_create() returned null"

out = c_char_p()
rc = lib.ad_snapshot(adapter, None, 0, 10, False, False, ctypes.byref(out))
if out.value:
    envelope = json.loads(out.value)
    lib.ad_free_string(out)
    print("ok:", envelope.get("ok"))
else:
    msg = lib.ad_last_error_message()
    print("error:", msg.decode() if msg else "(no message)")

lib.ad_adapter_destroy(adapter)
```
