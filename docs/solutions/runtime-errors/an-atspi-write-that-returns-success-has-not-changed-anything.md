---
title: An AT-SPI write that returns success has not changed anything
date: 2026-10-19
category: runtime-errors
module: crates/linux/src/actions/write/write_call.rs, crates/linux/src/actions/write/mutation.rs, crates/linux/src/actions/write/text_write.rs, crates/linux/src/actions/write/numeric_write.rs, crates/linux/src/actions/write/secure_write.rs, crates/linux/src/tree/capabilities/action_read.rs
problem_type: runtime_error
component: atspi-write-path
symptoms:
  - "set-value or type reports success and the field still holds its old text."
  - "A write to a destroyed widget, a read-only field or a Gecko input answers true with no error."
  - "A write that timed out lands later, when the stopped application resumes."
  - "A password value appears in an error message or in a read-back."
tags: [linux, at-spi, d-bus, write-path, read-back, delivery, secure-fields, toolkits]
---

## Problem

`EditableText.SetTextContents`, `InsertText`, `Value` writes and `Action.DoAction` return a boolean or nothing. On Linux that return says the call reached an object that answered. It does not say the object acted. Measured on live GTK3, GTK4, Qt 6, Firefox and Chromium (`probes/linux/FINDINGS.md` L30-2 to L30-16), every toolkit has a case where a write answers success and nothing changes, and a case where a write answers failure and the effect lands anyway.

## What the toolkits actually do

- **Success with no effect.** A destroyed GTK3 widget answers `DoAction`, `SetTextContents`, `InsertText` and `Properties.Set` without error, while `GetState` returns two zero words and the name is empty (L30-2). Gecko answers `true` to `SetTextContents` and `InsertText` on every input and changes nothing, although `EditableText` is present on nearly every Gecko node (L30-14). A read-only GTK3 field and a Qt read-only field accept the write silently; Qt does not enforce `READ_ONLY` on this path (L30-6, L30-11). Gecko ignores an out-of-range `Value` write with no clamp and no error (L30-16).
- **Failure with the effect delivered.** A stopped application answers every write with `NoReply` after the caller's timeout, and the bus delivers the queued call at `SIGCONT`, so each write lands (L30-3). `NoReply` with "recipient disconnected" means the application died during the call and the outcome is unknown (L30-4).
- **Failure that is only a missing interface.** Chromium inputs have no `EditableText`, so both text writes fail `UnknownMethod` (L30-15). A destroyed GTK4 object fails the same name with "Object does not exist" (L30-2), and Qt uses `UnknownObject` for a destroyed object and `UnknownInterface` for either a destroyed path or a missing interface (L30-10). The name alone cannot tell the cases apart (see `an-atspi-error-name-is-not-proof-an-object-is-gone.md`).
- **The secret is in the read-back.** Secure fields are found by the raw role number 40, not the name: GTK reports the name `text box` for role 40 (L30-17, L30-19). A GTK4 entry with visibility off, `InputPurpose.PASSWORD` or an `Adw.PasswordEntryRow` is role 61 and carries no secure signal at all, so its `GetText` returns the plain secret.

## Solution

Judge every write by what a read of the node shows afterwards, never by the reply.

1. Refuse before the call when the node is known unwritable: `READ_ONLY` set or `EDITABLE` clear (`text_write::writable`). The toolkit would accept the write and ignore it, so nothing after the call can be trusted to catch it on Qt.
2. Read the value before the write, send it, then read it again (`text_write::judge`). Equal to the request is `Matched` and the result carries `verified`. Equal to the value before is `write_ignored`: `ACTION_FAILED` with `details.cause = "write_ignored"`, disposition not delivered. Any other value, or no readable value, leaves the result `delivered_unverified`. `numeric_write.rs` and `select/mod.rs` apply the same rule. A numeric read-back is compared through core's `value_matches`. It counts as proof only when the value is whole, the control is not a spin button, or its `MinimumIncrement` is known, because the toolkit snaps (`numeric_write.rs:41-44`, L30-12).
3. Classify a failed call in `mutation::classify_write`. An error name that could mean "gone" or "not implemented" (`ObjectUnknown`, `InterfaceMissing` on an advertised interface) is settled by one `GetRole` read of the node (`action_read::corroborate`): alive means the method is absent, dead means `STALE_REF`, unread means unconfirmed. `NoReply` and a spent deadline are `delivery_uncertain`, never a retryable not-delivered.
4. Treat a field as secure when its role number is 40, when its state flags it, or when its tokens say `secure` (`secure_write::is_secure`). A secure write is never read back, so it carries no `verified`, and `scrub` replaces the written text with `[withheld]` in the error's message, suggestion and platform detail and drops `details` that echo it (`secure_write.rs:18-36`). A GTK4 role-61 field cannot be recognised, so its read-back must not be copied into an envelope.

## Prevention

- Do not report `ok` for an AT-SPI write from the reply alone. A reply proves delivery to a live object at most, and not even that on GTK3.
- Never re-send a write after `NoReply` without reading the node first: the first call may land at resume and the second one doubles it.
- Do not decide editability from interface presence (Gecko) or from the absence of `read-only` (GTK3); use the `EDITABLE` and `READ_ONLY` bits together.
- A read-back that is skipped for secrecy gives up `verified`. Report that honestly as `delivered_unverified`; do not read the secret to regain the verdict.
- Probe each toolkit before encoding a write rule. Gecko, Chromium, Qt, GTK3 and GTK4 each disagreed with the others on at least one case above.
