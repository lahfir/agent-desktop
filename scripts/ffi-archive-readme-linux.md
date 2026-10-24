
Linux needs glibc 2.35 or newer (Ubuntu 22.04 and later). `dlopen` / Python
`ctypes` callers load `lib/libagent_desktop_ffi.so` by path. The library has no
SONAME, so a linking consumer records the file name and needs `-L` for the
`lib/` directory and `-rpath` pointing at it at run time.

- Calls on one adapter run one at a time. Many threads can share an adapter,
  but they do not run in parallel.
- `ad_snapshot` takes an application name only. It has no window id or root.
- The clipboard calls carry text only. `ad_set_clipboard` starts the detached
  `lib/agent-desktop-linux-clipboard-owner` process, which keeps serving the
  text after the host exits until another client takes the selection or
  `ad_clear_clipboard` ends it. The library finds that file beside its own
  image, never through `PATH`, so keep the two files in the same directory.
  Wayland needs `wl-clipboard` installed.
- Observation needs a session accessibility bus (`at-spi2-core`).
