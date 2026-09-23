---
module: agent-desktop
date: 2026-09-20
problem_type: runtime_error
tags: [acceptance, reliability, numbers, headless]
---

# Acceptance audit after the shared reliability repairs

The full requested outcome is not achieved. The earlier 8/10 judgment has been
withdrawn. No measured overall score exists under the evaluation plan. The
frozen trajectory is [acceptance v1](../../../tests/real-apps/acceptance-v1.md).

| Requirement | Current evidence | Verdict |
|---|---|---|
| N1 Numbers discovery | Cold-start skeleton now exposes table and scoped cell find | Improved; repeated acceptance missing |
| N2 exact cell selection | Public AXPress and raw selected-cell evidence | Bounded success; earlier failures retained |
| N3 saved cell ref through edit | Original `@s2afdbd2ja0t3j:e1` currently returns STALE_REF after editor opens | Unmet |
| N4 text/numeric commit | Public editor setter and raw AX setter both report native success with unchanged seed-target | Unmet |
| N5 formula | No working text commit path established | Not run to completion |
| N6 duplicate-name sheets | Scoped transitions verified; repeated runner also retained a failed transition | Incomplete |
| N7 dimensions | Earlier document oracle verified row count changes and guard preservation | Bounded success only |
| N8 Numbers persistence | No successful edited values/formula to persist | Unmet |
| Cross-command consistency | Live get, canonical find names and root viewport repaired | Tested cases pass; not all commands proven |
| Scrolling and selection | SF Symbols saved-ref scroll cycles and named category selection verified | Bounded success only |
| Menu headlessness | Xcode effects observed; SF Symbols trial included target activation | No universal headless pass |
| Drag completion | Headed-only refusal is correct policy behavior | Requested headless task unmet |
| Other complex native app | Xcode text replacement/insertion and persisted file verified | Does not replace Numbers requirements |
| Thirty runs across three resets | Not completed for all supported cases | Incomplete |
| Latest code gates | Workspace, Clippy, fmt, native semantic seven assertions, live paired reads | Pass at window-context candidate |
| Latest performance | Candidate observations succeed on Numbers and SF Symbols; partial deep trees differ | Bounded comparison, not full task acceptance |
| Release/push | No publishing performed | Local branch only |

Numbers cell ref identity needs additional stable evidence before relaxing any
matching rule. The cell has mutable title content and an editing suffix; row and
column ranges exist in raw AX but are not currently stored as table-scoped ref
identity. Globally ignoring cell names, using coordinates alone, or stripping an
English suffix would risk wrong-target resolution across sheets and tables.

Numbers text mutation is separately unresolved at the native boundary. A stable
cell identity repair would not make the no-op editor setter commit text. Retain
both failures instead of counting one repair as completion of the other.
