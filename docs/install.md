# Installing agent-desktop

Install the `agent-desktop` npm package with npm, pnpm, Yarn or Bun. It bundles native executables and platform
helpers for macOS, Windows and Linux, on x64 and ARM64:

```bash
npm install -g agent-desktop
agent-desktop version
```

For a direct native executable without a JavaScript launcher, download the archive for your OS and architecture
from [GitHub Releases](https://github.com/vercel-labs/agent-desktop/releases). Extract the executable and its platform
helper together into a directory on PATH. Upgrade by replacing both with files from the same release.

## Homebrew and Scoop

Official packages are being submitted to `Homebrew/homebrew-core` and `ScoopInstaller/Main`. They are not yet
available through the default catalogs. This repository no longer maintains a custom tap or bucket; existing
custom installations remain pinned to their installed version.

Once accepted, Homebrew builds from the tagged source and manages its own bottles. Scoop installs the Windows
release archive and checks GitHub releases for updates. Catalog updates and approvals are managed by those
projects, separately from our npm and GitHub release workflow.

If an older installation shadows the command, uninstall it through its original package manager. Check resolution
with `command -v agent-desktop` on macOS/Linux or `Get-Command agent-desktop` on Windows.

## Blocked install scripts

The npm package includes the native binaries and helpers. Its `postinstall` script replaces the Node launcher with a
link to the native binary (about 21 ms per call). If the package manager blocks install scripts (npm 12 does by
default), the Node launcher runs instead and each call costs about 146 ms. Approve the script with
`npm install-scripts approve agent-desktop`, or install with `--allow-scripts`. Some package managers retain their
own launcher.

A plain `npm install agent-desktop` without `-g` puts the command in `./node_modules/.bin`, not on PATH; run it with
`npx agent-desktop` or install globally.

## Release archives

Release archives are on the [releases page](https://github.com/vercel-labs/agent-desktop/releases). Their checksums come
from the same release, so they catch corruption but do not prove origin. Check origin with:

```bash
gh attestation verify <downloaded-archive> --repo vercel-labs/agent-desktop
```

## From source

You need Rust 1.89 or later, plus the Xcode command-line tools on macOS 13+, the MSVC build tools on Windows, or a C
linker on Linux:

```bash
git clone https://github.com/vercel-labs/agent-desktop && cd agent-desktop
cargo build --release -p agent-desktop
```

On Linux, also run `cargo build --release -p agent-desktop-linux --bins` and keep
`agent-desktop-linux-clipboard-owner` beside the CLI. npm and the release archives include it.

## Permissions

**macOS** needs Accessibility permission. Screenshots also need Screen Recording. On macOS 27, opening Notification
Center clicks the menu bar clock; earlier versions open it through System Events, which needs Automation permission.
`agent-desktop permissions` reports the state without prompting,
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
