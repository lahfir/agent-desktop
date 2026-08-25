# Residual Review Findings

- **Run:** ce-code-review `20260825-154618-a7f3` (mode:agent, plan: `docs/plans/2026-08-24-001-feat-windows-2-13-ffi-npm-release-plan.md`)
- **Scope:** `feat/windows-2.13-ffi-npm-release` vs merge-base `2b402a0`
- **Reviewers:** correctness, project-standards, testing, maintainability, agent-native, learnings, reliability, adversarial (in-process; cross-model peer skipped - host serving family unattestable)
- **Validation:** one batch of 6 selected findings -> 5 validated, 1 invalidated and dropped (#5 self-test fixtures: the pwsh `.zip` fixture already exercises the Windows fail-closed branch)
- **Applied in `fix(review): apply review findings` (de719c7):** #2 manual-fallback placement names, #3 win32 rmSync guard, #4 checkout-ref consolidation (verified green by dry run 32905878228), #6 promptSkillInstall executed instead of source-scraped
- **PR carrying this change:** https://github.com/lahfir/agent-desktop/pull/146 (back-filled into the tickets below)

## Filed (unapplied decision-gate findings)

| Sev | File:line | Title | Ticket |
|---|---|---|---|
| P1 | .github/workflows/release.yml:566 | Release re-dispatch clobbers published assets behind green gates | [#141](https://github.com/lahfir/agent-desktop/issues/141) |
| P3 | .github/workflows/release.yml:530 | FFI panic-boundary proof covers only macOS of six release legs | [#142](https://github.com/lahfir/agent-desktop/issues/142) |
| P3 | npm/scripts/postinstall.js:276 | npm install fast path skips verification of existing binaries | [#143](https://github.com/lahfir/agent-desktop/issues/143) |
| P3 | src/cli/windows_capability_claims_tests.rs:5 | Capability-table drift passes outside six pinned refusals | [#144](https://github.com/lahfir/agent-desktop/issues/144) |

## No sink (carried here verbatim)

Agent-native warnings (report-only, corroborated by testing/adversarial observations):

- P3 - `npm/package.json:3,24-32` - install-decision metadata still claims macOS only (`description`, `keywords` lack windows/uia). An agent or registry search filtering on Windows automation will not surface the package.
- P3 - `skills/agent-desktop/SKILL.md:49` - the served main skill advertises `references/macos.md`, which non-macOS builds refuse to serve (`#[cfg]`-gated out of the SKILLS table); annotate the row or ungate the content.
- P3 - held-input fail-closed row and `wait --notification` unavailability are documented but not probed against the adapter refusals (extending the existing `refusals` array pattern would close it).
- P3 - README trust steps name POSIX tools (`sha256sum`); stock Windows PowerShell has neither it nor `gh attestation verify` preinstalled - add a `Get-FileHash` variant.

Residual risks accepted with the run (full lists in `<run-dir>/review.json`):

- Parity job compares summed counts, not suite identity; identical silent skips on both Windows images pass by design.
- checksums.txt CRLF/LF textual coupling between workflow Out-File legs and postinstall's parser.
- check-npm-package.js YAML-as-text parsers fail closed on valid reformats (availability churn, not integrity loss).
- Dry run cannot exercise checksums aggregation, EXPECTED_ASSETS, and attestations until the first real release after this matrix change.

Failed: none. All four tracker filings succeeded on first attempt.
