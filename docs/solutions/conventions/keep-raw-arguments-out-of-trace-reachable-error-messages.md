---
title: Keep raw caller arguments out of trace-reachable error messages
date: 2026-07-01
category: conventions
module: crates/core, crates/macos
problem_type: convention
component: tooling
severity: high
applies_when:
  - Building an AdapterError or AppError message that can surface through command.end or actionability.check.error
  - Adding or changing any error message in an action path (select, set-value, wait, click)
  - Adding a new trace event field that carries user- or app-derived text
related_components:
  - trace_sanitize
  - trace
  - context
tags:
  - tracing
  - redaction
  - privacy
  - error-messages
  - sanitization
---

# Keep raw caller arguments out of trace-reachable error messages

## Context

Trace redaction (`sanitize_trace_value` in `crates/core/src/trace_sanitize.rs`) is a field-**name** allowlist: it redacts values whose key matches `SENSITIVE_KEYS` (`text`, `value`, `name`, `title`, `selector`, …) and passes everything else through verbatim. It does not scan free-text string values for embedded secrets.

The `message` field on trace events is deliberately not in that allowlist — it is meant to be diagnostic free text. The trap: several error builders interpolated the caller's raw argument (a `wait --text` selector, a `select --value`, a `set-value` payload) directly into that message, e.g. `format!("Text '{text}' did not match…")`. Those errors flow into `command.end` and `actionability.check.error` events, which are written to per-session JSONL segments and embedded into `trace export` HTML — in default `events` mode, with no `--screenshots` opt-in. So the raw user value leaks despite redaction being "on".

## Guidance

Never interpolate a raw user- or app-supplied value into an error message that can reach a trace sink. Keep the message diagnostic without the content: report a shape (a character count, a role, a bounded enum), not the value.

Where raw values are genuinely useful for the immediate CLI caller, put them in the error's `details` object — `details` is returned to the direct caller but never emitted into a trace event, so it stays safe.

Bounded, non-sensitive vocabularies (fixed key-combo names, a closed set of predicate names, role strings) are fine to interpolate — they carry no user content. The rule targets open-ended caller input.

## Why This Matters

A field-name allowlist can only redact fields it knows the name of. Any code path that folds attacker- or user-influenceable text into a differently-named, non-listed field silently defeats redaction — and the failure is invisible in review because redaction still appears to be applied. Escaping (the XSS defense in `trace export`) is orthogonal: it stops the text from *executing*, not from being *read*. An exported trace attached to an issue would carry the leaked value in plain sight.

## When to Apply

- Any `format!` that builds an `AdapterError`/`AppError` message in an action path and embeds a `&str`/`Value` derived from a command argument.
- Any new trace event field: if it can carry user/app text, either add its key to `SENSITIVE_KEYS` or guarantee it only ever holds a shape, not content.

## Examples

Before (leaks the raw value into the trace):

```rust
return Err(AdapterError::new(
    ErrorCode::ActionFailed,
    format!("Selection did not change to '{value}'"),
));
```

After (mirrors the existing `text_chars` count idiom — diagnostic, no content):

```rust
return Err(AdapterError::new(
    ErrorCode::ActionFailed,
    format!("Selection did not change to the requested value ({} chars)", value.chars().count()),
));
```

Fixed sites (commit `b35eea9`): `crates/core/src/commands/wait_timeout.rs` (window/text/selector builders), `crates/macos/src/actions/extras.rs` (`select_value` list arm, `wait_for_value`, `option_not_found`), `crates/macos/src/actions/ax_helpers.rs` (`number_cf_from_str`). Regression guard: `wait_text_timeout_message_omits_raw_text_from_trace_segment` asserts a unique marker passed as `wait --text` never appears in the written segment.

## Related

- `docs/solutions/best-practices/playwright-grade-desktop-reliability-2026-06-02.md` — the broad tracing/reliability contract; its redaction guidance is refined by this convention (key-name redaction does not cover free-text `message` content).
- `crates/core/src/trace_sanitize.rs` — the `SENSITIVE_KEYS` allowlist this convention works around.
