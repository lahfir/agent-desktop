---
title: Bump envelope version when top-level response contracts change
date: 2026-05-13
category: best-practices
module: crates/core, src
problem_type: best_practice
component: output-contract
severity: high
applies_when:
  - Top-level JSON response fields change
  - Permission or status envelopes change shape
  - Error code semantics change for an existing command
  - A path that returned a structured error now returns success
  - A success payload gains an always-present field callers must read
  - CLI and batch output contracts are unified
tags:
  - json-contract
  - release-please
  - cli
  - batch
  - compatibility
---

# Bump envelope version when top-level response contracts change

## Guidance

Keep `ENVELOPE_VERSION` in `crates/core/src/response/output.rs` as the single source of truth. The envelope is how clients pick a parser branch. If it stays the same while top-level fields or error semantics change, pinned clients cannot tell an old response from a new one.

Bump it whenever a client that already parses successful JSON needs a different parser branch for the same command name. That covers:

- top-level response shape changes, and nested shape changes in always-present fields such as permissions or status
- command success becoming a structured error for a previously successful path
- a structured error becoming a success whose condition now has to be read from a field. A snapshot that exhausts its budget returns `ok: true` with `complete: false` instead of `TIMEOUT`, so a caller that branched on the error code to detect an oversized tree silently stops detecting it. `docs/json-output.md` shows this case.
- new required fields in success payloads, including an always-present field that is only meaningful in one state, such as `complete`
- error-code changes that callers reasonably branch on

Do not bump it for additive optional fields, new commands, new flags, or more specific human-readable messages.

## Blast Radius

`ENVELOPE_VERSION` owns the value, but the value and its semantics are restated by hand in non-test surfaces, and none of them is generated from the constant. Check each one in the same change and correct it where it is wrong: `docs/json-output.md`, the JSON placeholder in `.github/ISSUE_TEMPLATE/bug_report.yml`, the envelope examples and error prose in `CLAUDE.md`, and the rustdoc example in `crates/ffi/src/commands/snapshot.rs`. The header `crates/ffi/include/agent_desktop.h` is regenerated, not hand-edited.

`src/cli/doc_literal_drift_tests.rs` fails when a version literal in those files disagrees with `ENVELOPE_VERSION`. It does not check the prose around the literal, so a person still has to. A surface bumped to the new number while it still explains the old one is the failure this checklist exists to catch.

## Release Notes

Every envelope bump needs a `BREAKING CHANGE:` footer in the commit body. Do not hand-edit `CHANGELOG.md`. Make the footer precise enough for release-please to generate the note.

## Tests

Assert the version through `ENVELOPE_VERSION`, not string literals, so `main`, `batch`, the FFI JSON-command tests and the unit-level envelope tests stay aligned after a bump.
