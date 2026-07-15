# agent-desktop FAQ

## What is agent-desktop?

agent-desktop is a native desktop automation CLI for AI agents. It lets agents observe and control desktop apps through OS accessibility trees, using structured JSON instead of screenshots, pixel matching, or browser-only automation.

## Does agent-desktop require screenshots or pixel matching?

No. The core workflow reads native accessibility trees and assigns refs to interactive elements. Screenshots are available as a separate command, but agents do not need screenshots or pixel matching to click buttons, type into fields, inspect menus, or navigate app windows.

## How does agent-desktop work?

| Component | Function |
|-----------|----------|
| **Native Rust CLI** | Fast, single binary, no runtime dependencies |
| **C-ABI cdylib** | Load once from Python, Swift, Go, Ruby, Node, or C instead of forking |
| **58 Commands** | Observation, interaction, keyboard, mouse, notifications, clipboard, window management, session lifecycle, trace read/export, and bundled `skills` docs |
| **Snapshot & Refs** | Compact snapshot IDs and deterministic element refs like `@e1`, `@e2` |
| **Structured JSON** | Machine-readable responses with error codes and recovery hints |

## What makes agent-desktop useful for AI agents?

| Feature | Benefit |
|---------|---------|
| **Progressive Skeleton Traversal** | 78–96% token reduction on dense apps |
| **Headless-by-Default Actions** | Ref actions use accessibility APIs and block unintended physical side effects |
| **Snapshot Refs** | Agents act on stable refs within a snapshot instead of guessing coordinates |
| **Recovery Hints** | Errors include machine-readable codes and suggestions for the next agent step |
| **Cross-Language FFI** | Python, Swift, Go, Ruby, Node, C, and C++ hosts can call the native library directly |

## Which platforms are supported?

| Feature | macOS | Windows | Linux |
|---------|:-----:|:-------:|:-----:|
| Accessibility tree | **Yes** | Planned | Planned |
| Click/type/keyboard | **Yes** | Planned | Planned |
| Mouse input | **Yes** | Planned | Planned |
| Screenshot | **Yes** | Planned | Planned |
| Clipboard | **Yes** | Planned | Planned |
| App/window management | **Yes** | Planned | Planned |
| Notifications | **Yes** | Planned | Planned |

## How do I install agent-desktop?

Install the CLI from npm:

```bash
npm install -g agent-desktop
agent-desktop snapshot --app Safari
```

Build the FFI library from source:

```bash
cargo build --profile release-ffi -p agent-desktop-ffi
# Outputs under target/release-ffi/: libagent_desktop_ffi.dylib/.so or agent_desktop_ffi.dll
```

## What is the ref system?

`snapshot` assigns qualified refs to interactive elements in depth-first order, such as `@s8f3k2p9:e1`. The embedded snapshot ID removes the separate `--snapshot` flag. Snapshot lookup remains confined to the selected namespace, so refs created under a session still require that session's `--session` or `AGENT_DESKTOP_SESSION` scope. Legacy bare refs such as `@e1` require an explicit `--snapshot` in the same namespace.

Interactive roles that receive refs:

`button`, `textfield`, `checkbox`, `link`, `menuitem`, `tab`, `slider`, `combobox`, `treeitem`, `cell`, `radiobutton`, `incrementor`, `menubutton`, `switch`, `colorwell`, `dockitem`.

Stale ref recovery:

```text
snapshot -> act -> STALE_REF? -> snapshot again -> retry
```

## Is agent-desktop free and open source?

Yes. agent-desktop is Apache-2.0 licensed for personal and commercial use.

## Where can I get help?

| Resource | Link |
|----------|------|
| **Repository** | [github.com/lahfir/agent-desktop](https://github.com/lahfir/agent-desktop) |
| **ClawHub Skill** | [clawhub.ai/lahfir/agent-desktop](https://clawhub.ai/lahfir/agent-desktop) |
| **skills.sh Listing** | [skills.sh/lahfir/agent-desktop/agent-desktop](https://skills.sh/lahfir/agent-desktop/agent-desktop) |
| **npm Package** | [npmjs.com/package/agent-desktop](https://www.npmjs.com/package/agent-desktop) |
| **CI Status** | [GitHub Actions](https://github.com/lahfir/agent-desktop/actions/workflows/ci.yml?query=branch%3Amain) |
| **Releases** | [GitHub Releases](https://github.com/lahfir/agent-desktop/releases) |
| **Issues** | [GitHub Issues](https://github.com/lahfir/agent-desktop/issues) |
