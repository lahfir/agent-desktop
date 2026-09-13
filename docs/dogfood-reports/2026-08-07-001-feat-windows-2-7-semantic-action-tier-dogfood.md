# Dogfood report — Semantic action tier (sub-phase 2.7 U9)

**Date:** 2026-08-07 · **Branch:** `feat/windows-2.7-semantic-action-tier` · **Plan:** `docs/plans/2026-08-07-001-feat-windows-semantic-action-tier-plan.md`

The semantic dispatch tier cannot be validated by a test that restates it. This
run drives the release binary against real software and judges by reading JSON
envelopes — never the suite's opinion of itself. The 2.6 dogfood's J2 arm
(`PLATFORM_NOT_SUPPORTED` naming `execute_action`) must be gone on click; headed
`double-click` must name `multi-click` (2.8 boundary). The runner exits
non-zero on any judgement failure after writing the capture summary.

## Environment

| fact | value |
| --- | --- |
| OS | Windows Server 2019 Datacenter, build 17763 |
| UIA runtime | UIA3 COM (`CUIAutomation8`), `uiautomation` crate 0.25.0 |
| Binary | `target/release/agent-desktop.exe` (2,108,928 B release build) |
| Runner | `probes/windows/scratch/run-semantic-dogfood.ps1`, release binary driven directly, JSON read |
| Capture | `docs/dogfood-reports/2026-08-07-001-captures/semantic-dogfood-run.json` (redaction gate passed) |
| Targets | Notepad (classic Document→`textfield`), Explorer, WinForms + WPF scratch, Obsidian |

## Per-target matrix

| target | UI stack | result | judgements |
| --- | --- | --- | --- |
| Notepad (synthetic scratch file) | Win32 Edit / classic Notepad | ran | J1 pass, J6 pass, J7 pass, J8 pass |
| Explorer on synthetic scratch dir | shell DirectUI (`option` rows) | ran | J2 pass, J3 pass |
| WinForms scratch | WinForms | ran | J4 pass (click/toggle/expand) |
| WPF scratch | WPF | ran | J5 pass (RangeValue slider) |
| Obsidian (Chromium/Electron) | Chromium + Electron | ran | J9 honest TIMEOUT on semantic click |

Every target shows **repo-controlled content**: synthetic notepad/explorer
files, repo fixtures, Obsidian shapes/codes only. Absent targets would be
skipped with a reason; none were absent.

## J1. Notepad Document set-value / clear payload matrix

Classic Notepad's edit surface maps to `role: textfield` (A2-4 Document on
COM). Headless `set-value` / `clear` round-trip through `ValuePattern.SetValue`.

**ASCII envelope (quoted keys):**

- `ok: true`, `command: "set-value"`
- `disposition.delivery: "delivered_verified"`, `retry: "unsafe"`
- `steps: [{ label: "ValuePattern.SetValue", mechanism: "semantic_api", outcome: "succeeded", verified: true }]`
- matching `clear` → `delivered_verified` with empty post-state value

**Payload notes:** ASCII required path passed (`chars=16`). CJK and astral
writes also returned `delivered_verified` on `ValuePattern.SetValue`, but the
harness `get` length check did not match (`cjk get_chars=6`, `astral
get_chars=4`) — recorded as residual, not a required-path failure.

**Verdict:** pass — required ASCII set-value/clear round-trip through the
product COM Value path.

## J2. Explorer list item select by visible name

Explorer Items View rows advertise as `option` (not `listitem`). Selected
`file-05` by exact name.

**Envelope:**

- `ok: true`, `command: "select"`
- `disposition.delivery: "delivered_verified"`
- `steps: [{ label: "SelectionItemPattern.Select", mechanism: "semantic_api", outcome: "succeeded", verified: true }]`

**Verdict:** pass — select by visible name verified.

## J3. Explorer below-fold re-judgement (2.6 J4 residual)

Among realized `option` rows on this host (24 of 40 synthetic files; shell
virtualization), the late candidate was already `visible: true` before click —
so the ancestor ladder was not forced to prove a below-fold geometry change.
The click nonetheless reached semantic dispatch:

**Envelope:**

- `ok: true`, `command: "click"`
- `disposition.delivery: "delivered_unverified"`
- `steps: [{ label: "InvokePattern.Invoke", mechanism: "semantic_api", outcome: "succeeded", verified: false }]`
- `message_names_execute_action` absent — the 2.6 J2 `execute_action` arm is gone

**Verdict:** pass — 2.6 residual re-judged: Explorer click delivers through
Invoke (honest `delivered_unverified`), not `PLATFORM_NOT_SUPPORTED` /
`execute_action`. Below-fold geometry among unrealized rows remains a shell
virtualization limit (see residuals).

## J4. Fixture click / toggle / expand full envelopes

ScratchForms `btnAction`, `chkToggle`, and treeitem `Node-Sibling`.

| action | delivery | step |
| --- | --- | --- |
| click | `delivered_unverified` | `InvokePattern.Invoke` / `succeeded` / `verified: false` |
| toggle | `delivered_verified` | `TogglePattern.Toggle` / `succeeded` / `verified: true` |
| expand | `delivered_verified` | `ExpandCollapsePattern.Expand` / `succeeded` / `verified: true` |

All steps carry `mechanism: "semantic_api"`. No envelope names `execute_action`.

**Verdict:** pass.

## J5. Fixture slider set-value through RangeValue

ScratchWpf `tbSlider`, commanded value `42`.

**Envelope:**

- `ok: true`, `command: "set-value"`
- `disposition.delivery: "delivered_verified"`
- `steps:`
  - `ValuePattern.SetValue` / `skipped`
  - `RangeValuePattern.SetValue` / `succeeded` / `verified: true`

**Verdict:** pass — RangeValue rung won; re-read verification `verified: true`
for commanded `42`.

## J6. Headless focus → POLICY_DENIED (A3-4 / A19-5)

**Envelope:**

- `ok: false`, `command: "focus"`, `error.code: "POLICY_DENIED"`
- `disposition.delivery: "not_delivered"`, `retry: "safe"`
- `details.foreground_effect: true`
- `details.evidence: ["A3-4", "A19-5"]`
- suggestion present (`--headed` / policy guidance)

**Verdict:** pass.

## J7. Headless type → honest preflight denial

**Envelope:**

- `ok: false`, `command: "type"`, `error.code: "POLICY_DENIED"`
- `disposition.delivery: "not_delivered"`
- `details.checks[]` includes `supported_action` / `fail` with reason shape
  `semantic action unavailable / focus fallback denied`

**Verdict:** pass — TypeText denied at preflight before dispatch (2.8 owns
key synthesis).

## J8. Headed double-click → PLATFORM_NOT_SUPPORTED naming multi-click

**Envelope:**

- `ok: false`, `command: "double-click"`, `error.code: "PLATFORM_NOT_SUPPORTED"`
- message names `multi-click` (`message_names_multi_click: true`)
- does **not** name `execute_action` (`message_names_execute_action: false`)
- `disposition.delivery: "not_delivered"`

**Verdict:** pass — J2-style discriminator now guards the 2.8 multi-click
boundary.

## J9. Obsidian one semantic action attempt

Obsidian present. A positive-area leaf was clicked; the envelope returned
`TIMEOUT` / `not_delivered` — recorded honestly, not coerced to success. Prior
runs on this host also observe the A18-3 shell-bound shape (15 refs, no
positive-area leaf). Neither shape invents semantic success.

**Verdict:** ran — honest failure; Chromium semantic action remains
environment-bound.

## Residuals (owners for U10 / later sub-phases)

| residual | owner | status |
| --- | --- | --- |
| CJK/astral Notepad `set-value` returns `delivered_verified` but harness `get` length check mismatched (not required-path) | harness comparison / Notepad Document value-read shape; escalate only if product re-read lies | recorded; ASCII path is the gate |
| Explorer realized option set has no off-screen row on this host (24 visible); ladder geometry not forced — 2.6 J4 residual closed on dispatch honesty (`Invoke` / `delivered_unverified`, no `execute_action`) | §2.7 ladder already shipped; fuller below-fold proof needs denser/unrealized Explorer surface or 2.12 fixture | recorded |
| Obsidian semantic click `TIMEOUT` / shell-bound (A18-3) | 2.12 self-hosted / settled Chromium environment | recorded |
| Headed `double-click` / physical multi-click / key synthesis / `type` | §2.8 | boundary proven honest (J7/J8) |
| WinForms slider prefers `ValuePattern` when both available; WPF slider exercises RangeValue (J5) | none — expected chain preference | noted |

## Verification Contract result (U9 dogfood gate set)

| gate | result |
| --- | --- |
| run with repo-controlled content | yes — synthetic notepad/explorer content, repo fixtures, Obsidian codes/shapes only |
| skips reasoned | yes — no absent targets; J9 recorded as ran/honest, not faked |
| findings closed-with-failing-test or escalated | `execute_action` click arm gone (J3/J4); residuals owned above for U10 |
| durable redaction-compliant report | this report + capture JSON (redaction gate passed) |
| environment header + per-target matrix | above |
| every judgement backed by a quoted envelope | J1–J9 above; capture JSON retains shapes/counts only |
| click must not name `execute_action` | held — J3/J4/J8 discriminators |

Release binary ≈2.01 MiB (under 15 MiB). U10 docs are **not** in this unit —
residuals above are the handoff.
