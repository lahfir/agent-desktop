---
title: Use explicit arms for string-keyed policy mirrors
date: 2026-07-11
category: best-practices
module: crates/core command policy
problem_type: best_practice
component: tooling
severity: high
applies_when:
  - "Mapping a command-name string to an action or policy"
  - "Adding a wait predicate that mirrors command behavior"
  - "Reviewing a catch-all branch in a policy mapping"
tags: [policy, match, actionability, wait, regression]
---

# Use explicit arms for string-keyed policy mirrors

## Context

The compiler cannot prove exhaustiveness when user strings select behavior.
A fallback that assigns a default policy can therefore become silently wrong
when a new action is added.

## Guidance

`commands/wait_predicate.rs::parse_actionability_action` names every supported
`--action` value and constructs the exact `ActionRequest` used for its
preflight. The final branch rejects unknown input; it must never infer a policy
for an unrecognized action.

For any similar mirror, use one explicit arm per supported value and tests that
pin both the accepted vocabulary and the policy-sensitive cases. Prefer a typed
enum and compiler exhaustiveness when the external string boundary can be
converted first.

## Prevention

- Reject unknown strings with `INVALID_ARGS` and an enumerated suggestion.
- Add a test before extending the accepted vocabulary.
- Do not use `_ => headless(...)` or another semantic default in a
  string-to-policy mapping.

## Related

- [Preserve command policy semantics during refactor](preserve-command-policy-semantics-during-refactor-2026-05-12.md)
- [Make FFI ref actions share CLI policy semantics](keep-ffi-action-policy-aligned-with-cli-2026-05-12.md)
