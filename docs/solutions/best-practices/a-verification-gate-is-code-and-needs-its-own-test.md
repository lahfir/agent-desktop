---
title: A verification gate is code and needs its own test
date: 2026-08-01
category: best-practices
module: scripts/check-no-phase-references.sh, scripts/check-rust-file-size.sh
problem_type: process_gap
component: tooling
symptoms:
  - "A pre-commit/CI gate step fails with 'Permission denied' the first time it runs on a fresh checkout."
  - "A gate that ran clean on the author's machine reports every version string (v0.5.0, pre-1.0, sub-1.0) as a violation on the CI runner."
  - "The exact pattern a gate exists to catch (phase-2.4) passes, while an innocent prose decimal (a 2.5 ms timeout) is flagged."
  - "A gate exits 0 on a machine where the interpreter it invokes does not exist, and a clean report is indistinguishable from a clean tree."
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
`scripts/check-no-phase-references.sh`. It carries a committed fixture:
`must_catch` lines including `phase-2.4` and `2.2 ships the seam`, `must_pass`
lines including `pre-1.0 and sub-1.0`, `v0.5.0`, `"2.1"` on the wire, `1.35x
against 0.80x`, `a 2.5 ms timeout`, and `uiautomation 0.25.0`, and two
wrapped-record fixtures whose phrase exists only across a doc-comment line
break. `self_test()` runs before any scan and fails the whole gate if any set
misbehaves.

The detail that makes this trustworthy rather than decorative: each rule is
declared exactly once, and the tree scan and the fixture both invoke that one
declaration. The gate has grown to four rule families — `token_rules()` for
ids and numbered slices, `BARE_REFERENCE_AWK` for a bare `2.4`, `BARE_UNIT_AWK`
for a bare `U4`, and `PLAN_PROSE_AWK` for authority constructions in prose
(`per the plan`, `the plan requires`, `exit criteria`) over a two-line window
that closes the wrap gap. `all_offences()` applies all four to a single line,
exactly as the real run applies them collectively to the tree, and both
fixture sets go through it. So every fixture line is checked against every
rule, and a new rule that false-fires on an old `must_pass` line is caught by
that old line. If a fixture instead embedded its own copy of a pattern, it
would verify that the copy matches the fixture's expectations of itself —
consistent, and worthless the moment the real rule drifts. Sharing the program
text makes the self-test exercise exactly what the real scan runs, so a
regression in the shipped rule is a regression the fixture can see.

## Portability

These gates look like they run everywhere, and they do not. Both
`check-no-phase-references.sh` and `check-rust-file-size.sh` are steps of the
macOS-only `Test` job in `.github/workflows/ci.yml` (lines 126-130); the Linux
and Windows CI lanes (`test-linux`, `test-windows`) never invoke them.
`.githooks/pre-commit` used to be the reason neither script ran on the
Windows dev box at all, back when its documented invocation there was
`SKIP_PRECOMMIT=1` and the hook exited at its first line before reaching
either one. Commit `adf2c36` restructured it: today the hook exits early only
on an explicit `SKIP_PRECOMMIT` or when nothing staged matches a Rust/FFI
path (lines 13-16, 58-73). Once a Rust change is staged, it computes a
per-OS `HOST_PACKAGES` scope from `uname -s` — the `HOST_KERNEL` case
matching `MINGW*|MSYS*|CYGWIN*|Windows_NT` for the Windows dev box (lines
84-100) — but that scope only ever changes which cargo packages the later
`clippy` and `test` steps build against; `check-rust-file-size.sh` and
`check-no-phase-references.sh` both run ahead of it, unconditionally, on
every host (lines 127-128). The two scripts are therefore no longer
"authored under GNU, executed only under BSD": the Windows dev box's own Git
Bash now runs them on every Rust commit, in the same GNU userland they are
authored and edited in, and the macOS CI job is the one place they still
meet a BSD `sed`/`grep` at all — which is why that job, and not the
pre-commit hook, remains the lane a GNU-only regex escape like the `\b` bug
depends on to be caught.

A shell gate is platform-conditional code, and it inherits the same
cross-platform rules as anything under
`crates/`: GNU and BSD userlands differ (`\b`, `sed -E` dialects), and the
difference fails *silently* — the gate keeps exiting 0 on the platform where
the feature works, or starts flagging everything on the platform where it
doesn't, and neither failure looks like a crash. Nobody reads a passing
gate's output closely, so the fix was to make the divergence a `self_test()`
failure, checkable on every run and on whichever userland is running it,
instead of discoverable only when a real commit happens to exercise the gap.

## Prevention

- A verification gate is production code for the invariant it protects. It
  needs the executable bit checked into the index (`git ls-files -s`, not
  `chmod` alone on Windows), and it needs its own tests, same as the code it
  gates.
- An untested gate is worse than no gate: it converts "we checked" into a
  false claim, silently, until the exact case it exists to catch slips
  through.
- Test the gate's rule *and* its ability to run at all. These are different
  failures and only the first announces itself. `scripts/check-rust-file-size.sh`
  hardcoded `python3`, which does not exist on the Windows dev box where the
  script is invoked by hand, so its comment half exited without checking
  anything and 27 violations across 7 files reached CI; probing more names
  then admitted a Python 2, whose
  `SyntaxError` would read as a violation rather than as an environment
  problem. The rule is **finding nothing is a failure, never a skip**:
  `require_interpreter()` tries `python3`, `python`, `py`, requires each to
  answer a version probe as 3 or higher, and fails the gate when none does —
  and `self_test()` drives that rejection branch, failing if the probe ever
  reports success without a working interpreter behind it.
- The general shape is **a check that runs, reports success, and asserted
  nothing**, and it is indistinguishable from a clean result at every level.
  The interpreter probe checking nothing is one face. Four probe harnesses
  under `probes/windows/` were another: a pass whose binary exited non-zero
  wrote a placeholder capture, the run reported ok, and the placeholder
  satisfied the workflow's artifact gate — the guard failed by going green.
  The measurement gate built to catch that was a third: with an empty
  `MandatoryExpected` set, a probe that never declared its captures had
  nothing to compare against, and `Get-MandatoryMeasurementGap` now reports
  `the probe declared no mandatory captures, so the run asserted nothing` as a
  failure in its own right. A fourth face is quieter than the other three: an
  env-var-gated test is a permanently disabled test unless something actually
  sets the variable. A live WPF zero-bounds regression was gated behind
  `AGENT_DESKTOP_LIVE_WPF`, set nowhere in the repo, its workflows, or its
  docs, so the body skipped on every run and reported green forever —
  indistinguishable, again, from a test that ran and found nothing wrong. The
  fix set the variable on the CI lane that owns staging the fixture
  (`.github/workflows/ci.yml:339-342`) and added
  `the_windows_lane_stages_the_live_wpf_host`
  (`crates/windows/src/tree/envelope_live_tests.rs:108-124`), which reads
  `ci.yml` itself and asserts the assignment sits on the step that runs the
  library tests, so the variable cannot silently vanish from the one place
  that makes it real. Recording that nothing was found is data; recording
  that nothing was looked at is not, and only the check itself can tell them
  apart.
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
