# Contributing to agent-desktop

agent-desktop is a native Rust CLI and FFI library that gives AI agents structured access to desktop applications through OS accessibility trees. Contributions that sharpen that core mission are welcome.

## Kinds of contributions that fit this project

- **Bug fixes** — wrong JSON output, incorrect ref resolution, `STALE_REF` on a stable target, etc.
- **New commands** — additions to the 60-command surface (56 operational, four held-input names fail closed); see the steps in [CLAUDE.md](CLAUDE.md#architecture)
- **Platform adapters** — fixes and features for the macOS, Windows and Linux adapters, each implementing `PlatformAdapter`
- **App-specific quirks** — documented edge cases for specific apps (Electron, game engines, etc.) under `skills/`
- **Docs and skill files** — keeping `skills/agent-desktop*/` accurate when behaviour changes

Out of scope: embedding LLMs, GUI/TUI interfaces, browser automation, macro recording.

## Code of Conduct

This project follows the guidelines in [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). All contributors are expected to uphold them.

## Prerequisites

| Requirement | Notes |
|---|---|
| **Rust toolchain** | Pinned to 1.89.0 via `rust-toolchain.toml` (`rust-version` 1.89 is the MSRV). `rustup` picks the correct channel automatically. |
| **macOS 13+** | Required to build and run the macOS adapter (`crates/macos/`), with the Xcode command-line tools. |
| **Windows** | The Windows adapter (`crates/windows/`) builds on Windows only, with the MSVC build tools. |
| **Linux** | The Linux adapter (`crates/linux/`) builds on Linux only. Its live tests need an AT-SPI session; see `tests/e2e-linux/README.md`. |
| **Python 3** | `scripts/check-rust-file-size.sh` runs a Python helper and fails when no Python 3 is on `PATH`. |
| **Accessibility permission** | Required on macOS for integration and E2E tests against real apps. Grant it in **System Settings > Privacy & Security > Accessibility** by adding the terminal you run tests from. |
| **Screen Recording permission** | Required on macOS only for `screenshot` tests. Same path in System Settings. |

## Getting started

```bash
git clone https://github.com/lahfir/agent-desktop
cd agent-desktop

# Wire up the pre-commit hook (one time per clone)
git config core.hooksPath .githooks

# Every platform adapter compiles on its own OS only, so unscoped workspace
# commands fail on every host. Scope cargo to the host package set.
# macOS:
HOST_PKGS=(-p agent-desktop-core -p agent-desktop-macos -p agent-desktop -p agent-desktop-ffi)
# Windows: swap agent-desktop-macos for agent-desktop-windows.
# Linux:   swap agent-desktop-macos for agent-desktop-linux.

# Debug build
cargo build "${HOST_PKGS[@]}"

# Release build (optimised, < 15 MB)
cargo build --release -p agent-desktop

# Run the binary
./target/release/agent-desktop snapshot --app Finder -i
```

`HOST_PKGS` is an array so that it expands to separate arguments in both bash and zsh, the macOS default shell.

## Dev workflow and quality gates

All of these must pass before a PR is merged. Set `HOST_PKGS` as shown in Getting started.

```bash
# Format check
cargo fmt --all -- --check

# Lint — zero warnings required
cargo clippy "${HOST_PKGS[@]}" --all-targets -- -D warnings

# Library unit tests (adapter stubs, golden fixtures); skips the agent-desktop binary crate
cargo test "${HOST_PKGS[@]}" --lib

# Binary-level command tests (CLI contract, dispatch, batch, policy)
cargo test -p agent-desktop

# FFI integration tests (C-ABI, enum fuzzing, out-param zeroing)
cargo test -p agent-desktop-ffi --tests

# Core isolation — output must contain no platform crate names
cargo tree -p agent-desktop-core

# macOS only: E2E harness against the SwiftUI fixture app (needs an exclusive desktop and AX permission)
AGENT_DESKTOP_E2E_EXCLUSIVE=1 bash tests/e2e/run.sh
```

CI and the pre-commit hook run `cargo test` through `scripts/cargo-test-isolated-home.sh test ...`, which gives cargo a throwaway `HOME` so tests never touch `~/.agent-desktop`. Use it the same way locally when you want that isolation.

`tests/e2e/run.sh` builds the release CLI, macOS helper and FFI library itself and refuses to run without `AGENT_DESKTOP_E2E_EXCLUSIVE=1`, because headed cases move the real cursor. `tests/e2e/README.md` covers what it asserts and how to add a scenario.

CI also enforces what a local run can miss: the 15 MB limit on each shipped executable, the Rust 1.89 MSRV check, drift of the committed C header `crates/ffi/include/agent_desktop.h` from the Rust ABI declarations (regenerate it with `scripts/update-ffi-header.sh`, which needs cbindgen 0.29.4), the `cargo-deny` dependency policy in `deny.toml`, secret scanning, CodeQL, and workflow linting.

## Pre-commit hook

`.githooks/pre-commit` is the local mirror of the CI gates, and the script is the authoritative list. On a commit that stages `.rs` or `.toml` files it runs, over the package set for the host OS, the source-rule scripts in `scripts/` (`check-rust-file-size.sh`, `check-no-phase-references.sh`, `check-stale-ref-constructor-misuse.sh`), `cargo fmt --all -- --check`, clippy with `-D warnings`, and the `--lib` tests. When the commit touches `crates/ffi/` it also runs the stub-adapter passthrough test and the cbindgen header-drift check (skipped when cbindgen 0.29.4 is not installed; CI still runs it). A commit that only touches `scripts/`, `.githooks/`, `.github/workflows/` or `tests/e2e-linux/` runs the Bash 3 compatibility and no-phase-reference checks.

To bypass in a genuine emergency:

```bash
git commit --no-verify
# or
SKIP_PRECOMMIT=1 git commit -m "..."
```

## Engineering rules

[CLAUDE.md](CLAUDE.md) holds the engineering rules, and they bind people and coding agents alike: Conventional
Commits, the 400-line cap per Rust file, no inline comments, no delivery-plan references in shipped source, one domain
type per file, error handling, core isolation, and the steps for adding a command. The pre-commit hook and CI enforce
most of them.

**Skills stay in sync.** Every new command or changed CLI flag must be reflected in the matching skill. Cross-platform behavior goes in the core skill, `skills/agent-desktop/` (its `references/` hold the per-command detail). Platform behavior goes in `skills/agent-desktop-macos/`, `skills/agent-desktop-windows/` or `skills/agent-desktop-linux/`, and C ABI changes in `skills/agent-desktop-ffi/`. The binary embeds these skills and AI agents read them, so they must match the implementation.

## Submitting a pull request

1. **Pick the right base branch.** Branch from `main` and open the PR into `main` for macOS, Windows, core and FFI work. Linux adapter work goes to the `feat/linux-adapter` integration branch instead: cut your branch from it and merge back into it, never into `main`.
2. **Keep PRs focused.** One logical change per PR; separate refactors from features.
3. **Make sure all quality gates pass** locally before pushing (see Dev workflow above).
4. **Use a Conventional Commit title** for the PR title. It becomes the squash-merge commit message.
5. **Link related issues** with `Fixes #N` or `Refs #N` in the PR description.
6. **Check that `cargo tree -p agent-desktop-core` is clean** if you touched any crate dependency.
7. CI runs automatically on every push. A red CI is a blocker.

## Reporting bugs and security issues

**Bugs and feature requests:** use the [issue templates](https://github.com/lahfir/agent-desktop/issues/new/choose); [SUPPORT.md](SUPPORT.md) says what to include.

**Security vulnerabilities:** do **not** open a public issue. Follow [SECURITY.md](SECURITY.md).

## License

By contributing, you agree that your contributions are licensed under the [Apache-2.0 License](LICENSE) that covers this project.
