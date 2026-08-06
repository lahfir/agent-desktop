---
title: Never ship platform code that CI cannot execute
date: 2026-07-25
category: best-practices
module: crates/core/src/private_file_ops.rs, crates/windows/src/system/private_file
problem_type: process_gap
component: tooling
symptoms:
  - "A platform-conditional branch in core compiles everywhere but has only ever run on one OS."
  - "225 of 940 core unit tests fail the first time the suite is run on Windows."
  - "status fails closed on a plain local NTFS disk with 'cannot verify that the Windows storage is local'."
  - "A platform unit test compares two constants and can never fail."
root_cause: process_gap
resolution_type: code_removal
severity: high
tags: [platform-isolation, ci, windows, private-file, cfg, dead-code, test-lane-scope]
---

# Never ship platform code that CI cannot execute

## Problem

`agent-desktop-core` is defined as platform-agnostic. Its domain logic honoured that —
five `target_os` lines in the whole crate, every command test driven by `MockAdapter`.

Its filesystem layer did not. `private_file*` carried 1,062 LOC of Windows-only `unsafe`
Win32: ACL construction, SID comparison, ancestor guard chains, reparse-point rejection,
remote-volume detection. The Windows adapter it served was a 76-LOC stub with no
`windows-sys` dependency and no implemented command — 14x more Windows code in
"platform-agnostic" core than in the Windows crate.

CI ran `cargo check` on Windows and Linux, and `cargo test` on macOS alone. The Win32
layer shipped in v0.5.0 type-checked and never once executed. Its first execution, on a
real Windows machine, failed 225 of 940 core tests across four clusters:

- 122 sharing violations. `LEAF_SHARING` deliberately excluded `FILE_SHARE_DELETE`, so
  `SetFileInformationByHandle(FileRenameInfo)` collided with the validation handle the
  same code still held open. POSIX `rename(2)` ignores open descriptors; Windows does not.
- 69 owner-only-DACL rejections. Owner validation compared against `TokenUser`; elevated
  tokens own new objects as `BUILTIN\Administrators`, which is how most CI runners execute.
- 9 trace-writer access denials and 25 cascading assertion failures.
- `status` dead on Windows: locality was inferred from
  `GetFileInformationByHandleEx(FileRemoteProtocolInfo)` failing with exactly
  `ERROR_INVALID_PARAMETER`. Local NTFS returns other codes, so the gate failed closed —
  and it ran on every ancestor from the volume root down.

The layer also diverged from the contract it claimed to implement. On unix, ownership and
mode are checked on the leaf directory only; ancestors merely have to be non-symlink
directories. The Windows path validated every ancestor and rejected any reparse point.
Nobody decided that. It drifted, and no test could notice.

The single Windows behaviour test in the deleted code was:

```rust
assert_eq!(LEAF_SHARING & FILE_SHARE_DELETE, 0);
```

It asserted the defect was correct, compared constants to constants, and could never fail.

## Resolution

Deleted all 1,062 LOC and removed `windows-sys` from core entirely. Windows now uses the
same portable `std::fs` path as every other non-unix target. This is honest: the hardening
guarded refmap, trace, and session artifacts on a platform where no command can produce
them.

Added real `cargo test` lanes for Windows and Linux so every platform-conditional branch
in core is executed on every PR. That lane, not the deleted code, is the actual fix — it
is what stops the same mistake reaching the Linux adapter, whose
`validate_local_filesystem` had been equally unrun.

## Recurrence

The Windows vocabulary work (2026-08-01) hit the same shape again, one level down. The census
tool's redaction guard — a marker-planted test proving `render_node` never serializes a
real application's `Name`, `HelpText`, `FullDescription`, or `LegacyDefaultAction` into a
committed capture — was written in
`crates/windows/examples/uia_tree_dump/render_node_tests.rs`. The Windows CI lane ran
`cargo test -p agent-desktop-windows --lib`, which builds and executes the library target
only; `--lib` does not reach anything under `examples/`. The guard compiled, asserted a
real security property, and could never fire on the runner — not because no lane existed,
but because the lane's own flags excluded the target the guard lived in. Same defect,
different mechanism: the 2026-07-25 instance was a `#[cfg]` branch with no lane at all,
this one is a guard sitting in a target a lane deliberately does not build.

Fixed by adding a `Windows example tests` step (`cargo test --locked -p
agent-desktop-windows --examples`) to `.github/workflows/ci.yml`, and pinning that exact
command string as an assertion in `src/cli/contract_tests.rs` so the step cannot silently
disappear from the workflow file the way the coverage gap itself was invisible.

## Rules

- A `#[cfg]` branch that CI cannot execute is not shipped code, it is a hypothesis. Either
  add a lane that runs it, or do not merge it. The same holds for a guard CI *can* build
  but a lane's flags exclude from execution — a `--lib` run skipping `examples/`, a
  `--tests` run skipping a doctest. Pin the exact command that runs the guard, not just a
  lane's existence, so a later flag change can't quietly narrow it back out.
- Do not write platform hardening ahead of the platform adapter it protects. Write it on
  that platform, against a lane that runs it, informed by probes.
- A test that compares constants to constants is not a test. Every test must be able to
  fail.
- When a platform layer needs a stricter policy than the shared contract, that is a
  contract change to make deliberately and document — not a detail to bury in one OS.

## How the Windows private-file hardening returned

The hardening is back, and it is back the only way it was allowed to be: in the platform
crate, behind a trait seam, on a lane that runs it. `crates/core/src/private_file_ops.rs`
defines the `PrivateFileOps` trait and `install_private_file_ops`; core's default stays
portable `std::fs`. `crates/windows/src/system/private_file/` implements it as
`WindowsPrivateFile` across `path`, `replace`, `owner` and `locality` with their test
files beside them, and `src/main.rs` installs it under `#[cfg(target_os = "windows")]`.
The `windows-latest` lane executes all of it on every PR through
`cargo test --locked -p agent-desktop-core -p agent-desktop-windows --lib`.

Each of the four measured defects that sank the deleted layer is discharged in that code:

1. **Atomic replace requires `FILE_SHARE_DELETE` on every concurrently-open handle to the
   target; POSIX rename semantics do not transfer.** `replace.rs` promotes with
   `ReplaceFileW`, never `MoveFileEx`, on a measured 42/42 matrix with zero successes at
   any share mode lacking share-delete. Artifact opens keep Rust's wide
   `FILE_SHARE_READ|WRITE|DELETE` mask precisely because narrowing it re-creates the
   122-failure sharing cluster. The one handle that does omit `FILE_SHARE_DELETE` is the
   internal lease-directory handle, where the sharing constraint *is* the mechanism — it is
   the live-writer guard that stops a sweep reclaiming a directory another write still
   holds.
2. **Owner validation compares against `TokenOwner`, not `TokenUser`.** `owner.rs` reads
   `TokenOwner` only, on evidence measured at both High and Medium integrity: a file
   created by an admin-group account is owned by Administrators, so
   `OwnerMatchesTokenUser` is false while `OwnerMatchesTokenOwner` is true — group
   membership is the variable, not integrity, which is why the old code's 69 rejections
   tracked CI runners rather than elevation. The module reads the owner alone
   (`OWNER_SECURITY_INFORMATION`, no DACL requested) and never touches an ACE.
3. **`FileRemoteProtocolInfo` cannot establish volume locality on its own.** The general
   technique, and the reusable part: *when an API signals a condition by failing, prove the
   plumbing with a control call on a known-good input first.*
   `GetFileInformationByHandleEx(FileRemoteProtocolInfo)` reports a local volume by failing
   with `ERROR_INVALID_PARAMETER` (87) — measured across six local and three remote targets
   — but an out-of-range information class returns that same 87, so the sentinel alone
   cannot distinguish "local volume" from "this call never dispatched". `locality.rs`
   therefore requires `FileBasicInfo` (class 0) to succeed on the same handle before it
   reads 87 as a locality signal, and returns `Unknown` when the control call fails.
   `Unknown` refuses a private-artifact *write*; reads are never gated, which is what stops
   a repeat of `status` dying on ordinary local disk.
4. **Ancestor-wide validation must match the unix leaf-only contract, or the contract must
   change for both.** It matches. `path.rs` rejects a reparse point per component on write
   paths, mirroring the unix per-component symlink rejection in core's
   `private_file_parent.rs`, and guards only the leaf on reads, exactly as `O_NOFOLLOW`
   does. The scopes now agree by construction instead of drifting apart unnoticed.

Two things stayed out deliberately, and the reason is the same rule: descriptor authoring
and DACL validation would re-check what Windows already grants, using exactly the
ACE-parsing code whose `AceSize` handling sank the old layer. A test pins the absence of
any ACL/ACE call, and a structural test pins the grants that survive creation and replace,
so an OS change breaks a test rather than the product.
