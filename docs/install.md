# Installing agent-desktop

The npm package works on macOS, Windows and Linux, on x64 and ARM64:

```bash
npm install -g agent-desktop
agent-desktop status            # platform, permissions, supported surfaces
```

## Blocked install scripts

The package's `postinstall` script downloads the binary from the GitHub release and checks its SHA-256. A package
manager that blocks install scripts leaves no binary, and the first run fails with binary-not-found. npm 11 runs the
script but warns that it is not covered by `allowScripts`. On Linux the script also needs `curl`, `tar` and `gzip`. To
allow it:

```bash
npm install -g --allow-scripts=agent-desktop agent-desktop
npm config set allow-scripts=agent-desktop --location=user
pnpm add -g --allow-build=agent-desktop agent-desktop
bun add -g --trust agent-desktop
```

## Release archives

Release archives are on the [releases page](https://github.com/lahfir/agent-desktop/releases). Their checksums come
from the same release, so they catch corruption but do not prove origin. Check origin with:

```bash
gh attestation verify <downloaded-archive> --repo lahfir/agent-desktop
```

## From source

You need Rust 1.89 or later, plus the Xcode command-line tools on macOS 13+, the MSVC build tools on Windows, or a C
linker on Linux:

```bash
git clone https://github.com/lahfir/agent-desktop && cd agent-desktop
cargo build --release -p agent-desktop
```

On Linux, also run `cargo build --release -p agent-desktop-linux --bins` and keep
`agent-desktop-linux-clipboard-owner` beside the CLI. npm and the release archives include it.

## Permissions

**macOS** needs Accessibility permission. Screenshots also need Screen Recording, and opening Notification Center
needs Automation permission for System Events. `agent-desktop permissions` reports the state without prompting,
`--request` asks for what is missing, and `--guide` opens System Settings at the missing permission with an on-screen
guide.

**Windows** needs no grant for apps at your terminal's integrity level. An app that runs as administrator can be read,
but only an elevated agent can send it input. The binary is unsigned, so SmartScreen may warn on a browser download,
antivirus may quarantine it, and Smart App Control on a clean Windows 11 install blocks it. The npm install path does
not trigger SmartScreen.

**Linux** supports Ubuntu 24.04+ with GNOME 46 as the primary target and a glibc 2.35 floor; Ubuntu 22.04's ATK bridge
aborts on `Action.GetActions`. Run inside the user's graphical session with `at-spi2-core` installed. A TTY or SSH
shell without that session's bus, display and runtime environment cannot see its apps. `agent-desktop status`
activates and checks the accessibility bus. X11 keys need XTEST,
Wayland keys need GNOME Mutter RemoteDesktop, and the Wayland clipboard needs `wl-clipboard`. The Linux skill covers
the rest: `agent-desktop skills get linux`.

## Platform notes

**Windows.** `list-surfaces` lists each process's `window`, `focused`, `sheet` and `menu` surfaces. `snapshot
--surface` also reads `taskbar`, `system-tray`, `system-tray-overflow`, `start-menu` and `action-center` without
`--app`. `quick-settings` returns `PLATFORM_NOT_SUPPORTED` and names `action-center`, which carries those controls.

`launch` on Windows takes an absolute path, or a bare name under System32 or the Windows directory, with or
without `.exe`. It cannot resolve a display name such as "Google Chrome".

**Linux.** Semantic actions work on X11 and Wayland. On Wayland, app pointer input refuses and native window control
is unsupported.
