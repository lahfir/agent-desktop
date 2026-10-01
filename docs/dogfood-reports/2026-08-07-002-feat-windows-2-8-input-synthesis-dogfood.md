# Dogfood report — Input synthesis (sub-phase 2.8 U8)

**Date:** 2026-08-07 · **Branch:** `feat/windows-2.8-input-synthesis` · **Plan:** `docs/plans/2026-08-07-002-feat-windows-input-synthesis-plan.md`

The input layer cannot be validated by tests that restate SendInput return
codes. This run drives the release binary against repo-controlled targets
(Notepad, ScratchForms) with the corpus safety envelope: Assert-Foreground
brackets every injection, clipboard/cursor/modifier restore, PID-tracked
scratch only, and redaction at point of record. Judgements use JSON envelope
shapes plus independent observation (WM_GETTEXT SHA-256, native control text,
clipboard hash only — value never recorded).

The runner exits non-zero when any judgement records `fail`.

## Environment

| fact | value |
| --- | --- |
| OS | Windows Server 2019 Datacenter, build 17763 |
| UIA runtime | UIA3 COM (`CUIAutomation8`), `uiautomation` crate 0.25.0 |
| Binary | `target/release/agent-desktop.exe` (2,137,600 B release build) |
| Runner | `probes/windows/scratch/run-input-dogfood.ps1`, release binary driven directly |
| Capture | `docs/dogfood-reports/2026-08-07-002-captures/input-dogfood-run.json` (redaction gate passed) |
| Targets | Notepad (classic Edit / `textfield`), ScratchForms (`--host-providers`) |

Explorer and Obsidian were not required for the input judgement set; none were
absent in a way that blocked the matrix.

## Per-target matrix

| target | UI stack | result | judgements |
| --- | --- | --- | --- |
| Notepad (synthetic scratch file) | Win32 Edit / classic Notepad | ran | J1 pass, J2 pass, J5 pass, J8 skipped |
| ScratchForms | WinForms | ran | J3 pass, J4 pass, J6 pass, J7 pass |
| All injections | harness | ran | J9 pass |

Every target uses **repo-controlled content**: synthetic notepad file,
repo-built ScratchForms fixture.

## J1. Notepad type A4-1 payload matrix (headed physical)

Independent re-read via WM_GETTEXT SHA-256 (never SendInput return).

**Representative envelope (ascii):**

- `ok: true`, `command: "type"`
- `disposition.delivery: "delivered_unverified"`, `retry: "unsafe"`
- `steps: [{ label: "SendInput.type_text", mechanism: "physical_synthetic", outcome: "succeeded", verified: false }]`

**Payload notes:** ascii/cjk/astral/mixed all passed hash match (`utf16=14/3/4/5`).

**Verdict:** pass — all four A4-1 payloads round-tripped through headed
physical `type`.

## J2. press ctrl+a / ctrl+c

After headed `clear` + `type` + `click` on the textfield and foreground restore:

**Envelopes:**

- `press ctrl+a` → `ok: true`, `SendInput.press_key` succeeded
- `press ctrl+c` → `ok: true`, `SendInput.press_key` succeeded
- Independent clipboard SHA-256 matched marker (`clip_hash_match=True`; value not recorded)

**Verdict:** pass — chord delivery plus clipboard hash match.

## J3. mouse-move / mouse-click / mouse-wheel

ScratchForms `btnAction` + `pnlScroll` via bare coordinates (`--headed`).

**Observation:** `txtStatusMirror` value changed to `action:N` pattern; native
`lblScrollPos` text changed after wheel (`scroll_before_shape=scroll:N`).

**Wheel envelope:** `ok: true`, `command: "mouse-wheel"`, `scrolled: true`
(`--dy=-3`; PowerShell must pass `--dy=-3` as one token — bare `-120` parses as
flags).

**Verdict:** pass — move, click, and wheel drove real controls.

## J4. double-click on ListBox sink

Headed `double-click` on `btnDoubleClick` after foreground restore.

**Envelope:**

- `ok: true`, `command: "double-click"`
- `disposition.delivery: "delivered_unverified"`
- `steps: [{ label: "SendInput.click", mechanism: "physical_synthetic", outcome: "succeeded", verified: false }]`

**Observation:** native `lblDoubleClick` counter advanced (shape `dbl:N` →
`dbl:N`, digits redacted in capture).

**Verdict:** pass on a HWND-bearing stack — multi-click physical path
landed; required root-window foreground check (child HWND ≠ foreground)
fixed during this run in `physical_target.rs`. Scope of this judgement:
WinForms controls own a HWND, as does Notepad in J5, so this run exercised
the multi-click path only where the target element itself resolves a window
handle. A WPF, WinUI or Chromium element reports `NativeWindowHandle` 0 and
was never multi-clicked here — see the residual below.

## J5. right-click context menu

Headed `right-click` on Notepad textfield.

**Envelope:**

- `ok: true`, `command: "right-click"`
- `steps: [{ label: "SendInput.click", outcome: "succeeded", mechanism: "physical_synthetic", verified: false }]`

**Observation:** `find --role menuitem` returned 5 rows post-click
(independent of envelope alone).

**Verdict:** pass — context menu observed.

## J6. drag moves tbSlider

Headed `drag --from-xy … --to-xy … --drop-delay 0` on ScratchForms
`tbSlider`. Pickup uses thumb-fraction X from UIA bounds and native
`GetWindowRect` Y at `Top + 14` (A06/A4-3 TrackBar thumb row — UIA center Y
misses the horizontal thumb track on this host).

**Envelope:**

- `ok: true`, `command: "drag"`, `data.dragged: true`

**Independent re-read:** RangeValue `get` and native `lblSliderValue` both
changed after headed drag (label or value monotonic increase).

**Verdict:** pass — product `SendInput` drag synthesis lands when pickup Y
targets the thumb row; prior fail was harness center-Y coordinates, not a
delivery gap in `drag.rs`.

## J7. interrupted drag (A20-3)

Harness-native abort after mouse-down on `tbSlider`; `GetAsyncKeyState` re-read
before exit.

**Verdict:** pass — button up before exit; `interference_rows=0`.

## J8. Medium→High PERM_DENIED

Notepad and agent-desktop both run at `S-1-16-8192` on this host
(`notepad_rid=8192`, `medium_rid=8192`).

**Verdict:** skipped — target not strictly above Medium (A19-4/A20-2); no
elevated Notepad to stage against.

## J9. Foreground interference

**Verdict:** pass — `interference_count=0`; every Assert-Foreground bracket
passed.

## J10. double-click on a non-HWND (WPF) target — post-run addendum

The multi-click judgements above ran only against HWND-bearing targets, so
this leg was added after the review found the foreground gate reading the
element's own `NativeWindowHandle`. WPF controls report that handle as 0,
which is the shape the gate mishandled. Run twice against the same
`ScratchWpf` `btnAction`, once with the binary as it shipped into review and
once with the fix, with the fixture raised `SWP_NOACTIVATE` so nothing
covered it.

**Pre-fix binary:**

- `ok: false`, `error.code: "ACTION_FAILED"`
- message names losing focus before physical input delivery
- `disposition.delivery: "not_delivered"`
- sink unchanged at `status:ready` — nothing was injected

**Fixed binary, same button:**

- `ok: true`, `disposition.delivery: "delivered_unverified"`, retry `unsafe`
- `steps: [{ label: "SendInput.click", mechanism: "physical_synthetic", outcome: "succeeded", verified: false }]`

**Observation:** the fixture's own click counter advanced `status:ready` →
`action:2` — the WPF `Click` handler fired twice, read back independently of
the command's envelope.

**Verdict:** pass — physical multi-click lands on an element that owns no
window handle. Two incidental confirmations from the same run: the occlusion
gate refused an earlier attempt naming the terminal window that genuinely
covered the fixture (`receives_events` → `occluded by window`), and the
private-file guard refused a store directory owned by `BUILTIN\Administradores`
rather than the user, the ownership an elevated run leaves behind — both
correct refusals, both observed rather than inferred.

## Residuals (owners for U9 / later)

| residual | owner | status |
| --- | --- | --- |
| J6 harness center-Y pickup on WinForms TrackBar | closed — dogfood uses native `Top + 14` per A06/A4-3 | closed |
| `--from <sliderRef>` drag still resolves center Y (core); TrackBar needs thumb-row pickup | core point_resolve / future slider-aware pickup | recorded |
| J8: no High-integrity Notepad on Server 2019 dev box for live PERM_DENIED envelope | U9 — detection unit-tested; cross-boundary effect inherits A19-4 skip | recorded |
| Child-control foreground gate (`is_root_foreground_window`) | fixed in this run (`physical_target.rs`, `window_ops.rs`) | closed |
| Multi-click and right-click judged only on HWND-bearing targets (WinForms, Notepad); a non-HWND element reports `NativeWindowHandle` 0 and was never exercised in the original run | fixed in `physical_target.rs` (climb to the first ancestor owning a handle), pinned live, and judged in J10 against WPF with before/after binaries | closed |
| Chromium/Electron multi-click still unjudged — J10 covers WPF, which is the same zero-handle shape, but a Chromium target adds the render-host pane the occlusion gate resolves to `Unknown` (A18-3) | §2.12 settled-Chromium environment | recorded |
| A store directory created by an elevated run is owned by `BUILTIN\Administradores`, and the private-file guard then refuses it for the same human user (observed during J10; worked around with a scratch `HOME`) | recorded for the private-file owner; not caused by this sub-phase | recorded |
| Drag mouse-down batched with move | split into separate `SendInput` posts (`drag.rs`) | closed |

## Verification Contract result (U8 dogfood gate set)

| gate | result |
| --- | --- |
| run with repo-controlled content | yes — synthetic notepad, repo ScratchForms |
| safety envelope enforced | yes — Assert-Foreground bracket, hygiene restore, redaction gate passed |
| skips reasoned | yes — J8 skipped with A19-4 integrity note |
| findings escalated | J6 harness Y pickup corrected; foreground fix landed earlier |
| durable redaction-compliant report | this report + capture JSON |
| environment header + per-target matrix | above |
| judgements backed by quoted envelope shapes | J1–J9 above |

Release binary ≈2.04 MiB (under 15 MiB cap). Runner exit code **0** (all
required judgements pass; J8 skipped with reason).
