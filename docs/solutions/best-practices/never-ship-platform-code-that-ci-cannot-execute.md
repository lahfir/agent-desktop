---
title: Never ship platform code that CI cannot execute
date: 2026-07-25
category: best-practices
module: crates/core private file I/O
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
tags: [platform-isolation, ci, windows, private-file, cfg, dead-code]
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

## Rules

- A `#[cfg]` branch that CI cannot execute is not shipped code, it is a hypothesis. Either
  add a lane that runs it, or do not merge it.
- Do not write platform hardening ahead of the platform adapter it protects. Write it on
  that platform, against a lane that runs it, informed by probes.
- A test that compares constants to constants is not a test. Every test must be able to
  fail.
- When a platform layer needs a stricter policy than the shared contract, that is a
  contract change to make deliberately and document — not a detail to bury in one OS.

## Constraints for future Windows private-file hardening

When hardened Windows I/O returns, it belongs behind `PlatformAdapter` or in a
Windows-gated dependency of the platform crate — never as unconditional core surface. It
must satisfy, with evidence:

1. Atomic replace requires `FILE_SHARE_DELETE` on every concurrently-open handle to the
   target. POSIX rename semantics do not transfer.
2. Owner validation compares against `TokenOwner`, not `TokenUser`.
3. `FileRemoteProtocolInfo` cannot establish volume locality. Use a verified primitive or
   drop the gate rather than failing closed on local disks.
4. Ancestor-wide validation must match the unix leaf-only contract, or the contract must
   change for both.
