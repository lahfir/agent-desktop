# Release Automation & npm Distribution Brainstorm

Date: 2026-02-23

## What We're Building

An automated release pipeline that:
1. Uses **Conventional Commits** to determine version bumps (SemVer)
2. Uses **release-please** to create Release PRs with auto-generated CHANGELOGs
3. On Release PR merge: builds macOS binaries, creates GitHub Releases with tarballs, publishes to npm
4. Distributes via a **single npm package** (`agent-desktop`) with a postinstall script that downloads the correct platform binary from GitHub Releases

## Why This Approach

- **release-please** gives a natural gate (Release PR) before shipping, while automating CHANGELOG and version bumps
- **Single npm package + postinstall download** (like agent-browser) — one package to maintain, simpler publishing, npx support works out of the box
- **GitHub Release tarballs** serve dual purpose: npm postinstall downloads from them, and users can also download directly
- **Conventional Commits** already align with the project's imperative-mood commit style (`feat:`, `fix:`, `style:`, `docs:`)

## Key Decisions

### Versioning
- **SemVer** (`MAJOR.MINOR.PATCH`), starting from current `0.1.0`
- Single source of truth: `workspace.package.version` in root `Cargo.toml`
- release-please syncs version to both `Cargo.toml` and `package.json`

### Commit Convention
- `feat:` → minor bump
- `fix:` → patch bump
- `feat!:` or `BREAKING CHANGE:` footer → major bump
- `style:`, `docs:`, `refactor:`, `chore:`, `ci:` → no release (unless paired with feat/fix)

### npm Distribution Pattern (agent-browser style)

Single package: `agent-desktop`

```
npm install -g agent-desktop     # Global install
npx agent-desktop snapshot ...   # Quick use via npx
```

**How it works:**
1. User installs `agent-desktop` from npm
2. `postinstall` script detects platform (`darwin-arm64`, `darwin-x64`, etc.)
3. Downloads the matching binary from the GitHub Release (`v{version}/agent-desktop-{platform}`)
4. Places it in the package's `bin/` directory
5. A thin JS wrapper (`bin/agent-desktop.js`) resolves and spawns the native binary
6. On global installs, postinstall patches npm's symlink to point directly to the native binary (zero Node.js overhead)

**Fallback:** If postinstall download fails (corporate firewall, etc.), the JS wrapper shows a helpful error with manual download instructions.

### GitHub Actions Workflow (new: `release.yml`)

**Trigger:** Push to `main`

**Jobs:**
1. **release-please** — creates/updates Release PR, or creates a GitHub Release on PR merge
2. **build** (if release created) — matrix build for each platform target
   - `aarch64-apple-darwin` (macOS ARM)
   - `x86_64-apple-darwin` (macOS Intel)
   - Future: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`
3. **publish-github** — attach platform tarballs to the GitHub Release
4. **publish-npm** — sync version to package.json, build JS wrapper, `npm publish`

### File Structure (new files)

```
agent-desktop/
├── .github/
│   └── workflows/
│       ├── ci.yml                    # Existing (unchanged)
│       └── release.yml               # New: release-please + build + publish
├── .release-please-manifest.json     # Version tracking for release-please
├── release-please-config.json        # release-please configuration
├── npm/                              # npm package scaffolding
│   ├── package.json                  # name: "agent-desktop", bin, postinstall
│   ├── bin/
│   │   └── agent-desktop.js          # JS wrapper: detects platform, spawns native binary
│   └── scripts/
│       └── postinstall.js            # Downloads platform binary from GitHub Release
```

### Secrets Required
- `NPM_TOKEN` — npm publish token (stored as GitHub repo secret)
- `GITHUB_TOKEN` — already available in Actions (for release-please and GH Releases)

### Version Sync Strategy
- release-please bumps version in root `Cargo.toml` (`workspace.package.version`)
- CI script propagates the version to `npm/package.json` before publish
- `Cargo.lock` updated automatically by the version bump

### Installation Methods (at launch)

| Method | Command | Notes |
|--------|---------|-------|
| npm (global) | `npm install -g agent-desktop` | Recommended, postinstall downloads native binary |
| npx | `npx agent-desktop snapshot --app Finder -i` | Slightly slower (Node.js wrapper overhead) |
| yarn | `yarn global add agent-desktop` | Same as npm |
| bun | `bun install -g agent-desktop` | May need `--trust` for postinstall scripts |
| Direct download | Download from GitHub Releases | No Node.js required |
| Build from source | `cargo build --release` | For contributors |

## Open Questions

None — all key decisions resolved during brainstorm.
