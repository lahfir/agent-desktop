---
title: "feat: Automated releases with GitHub Releases and npm distribution"
type: feat
status: active
date: 2026-02-23
origin: docs/brainstorms/2026-02-23-release-automation-brainstorm.md
---

# feat: Automated releases with GitHub Releases and npm distribution

## Overview

Set up a fully automated release pipeline for agent-desktop: Conventional Commits determine SemVer version bumps, release-please creates gated Release PRs with auto-generated CHANGELOGs, merging the Release PR triggers macOS binary builds, GitHub Release creation with tarballs, and npm publication of the `agent-desktop` package with a postinstall binary downloader.

## Problem Statement

agent-desktop is currently install-from-source only (`cargo build --release`). There is no versioning strategy, no CHANGELOG, no GitHub Releases, and no package manager distribution. Users must have the Rust toolchain installed to use the tool. This limits adoption, especially for AI agent developers who may not have Rust set up.

## Proposed Solution

Follow the agent-browser pattern (see brainstorm: `docs/brainstorms/2026-02-23-release-automation-brainstorm.md`):

1. **Conventional Commits** → SemVer version bumps
2. **release-please** → Release PRs with CHANGELOG
3. **GitHub Actions** → build macOS binaries on Release PR merge
4. **GitHub Releases** → attach platform tarballs + SHA-256 checksums
5. **npm** → single `agent-desktop` package with postinstall binary downloader

## Technical Approach

### Architecture

```
Push to main (feat:/fix: commits)
    │
    ▼
release-please creates/updates Release PR
  (bumps Cargo.toml version, generates CHANGELOG)
    │
    ▼
Developer merges Release PR
    │
    ▼
release-please creates GitHub Release (tag: v0.2.0)
    │
    ▼
Build job (matrix: aarch64-apple-darwin, x86_64-apple-darwin)
  ├── cargo build --release --target <target>
  ├── tar + gzip binary
  └── generate SHA-256 checksum
    │
    ▼
Upload tarballs + checksums.txt to GitHub Release
    │
    ▼
Verify all assets present → npm publish --provenance
    │
    ▼
User: npm install -g agent-desktop
  ├── postinstall: detect platform → download binary from GH Release
  └── bin/agent-desktop.js: spawn native binary
```

### Implementation Phases

#### Phase 1: release-please + CHANGELOG

**Deliverables:**
- `release-please-config.json` — configuration targeting root `Cargo.toml`
- `.release-please-manifest.json` — version tracker (initial: `{"." : "0.1.0"}`)
- `.github/workflows/release.yml` — release-please job
- Cargo.lock auto-update step in Release PR

**Files to create:**
- `release-please-config.json`
- `.release-please-manifest.json`
- `.github/workflows/release.yml`

**Success criteria:**
- [ ] Pushing a `feat:` commit to `main` creates a Release PR
- [ ] Release PR bumps `workspace.package.version` in root `Cargo.toml`
- [ ] Release PR includes auto-generated CHANGELOG.md
- [ ] Cargo.lock is updated in the Release PR
- [ ] Merging the Release PR creates a GitHub Release with tag `v{version}`

**Key decisions (see brainstorm):**
- `feat:` → minor, `fix:` → patch, `feat!:` → major
- `style:`, `docs:`, `refactor:`, `chore:`, `ci:` → excluded from CHANGELOG, no release

#### Phase 2: Binary builds + GitHub Release assets

**Deliverables:**
- Build matrix in `release.yml` for macOS targets
- Tarball creation + SHA-256 checksums
- Asset upload to GitHub Release
- Verification step before npm publish

**Files to modify:**
- `.github/workflows/release.yml` (add build + upload jobs)

**Build matrix:**

| Target | Runner | Notes |
|--------|--------|-------|
| `aarch64-apple-darwin` | `macos-latest` (ARM) | Native build |
| `x86_64-apple-darwin` | `macos-13` (Intel) | Native build on Intel runner |

**Tarball naming:** `agent-desktop-v{version}-{target}.tar.gz`
- e.g., `agent-desktop-v0.2.0-aarch64-apple-darwin.tar.gz`
- Binary inside tarball is just `agent-desktop` (no platform suffix)

**Checksum file:** `checksums.txt` attached to the release, containing SHA-256 hashes for all tarballs.

**Success criteria:**
- [ ] Release creates tarballs for both macOS architectures
- [ ] `checksums.txt` is attached to the GitHub Release
- [ ] Binary size is under 15MB (enforced)
- [ ] Both binaries execute correctly on their respective platforms

**CI runner note:** Use `macos-13` for x86_64 (Intel runner) and `macos-latest` for aarch64 (ARM runner) to avoid cross-compilation issues.

#### Phase 3: npm package + postinstall

**Deliverables:**
- `npm/package.json` — package metadata, bin entry, postinstall script
- `npm/bin/agent-desktop.js` — JS wrapper that spawns native binary
- `npm/scripts/postinstall.js` — downloads platform binary from GitHub Release
- npm publish step in `release.yml`

**Files to create:**
- `npm/package.json`
- `npm/bin/agent-desktop.js`
- `npm/scripts/postinstall.js`

**npm package structure:**
```
npm/
├── package.json              # name: "agent-desktop", bin, postinstall
├── bin/
│   └── agent-desktop.js      # JS wrapper: platform detect → spawn binary
└── scripts/
    └── postinstall.js        # Download binary from GitHub Release
```

**package.json key fields:**
```json
{
  "name": "agent-desktop",
  "bin": { "agent-desktop": "./bin/agent-desktop.js" },
  "scripts": { "postinstall": "node scripts/postinstall.js" },
  "files": ["bin", "scripts"],
  "os": ["darwin"],
  "engines": { "node": ">=18" }
}
```

**postinstall.js behavior:**
1. Detect platform via `process.platform` + `process.arch`
2. Map to Rust target triple (`darwin`+`arm64` → `aarch64-apple-darwin`)
3. If unsupported platform → print friendly message, exit 0 (not error)
4. Download tarball from `https://github.com/lahfir/agent-desktop/releases/download/v{version}/agent-desktop-v{version}-{target}.tar.gz`
5. Download `checksums.txt`, verify SHA-256
6. Extract binary to `bin/agent-desktop-{platform-arch}`
7. `chmod 755` the binary
8. On global installs: attempt to patch npm's symlink for zero-overhead execution

**postinstall robustness:**
- Respect `HTTPS_PROXY` / `HTTP_PROXY` env vars
- Support `AGENT_DESKTOP_BINARY_PATH` override for pre-placed binaries
- Support `AGENT_DESKTOP_SKIP_DOWNLOAD=1` for offline environments
- 3 retries with exponential backoff (2s, 4s, 8s)
- 60-second timeout per download attempt
- Atomic file writes (download to `.tmp`, then `rename()`)
- Detect Bun (`process.versions.bun`) and suggest `--trust` if binary missing
- Print progress to stderr (not stdout, to avoid interfering with JSON output)

**bin/agent-desktop.js behavior:**
1. Detect platform, resolve binary path
2. If binary missing → clear error with recovery instructions
3. Ensure binary is executable (`chmod 755` if needed)
4. `spawn(binaryPath, process.argv.slice(2), { stdio: 'inherit' })`
5. Forward exit code

**Success criteria:**
- [ ] `npm install -g agent-desktop` works on macOS (ARM + Intel)
- [ ] `npx agent-desktop version` returns correct version
- [ ] `npm install -g agent-desktop` on Linux/Windows prints "macOS only" message and exits cleanly
- [ ] Binary integrity verified via SHA-256 checksum
- [ ] Postinstall handles network failures gracefully with retry
- [ ] Global install symlink optimization works (best-effort)

## Alternative Approaches Considered

1. **Per-platform npm packages** (`@agent-desktop/darwin-arm64`, etc.) — like esbuild/turbo. More complex: multiple packages to publish, npm org required, optionalDependencies wiring. Rejected in favor of the simpler single-package approach (see brainstorm).

2. **semantic-release** (fully automatic) — every merge to main auto-releases. No gate. Rejected because release-please's Release PR provides a natural review point (see brainstorm).

3. **cargo-release** — Rust-native tooling. More manual, requires running commands locally. Rejected for being less automated (see brainstorm).

## System-Wide Impact

### Interaction Graph

- Push to `main` → `release.yml` triggers → release-please creates/updates Release PR
- Release PR merge → release-please creates GitHub Release → build job → publish-github job → publish-npm job
- `ci.yml` (existing) is NOT modified — continues running on all pushes/PRs independently
- npm postinstall → HTTPS request to GitHub Releases CDN → binary placed in package

### Error Propagation

- Build failure → no tarballs uploaded → asset verification fails → npm publish blocked (safe)
- Postinstall download failure → JS wrapper detects missing binary → clear error message (graceful degradation)
- Version sync failure (Cargo.toml vs package.json) → CI script handles sync before `npm publish` (single source of truth is Cargo.toml)

### State Lifecycle Risks

- **Partial release:** GitHub Release created but binaries not yet uploaded. Mitigated by strict `needs:` job dependencies — npm publish waits for all assets.
- **npm publish without assets:** Mitigated by explicit asset verification step before publish.
- **Stale npm cache:** `npx` may cache old versions. Documented: use `npx agent-desktop@latest`.

### API Surface Parity

- No API changes. The binary's CLI interface, JSON output contract, and exit codes are unchanged.
- The `version` command output will reflect the new version number automatically (uses `env!("CARGO_PKG_VERSION")`).

## Acceptance Criteria

### Functional Requirements

- [ ] Conventional commits (`feat:`, `fix:`, `feat!:`) correctly determine version bumps
- [ ] release-please creates a Release PR with CHANGELOG on releasable commits
- [ ] Merging the Release PR creates a GitHub Release with tag `v{version}`
- [ ] macOS binaries (ARM + Intel) are built and attached as tarballs
- [ ] SHA-256 checksums are attached to the release
- [ ] `npm install -g agent-desktop` downloads and installs the correct binary
- [ ] `npx agent-desktop version` works
- [ ] `agent-desktop` (after global install) runs with zero Node.js overhead
- [ ] Non-macOS platforms get a friendly "not supported yet" message on npm install

### Non-Functional Requirements

- [ ] Binary size under 15MB (existing CI check)
- [ ] Postinstall download completes within 60 seconds on reasonable connections
- [ ] Postinstall respects proxy environment variables
- [ ] npm publish includes Sigstore provenance attestation (`--provenance`)
- [ ] GitHub Actions permissions are minimal per job

### Quality Gates

- [ ] Existing CI (`ci.yml`) continues to pass — no modifications to it
- [ ] `cargo clippy --all-targets -- -D warnings` passes
- [ ] `cargo test --lib --workspace` passes
- [ ] Manual end-to-end test: merge a Release PR → verify npm install works

## Dependencies & Prerequisites

- **npm account** with publish access to the `agent-desktop` package name (must be available)
- **NPM_TOKEN** GitHub secret configured for the repository
- **Conventional commit discipline** — all future commits to `main` should follow the convention
- No new Rust dependencies required
- Node.js 18+ for the postinstall/wrapper scripts

## Risk Analysis & Mitigation

| Risk | Severity | Mitigation |
|------|----------|------------|
| npm package name `agent-desktop` taken | Critical | Check availability first. Fallback: `@agent-desktop/cli` |
| Binary download blocked by firewall | Important | Proxy support, `AGENT_DESKTOP_BINARY_PATH` override, manual download docs |
| release-please workspace version handling | Important | Test with dry run, use `extra-files` config if needed |
| Race: Release exists before binaries uploaded | Important | Strict `needs:` job deps + asset verification before npm publish |
| Broken version published to npm | Important | `npm deprecate` + quick patch release procedure |
| macOS Gatekeeper warnings on unsigned binaries | Important | Document `xattr -cr` workaround; defer code signing to later |
| Cargo.lock out of sync in Release PR | Important | Auto-update step: `cargo update --workspace` in Release PR |
| CI token cannot trigger workflows on Release PR | Important | Use PAT or GitHub App token for release-please |

## Future Considerations

- **Phase 2 platforms:** Add Linux and Windows targets to the build matrix. Extend postinstall platform mapping. Consider musl vs glibc for Linux. Add Windows `.exe` handling.
- **macOS code signing + notarization:** Apple Developer ID for signed binaries. Eliminates Gatekeeper warnings.
- **Homebrew tap:** Leverage GitHub Release tarballs to create a Homebrew formula.
- **Prerelease channel:** `0.3.0-rc.1` releases via release-please prerelease config.
- **crates.io publishing:** Publish `agent-desktop-core` to crates.io for downstream Rust consumers.

## Sources & References

### Origin

- **Brainstorm document:** [docs/brainstorms/2026-02-23-release-automation-brainstorm.md](docs/brainstorms/2026-02-23-release-automation-brainstorm.md) — Key decisions carried forward: SemVer + Conventional Commits, release-please for gated releases, single npm package with postinstall download (agent-browser pattern), GitHub Release tarballs for non-npm distribution.

### Internal References

- Existing CI: `.github/workflows/ci.yml`
- Version source: `Cargo.toml:10` (`workspace.package.version`)
- Version command: `crates/core/src/commands/version.rs`
- Rust targets: `rust-toolchain.toml`
- Release build profile: `Cargo.toml:22-28`

### External References

- [release-please documentation](https://github.com/googleapis/release-please)
- [release-please Cargo/Rust support](https://github.com/googleapis/release-please/blob/main/docs/customizing.md)
- [agent-browser npm distribution pattern](https://github.com/vercel-labs/agent-browser) — reference implementation for postinstall binary download
- [npm provenance attestations](https://docs.npmjs.com/generating-provenance-statements)
