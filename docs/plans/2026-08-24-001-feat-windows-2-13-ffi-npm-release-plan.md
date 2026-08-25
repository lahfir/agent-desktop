---
title: FFI, npm, Release (Sub-phase 2.13) - Plan
type: feat
date: 2026-08-24
origin: docs/phases.md
artifact_contract: ce-unified-plan/v1
artifact_readiness: implementation-ready
product_contract_source: docs/phases.md §Phase 2 sub-phase 2.13
execution: code
---

# FFI, npm, Release (Sub-phase 2.13) - Plan

## Goal Capsule

- **Objective:** Make the Windows adapter reachable through every distribution channel that already ships for macOS, and make the documents that describe it true. The adapter itself is finished — 2.2 through 2.12 shipped observation, semantic actions, input synthesis, lifecycle, capture, clipboard, signals and a live e2e harness — but a user cannot get it. `npm/scripts/postinstall.js:46` reads `const SUPPORTED_PLATFORMS = ['darwin'];` and prints *"Windows and Linux support is coming in Phase 2"* on every Windows install; `.github/workflows/release.yml:134-137` matrixes the CLI build over `aarch64-apple-darwin` and `x86_64-apple-darwin` only, so no Windows `.exe` has ever been published; `README.md:412-423` still says **Planned** in six rows that are shipped. This sub-phase is packaging and truth-telling, not adapter code — no `PlatformAdapter` method is added and no UIA call is written.
- **Authority hierarchy:** `docs/phases.md` §2.13 and §Release, Skill & Docs > `probes/windows/FINDINGS.md` > measured evidence gathered by this plan's probe area > vendor documentation cited here > this plan > implementer judgment. Where measured evidence contradicts a document, U10 amends the document in this same PR rather than planning around it.
- **Stop conditions:** Do not implement `list_surfaces`, notification management, or system tray on Windows (§2.14 owns all three). Do not implement Windows cursor-overlay rendering. Do not migrate npm distribution to `optionalDependencies` (KTD3). Do not sign binaries (KTD5). Do not touch `crates/macos`, and do not change any macOS release asset's name, shape, or contents. Do not add a `#[cfg(windows)]` to `crates/core`: CI pins the count at exactly two shims in `crates/core/src/private_file.rs` and fails on a third (`.github/workflows/ci.yml:306-329`). Do not carry 2.12's residual scroll-ladder and headed-double-click work — that is its own PR by the owner's decision.
- **Execution profile:** One PR from `feat/windows-2.13-ffi-npm-release` into `feat/windows-adapter`, never `main`. Workflow changes, npm package changes, one FFI test with its own fixture, one new skill package, README and `docs/phases.md` corrections, and probe area 25. Conventional Commits, authored by Lahfir, no co-authors. See §LOC Budget — the origin's `~1.2k LOC` estimate holds for product code once documentation and the probe corpus are counted separately, as this document's own delivery model directs.
- **Tail ownership:** The implementer opens the PR against `feat/windows-adapter` and reports the Verification Contract results, including the release dry-run's run URL and the artifacts it produced.

---

## Product Contract

### Summary

Three channels carry agent-desktop to a user: a GitHub Release asset, an npm package that downloads that asset, and an FFI cdylib for embedders. Windows is absent from the first, gated out of the second, and present-but-unproven in the third. The work is small in each channel and the risk is concentrated in one place: **every exit criterion this sub-phase's scope states is about published software, and nothing published is reachable from a sub-phase branch.** `npm publish` and `gh release create` fire from `main` (`.github/workflows/release.yml:3-8`), and Windows reaches `main` exactly once, at the end of Phase 2. A plan that writes *"`npm install -g` works on Windows"* as its gate writes a gate this PR cannot run, which is the shape this repository names as a defect in its own right (`docs/solutions/best-practices/a-test-that-cannot-fail-is-not-coverage.md`). So the design question is not *what to build* — the code is nearly obvious — it is *what proof is available before publication*, and every requirement below is stated in a form this PR can falsify.

The second theme is that the documents are the deliverable. `skills/agent-desktop-windows/SKILL.md` and the README platform table are read by agents and users as statements of fact about what the binary does. Six README rows are wrong in the pessimistic direction today, and three claims would be wrong in the optimistic direction if the table were flipped naively: `list-surfaces`, notification management, and cursor-overlay rendering do not work on Windows. Getting those three right is the difference between a truthful document and a marketing one.

### Problem Frame

**The gate that matters is unfalsifiable as `docs/phases.md` states it, and the fix is to restate it in pre-publication terms.** §2.13's exit criteria read *"`npm install -g` works on Windows; release dry-run artifacts verified."* The second half is already the right shape and the first half is not. `release.yml` runs on push to `main` or `workflow_dispatch`, and its `publish-github` / `publish-npm` jobs are gated on `needs.release-please.outputs.release_created == 'true'` — a condition only a release commit on `main` satisfies. This sub-phase therefore proves the chain in two verifiable halves that meet in the middle: a `workflow_dispatch` run of the build jobs on this branch produces the real Windows assets, and the npm package installs **from those artifacts** on this machine. What remains unproven at merge is only the GitHub-Release *hosting* of an asset the workflow demonstrably produced — a step whose Windows-specific content is zero, since `publish-github` globs `*.tar.gz` / `*.zip` and is target-agnostic (`release.yml:445-459`).

**The Windows environment already provides everything the existing macOS install path needs, which removes an entire branch this sub-phase would otherwise have to write.** `postinstall.js` downloads with `curl` (`:68-80`) and lists and extracts with `tar -tzf` / `tar -xzf` (`:126, :148`). The conventional assumption is that Windows needs a `.zip` and a Node-side unzip, because Node ships no archive support. Measured on the Windows Server 2019 box this plan was written on: `C:\Windows\System32\curl.exe` is curl 8.9.1 and `C:\Windows\System32\tar.exe` is bsdtar 3.5.2, both in `System32` and therefore always on `PATH`; a create-list-extract round trip of a gzip tarball containing `agent-desktop.exe` succeeds. Microsoft has shipped both in-box since Windows 10 build 17063, comfortably below the 1809 API floor this phase already requires (§Minimum OS Requirements). U1 records this as probe rows rather than leaving it as a paragraph, because the entire archive-format decision (KTD1) rests on it, and a decision resting on an unrecorded observation is a guess with a confident tone.

**The npm package holds the platform mapping three times, and this sub-phase adds a fourth platform to each.** `TARGET_MAP` and `BINARY_NAME_MAP` in `npm/scripts/postinstall.js:30-44`, `platformMap` in `npm/bin/agent-desktop.js:16-22`, and a `uname`-keyed `case` in `scripts/ci-npm-wrapper-smoke.sh:7-14` are three independently-maintained copies of one fact. They already disagree: postinstall's `SUPPORTED_PLATFORMS` gate refuses `win32` while both of its maps carry complete, unreachable `win32-x64` entries, and the wrapper's map carries the same entry with no gate at all — so a Windows user today gets *"binary not found"* from the wrapper rather than the *"macOS only"* message postinstall printed minutes earlier. This is `a-test-that-cannot-fail-is-not-coverage.md`'s hand-maintained-parallel-list shape, and adding `win32-arm64` to three lists is how three disagreements become four.

**Nothing checks that the asset name npm asks for is the asset name the release produces.** postinstall builds `agent-desktop-v${version}-${target}.tar.gz` (`:271`) and `release.yml:179` builds the same string from its matrix, and the agreement is a coincidence maintained by hand. `scripts/check-release-consistency.sh` enforces version equality across `Cargo.toml`, `npm/package.json`, `.release-please-manifest.json` and `Cargo.lock` (`:56-84`) and says nothing about asset names. `scripts/check-npm-package.js` verifies the packed file list, the `bin` entry, and `release.yml`'s npm-publish security properties (`:137-173`) and says nothing about targets. A Windows target added to one side and not the other produces a package that installs cleanly on macOS, passes every gate, and 404s on every Windows machine — discovered by users, after publication. U5 closes this, and it is the highest-value gate in the sub-phase because it is the only one that can fail *before* the thing it protects becomes unreachable.

**The FFI claim in `docs/phases.md` §2.13 is wrong in both halves, and the real gap is narrower and different.** The scope reads *"the stub-adapter tests already run cross-platform in CI, but the real `WindowsAdapter` behind the C ABI needs its own pass."* Neither clause holds. The stub passthrough job runs on `ubuntu-latest` only (`.github/workflows/ci.yml:689-713`, `--features stub-adapter --test c_abi_passthrough`), so it is not cross-platform. And the real `WindowsAdapter` already gets a pass: `stub-adapter` is not a default feature (`crates/ffi/Cargo.toml:9`), so `build_adapter` compiles in `agent_desktop_windows::WindowsAdapter` (`crates/ffi/src/adapter.rs:115-119`), and `ci.yml:427` runs `cargo test --locked -p agent-desktop-ffi --tests` on `windows-latest` against exactly that. Measured on this box: 182 lib tests and 91 integration tests pass on Windows with no stub feature. What is actually missing is two specific things. First, `-p agent-desktop-ffi --lib` **never runs on any Windows lane** — the Windows lane's `--lib` invocation names `-p agent-desktop-core -p agent-desktop-windows` only (`ci.yml:411`), so those 182 tests are macOS-and-Linux-only coverage of a crate that compiles Windows-specific code. Second, no test anywhere drives a **real window** through the C ABI: `c_abi_snapshot.rs` asserts envelope shape against whatever the desktop happens to contain, and `c_abi_execute_by_ref.rs`'s round-trip test is named `execute_by_ref_returns_error_envelope_when_no_refmap_exists` and deliberately never snapshots first. U10 corrects the document; U6 closes the two real gaps.

**The Windows FFI release archive omits the MSVC import library, so a linking consumer gets nothing to link against.** `release.yml:294-306` stages `target/${target}/release-ffi/${lib_name}` and for Windows `lib_name` is `agent_desktop_ffi.dll`. Built on this box at the pinned profile, `cargo` also produces `agent_desktop_ffi.dll.lib` (26,298 bytes) and `agent_desktop_ffi.pdb`. `dlopen` / `ctypes` / `ffi-napi` consumers resolve by path and need neither, which is why this has gone unnoticed; a C++ or Rust consumer linking against the DLL on MSVC needs the `.lib` and cannot produce one from the shipped archive. The archive's own generated README already tells macOS linking consumers what they need (`release.yml:315-317`); the Windows equivalent is absent because the file is.

**Adding a skill directory on disk does not make the binary serve it.** `crates/core/src/commands/skills.rs:4-26` embeds every skill file through an individually-written `include_str!` and lists them in a static `SKILLS` array (`:92-107`); there is no directory walk. A `skills/agent-desktop-windows/SKILL.md` committed without a matching edit to that file is invisible to `agent-desktop skills list` and `skills get`, and **nothing detects it** — `crates/core/src/commands/skills_tests.rs` asserts only that the two currently-known skills exist. `scripts/check-no-phase-references.sh` already scans `skills/` for plan references (`:53-54`, `TOKEN_SCAN_MD_ROOTS="skills"`) precisely because those files are embedded, so the project has already reasoned once about this directory being part of the binary; the coverage half of that reasoning is missing. This is `an-enforcement-gate-must-cover-everything-the-binary-embeds.md` with the gate absent rather than mis-scoped, and U7 closes it.

**Flipping the README platform table naively would replace six pessimistic errors with three optimistic ones.** Against the adapter as it stands: `ObservationOps::list_surfaces` has no Windows override, so `list-surfaces` returns `PLATFORM_NOT_SUPPORTED`; the four notification methods have no Windows override, which is correct and §2.14's to change; and `update_cursor_overlay` is not overridden either, so Windows inherits core's `Ok(())` default and `cursor-overlay enable` reports success while nothing renders. The first two are honest refusals the CLI already surfaces. The third is the one shape the cross-cutting DoD singles out — *"the CLI surfaces `PLATFORM_NOT_SUPPORTED` honestly rather than a stub success"* — and it is worth being precise about why 2.13 does not close it, because "we chose not to" and "we could not" are different answers. **Overriding the adapter method would change nothing.** `src/dispatch/cursor_overlay.rs:33-37` calls `adapter.update_cursor_overlay` inside an `if let Err(...)` that logs a `tracing::warn!` and falls through to `Ok(value)` — the adapter's answer is swallowed by construction, so a Windows override returning `not_supported()` would produce the identical `ok: true` envelope with one more warning nobody reads. Making the command honest means changing the dispatch to propagate, which changes the contract on macOS too, for a command whose session-scoped setting genuinely *is* recorded correctly on both platforms. That is a decision about a shipped cross-platform command, not a Windows packaging question, and U10 writes it into §2.15's scope where a hardening review can weigh it. 2.13 handles all three by documentation: the capability tables it writes say what is true, and R20 makes that falsifiable.

**No row in `probes/windows/FINDINGS.md` names sub-phase 2.13**, so the cross-cutting row-disposition obligation is discharged by verification rather than by work — stated here because "no rows named this sub-phase" and "nobody checked" are indistinguishable in a report that omits it. The corpus runs to area 24 and `.github/workflows/windows-capability-probe.yml` registers areas 14-24, so 2.13's area is **25** and must be registered in the same PR, in both the `paths` filter and a run step.

### Requirements

Release assets:

- R1. **A Windows CLI release asset is produced for `x86_64-pc-windows-msvc` and `aarch64-pc-windows-msvc`**, named by the same `agent-desktop-v${VERSION}-${target}.tar.gz` template every existing target uses (`release.yml:179`). The macOS legs keep their current runner, steps, asset names and archive contents byte for byte.
- R2. **The Windows archive contains exactly one entry, `agent-desktop.exe`.** There is no Windows analogue of `agent-desktop-macos-helper`, so the archive shape differs from macOS by exactly that omission and by nothing else.
- R3. **Every new asset carries a SHA-256 line in `checksums.txt` and is covered by the existing provenance attestation.** `publish-github` concatenates `*.sha256` and attests `*.tar.gz` / `*.zip` / `checksums.txt` (`release.yml:431-459`), so this holds by construction provided the Windows leg emits a `.sha256` beside its archive in the format the concatenation expects.
- R4. **The 15 MB shipped-executable ceiling is enforced on the Windows release binaries by a check that executes on the Windows runner.** The macOS check is `stat -f%z`, which is BSD-specific (`release.yml:164-173`); the Windows form already exists and is proven in `ci.yml:502-512` (`Get-Item ... .Length`, `$limit = 15MB`).
- R5. **The release asset-count assertion is exact and derived, not a floor with a stale comment.** `publish-npm` currently checks `>= 8` against a comment enumerating a composition this sub-phase changes (`release.yml:486-494`).

npm distribution:

- R6. **`npm install -g agent-desktop` on `win32-x64` and `win32-arm64` downloads, checksum-verifies, and installs the matching binary, and the `bin` wrapper executes it.** The existing `curl` download, `checksums.txt` parse, SHA-256 comparison and atomic install path are reused unchanged (`postinstall.js:68-112, 280-289`).
- R7. **The platform mapping exists exactly once inside the npm package.** postinstall's two maps and the wrapper's third become one module both require, so a platform cannot be half-added.
- R8. **The archive-shape check is platform-correct rather than macOS-shaped.** `validateArchive` and `installArchive` both compare the listing against a literal `['agent-desktop', MACOS_HELPER_NAME]` (`:134, :149`); the expected set becomes a function of the platform, computed once and consumed by both call sites.
- R9. **The wrapper reports a non-zero exit status when the child is terminated by a signal.** `child.on('close', (code) => process.exit(code ?? 0))` (`bin/agent-desktop.js:107-109`) discards the `signal` argument, so a killed child is reported to the caller as success.
- R10. **postinstall names only skills that exist.** `promptSkillInstall` (`postinstall.js:199-214`) advertises `lahfir/agent-desktop-macos`, `lahfir/agent-desktop-windows` and `lahfir/agent-desktop-linux`; `skills/` contains `agent-desktop` and `agent-desktop-ffi`, and `publish-skills` publishes exactly the `skills/*` directories, so all three names are unpublished today.
- R11. **A gate fails when npm and the release workflow disagree about asset names or counts.** Every target in the package's platform table must appear as a target in `release.yml`'s CLI build matrix; the tarball template both sides construct must match; and `publish-npm`'s expected asset count must equal the count the workflow's own matrices imply.

FFI:

- R12. **`cargo test -p agent-desktop-ffi --lib` executes on a Windows CI lane.**
- R13. **A test drives the real `WindowsAdapter` through the C ABI end to end**: create an adapter, snapshot a window the test itself staged, take a ref out of the returned tree, execute a click against that ref, and confirm the click landed by reading the window's own state — never by reading the action's `ok` field.
- R14. **The Windows FFI release archive contains the MSVC import library** beside the DLL, and its generated README says what a linking consumer needs, as the macOS archive already does for `install_name`.
- R15. **The FFI cdylib is built and released for `aarch64-pc-windows-msvc`.**

ARM64:

- R16. **`aarch64-pc-windows-msvc` is compiled on every pull request, and the Windows unit suite runs on a native ARM64 lane** under the same live-staging opt-in the x64 lane uses. §2.13's scope states ARM64 validation is no longer deferred; a target built only at release time is not validated.
- R17. **The released ARM64 CLI binary is executed on the ARM64 runner before it is archived.** A binary that cannot start is not a release artifact, and the ARM64 runner is the only place this can be observed.

Documentation truth:

- R18. **`skills/agent-desktop-windows/` ships and `agent-desktop skills` serves it** — `list` names it, `get` returns its body, and `get --reference` returns each of its references.
- R19. **A test fails when a skill document on disk is not reachable from the embedding module.** Coverage is derived from the module's own source, never from a second hand-maintained list of the first.
- R20. **Every capability claim in the new and updated documents matches the adapter, and no shipped document instructs a Windows agent to use something that exists only on macOS.** `list-surfaces`, the four notification commands, and cursor-overlay rendering are documented as unavailable on Windows; every command documented as working is one `WindowsAdapter` implements; and `skills/agent-desktop/references/workflows.md`, which the binary embeds and serves on every platform, gains a Windows branch for its macOS-only first-time-setup instructions.
- R21. **`README.md`'s Platform Support table, installation section and permissions section state what is true on Windows** — six rows flip to **Yes**, Notifications stays **Planned**, and the macOS-only build and permission prose gains its Windows counterpart.

Evidence:

- R22. **Probe area 25 records the measurements this plan's decisions rest on**, is registered in `.github/workflows/windows-capability-probe.yml` in both the `paths` filter and a run step, and uploads its captures from the CI run.
- R23. **`docs/phases.md` reads true against what shipped**, including the two false clauses in §2.13's FFI scope that this plan's research disproved.

### Key Decisions

- **2.13 is planned as `docs/phases.md` defines it, with contradictions corrected rather than planned around.** (session-settled: user-directed — the standing instruction across this phase.) Research disproved both clauses of §2.13's FFI sentence and showed the primary exit criterion is unfalsifiable before publication. Governs R11, R12, R13, R23. See KTD10, U10.
- **A capability is documented as working only where the adapter implements it.** The README flip is the point of this sub-phase, and a table that overstates is worse than one that understates, because an agent reading it will attempt the command and receive a refusal it was told would not happen. Governs R20, R21. See U7, U8.
- **Publication is out of reach, so the gate moves to the artifact.** Every exit criterion is restated as something a `workflow_dispatch` run plus a local install can falsify on this branch. Governs R1-R6, R11. See KTD10, U9.

### Scope Boundaries

In scope: the CLI release build matrix, the FFI release matrix's ARM64 leg and archive contents, the npm package's platform support and its gates, the two real FFI Windows coverage gaps, the Windows skill package and its embedding, the README, `docs/phases.md` corrections, probe area 25, and the dogfood gate.

Not in scope, and not deferred by this sub-phase — already owned elsewhere in `docs/phases.md`:

- `list_surfaces`, notification management and system tray on Windows — §2.14.
- Windows cursor-overlay rendering — no sub-phase claims it; §2.15's hardening review is where a decision belongs, and 2.13 documents the current behaviour rather than changing it.
- The 2.12 residual scroll-ladder cap and headed-double-click legs — their own PR, by the owner's decision.

#### Deferred to Follow-Up Work

- **Windows code signing.** KTD5 records the decision and its evidence; it is a business/credential action, not an engineering deferral, and no sub-phase's exit criteria depend on it.

---

## Planning Contract

### Key Technical Decisions

- **KTD1. The Windows CLI release asset is a `.tar.gz`, identical in shape to the macOS asset minus the helper — no `.zip`, and no new extraction code in `postinstall.js`.** Measured: `C:\Windows\System32\tar.exe` is bsdtar 3.5.2 and `C:\Windows\System32\curl.exe` is curl 8.9.1, both in-box since Windows 10 build 17063, and a gzip-tarball create/list/extract round trip succeeds (U1 records this as `A25-1` and `A25-2`). The postinstall download and extraction path therefore works on Windows **unmodified**; the only Windows-specific change it needs is the expected-entries set (R8). **Rejected: a `.zip` asset with a Node-side unzip.** Node has no built-in archive support, so that path means either a new dependency in a zero-dependency package or shelling out to `tar`/`Expand-Archive` anyway — strictly more code to reach the same place. **Rejected: matching the FFI job's existing Windows `.zip`.** That archive is unpacked by a human reading a README; this one is unpacked programmatically by code that already speaks tar. Consistency between two archives with different consumers is not worth a branch in the consumer that has none. What this costs: the repository now ships `.zip` for the Windows FFI cdylib and `.tar.gz` for the Windows CLI. U8's release documentation states which is which so the difference is deliberate on the page rather than surprising in a download.
- **KTD2. ARM64 builds natively on the GitHub-hosted `windows-11-arm` runner; it is not cross-compiled from `windows-latest`.** GA for public repositories since 2025-08-07 and for private ones since 2026-01-29; the image ships a Rust toolchain and Visual Studio, and `rustup show` honours the repository's pinned `1.89.0`. Cross-compiling to `aarch64-pc-windows-msvc` from an x64 image requires the `MSVC v143 ARM64 build tools` component, which `actions/runner-images#14215` reports missing on the VS2026 Windows image — a live availability risk on an image GitHub is currently migrating. The decisive argument is not the linker, though: a cross-compiled binary cannot be **run** on the host that built it, and §2.13's scope says ARM64 validation is no longer deferred. Native building is what makes R16 and R17 possible at all. **Rejected: cross-compilation with a build-only exit criterion.** That is the shape `never-ship-platform-code-that-ci-cannot-execute.md` names. What this costs: `windows-11-arm` rolls over to a VS2026-based image between 2026-09-21 and 2026-09-30, so the lane may see a toolchain change shortly after this lands; U3 pins nothing beyond the label because pinning a transitional label is worse than following it.
- **KTD3. npm distribution keeps the postinstall-download model; it does not move to `optionalDependencies` per-platform packages.** The `optionalDependencies` pattern is the wider 2026 convention and has real advantages under `--ignore-scripts`, but adopting it means republishing every platform as its own package, rewriting the wrapper's resolution, changing what `release.yml` publishes, and re-verifying macOS — a distribution-model rewrite for all platforms inside a sub-phase whose job is to add one. **Rejected on scope, not on merit.** What this costs: a Windows user installing with `--ignore-scripts` gets no binary. The wrapper already fails loudly in that case with a "binary not found" path (`bin/agent-desktop.js:40-57`), and U4 makes that message name the cause rather than only the symptom.
- **KTD4. The npm package's platform table lives in one module, `npm/lib/platform.js`, required by both postinstall and the wrapper.** Three hand-maintained copies of one mapping already disagree in a user-visible way today. Extracting them is smaller than adding `win32-arm64` to three lists correctly, and it converts a class of silent divergence into a compile-time-adjacent one. `scripts/ci-npm-wrapper-smoke.sh` keeps its own `uname` case because it runs before the package is assembled and is macOS-only by construction; U5's gate covers the divergence that copy can cause. **Rejected: adding the fourth platform to three lists.** What this costs: `npm/package.json`'s `files` array and `scripts/check-npm-package.js`'s exact packed-file assertion both gain one entry; both are updated in U4 and the assertion stays exact rather than becoming a prefix match.
- **KTD5. Windows binaries ship unsigned, and the SmartScreen consequence is documented rather than engineered away.** Microsoft Defender SmartScreen's interactive warning is triggered by the Mark-of-the-Web on a downloaded file and fires on GUI launch through Explorer; command-line invocation — which is how npm's shim, and every real use of this CLI, starts the binary — is not interactively blocked. Azure Trusted Signing is now cheap enough (Basic tier, ~$10/month) that signing is a reasonable future step, but it requires an organisational identity and credential provisioning that no engineering unit can complete. **Rejected: making signing an exit criterion.** A gate nobody in the PR can satisfy blocks the sub-phase on an account application. What this costs: a user who downloads the `.exe` in a browser and double-clicks it sees a warning. U8's README section says so plainly and tells them what to expect; U1 measures the actual behaviour (`A25-5`) rather than trusting the vendor description.
- **KTD6. The Windows skill ships as its own top-level `skills/agent-desktop-windows/` package, embedded unconditionally.** `postinstall.js:202` already advertises `lahfir/agent-desktop-windows` as the per-platform skill name, `publish-skills` publishes each `skills/*` directory, and `scripts/link-skills.sh:12` globs `skills/*/` — three shipped consumers already expect a directory by that name. The alternative precedent, `skills/agent-desktop/references/macos.md` behind `#[cfg(target_os = "macos")]`, would put Windows guidance where those three consumers cannot reach it. Embedding is unconditional because a `cfg`-gated entry makes the coverage test in KTD7 platform-dependent for no benefit: the total skill corpus is well under the binary's 15 MB ceiling, and a macOS user asking for Windows guidance is a reasonable thing to be able to do. **Rejected: a `references/windows.md` under the existing skill.** What this costs: `skills/agent-desktop/references/macos.md` stays `cfg`-gated and asymmetric with the new package. That asymmetry is pre-existing and changing it would alter what a shipped macOS binary serves, which is outside this sub-phase.
- **KTD7. Skill-embedding coverage is enforced by a Rust test that reads `skills.rs`'s own source text, not by a hand-maintained expected list.** The test walks `skills/` from `CARGO_MANIFEST_DIR`, and for each `.md` found asserts its repository-relative path appears in the text of `crates/core/src/commands/skills.rs`. It needs no per-platform exemption (a `cfg`-gated `include_str!` is still present in the source on every platform), and there is no second list to drift. Reading source text from `CARGO_MANIFEST_DIR` in a test is established practice in this crate rather than a novelty: `crates/core/src/commands/ref_policy_tests.rs:349` and `is_check_vocabulary_tests.rs:13` both do it, and `src/cli/contract_tests.rs:377` reaches across crates the same way. A behavioural test beside it asserts `skills list` names the new skill and `skills get --reference` returns each of its references — together they cover both "wired in" and "actually served", which neither proves alone. **Rejected: a shell gate with MUST-CATCH/MUST-PASS fixtures.** The repository has that pattern for the e2e contract gate, and it is the right tool where the gate is itself a script; here a 25-line test runs on all three OS lanes already, with no portability surface and no fixture directory. What this costs: the test fails if `include_str!` is ever replaced by a build-script-generated list — at which point the test is deleted along with the problem it guarded.
- **KTD8. The real-window FFI test stages its own minimal Win32 window from `crates/ffi/tests/common/`, backed by a target-gated `dev-dependency` on `windows-sys`.** The windows crate's `LocalFixture` cannot be reused across the crate boundary for two independent reasons: `tree::fixture` and `tree::fixture_window` are declared `#[cfg(all(test, target_os = "windows"))]` (`crates/windows/src/tree/mod.rs:37-50`), and `cfg(test)` is not set when a crate is compiled as a dependency, so those modules are not present in the rlib the FFI tests link at all; and every item in them is `pub(crate)`, so they would be unnameable even if compiled. `HostedFixture` fails a third way — it re-executes `current_exe()` looking for a test named `tree::fixture::tests::fixture_host_process_entry`, which does not exist in the FFI test binary. **Rejected: promoting those modules to `pub` and removing their `cfg(test)` gate.** That ships test-only window-creation code inside the production `agent-desktop-windows` library to satisfy a different crate's test. The FFI-local fixture is roughly 120 lines against 753 in the two files it would otherwise expose, needs a click counter neither of them has, and has a Cargo precedent (`crates/windows/Cargo.toml:34-52`) and an isolation precedent (`crates/ffi/tests/common/mod.rs:23-58`'s `IsolatedHome`, which already redirects `HOME` per test and is entered by `with_adapter`). What this costs: two small Win32 window fixtures exist in the workspace. They serve different crates and neither can see the other, which is the reason for the duplication rather than an argument against it.
- **KTD9. `publish-npm`'s asset check becomes an exact equality whose expected value is computed from the workflow's own matrices by U5's script, not a floor with a prose comment.** A `>= 8` floor passes for every count above 8, so it cannot detect the failure it exists to detect: a matrix leg that silently stopped producing an asset. **Rejected: raising the floor to 11.** What this costs: the check now fails when a target is added and the count is not updated — which is the point, and U5's script names the discrepancy and the correct number in its failure message.
- **KTD10. Release verification is a `workflow_dispatch` dry run on this branch plus a local install of the packed tarball against the artifacts that run produced.** `release.yml`'s build jobs are gated on `release_created`, so U3 adds a dry-run path that builds and uploads artifacts without creating a release or publishing — the `workflow_dispatch` inputs the workflow already accepts (`tag_name`, `version`, `publish_npm`, `publish_skills`) are the seam. The npm half installs the packed package with the downloaded artifact placed where postinstall expects it, exercising `validateArchive`, `installArchive` and the wrapper against a genuine release archive rather than a hand-made one. **Rejected: asserting the exit criterion as written and marking it verified at Phase 2 close.** That defers the only proof to a merge nobody will re-open this PR for.
- **KTD11. `list-surfaces`, notification management and cursor-overlay rendering are documented as unavailable on Windows; none is implemented here.** The first two are §2.14's by scope. The third is left as core's `Ok(())` default because the adapter's answer is discarded by the dispatch either way (see Problem Frame) — an override would be a change with no observable effect, and the change that *would* have an effect is a cross-platform contract decision U10 assigns to §2.15. **Rejected: overriding `update_cursor_overlay` on Windows to return `not_supported()`.** It looks like honesty and produces the same `ok: true` envelope, which is worse than leaving the default: a reader of `crates/windows` would believe the command now refuses. Governed by R20 and enforced by U7's capability-table test.

### Error and Disposition Mapping

| Situation | Reported as | Why |
|---|---|---|
| Windows install on an unmapped platform key (e.g. `win32-ia32`) | postinstall logs the unsupported platform and its key, exits 0 | An unmappable platform is not an install failure of a package that supports others; the wrapper then fails loudly on first invocation with the same key named |
| Release asset 404 during postinstall | existing download failure path: manual-download instructions with the constructed URL, `process.exitCode = 1` | Unchanged from macOS; U5's gate exists so this is a network fault rather than a naming defect |
| Archive listing does not match the platform's expected entries | `Release archive has unexpected entries: <listing>` | Unchanged shape; the expected set is now platform-derived (R8) |
| Child terminated by signal | wrapper exits non-zero, naming the signal | A signal-killed child today exits 0 (R9) |
| `list-surfaces`, notification commands on Windows | `PLATFORM_NOT_SUPPORTED` from core's trait default | Already correct; U7 documents it rather than changing it |
| `cursor-overlay enable` on Windows | `ok: true`, setting recorded, nothing rendered | Core's `Ok(())` default; documented, not changed (KTD11) |

### High-Level Technical Design

The distribution chain and where each requirement's proof attaches:

```mermaid
flowchart TD
    A["release.yml build matrix<br/>4 CLI targets"] -->|"agent-desktop-v{V}-{target}.tar.gz<br/>+ .sha256"| B["publish-github<br/>checksums.txt + attestation"]
    A2["release.yml build-ffi matrix<br/>6 FFI targets"] -->|"tar.gz (unix) / zip (win)"| B
    B --> C["GitHub Release assets"]
    C -->|"curl + checksums.txt"| D["npm postinstall<br/>TARGET_MAP -> tarball name"]
    D -->|"tar -tzf / -xzf"| E["npm/bin/agent-desktop.exe"]
    E --> F["bin/agent-desktop.js<br/>spawn + exit code"]

    G["U5 gate: names + count"] -.->|"asserts A's matrix == D's table"| A
    G -.-> D
    H["U9: workflow_dispatch dry run"] -.->|"produces real assets on this branch"| A
    H -.->|"feeds a local npm pack install"| D
    I["U1 probe area 25"] -.->|"A25-1/A25-2 justify tar.gz"| D
```

The two dotted paths are the entire answer to "how is this verified before publication": U5 proves the two ends of the chain agree, and U9 runs the chain's producing half for real and hands its output to the consuming half.

### Assumptions

- The repository is public, so `windows-11-arm` minutes are free and unlimited. If it were private, the lane still works and bills normally — nothing in the design changes, only the cost.
- `workflow_dispatch` on `release.yml` is available to the implementer on this branch. The workflow file is present on `main`, which is what makes a dispatch trigger selectable, and the branch selector then chooses this branch's file.
- No consumer today links the Windows FFI cdylib at build time. U6's addition of the import library is therefore purely additive; nobody's build breaks either way.
- Nothing this sub-phase adds enters the dependency graph. `aarch64-pc-windows-msvc` is a new triple for crates already in `Cargo.lock`, not a new crate, and `windows-sys 0.61` is already there as a dependency of `agent-desktop-windows` — so U6's dev-dependency adds a lock entry for no new package. `deny.toml` declares no `[graph] targets` restriction, so `cargo-deny` already resolves every target in the graph; the supply-chain lane needs no change and `scripts/check-release-consistency.sh`'s six-package version check is unaffected. This is stated because "does the new target get audited" is the obvious question and the answer is that there is nothing new to audit.

---

## Implementation Units

Rows are listed in dependency order; U-IDs are stable identifiers, not sequence numbers.

| U-ID | Title | Key files | Depends on |
|---|---|---|---|
| U1 | Probe area 25 — measure the packaging environment | `probes/windows/25-packaging/`, `probes/windows/FINDINGS.md`, `.github/workflows/windows-capability-probe.yml` | — |
| U2 | One platform table in the npm package | `npm/lib/platform.js`, `npm/package.json`, `npm/scripts/postinstall.js`, `npm/bin/agent-desktop.js` | — |
| U3 | Windows targets in CI and release, x64 and ARM64 | `.github/workflows/release.yml`, `.github/workflows/ci.yml` | U1 |
| U4 | Windows install path in postinstall and the wrapper | `npm/scripts/postinstall.js`, `npm/bin/agent-desktop.js` | U2, U1 |
| U5 | The asset-name and asset-count contract gate | `scripts/check-npm-package.js` | U2, U3 |
| U6 | Real-adapter FFI coverage on Windows | `crates/ffi/Cargo.toml`, `crates/ffi/tests/common/`, `crates/ffi/tests/c_abi_windows_live_round_trip.rs`, `.github/workflows/ci.yml`, `.github/workflows/release.yml` | — |
| U7 | The Windows skill package, its embedding, and the coverage test | `skills/agent-desktop-windows/`, `crates/core/src/commands/skills.rs`, `crates/core/src/commands/skills_tests.rs`, `skills/agent-desktop/SKILL.md` | — |
| U8 | README platform truth | `README.md` | U7 |
| U9 | Release dry run and local install proof | evidence only — no source change | U3, U4, U5 |
| U10 | `docs/phases.md` reads true | `docs/phases.md` | U1, U6 |
| U11 | Dogfood the shipped channels | `docs/dogfood-reports/2026-08-24-001-feat-windows-2-13-ffi-npm-release-dogfood.md` | U4, U6, U7 |

### U1. Probe area 25 — measure the packaging environment

- **Goal:** Record, as ledger rows against committed captures, the environment facts this plan's decisions rest on. Four KTDs cite measurements, and a KTD citing an unrecorded observation is indistinguishable from a KTD citing an assumption.
- **Requirements:** R22, and the evidence under KTD1, KTD5.
- **Dependencies:** none.
- **Files:** `probes/windows/25-packaging/01-archive-and-download-tools.ps1`, `probes/windows/25-packaging/02-npm-global-install.ps1`, `probes/windows/25-packaging/03-motw-and-execution.ps1`, `probes/windows/25-packaging/04-cdylib-artifacts.ps1`, `probes/windows/25-packaging/captures/`, `probes/windows/FINDINGS.md`, `.github/workflows/windows-capability-probe.yml`.
- **Approach:**
  1. `01-archive-and-download-tools.ps1` resolves `tar.exe` and `curl.exe` by absolute `System32` path and by `PATH` order, records each one's version banner, then performs a create/list/extract round trip of a gzip tarball whose single entry is a stand-in `agent-desktop.exe`, recording the listing and the extracted set. Rows `A25-1` (in-box tool presence and versions, with the `PATH` order relative to a Git-for-Windows install) and `A25-2` (gzip-tarball round trip succeeds through the in-box tool).
  2. `02-npm-global-install.ps1` records `npm prefix -g`, the generated shim set for a `bin` entry (`.cmd`, `.ps1`, extensionless), and whether a postinstall-shaped write into the installed package directory succeeds, repeated enough times to distinguish a reliable write from an antivirus-timing flake. Rows `A25-3` (global prefix and shim triad) and `A25-4` (package-directory write outcome over N iterations).
  3. `03-motw-and-execution.ps1` downloads a locally-built `agent-desktop.exe` through `curl.exe` to a scratch path, records whether a `Zone.Identifier` alternate data stream was attached, and records whether command-line invocation of that file runs (`version` returning a parseable envelope) without an interactive prompt. Row `A25-5`. This is the row KTD5 rests on, and it is measured rather than quoted because the vendor description covers GUI launch and this product is invoked from a shell.
  4. `04-cdylib-artifacts.ps1` builds `-p agent-desktop-ffi --profile release-ffi` and records the produced artifact set with sizes. Row `A25-6` (`.dll`, `.dll.lib`, `.dll.exp`, `.pdb` are all produced; the import library is 26 KB), which is the row U6's archive change cites.
  5. Cost baseline, row `A25-7`: time a full `npm pack` plus install-from-local-artifact cycle using the probe corpus methodology — discarded warm-up, min of seven, reported as min with median and max beside it (`A15-13`, applied in `A18-7`). This is the sub-phase's performance vehicle; `scripts/perf-baseline-compare.sh` is structurally macOS-bound and does not run here.
  6. Register area 25 in `.github/workflows/windows-capability-probe.yml` in the `paths` filter, as run steps invoking each script with `-Label ci`, and in the capture-upload glob — matching how area 24 is registered at lines 22, 111-121 and 186.
- **Patterns to follow:** `probes/windows/24-fixture-e2e/01-fixture-toolchain.ps1` for the script shape, `-Label` handling and capture emission; `probes/windows/common.ps1` for shared helpers. Probe scripts are organised by measurement area and are exempt from the 400-line cap. Windows PowerShell 5.1 only: no ternary, no `??`, no `?.`, and never assign to an automatic variable.
- **Test scenarios:**
  - `scripts/check-capture-redaction.ps1` passes over every new capture — shapes and counts only, never a title, path, pid, machine name, user name, or message text.
  - `probes/windows/13-ledger-check.ps1` passes, including its row-versus-capture content audit for every new row that quotes a `field: value` pair.
  - Each new row's quoted numbers match its cited capture leaf exactly.
- **Verification:** the area runs end to end on this machine and in the capability-probe workflow, and every KTD that cites `A25-*` cites a row that exists and reads true against its capture.

### U2. One platform table in the npm package

- **Goal:** Collapse the three hand-maintained platform mappings inside the npm package into one module, before a fourth platform is added to any of them.
- **Requirements:** R7.
- **Dependencies:** none.
- **Files:** `npm/lib/platform.js` (new), `npm/package.json`, `npm/scripts/postinstall.js`, `npm/bin/agent-desktop.js`, `scripts/check-npm-package.js`.
- **Approach:**
  1. `npm/lib/platform.js` exports one table keyed by `${platform()}-${arch()}` whose entries carry the Rust target triple, the installed binary filename, and the archive's expected entry set. It also exports the tarball-name template as a function of version and target, so the string is constructed in exactly one place.
  2. `postinstall.js` and `bin/agent-desktop.js` require it and delete their local maps. Neither gains behaviour in this unit — this is a pure extraction, and the platform set it exports is exactly the union of what the three copies contain today, so `darwin`/`linux` behaviour is unchanged and `win32` remains gated by `SUPPORTED_PLATFORMS` until U4.
  3. `npm/package.json`'s `files` array gains `lib`; `scripts/check-npm-package.js`'s exact packed-file list gains `lib/platform.js` and stays an exact list rather than becoming a prefix match.
- **Non-goals for this unit:** no Windows support is enabled here. Keeping the extraction and the behaviour change in separate commits is what makes a regression in the macOS install path attributable.
- **Patterns to follow:** the package has no dependencies and must keep none; `lib/platform.js` is plain CommonJS like its two consumers.
- **Test scenarios:**
  - `node scripts/check-npm-package.js` passes: the packed file list is exactly `bin/agent-desktop.js`, `lib/platform.js`, `package.json`, `scripts/postinstall.js`, and `unpackedSize` remains under the existing ceiling.
  - `bash scripts/ci-npm-wrapper-smoke.sh` passes unchanged on macOS CI — the wrapper resolves the same binary name for `darwin-arm64` and `darwin-x64` after the extraction as before it.
  - A unit-level assertion that the table's key set equals the union of the three pre-extraction maps, so the extraction cannot silently drop a platform.
- **Verification:** the macOS install path is byte-for-byte unchanged in behaviour while the mapping exists once, proven by the smoke script passing against a binary resolved through the new module.

### U3. Windows targets in CI and release, x64 and ARM64

- **Goal:** Produce, checksum and size-check a Windows CLI release asset for both Windows targets without changing anything about the macOS legs, and give `aarch64-pc-windows-msvc` a CI lane so it is validated rather than merely built.
- **Requirements:** R1, R2, R3, R4, R5, R16, R17, and the dry-run seam KTD10 needs.
- **Dependencies:** U1 (KTD1's archive-format evidence).
- **Files:** `.github/workflows/release.yml`, `.github/workflows/ci.yml`.
- **Approach:**
  1. Convert the `build` job's `strategy.matrix` from a bare `target` list to an `include` list carrying `target`, `runner`, and whether the macOS helper is built — the same shape `build-ffi` already uses (`release.yml:213-234`). Four legs: the two existing macOS targets on `macos-latest`, `x86_64-pc-windows-msvc` on `windows-latest`, `aarch64-pc-windows-msvc` on `windows-11-arm`. `runs-on` becomes `${{ matrix.runner }}`.
  2. Split the build, size-check and archive steps by `runner.os`, leaving the existing bash steps untouched behind `if: runner.os == 'macOS'` and adding `shell: pwsh` counterparts for Windows. The Windows build is `cargo build --locked --release -p agent-desktop --target ${{ matrix.target }}` and no helper. The Windows size check reuses the `Get-Item ... .Length` / `$limit = 15MB` form already proven at `ci.yml:502-512`.
  3. The Windows archive step runs the in-box `tar` to produce `agent-desktop-v${VERSION}-${target}.tar.gz` containing the single entry `agent-desktop.exe`, then writes `${TARBALL}.sha256` with `Get-FileHash -Algorithm SHA256`, lower-cased, in the two-space-separated `<hash>  <filename>` form `publish-github`'s `cat ./*.sha256 > checksums.txt` and postinstall's parser both expect (`postinstall.js:82-98`). The existing Windows FFI leg writes exactly this form at `release.yml:344-347` and is the pattern to copy.
  4. Before archiving, the Windows legs run the freshly built binary once — `agent-desktop.exe version` — and fail if it does not emit a parseable envelope. On the ARM64 leg this is R17's proof and the only place the ARM64 binary can be executed; running it on both legs keeps the step uniform rather than special-cased.
  5. Add the dry-run seam: a `workflow_dispatch` input that lets `build` and `build-ffi` run and upload artifacts while `publish-github`, `publish-npm` and `publish-skills` stay skipped. The workflow already carries `publish_npm` and `publish_skills` inputs, so this follows their shape and defaults to the current behaviour, leaving a normal release run identical.
  6. Update `publish-npm`'s asset check to the exact count KTD9 requires — after this unit and U6 the release carries 4 CLI tarballs, 4 FFI tarballs, 2 FFI zips and `checksums.txt`, so eleven — and replace the prose comment with the composition U5's gate recomputes.
  7. In `ci.yml`, add a fourth `platform-check` include entry — `platform: Windows ARM64`, `os: windows-11-arm`, `package: agent-desktop-windows` — so every PR compiles `aarch64-pc-windows-msvc` (`ci.yml:113-131` is already an include matrix, so this is one block and no restructuring).
  8. In `ci.yml`, add a `test-windows-arm` job on `windows-11-arm` running the Windows lane's unit suite — the isolated-`HOME` preamble, `cargo test --locked -p agent-desktop-core -p agent-desktop-windows --lib` under `AGENT_DESKTOP_LIVE_WPF: "1"`, and the profile-isolation guard that closes it. It does **not** duplicate the x64 lane's harness contract gates, fixture compile smoke, or release-binary build: those exercise scripts and toolchains, not the target architecture, and running them twice buys nothing. What this lane exists to answer is whether the adapter behaves the same on ARM64, and the unit suite with live staging is the shortest question that answers it.
- **Non-goals for this unit:** no change to `publish-github`'s upload globs or attestation step; `*.tar.gz` and `*.zip` already cover the new assets, and R3 holds by construction. No self-hosted runner is involved — `windows-e2e.yml` still has no registered runner and this unit does not change that.
- **Patterns to follow:** `release.yml:213-234` for the include-matrix shape and per-OS archive branching; `ci.yml:502-512` for the pwsh size check; `release.yml:344-347` for the pwsh checksum format; `ci.yml:287-411` for the Windows lane's `HOME`/`USERPROFILE` isolation preamble and live-staging opt-in, which the ARM lane copies rather than re-invents.
- **Test scenarios:**
  - `actionlint` passes over the modified workflow (already wired into `ci.yml`'s `fmt` job).
  - A `workflow_dispatch` dry run on this branch produces `agent-desktop-v<v>-x86_64-pc-windows-msvc.tar.gz` and `agent-desktop-v<v>-aarch64-pc-windows-msvc.tar.gz` with their `.sha256` files (U9 records the run).
  - Each Windows archive lists exactly one entry, `agent-desktop.exe`.
  - Each `.sha256` file's recorded hash matches the archive as downloaded, and its line format parses under `postinstall.js`'s `^([0-9a-fA-F]{64})\s+\*?(.+)$` regex.
  - The macOS legs' asset names, archive entry lists and `.sha256` contents are unchanged relative to the merge-base run.
  - A deliberately oversized Windows binary fails the size step (invert-verified by lowering the limit on a scratch run rather than by inflating the binary).
  - `platform-check` on `windows-11-arm` fails when `aarch64-pc-windows-msvc` does not compile — invert-verified by introducing a target-gated compile error on a scratch commit.
  - `test-windows-arm` runs the same unit suite the x64 lane runs and reports the same test count; a failure there is a real ARM64 finding, dispositioned under the dogfood rules rather than skipped.
- **Verification:** the release chain's producing half runs for real on this branch for both Windows targets, its output is the input U9 hands to the npm half, and ARM64 is compiled and exercised on every PR rather than first meeting a compiler at release time.

### U4. Windows install path in postinstall and the wrapper

- **Goal:** Make `npm install -g agent-desktop` install and run on `win32-x64` and `win32-arm64`, and correct the three shipped inaccuracies the Windows path exposes.
- **Requirements:** R6, R8, R9, R10.
- **Dependencies:** U2 (the single platform table), U1 (KTD1's evidence that the existing download and extraction path works in-box).
- **Files:** `npm/scripts/postinstall.js`, `npm/bin/agent-desktop.js`, `npm/lib/platform.js`.
- **Approach:**
  1. Add `win32-arm64` to the platform table with target `aarch64-pc-windows-msvc` and binary name `agent-desktop-win32-arm64.exe`; `win32-x64` already carries its entry. Give each `win32` entry an expected-archive-entry set of `['agent-desktop.exe']`, and each `darwin` entry `['agent-desktop', 'agent-desktop-macos-helper']`.
  2. Replace `SUPPORTED_PLATFORMS` with the table's own key set — a platform is supported when it has an entry, which removes the second gate that today contradicts the first. The unsupported-platform message names the resolved platform key and states which keys are supported, instead of the fixed "macOS only … coming in Phase 2" text.
  3. Drive `validateArchive` and `installArchive` from the entry's expected set rather than the literal `['agent-desktop', MACOS_HELPER_NAME]`, and make the helper install and the `chmodSync` calls conditional on that set containing a helper. Extraction and listing continue to use `tar`, unchanged.
  4. In `bin/agent-desktop.js`, forward signal termination: when `close` reports a non-null `signal`, exit non-zero and name the signal rather than exiting 0. Keep `spawn` with an argument array and `stdio: 'inherit'` — never a shell string, which is the quoting hazard on paths containing spaces.
  5. Correct the wrapper's binary-not-found message so it names the likely cause (`--ignore-scripts`, or a postinstall that failed) rather than only the missing path, which is the loud failure KTD3 accepts.
  6. Correct `promptSkillInstall` to advertise only skills that exist: after U7 that is `agent-desktop`, `agent-desktop-ffi` and, on Windows, `agent-desktop-windows`. Do not advertise `agent-desktop-macos` or `agent-desktop-linux`.
- **Patterns to follow:** the existing download, checksum-parse, hash-compare and atomic-install helpers (`postinstall.js:68-112`) are reused untouched — this unit changes what is expected, not how it is fetched or verified.
- **Test scenarios:**
  - On this Windows machine, `validateArchive` and `installArchive` run against the U9 dry-run archive place `agent-desktop-win32-x64.exe` into a scratch `bin/`, and `agent-desktop version` through the packed npm shim returns a parseable envelope (U9 owns the staging; this is the assertion it proves).
  - A tampered archive — one whose listing carries an extra entry — is rejected with `Release archive has unexpected entries`, on both `win32` and `darwin` expected sets.
  - A checksum mismatch is rejected before installation, and no partial binary is left in `bin/`.
  - `win32-ia32` (an unmapped key) logs the unsupported platform, names the key, and exits 0; the wrapper then fails on first invocation naming the same key.
  - A child killed by a signal produces a non-zero wrapper exit status; a child exiting `3` produces `3`. Invert-verified by restoring `process.exit(code ?? 0)` and watching the signal case report success.
  - `bash scripts/ci-npm-wrapper-smoke.sh` still passes on macOS CI.
- **Verification:** a real release archive produced by U3 installs and runs through the packaged wrapper on Windows, and the macOS path's behaviour is unchanged.

### U5. The asset-name and asset-count contract gate

- **Goal:** Make a disagreement between the npm package's platform table and the release workflow's build matrix fail a check, instead of failing a user after publication.
- **Requirements:** R11, R5.
- **Dependencies:** U2, U3.
- **Files:** `scripts/check-npm-package.js`.
- **Approach:**
  1. Parse `.github/workflows/release.yml` for the `build` job's matrix targets and the tarball template it constructs, and for the `build-ffi` matrix's targets and archive kinds.
  2. Assert every target in `npm/lib/platform.js`'s table appears among the CLI build matrix targets. A target npm can ask for that the release never builds is the exact failure this gate exists to catch.
  3. Assert the tarball name the package would construct for each of its targets equals the name the workflow constructs for that target, by comparing the two templates rather than two hand-written strings.
  4. Compute the expected release-asset count from the matrices — CLI archives, FFI archives, plus `checksums.txt` — and assert the number `publish-npm`'s check uses equals it.
  5. **The parse fails closed.** These rules read `release.yml` as text, as the script's existing `workflowViolations` already does — the package has no dependencies and adding a YAML parser to a 25 KB package to read four lines is not a trade worth making. Text-matching a matrix is brittle in one specific way: a restructured matrix stops matching, and a matcher that finds nothing must not be read as finding no violations. So every rule that extracts a set — matrix targets, the tarball template, the asset count — asserts it extracted a non-empty result first, and reports "could not locate the build matrix in release.yml" as a **failure** rather than silently passing. That inversion is itself one of the self-test's cases.
  6. Extend the script's existing `selfTest()` (`check-npm-package.js:76-133`) with fixtures for each new rule: a workflow text missing a target the table names, a divergent tarball template, a stale asset count, and a workflow whose matrix block has been restructured beyond recognition must each be caught, and a correct set must pass. The self-test drives the script's real functions, not a re-implementation of their patterns.
- **Patterns to follow:** `check-npm-package.js`'s existing `workflowViolations` / `selfTest` structure, which already reads `release.yml` as text and self-tests its own rules — this unit adds rules to a gate that already has the right shape.
- **Test scenarios:**
  - `node scripts/check-npm-package.js` passes on the finished tree.
  - Deleting the `x86_64-pc-windows-msvc` leg from the workflow's build matrix fails the gate, naming the target npm expects and the workflow does not build.
  - Changing the workflow's tarball template to `.zip` while the package still constructs `.tar.gz` fails the gate.
  - Adding a matrix leg without updating `publish-npm`'s count fails the gate, and the message states both the found and the expected count.
  - A `release.yml` whose build-matrix block has been restructured so no target is extracted **fails** the gate rather than passing it, and says it could not locate the matrix.
  - The self-test fails when any one of the new rules is disabled — invert-verified rule by rule.
- **Verification:** the gate runs where `check-npm-package.js` already runs — `ci.yml`'s macOS `test` job and `supply-chain.yml`'s `audit` job — so it fires on every PR on two runners, and each new rule has been observed failing.

### U6. Real-adapter FFI coverage on Windows

- **Goal:** Close the two real gaps in the FFI story on Windows: 182 lib tests that no Windows lane runs, and the absence of any test that drives the real adapter through the C ABI against a real window. Ship the ARM64 cdylib and the import library alongside.
- **Requirements:** R12, R13, R14, R15.
- **Dependencies:** none.
- **Files:** `.github/workflows/ci.yml`, `.github/workflows/release.yml`, `crates/ffi/Cargo.toml`, `crates/ffi/tests/common/win32_fixture.rs` (new), `crates/ffi/tests/common/mod.rs`, `crates/ffi/tests/c_abi_windows_live_round_trip.rs` (new).
- **Approach:**
  1. Add `-p agent-desktop-ffi` to the Windows lane's `--lib` invocation (`ci.yml:411`), so the crate's 182 lib tests execute on Windows. This is one line and is the largest single coverage gain in the unit.
  2. Add `[target.'cfg(target_os = "windows")'.dev-dependencies] windows-sys = { version = "0.61", features = [...] }` to `crates/ffi/Cargo.toml`, with the minimum feature set the fixture needs — `Win32_Foundation`, `Win32_System_LibraryLoader`, `Win32_UI_WindowsAndMessaging` — matching the version and target-gating precedent at `crates/windows/Cargo.toml:34-52`. It is a dev-dependency, so the shipped cdylib is unaffected, and the core-isolation check cannot see it: `ci.yml:177` runs `cargo tree --locked -p agent-desktop-core --edges normal,build`, which is both scoped to core and restricted to non-dev edges.
  3. `common/win32_fixture.rs` registers a uniquely-named window class, creates a top-level window with one `BUTTON` child, and pumps messages on its own thread, signalling readiness back through a channel before the test proceeds. Its `WM_COMMAND` / `BN_CLICKED` handler increments an atomic counter that the test reads directly — this counter is the independent observation R13 requires, and neither of the windows crate's fixtures has one. Teardown closes the window and joins the pump.
  4. `c_abi_windows_live_round_trip.rs` is `#![cfg(target_os = "windows")]`, enters `with_adapter` (which already redirects `HOME` per test through `IsolatedHome`, so the refmap lands in a temp directory and never in the developer's `~/.agent-desktop`), stages the fixture, calls `ad_snapshot` scoped to the fixture's own process, parses the returned envelope for the button's ref, calls `ad_execute_by_ref` with the click action, and then asserts **the fixture's click counter incremented** — not that the envelope said `ok`. It waits for the counter rather than sleeping a fixed span, so a loaded machine reports a real failure rather than a timing artifact.
  5. Stage `agent_desktop_ffi.dll.lib` beside the DLL in the Windows FFI archive (`release.yml`'s "Stage tarball contents" step), and extend the generated archive README's Windows branch to say that `dlopen` / `ctypes` callers need only the DLL while an MSVC-linking consumer needs the import library. `A25-6` records that the file is produced.
  6. Add `aarch64-pc-windows-msvc` on `windows-11-arm` to the `build-ffi` matrix with `archive: zip` and `lib_name: agent_desktop_ffi.dll`, matching the existing Windows leg.
- **Non-goals for this unit:** the fixture is a click target, not a second fixture app — no text fields, no secure fields, no modal or menu staging. If a later sub-phase needs those through the C ABI, it extends this file then.
- **Patterns to follow:** `crates/windows/src/tree/fixture_window.rs` for class registration, the ready-signal channel and the pump loop, none of which can be reused directly (KTD8) but all of which show the correct shape, including why the ready signal exists; `crates/ffi/tests/c_abi_windows_bootstrap.rs` for the `#![cfg(target_os = "windows")]` integration-test shape and for reaching `agent_desktop_windows`'s public items from an FFI test; `crates/ffi/tests/common/mod.rs:23-58, 219-227` for `IsolatedHome` and `with_adapter`.
- **Test scenarios:**
  - The round-trip test passes: the fixture's counter is 0 before the C ABI click and 1 after it.
  - Removing the `ad_execute_by_ref` call makes the test fail on the counter, not on an envelope field — invert-verified, and it is the assertion that distinguishes this test from the envelope-shape tests already present.
  - Snapshotting with no fixture staged does not accidentally satisfy the test: a run that fails to find the button's ref fails with a message naming what it searched for.
  - `cargo test -p agent-desktop-ffi --lib` passes on Windows (182 tests) and the lane runs it.
  - `cargo test -p agent-desktop-ffi --tests` still passes on Windows and on macOS, and `--features stub-adapter --test c_abi_passthrough` still passes on Linux.
  - `cargo build --locked --profile release-ffi -p agent-desktop-ffi` produces `agent_desktop_ffi.dll.lib`, and the dry-run Windows FFI zip contains it under `lib/`.
  - `cbindgen --verify` still reports no header drift, so the dev-dependency and the new test changed no exported symbol.
- **Verification:** the real `WindowsAdapter`, reached only through `extern "C"` entrypoints, performs an observation-to-action round trip whose effect is confirmed by the window under test rather than by the product's own report.

### U7. The Windows skill package, its embedding, and the coverage test

- **Goal:** Ship `skills/agent-desktop-windows/`, make the binary serve it, make its capability claims true, and make a future unwired skill file fail a test.
- **Requirements:** R18, R19, R20.
- **Dependencies:** none.
- **Files:** `skills/agent-desktop-windows/SKILL.md`, `skills/agent-desktop-windows/references/permissions-and-elevation.md`, `skills/agent-desktop-windows/references/chromium-and-electron.md`, `skills/agent-desktop-windows/references/troubleshooting.md`, `crates/core/src/commands/skills.rs`, `crates/core/src/commands/skills_tests.rs`, `skills/agent-desktop/SKILL.md`, `skills/agent-desktop/references/workflows.md`.
- **Approach:**
  1. `SKILL.md` follows the frontmatter and section shape of `skills/agent-desktop/SKILL.md:1-21` — `name`, `version`, `tags`, `requirements`, folded `description` — and opens with a capability table stating, per command group, what works on Windows and what returns `PLATFORM_NOT_SUPPORTED`. The three honest negatives are named explicitly: `list-surfaces`, the four notification commands, and cursor-overlay rendering (recorded as a session setting, not drawn).
  2. `references/permissions-and-elevation.md` covers what a Windows agent must know before it acts: UIA needs no special permission for same-integrity targets; UIPI blocks `SendInput` and `PostMessage` from Medium to High integrity and detection is a token-integrity read rather than a `SendInput` return value (`A9-2`, `A9-3`, `A20-1`); window activation is the measured exception that succeeds across that boundary (`A24-16`); the blocked-combo list and why `ctrl+alt+delete` is deliberately absent from it; protected process names; and the cross-process interaction lease that serialises interactive work.
  3. `references/chromium-and-electron.md` covers the settle behaviour an agent will otherwise misread as an empty tree: Chromium 138+ exposes UIA without a flag, but a first-contact read returns far fewer nodes than a settled one and the gap can take 10-25 seconds on a cold launch (`A1-4`, `A1-5`, `A16-11`), which is what `--timeout-ms` is for; a covered window can hold at first-contact counts indefinitely (`A1-6`); WPF read too early binds the wrong provider permanently and only re-resolution recovers it (`A1-7`); and content elements with no accessible name are currently unresolvable through the semantic tier (`A24-11`, §2.14's to fix).
  4. `references/troubleshooting.md` maps symptom to cause: an empty or tiny tree, a `PERM_DENIED` with an `E_ACCESSDENIED` detail, a COM apartment error, DPI-scaled coordinates, and the SmartScreen note from KTD5 with `A25-5`'s measured behaviour.
  5. Wire all four files into `crates/core/src/commands/skills.rs` as `include_str!` constants and a third `SKILLS` entry with canonical name `agent-desktop-windows` and the alias `windows`, embedded unconditionally per KTD6.
  6. Add the two tests KTD7 describes to `skills_tests.rs`: the derived coverage test that every `.md` under `skills/` appears as a path in `skills.rs`'s own source text, and a behavioural test that `list` names the new skill and `get --reference` returns each of its references.
  7. Update `skills/agent-desktop/SKILL.md`: add the Windows skill to the reference-files table, replace the macOS-only installation and permission prose with text that covers both platforms, and correct the Observation quick-reference so `list-surfaces` carries its macOS-only caveat.
  8. Update `skills/agent-desktop/references/workflows.md`, which is embedded in the binary as `SKILL_DESKTOP_REF_WORKFLOWS` and is therefore served to every agent on every platform. Its "First-Time Setup" section is macOS-only today — `agent-desktop permissions --request` followed by *"System Settings > Privacy & Security > Accessibility > enable your terminal"* (`workflows.md:9-18`) — so a Windows agent following the shipped guidance is sent to a menu that does not exist. Give the section a Windows branch, and add one Windows-specific workflow example, per the origin's Skill Update line. This file is the sharpest instance of the sub-phase's second theme: it is not merely absent on Windows, it is actively wrong there, and it ships inside the binary.
- **Non-goals for this unit:** no `references/windows.md` under the existing skill, and no change to the `cfg`-gating of `references/macos.md` (KTD6).
- **Patterns to follow:** `skills/agent-desktop/references/macos.md` for the per-platform reference voice and length; `skills/agent-desktop-ffi/` for a second top-level skill package's layout; `skills.rs:4-26, 92-107` for the embedding shape. `scripts/check-no-phase-references.sh` already scans `skills/`, so no plan or sub-phase id may appear in any of these files — probe row ids are exempt and are the correct way to cite evidence here.
- **Test scenarios:**
  - `agent-desktop skills list` names `agent-desktop-windows`; `skills get agent-desktop-windows` returns its body; `skills get agent-desktop-windows --reference permissions-and-elevation.md` returns that reference and an unknown reference name errors.
  - The coverage test fails when a new `.md` is added under `skills/` and not wired into `skills.rs` — invert-verified by adding a scratch file, watching the test fail, and removing it.
  - The coverage test fails when an existing `include_str!` line is deleted — invert-verified, since this is the regression it exists to catch.
  - A capability-claim test asserts that every command the Windows skill's capability table marks as working is one the Windows adapter implements, and that `list-surfaces` and the notification commands are marked unavailable. This is the test R20 needs, and it fails if the adapter later implements one without the document being updated.
  - No shipped skill document sends a Windows reader to a macOS-only affordance: a test asserts `workflows.md`'s setup section names both platforms, so the file cannot regress to single-platform instructions.
  - `bash scripts/check-no-phase-references.sh` passes with the new files in scope.
  - `bash scripts/link-skills.sh` picks up the new directory with no change to the script.
- **Verification:** the binary serves the new skill, its claims match the adapter under test rather than under description, and a future skill file that is committed but not embedded cannot pass CI.

### U8. README platform truth

- **Goal:** Make `README.md` state what is true about Windows for a reader deciding whether to install it.
- **Requirements:** R21.
- **Dependencies:** U7 (the capability table the README summarises).
- **Files:** `README.md`.
- **Approach:**
  1. Platform Support table (`README.md:412-423`): flip Accessibility tree, Click / type / keyboard, Mouse input, Screenshot, Clipboard, and App & window management to **Yes** for Windows. Notifications stays **Planned** — the four methods have no Windows override and §2.14 owns them.
  2. Installation: npm is the same command on every platform and needs no per-OS instruction beyond saying so; add direct `.exe` download from GitHub Releases naming the two Windows assets and the archive format KTD1 chose; add the from-source requirements for Windows (Rust plus the MSVC toolchain) beside the existing macOS line.
  3. Permissions: today the section is entirely macOS TCC. Add the Windows counterpart — UIA needs no grant for same-integrity targets, elevated targets need an elevated agent and the reason (UIPI), and the SmartScreen note from KTD5 with what a user will actually see.
- **Test scenarios:**
  - Every Windows row marked **Yes** corresponds to a `WindowsAdapter` method that exists, cross-checked against U7's capability table; every row still marked **Planned** corresponds to a trait default.
  - The named release assets match the names U3's matrix produces — the same equality U5's gate enforces mechanically, checked here by reading.
- **Verification:** a reader following the README's Windows path reaches a working install, and no row promises a command that returns `PLATFORM_NOT_SUPPORTED`.

### U9. Release dry run and local install proof

- **Goal:** Run the release chain's producing half for real on this branch, and feed its output to the consuming half, so the sub-phase's central claim is evidenced rather than asserted.
- **Requirements:** the pre-publication half of R1-R6 and R11; KTD10.
- **Dependencies:** U3, U4, U5.
- **Files:** none — this unit produces evidence, not source. Its results are recorded in the PR description and in U11's dogfood report.
- **Approach:**
  1. Dispatch `release.yml` on this branch in dry-run mode. Record the run URL, the artifact names, and each artifact's size and SHA-256.
  2. Download the `x86_64-pc-windows-msvc` CLI archive to this machine. Verify its checksum against the `.sha256` the workflow produced, list its entries, and confirm the single entry is `agent-desktop.exe`.
  3. Exercise the Windows-specific half of the install path against that genuine archive. `postinstall.js` has no download-base override — its only env seams are `AGENT_DESKTOP_SKIP_DOWNLOAD` and `AGENT_DESKTOP_BINARY_PATH` (`:217, :240`) — and **this unit does not add one**: an environment variable that redirects a download-and-execute is shipped attack surface bought to make a test convenient. So the proof is split along the line the code already draws. Drive `validateArchive` and `installArchive` directly against the downloaded archive with a scratch `binDir`, which is exactly the platform-dependent logic U4 changed, running against a real release artifact through the in-box `tar`.
  4. `npm pack` the package and install the resulting tarball globally on this machine with `AGENT_DESKTOP_BINARY_PATH` pointing at the binary step 3 installed, then run `agent-desktop version`, `agent-desktop snapshot --app <real app> -i`, and one interaction through the npm shim. This exercises packaging, the generated `.cmd` shim, the wrapper's binary resolution and its exit-code propagation against the real product.
  5. **State what this does not prove.** The `curl` fetch of a GitHub Release URL and of `checksums.txt` is not exercised here, because no Windows asset is published yet. That code is platform-agnostic, unchanged by this sub-phase, and exercised on every macOS install; the Windows-specific parts of the path — platform key resolution, expected-entry set, extraction, shim, wrapper — are all covered by steps 3 and 4. This split is recorded in the PR rather than glossed, because "verified end to end" and "verified except for the one step that needs publication" are different claims.
  6. Record the ARM64 archive's existence, size and checksum, and the ARM64 leg's `version` smoke output from the runner log. This machine is x64, so the ARM64 binary is not run here; R17 is satisfied on the runner and this unit records where.
  7. The origin's Release checklist also asks that the release notes document Windows support. Nothing in this PR writes that text: `release-please` composes the release body from Conventional Commit subjects, so the notes say what the commits say. Record in the PR description that this sub-phase's `feat:` subjects name Windows explicitly, which is the whole mechanism — there is no separate notes file to edit, and inventing one would add a surface the release pipeline does not read.
- **Test scenarios:**
  - The downloaded archive's SHA-256 matches its `.sha256` line, compared with the same lower-cased hex form postinstall's parser uses.
  - `validateArchive` accepts the real Windows archive and rejects it when a second entry is added to a copy.
  - `installArchive` places `agent-desktop-win32-x64.exe` in the scratch `binDir` and the installed file's hash matches the archive member's.
  - The npm-installed binary reports the same version the release was dispatched for.
  - A snapshot through the installed binary returns a tree with a non-zero `ref_count` against a real application.
- **Verification:** the exit criterion "release dry-run artifacts verified" is met with a named run and reproducible hashes; every Windows-specific step of the install path is exercised against a genuine release artifact; and the one step that cannot be reached before publication is named rather than implied.

### U10. `docs/phases.md` reads true

- **Goal:** Correct in place every statement this sub-phase's research disproved, and record what it shipped, so the next sub-phase's planner reads fact.
- **Requirements:** R23.
- **Dependencies:** U1, U6.
- **Files:** `docs/phases.md`.
- **Approach:**
  1. §2.13's FFI scope line is false in both clauses and is rewritten to state the two real gaps: the FFI crate's lib tests do not run on any Windows lane, and no test drives the real adapter through the C ABI against a real window. Cite the evidence — `ci.yml:411` and `ci.yml:689-713` — rather than annotating the history.
  2. §2.13's exit criteria are restated in the pre-publication form KTD10 establishes, so a later reader does not re-derive that the original is unfalsifiable on a sub-phase branch.
  3. §Release, Skill & Docs: tick what shipped, and correct the npm line, which reads as though only a `postinstall.js` branch is required — the archive-shape check, the platform-table extraction and the asset-name gate are part of making that true.
  4. §Release, Skill & Docs's FFI line already says the Windows cdylib ships and Phase 2 adds ARM64; confirm it reads true after U6 and correct it if not.
  5. Record the import-library omission and its fix, since it is a released-artifact contract change a consumer could depend on.
  6. Register the two capability facts this sub-phase documented but did not change, so §2.14 and §2.15 inherit them explicitly: `list_surfaces` has no Windows override, and `update_cursor_overlay` falls to core's `Ok(())` on Windows so `cursor-overlay enable` reports success while nothing renders. Write the cursor-overlay item into §2.15's scope, since no sub-phase claims it today and a documented no-op that reports success is exactly what a hardening review exists to settle.
- **Test scenarios:**
  - `pwsh scripts/check-phases-ledger-citations.ps1` passes.
  - Every `A25-*` row this plan cites exists in `probes/windows/FINDINGS.md` and reads true against its capture.
  - No statement in §2.13 contradicts what the PR shipped, checked line by line at review.
- **Verification:** the document a future planner treats as fact contains no clause this sub-phase's research disproved, and the two capabilities left unimplemented are owned by a named sub-phase rather than by nobody.

### U11. Dogfood the shipped channels

- **Goal:** Consume this sub-phase's own output the way a user and an embedder would, against real software, and dispose of every finding.
- **Requirements:** the cross-cutting dogfood gate; contributes evidence to R6, R13, R18.
- **Dependencies:** U4, U6, U7.
- **Files:** `docs/dogfood-reports/2026-08-24-001-feat-windows-2-13-ffi-npm-release-dogfood.md`, `docs/dogfood-reports/2026-08-24-001-captures/`.
- **Approach:**
  1. **The npm channel, as a user.** Install the packed package globally on this machine, then drive a real application — not the 2.12 fixture — entirely through the npm-installed binary: snapshot, find an element, act on it, read the effect back. Use an application with a genuinely awkward tree, which on this box means a Chromium or Electron host, since that is where the settle behaviour U7 documents actually bites.
  2. **The FFI channel, as an embedder.** Load `agent_desktop_ffi.dll` from a host language — Python `ctypes` is the shortest path and `tests/ffi-python/smoke.py` shows the shape — against the **real** adapter rather than the stub, and perform an observation-to-action round trip on a real window. This is the first time the real Windows cdylib is exercised from outside Rust, and it is the leg most likely to produce findings.
  3. **The documentation channel, as an agent.** Read `skills get agent-desktop-windows` from the built binary and follow its own instructions literally against a real application. An instruction that does not work as written is a finding, and this is the only way the document's accuracy gets tested by something other than its author.
  4. Judge every finding and give it exactly one disposition: *fixed here*, naming the test that fails without the fix and confirming the invert-verification was performed; *owned elsewhere*, written into the receiving sub-phase's scope in `docs/phases.md` in this same PR; or *accepted*, with the reason stated. "Recorded" is not a disposition.
- **Test scenarios:** the dogfood report is itself the artifact; each *fixed here* finding contributes a named regression test to the Verification Contract before this unit closes.
- **Verification:** a committed judged report exists, it contains findings, and every finding carries one of the three dispositions. A report with no findings is a failed dogfood and is re-scoped against harder targets rather than accepted.

---

## Verification Contract

Every requirement maps to at least one test that fails if the requirement is violated. Gates are package-scoped — bare and workspace `cargo` fail on this box.

| Requirement | Test that fails if violated | Unit |
|---|---|---|
| R1 | dry-run produces both Windows CLI archives under the shared name template; U5's gate fails if a table target has no matrix leg | U3, U5, U9 |
| R2 | Windows archive listing equals exactly `agent-desktop.exe`; `validateArchive` rejects any other listing | U3, U4 |
| R3 | each Windows `.sha256` parses under postinstall's regex and matches the downloaded archive | U3, U9 |
| R4 | Windows size step fails when the limit is lowered below the built binary | U3 |
| R5 | U5's gate fails when a matrix leg is added and `publish-npm`'s count is not updated | U5 |
| R6 | `npm pack` install on this machine yields a working `agent-desktop version` through the npm shim | U4, U9 |
| R7 | the table's key set equals the union of the three pre-extraction maps; a platform present in one consumer and not the other is unrepresentable | U2 |
| R8 | an archive with an unexpected entry is rejected on both the `win32` and `darwin` expected sets | U4 |
| R9 | a signal-killed child produces a non-zero wrapper exit status | U4 |
| R10 | postinstall's advertised skill names are a subset of the `skills/` directory names | U4, U7 |
| R11 | removing a Windows leg from the build matrix, or diverging the tarball template, fails `check-npm-package.js`; each rule fails when disabled | U5 |
| R12 | the Windows lane runs `-p agent-desktop-ffi --lib`; its absence is visible as 182 tests not executing | U6 |
| R13 | the fixture's click counter reads 1 after a C ABI click and 0 without one | U6 |
| R14 | the dry-run Windows FFI zip contains `agent_desktop_ffi.dll.lib` | U6, U9 |
| R15 | the `build-ffi` ARM64 Windows leg produces its zip in the dry run | U6, U9 |
| R16 | `platform-check` fails on `windows-11-arm` when `aarch64-pc-windows-msvc` does not compile; `test-windows-arm` fails when the unit suite does | U3 |
| R17 | the ARM64 leg's `version` smoke step fails if the built binary does not start | U3 |
| R18 | `skills list` / `get` / `get --reference` return the new skill and each reference | U7 |
| R19 | adding an unwired `.md` under `skills/`, or deleting an `include_str!`, fails the coverage test | U7 |
| R20 | the capability-claim test fails when a documented-working command is not implemented, or a documented-unavailable one is; the setup-section test fails when a shipped skill document names only one platform | U7, U8 |
| R21 | every **Yes** row corresponds to an implemented adapter method, checked against U7's table | U8 |
| R22 | `13-ledger-check.ps1` and `check-capture-redaction.ps1` pass over area 25 | U1 |
| R23 | `check-phases-ledger-citations.ps1` passes; no §2.13 clause contradicts what shipped | U10 |

**Invert-verification is required, not optional.** For each of the following, break the guarded line, watch the named test fail, restore it, and `touch` the file so the next `cargo` run does not reuse a stale binary:

1. R9's signal forwarding — restore `process.exit(code ?? 0)` and watch the signal case report success.
2. R11's three new gate rules — disable each in turn and watch `selfTest()` fail on that rule alone.
3. R13's independent observation — remove the `ad_execute_by_ref` call and watch the counter assertion fail, confirming the test does not pass on the envelope.
4. R19's coverage test — once by adding a scratch `.md` under `skills/`, once by deleting an existing `include_str!` line.
5. R20's capability-claim test — mark a documented-unavailable command as working and watch it fail.
6. R4's size ceiling — lower the limit on a scratch run and watch the Windows step fail.

**Gates.** The PR must pass, on this machine and in CI: `cargo fmt --all -- --check`; `cargo clippy --locked -p agent-desktop-core -p agent-desktop-windows -p agent-desktop -p agent-desktop-ffi --all-targets -- -D warnings`; `cargo test --locked -p agent-desktop-core -p agent-desktop-windows --lib`; `cargo test --locked -p agent-desktop-ffi --lib`; `cargo test --locked -p agent-desktop-ffi --tests`; `cargo test --locked -p agent-desktop`; `cargo check -p agent-desktop-core --all-targets --target x86_64-unknown-linux-gnu`; `bash scripts/check-rust-file-size.sh`; `bash scripts/check-no-phase-references.sh`; `bash scripts/check-release-consistency.sh`; `node scripts/check-npm-package.js`; `bash scripts/ci-npm-wrapper-smoke.sh` (macOS CI); `pwsh scripts/check-capture-redaction.ps1`; `pwsh scripts/check-phases-ledger-citations.ps1`; `probes/windows/13-ledger-check.ps1`; `actionlint` over the modified workflows; and `cbindgen --verify` header drift.

**Performance.** The vehicle is the probe corpus cost methodology, not `scripts/perf-baseline-compare.sh`, which is structurally macOS-bound. `A25-7` reports the pack-and-install cycle as min of seven with a discarded warm-up, median and max beside it (`A15-13`, applied in `A18-7`). No hot path changes in this sub-phase, so no adapter baseline is required.

---

## Definition of Done

1. A Windows CLI release asset exists for both Windows targets, produced by a dispatched run on this branch, with matching checksums and a single `agent-desktop.exe` entry, and the macOS assets are unchanged.
2. The 15 MB ceiling is enforced on both Windows release binaries by a check that ran on the Windows runner, and the ARM64 binary was executed on the ARM64 runner before it was archived.
3. `npm install -g` of the packed package installs and runs on this Windows machine against a real release archive, and the macOS install path is unchanged.
4. The npm package holds its platform mapping exactly once, and a disagreement between that table and the release matrix — in target set, name template, or asset count — fails `check-npm-package.js`, with each rule observed failing.
5. `cargo test -p agent-desktop-ffi --lib` runs on a Windows lane, and a test drives the real `WindowsAdapter` through the C ABI against a window it staged, confirming the click by the window's own state rather than by the returned envelope.
6. The Windows FFI archive carries the MSVC import library, the ARM64 cdylib is built and released, and `cbindgen --verify` reports no drift.
7. `aarch64-pc-windows-msvc` compiles on every PR and the Windows unit suite runs on a native ARM64 lane.
8. `skills/agent-desktop-windows/` ships, the binary serves it, every capability claim in it matches the adapter, the embedded `workflows.md` no longer sends a Windows agent to a macOS-only menu, and a skill document committed without being embedded fails a test.
9. `README.md`'s Platform Support table, installation and permissions sections state what is true on Windows, with Notifications still **Planned**.
10. Probe **area 25** is committed with rows written against their captures, and is registered in `.github/workflows/windows-capability-probe.yml` in both the `paths` filter and a run step, with its captures uploading from the CI run.
11. **Every `FINDINGS.md` row whose action column names this sub-phase is disposed of.** Verified at planning time: no row names 2.13, so the obligation is discharged by re-verification at close rather than by work.
12. **The dogfood gate, in its strict form:** a committed judged report driving real software through the npm channel, the FFI channel and the documentation channel; **a report with no findings is a failed dogfood** and is re-scoped rather than accepted; every finding carries exactly one of *fixed here* (naming an invert-verified test), *owned elsewhere* (written into that sub-phase's scope in `docs/phases.md` in this PR), or *accepted* (with a stated reason). **"Recorded" is not a disposition.**
13. Every requirement R1-R23 maps to at least one test that fails if it is violated, per the Verification Contract table, and every invert-verification listed there has been performed.
14. `docs/phases.md` reads true against what shipped: §2.13's false FFI clauses corrected, its exit criteria restated in pre-publication form, the Release/Skill/Docs checklist ticked where it shipped, and the two documented-but-unimplemented capabilities written into a named sub-phase's scope.
15. All gates green; zero `unwrap()`/`expect()` outside tests; no non-doc comments in `crates/**` or `src/**`; no file over 400 lines; no delivery-plan references in shipped source or in `skills/`; Conventional Commits authored by Lahfir with no co-authors.
16. The PR is opened against `feat/windows-adapter`, never `main`.

---

## LOC Budget

The origin estimates ~1.2k LOC. Counted the way this document's delivery model directs — hand-written product code, excluding committed evidence artifacts and the probe corpus:

| Area | Estimate | Counts against the cap |
|---|---|---|
| `release.yml` matrix, per-OS steps, dry-run seam | ~120 | yes |
| `ci.yml` ARM64 `platform-check` leg, `test-windows-arm`, and the FFI `--lib` addition | ~90 | yes |
| `npm/lib/platform.js` plus the two consumers' edits | ~150 | yes |
| `scripts/check-npm-package.js` rules and self-test | ~130 | yes |
| FFI Win32 fixture and round-trip test | ~250 | yes |
| `skills.rs` wiring and the two new tests | ~70 | yes |
| **Product code total** | **~810** | **yes** |
| `skills/agent-desktop-windows/` (4 documents) plus the `agent-desktop` skill's Windows branches | ~850 | documentation |
| `README.md`, `docs/phases.md` | ~90 | documentation |
| Probe area 25 (4 scripts) plus captures and rows | ~450 | evidence, exempt |

Product code lands well inside the ~2,000-line sub-phase guidance and slightly below the origin's estimate; the total diff is larger because the documentation *is* the deliverable here.

---

## Risks & Dependencies

- **`windows-11-arm` rolls to a VS2026-based image between 2026-09-21 and 2026-09-30.** A toolchain change on the ARM lane shortly after this lands is plausible. Mitigation: the lane runs on every PR (R16), so a break is caught by the next PR rather than at release time, and the failure is attributable to the image rather than to a code change.
- **The ARM64 unit lane is new territory and may surface real failures.** That is the point of R16 — §2.13's scope says ARM64 validation is no longer deferred. Any failure it finds is a 2.13 finding, dispositioned like any other; if a failure is genuinely an ARM64 platform limitation rather than a defect, it is *accepted* with its reason and written into §2.15.
- **The `workflow_dispatch` dry run depends on the workflow file's presence on `main` for the trigger to be selectable.** It is present. If dispatch is nevertheless unavailable to the implementer, the fallback is to run the same build and archive steps locally for `x86_64-pc-windows-msvc` and record that the ARM64 leg was not exercised — a materially weaker result that must be stated as such rather than presented as a passed gate.
- **Antivirus interference with a postinstall write into the global package directory is a known Windows failure mode** with no first-party fix. `A25-4` measures it on this box over repeated iterations. If it proves flaky here, the finding is *accepted* with the measured rate and documented in the troubleshooting reference, not papered over with a retry that hides it.
- **Depends on 2.2 through 2.11 for a working adapter** (as §2.13 states) and on 2.12 for the fixture-and-harness context the dogfood uses. It does not depend on 2.12.1, whose schema change is orthogonal to packaging.

## Open Questions

None. Every decision this sub-phase needs is settled in Key Technical Decisions, each against evidence cited above; the two capability gaps it deliberately does not close are assigned to named sub-phases in U10.

## Sources & Research

- `docs/phases.md` §Phase 2 sub-phase 2.13, §2.12.1, §Cross-cutting sub-phase DoD, §Release Skill & Docs, §Minimum OS Requirements, §New Dependencies, §Platform Delivery Model.
- Repository ground truth read directly at planning time: `.github/workflows/release.yml` (build and build-ffi matrices, staging, checksums, attestation, publish gating), `.github/workflows/ci.yml` (the `test-windows`, `platform-check`, `ffi-*` jobs and their exact cargo invocations), `.github/workflows/windows-capability-probe.yml` (area registration shape), `npm/package.json`, `npm/scripts/postinstall.js`, `npm/bin/agent-desktop.js`, `scripts/check-npm-package.js`, `scripts/check-release-consistency.sh`, `scripts/ci-npm-wrapper-smoke.sh`, `scripts/check-no-phase-references.sh`, `scripts/link-skills.sh`, `crates/ffi/Cargo.toml`, `crates/ffi/src/adapter.rs`, `crates/ffi/tests/common/mod.rs`, `crates/ffi/tests/c_abi_windows_bootstrap.rs`, `crates/core/src/commands/skills.rs`, `crates/core/src/adapter/{actions,input,observation,system}.rs`, `crates/windows/src/adapter.rs`, `crates/windows/src/system/adapter.rs`, `crates/windows/src/tree/{fixture,fixture_window}.rs`, `README.md`.
- Measured on this machine while planning: in-box `curl.exe` 8.9.1 and `tar.exe` (bsdtar 3.5.2) in `System32` with a successful gzip-tarball round trip; `cargo test -p agent-desktop-ffi` passing 182 lib and 91 integration tests against the real `WindowsAdapter` with no stub feature; `cargo build --profile release-ffi -p agent-desktop-ffi` producing `agent_desktop_ffi.dll`, `agent_desktop_ffi.dll.lib` (26,298 bytes), `.dll.exp` and `.pdb`. U1 converts each of these into a committed probe row rather than leaving it as a claim in this document.
- `probes/windows/FINDINGS.md`: `A1-4`, `A1-5`, `A1-6`, `A1-7`, `A4-4`, `A4-5`, `A9-2`, `A9-3`, `A10-3`, `A10-4`, `A14-10`, `A15-13`, `A16-11`, `A18-7`, `A20-1`, `A23-5`, `A24-11`, `A24-14`, `A24-15`, `A24-16` — cited by U7's references and by the performance methodology. **Verified at planning time: no row names 2.13**, so the cross-cutting row-disposition obligation is a re-verification at close, not work.
- External, verified 2026-08-24 with dates: GitHub Actions ARM64 hosted runners — public-preview 2025-04-14, GA for public repositories 2025-08-07, available in private repositories 2026-01-29, VS2026 ARM image GA 2026-08-20 with the `windows-11-arm` label rolling over 2026-09-21 to 2026-09-30 (GitHub Changelog). `aarch64-pc-windows-msvc` is a Tier 2 Rust target (rustc platform-support docs); the ARM64 MSVC build-tools component is reported missing on the VS2026 x64 image (`actions/runner-images#14215`), which is the cross-compilation risk KTD2 avoids. `actions/attest-build-provenance` v4.2.2 requires `id-token: write` and `attestations: write` and accepts both path separators on Windows. In-box `tar` and `curl` since Windows 10 build 17063. SmartScreen's interactive block is a Mark-of-the-Web plus GUI-launch behaviour and does not fire on command-line invocation; Azure Trusted Signing Basic is ~$10/month (Microsoft Learn, vendor writeups) — the evidence behind KTD5.
- **Institutional learnings applied.** `docs/solutions/best-practices/a-test-that-cannot-fail-is-not-coverage.md` (why the exit criteria are restated at KTD10, why the asset floor becomes an equality at KTD9, and the hand-maintained-parallel-list shape KTD4 removes); `never-ship-platform-code-that-ci-cannot-execute.md` (R16's ARM lane and R12's Windows FFI `--lib` addition); `an-enforcement-gate-must-cover-everything-the-binary-embeds.md` (the skill-embedding coverage test at KTD7); `a-verification-gate-is-code-and-needs-its-own-test.md` (U5's self-test extension and its per-rule invert-verification); `real-app-tests-are-the-platform-adapter-gate.md` (U11's shape and the independent-observation rule in R13); `fix-the-class-not-the-reported-instance.md` (why U2 extracts the mapping rather than adding one platform to three lists); `one-measurement-is-not-a-measurement.md` (`A25-4` and `A25-7`'s repetition methodology); `ffi-repr-c-struct-size-pinning.md` and `keep-ffi-action-policy-aligned-with-cli-2026-05-12.md` (checked and not triggered — this sub-phase adds no `repr(C)` struct and no FFI entrypoint, so neither obligation fires; stated because their absence should be a verified fact rather than an oversight).
