# Contributing to agent-desktop

agent-desktop is a native Rust CLI and FFI library that gives AI agents structured access to desktop applications through OS accessibility trees. Contributions that sharpen that core mission are welcome.

## Kinds of contributions that fit this project

- **Bug fixes** — wrong JSON output, incorrect ref resolution, `STALE_REF` on a stable target, etc.
- **New commands** — additions to the 60-command surface (56 operational, four held-input names fail closed) (follow the Extensibility Pattern below)
- **Platform adapters** — the Windows adapter (in progress) and the Linux adapter (planned), both implementing `PlatformAdapter`
- **App-specific quirks** — documented edge cases for specific apps (Electron, game engines, etc.) under `skills/`
- **Docs and skill files** — keeping `skills/agent-desktop*/` accurate when behaviour changes

Out of scope: embedding LLMs, GUI/TUI interfaces, browser automation, macro recording.

## Code of Conduct

This project follows the guidelines in [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). All contributors are expected to uphold them.

## Prerequisites

| Requirement | Notes |
|---|---|
| **Rust toolchain** | Pinned to `stable` via `rust-toolchain.toml` (`rust-version` 1.89). `rustup` picks the correct channel automatically. |
| **macOS 13.0+** | Required to build and run the macOS adapter (`crates/macos/`). The Windows adapter (`crates/windows/`) builds on Windows only. The Linux crate is still a stub and compiles anywhere. |
| **Accessibility permission** | Required for integration and E2E tests against real apps. Grant it in **System Settings > Privacy & Security > Accessibility** by adding the terminal you run tests from. |
| **Screen Recording permission** | Required only for `screenshot` tests. Same path in System Settings. |

## Getting started

```bash
git clone https://github.com/lahfir/agent-desktop
cd agent-desktop

# Wire up the pre-commit hook (one time per clone)
git config core.hooksPath .githooks

# Every platform adapter compiles on its own OS only, so unscoped
# workspace commands fail on every host. Scope cargo to the host package set
# (this is the macOS set; on Windows use
# -p agent-desktop-core -p agent-desktop-windows -p agent-desktop -p agent-desktop-ffi).
HOST_PKGS="-p agent-desktop-core -p agent-desktop-macos -p agent-desktop-linux -p agent-desktop -p agent-desktop-ffi"

# Debug build
cargo build $HOST_PKGS

# Release build (optimised, < 15 MB)
cargo build --release -p agent-desktop

# Run the binary
./target/release/agent-desktop snapshot --app Finder -i
```

## Dev workflow and quality gates

All of these must pass before a PR is merged. The pre-commit hook runs the format, lint and library-test gates automatically (see Pre-commit hook). Set `HOST_PKGS` as shown in Getting started.

```bash
# Format check
cargo fmt --all -- --check

# Lint — zero warnings required
cargo clippy $HOST_PKGS --all-targets -- -D warnings

# Unit tests (MockAdapter, golden fixtures)
cargo test $HOST_PKGS --lib

# Binary-level command tests
cargo test -p agent-desktop

# FFI integration tests (C-ABI, enum fuzzing, out-param zeroing)
cargo test -p agent-desktop-ffi --tests

# Core isolation — output must contain no platform crate names
cargo tree -p agent-desktop-core

# E2E harness against the SwiftUI fixture app (requires --release build + AX permission)
cargo build --release
bash tests/e2e/run.sh
```

The E2E harness drives the release binary against a real SwiftUI/AppKit fixture and asserts every effect by independent observation — never by trusting `ok: true`. See `tests/e2e/README.md` for what it covers and how to add a scenario.

CI additionally checks that the release binary stays under 15 MB and that `crates/ffi/src/commands/generated.rs` is not out of sync with `build.rs`.

## Pre-commit hook

The repo ships a pre-commit hook at `.githooks/pre-commit`. After cloning, enable it once:

```bash
git config core.hooksPath .githooks
```

On every commit that stages `.rs` or `.toml` files, the hook picks the package set for the host OS and runs:

1. `scripts/check-rust-file-size.sh` (400 LOC cap per Rust file and the inline-comment ban; only `///` and `//!` are permitted)
2. `scripts/check-no-phase-references.sh` (no phase, sub-phase or plan ids in `crates/**` or `src/**`)
3. `scripts/check-stale-ref-constructor-misuse.sh`
4. `cargo fmt --all -- --check`
5. `cargo clippy` on the host package set with `--all-targets -- -D warnings`
6. `scripts/cargo-test-isolated-home.sh test` on the host package set with `--lib`

When changes touch `crates/ffi/`, the hook also runs the stub-adapter passthrough test and the cbindgen header-drift check (skipped when `cbindgen` 0.29.4 is not installed; CI still runs it).

To bypass in a genuine emergency:

```bash
git commit --no-verify
# or
SKIP_PRECOMMIT=1 git commit -m "..."
```

## Coding standards

### Conventional Commits (required)

Every commit title must begin with one of these prefixes:

| Prefix | When to use |
|---|---|
| `feat:` | new capability |
| `fix:` | bug fix |
| `feat!:` / `BREAKING CHANGE:` footer | breaking API or CLI change |
| `refactor:` | restructures code without changing behaviour |
| `perf:` | measurable performance improvement |
| `test:` | adds or fixes tests |
| `docs:` | documentation only |
| `ci:` | CI/CD changes |
| `chore:` | maintenance, dependency bumps |
| `style:` | formatting only |

Format: `type: concise imperative description` — lowercase prefix, no capital after the colon, focus on the *why*.

Examples: `feat: add scroll-to command`, `fix: prevent stale ref on window resize`, `ci: add binary size check`

### File rules

- **400 LOC hard limit per file.** If a file approaches 400 lines, split by responsibility. Generated files marked `@generated` are exempt — fix the generator, not the output.
- **No inline comments.** Code must be self-documenting through naming. Use `///` doc-comments on public items when the name alone is insufficient. `//` and end-of-line `//` comments are rejected by the pre-commit hook.
- **One struct/enum per file** for domain types. `node.rs` defines `AccessibilityNode`. `action.rs` defines `Action`.
- **One command per file.** Each CLI command lives in its own file under `crates/core/src/commands/`. Filename matches the command name.
- **No God objects.** No struct with more than 7 fields; no function with more than 5 parameters. Use builder patterns or config structs.
- **Explicit `pub` boundaries.** Only `lib.rs` re-exports public items. Internal modules use `pub(crate)`. No wildcard re-exports.

### Error handling

- **Zero `unwrap()` in non-test code.** Propagate `Result`s with `?` or match explicitly. Panics are test-only.
- Every error carries an `ErrorCode` (machine-readable), `message` (human-readable), an optional `suggestion`, and an optional `platform_detail`.

### Core isolation (non-negotiable)

`agent-desktop-core` must never import a platform crate. Platform crates must never import each other. The only legitimate wiring points are the binary crate (`src/`) and the FFI crate (`crates/ffi/`). CI enforces this with `cargo tree -p agent-desktop-core`.

## Adding a new command

Follow the Extensibility Pattern exactly — no step may be skipped:

1. Create `crates/core/src/commands/{name}.rs` with a standalone `execute()` function.
2. Register it in `crates/core/src/commands/mod.rs`.
3. Add the CLI subcommand variant to `src/cli/mod.rs` and its argument struct under `src/cli_args/`.
4. Add a `match` arm in `dispatch()` in the binary crate.
5. If a new `Action` variant is needed, add it to `crates/core/src/action.rs`.
6. If a new adapter method is needed, add it to the `PlatformAdapter` trait with a default of `Err(AdapterError::not_supported())`.

No existing files are modified beyond these six registration points.

**Mandatory skill update:** every new command or changed CLI flag must be reflected in the matching skill. Cross-platform behavior goes in the core skill, `skills/agent-desktop/` (its `references/` hold the per-command detail). macOS behavior goes in `skills/agent-desktop-macos/` and Windows behavior in `skills/agent-desktop-windows/`. Skill files are documentation that AI agents read, and they must stay in sync with the implementation.

## Submitting a pull request

1. **Pick the right base branch.** Branch from `main` for macOS and core work. Windows adapter work branches from `feat/windows-adapter` (as `feat/windows-<n.n>-<slug>`) and opens its PR into `feat/windows-adapter`, never into `main`.
2. **Keep PRs focused.** One logical change per PR; separate refactors from features.
3. **Make sure all quality gates pass** locally before pushing (see Dev workflow above).
4. **Use a Conventional Commit title** for the PR title — it becomes the squash-merge commit message.
5. **Link related issues** with `Fixes #N` or `Refs #N` in the PR description.
6. **Check that `cargo tree -p agent-desktop-core` is clean** if you touched any crate dependency.
7. CI runs automatically on every push. A red CI is a blocker.

## Reporting bugs and security issues

**Bugs and feature requests:** open an issue on [GitHub Issues](https://github.com/lahfir/agent-desktop/issues). Include the OS version, the `agent-desktop version` output, the exact command that failed, and the full JSON response.

**Security vulnerabilities:** do **not** open a public issue. Use [GitHub private vulnerability reporting](https://github.com/lahfir/agent-desktop/security/advisories/new) for this repository. Include the affected version or commit, reproduction steps, expected impact, and any proof-of-concept details. See [SECURITY.md](SECURITY.md) for the full scope and response policy.

## License

By contributing, you agree that your contributions are licensed under the [Apache-2.0 License](LICENSE) that covers this project.
