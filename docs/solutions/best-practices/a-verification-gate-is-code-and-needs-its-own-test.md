---
title: A verification gate is code and needs its own test
date: 2026-08-01
category: best-practices
module: scripts/check-no-phase-references.sh
problem_type: process_gap
component: tooling
symptoms:
  - "A pre-commit/CI gate step fails with 'Permission denied' the first time it runs on a fresh checkout."
  - "A gate that ran clean on the author's machine reports every version string (v0.5.0, pre-1.0, sub-1.0) as a violation on the CI runner."
  - "The exact pattern a gate exists to catch (phase-2.4) passes, while an innocent prose decimal (a 2.5 ms timeout) is flagged."
root_cause: process_gap
resolution_type: added_self_test_and_shared_program_text
severity: high
tags: [ci, pre-commit, shell-portability, gnu-vs-bsd, self-test, tooling]
---

# A verification gate is code and needs its own test

## Problem

The Windows vocabulary work (2026-08-01) added `scripts/check-no-phase-references.sh`, wired into the
pre-commit hook and CI, to fail the build when `crates/` or `src/` references
the delivery plan — phase numbers, sub-phase numbers, `KTD<n>` decision ids.
Before it could be trusted, it broke four separate ways, each on first
contact with a different part of the environment:

1. It could not run at all.
2. It ran, but its own detection logic silently stopped working on macOS.
3. It ran on both platforms, but its allow/deny set was wrong in both
   directions.
4. It was correct, and failed CI anyway — on the lint applied to the gate
   itself.

Every one of these shipped, passed review, and was only found because the
gate's own next change exposed it — not because anything was watching the
gate itself.

## Root cause

**It could not run.** Commit `44cfa60` added the script at file mode
`100644`. Commit `084b078` fixed it: *"The mode bit did not reach git - chmod
on the Windows dev box does not set it - so the macOS lane failed with
'Permission denied' the first time the new step ran."* `chmod +x` on a
Windows checkout never touches git's index; the fix has to be
`git update-index --chmod=+x`. The file is `100755` today
(`git ls-files -s scripts/check-no-phase-references.sh`).

**It ran only on one platform.** The bare-version-number check used `\b`
word-boundary regexes in `sed`/`grep`. `\b` is a GNU extension; BSD `sed` and
`grep`, which is what the macOS runner ships, silently treat it as a no-op
rather than erroring. Commit `b709a3e`: *"the version-number stripping never
fired and every v0.5.0, pre-1.0 and sub-1.0 in a doc comment was reported as
a plan reference. It passed on the Windows dev box and failed the only lane
that runs it."* The fix replaced boundary-matching with tokenising in `awk`
— `BARE_REFERENCE_AWK` splits the line into whitespace tokens after
stripping known-safe shapes, and checks each token against `^[0-9]\.[0-9][0-9]?$`,
which needs no boundary support and behaves identically under GNU and BSD.

**Its allow/deny set was wrong in both directions.** The awk rewrite's first
strip was `gsub(/-[0-9]+\.[0-9]+/, " ", text)` — any hyphen followed by
`digit.digit`. Commit `635a537`: *"the gate's own allow/deny set was untested
and empirically wrong both ways: 'phase-2.4' escaped, because the
hyphenated-decimal strip swallowed exactly the thing the check exists to
catch, and 'a 2.5 ms timeout' false-fired."* The current script
replaces the blanket strip with named idioms — `(pre|post|sub|over|under)-N.N`
for `pre-1.0`/`sub-1.0`, and a unit-suffixed strip for `N.N` followed by
`x|ms|s|us|%|MiB|MB|KB|GB` — so `phase-2.4` no longer matches any strip and
survives to be caught, while `2.5 ms` and `1.35x` are named and removed.

**It failed the lint that runs over it.** The repo's Format lane shellchecks
every script under `scripts/`, and `shellcheck` reports SC2016 on the
single-quoted `BARE_REFERENCE_AWK` — "expressions don't expand in single
quotes." The quoting is not a mistake: `$0` inside the program is awk's
whole-line variable and must reach awk unexpanded, so double-quoting it would
have the shell substitute the script's own name into the program text and
break the gate outright. The fix is a scoped `# shellcheck disable=SC2016`
carrying that reason. Worth stating plainly because the reflex is the other
one: a gate that "just checks a string" invites silencing the linter globally
or rewriting correct code to appease it, and both are how a gate stops being
trustworthy.

## Solution

The fix that actually closes the loop is not the awk rewrite by itself, it's
the `self_test()` function added alongside it in
`scripts/check-no-phase-references.sh`. It carries a committed fixture: four `must_catch` lines
including `phase-2.4` and `2.4 ships the seam`, and six `must_pass` lines
including `pre-1.0 and sub-1.0`, `v0.5.0`, `"2.1"` on the wire, `1.35x
against 0.80x`, `a 2.5 ms timeout`, and `uiautomation 0.25.0`. `self_test()`
runs before `bare_reference_check()` and fails the whole gate if either set
misbehaves.

The detail that makes this trustworthy rather than decorative:
`BARE_REFERENCE_AWK` is declared once, as a single shell variable (line 65),
and both `bare_reference_check()` (line 87) and `self_test()` (lines 114 and
121) invoke `awk "$BARE_REFERENCE_AWK"` against it. Neither reimplements the
rule. If the fixture instead embedded its own copy of the pattern, it would
verify that the copy matches the fixture's expectations of itself —
consistent, and worthless the moment the real rule drifts. A shared variable
makes the self-test exercise the exact program text the real scan runs, so a
regression in the shipped rule is a regression the fixture can see.

## Portability

A gate that runs on more than one OS in CI is itself platform-conditional
code, and it inherits the same cross-platform rules as anything under
`crates/`: GNU and BSD userlands differ (`\b`, `sed -E` dialects), and the
difference fails *silently* — the gate keeps exiting 0 on the platform where
the feature works, or starts flagging everything on the platform where it
doesn't, and neither failure looks like a crash. Nobody reads a passing
gate's output closely, so the fix was to make the divergence a `self_test()`
failure, checkable on every run and every platform, instead of discoverable
only when a real commit happens to exercise the gap.

## Prevention

- A verification gate is production code for the invariant it protects. It
  needs the executable bit checked into the index (`git ls-files -s`, not
  `chmod` alone on Windows), and it needs its own tests, same as the code it
  gates.
- An untested gate is worse than no gate: it converts "we checked" into a
  false claim, silently, until the exact case it exists to catch slips
  through.
- Give every non-trivial gate rule a committed MUST-CATCH / MUST-PASS
  fixture, and make the fixture invoke the same program text as the real
  scan — a shared variable or function, never a duplicated pattern — so the
  self-test cannot pass by testing a copy that has drifted from the rule.
- Shell portability bugs in tooling are cross-platform bugs like any other:
  verify the primitive on every OS the gate runs on before trusting its
  output, per
  [Never ship platform code that CI cannot execute](never-ship-platform-code-that-ci-cannot-execute.md).
- A gate that compiles/runs is not a gate that fires on the target it's
  meant to guard — see
  [A test that cannot fail is not coverage](a-test-that-cannot-fail-is-not-coverage.md)
  for the sibling failure mode, where the lane's own flags excluded the file
  the guard lived in.
