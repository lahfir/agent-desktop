# Build and link

## Building the cdylib

```sh
cargo build --profile release-ffi -p agent-desktop-ffi
```

Output:

- macOS: `target/release-ffi/libagent_desktop_ffi.dylib` (~470 KB)
- Linux: `target/release-ffi/libagent_desktop_ffi.so`
- Windows: `target/release-ffi/agent_desktop_ffi.dll`

The generated header is at `crates/ffi/include/agent_desktop.h`.
CI validates that the committed header matches what `cargo build`
regenerates — if you change a type in `crates/ffi/src/`, rebuild
locally and commit the header.

## Minimal C example

```c
#include <stdio.h>
#include "agent_desktop.h"

int main(void) {
    AdAdapter *adapter = ad_adapter_create();
    if (!adapter) return 1;

    AdResult rc = ad_check_permissions(adapter);
    if (rc != AD_RESULT_OK) {
        fprintf(stderr, "permission denied: %s\n", ad_last_error_message());
        ad_adapter_destroy(adapter);
        return 1;
    }

    /* Opaque list handle — walk via _count / _get, free with _free. */
    AdAppList *list = NULL;
    rc = ad_list_apps(adapter, &list);
    if (rc == AD_RESULT_OK) {
        uint32_t count = ad_app_list_count(list);
        for (uint32_t i = 0; i < count; i++) {
            const AdAppInfo *app = ad_app_list_get(list, i);
            if (app) {
                printf("%s (pid %d)\n", app->name, app->pid);
            }
        }
        ad_app_list_free(list);
    } else {
        fprintf(stderr, "list_apps failed: %s\n", ad_last_error_message());
    }

    ad_adapter_destroy(adapter);
    return 0;
}
```

Compile:

```sh
clang -I./crates/ffi/include main.c \
      -L./target/release-ffi -lagent_desktop_ffi \
      -o list_apps
install_name_tool -change \
    libagent_desktop_ffi.dylib \
    @executable_path/target/release-ffi/libagent_desktop_ffi.dylib \
    list_apps
```

## Minimal Python ctypes example

```python
import ctypes
from ctypes import c_int32, c_char_p, POINTER, Structure, c_uint32

lib = ctypes.CDLL("./target/release-ffi/libagent_desktop_ffi.dylib")

# Opaque adapter handle
class AdAdapter(ctypes.Structure):
    pass

lib.ad_adapter_create.restype = POINTER(AdAdapter)
lib.ad_adapter_destroy.argtypes = [POINTER(AdAdapter)]
lib.ad_check_permissions.restype = c_int32
lib.ad_check_permissions.argtypes = [POINTER(AdAdapter)]
lib.ad_last_error_message.restype = c_char_p

adapter = lib.ad_adapter_create()
rc = lib.ad_check_permissions(adapter)
if rc != 0:
    msg = lib.ad_last_error_message()
    print("permission denied:", msg.decode() if msg else "(no message)")
lib.ad_adapter_destroy(adapter)
```

## Call graph reminder

All FFI calls above must run on the **main thread** on macOS. For
Python that typically means the script's entry point, not a worker
spawned via `threading`. See [threading.md](threading.md).
