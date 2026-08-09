# Dogfood report - System lifecycle (sub-phase 2.9 U9)

**Date:** 2026-08-08 | **Branch:** `feat/windows-2.9-system-lifecycle` | **Plan:** `docs/plans/2026-08-08-001-feat-windows-system-lifecycle-plan.md`

The lifecycle layer cannot be validated by tests that restate Win32 return
codes. This run drives the release binary against repo-controlled targets
(Notepad, Explorer scratch folder) with the corpus safety envelope:
Assert-Foreground brackets headed press, clipboard/cursor restore,
PID-tracked scratch only, and redaction at point of record. Judgements use
JSON envelope shapes plus independent observation (WM_GETTEXT SHA-256 /
length, Win32 `GetWindowPlacement`/`GetWindowRect`, process-gone re-read) -
never `ok:true` alone.

The runner exits non-zero when any judgement records `fail`.

## Environment

| fact | value |
| --- | --- |
| OS | Windows Server 2019 Datacenter, build 17763 |
| UIA runtime | UIA3 COM (`CUIAutomation8`), `uiautomation` crate 0.25.0 |
| Binary | `target/release/agent-desktop.exe` (2,180,608 B release build) |
| Runner | `probes/windows/scratch/run-lifecycle-dogfood.ps1`, release binary driven directly |
| Capture | `docs/dogfood-reports/2026-08-08-001-captures/lifecycle-dogfood-run.json` (redaction gate passed) |
| Targets | Notepad (classic Edit / `textfield`), Explorer folder window, StalledFixture (lib) |

UWP/`ApplicationFrameHost` targets were absent on this host (A1-3 gap).

## Per-target matrix

| target | UI stack | result | judgements |
| --- | --- | --- | --- |
| Notepad (absolute System32 path) | Win32 Edit / classic Notepad | ran | J1 pass, J4 pass, J5 pass |
| Explorer (scratch folder) | shell DirectUI | ran | J2 pass (interact + protected refuse) |
| StalledFixture | hung top-level (lib) | ran | J3 pass |
| UWP / ApplicationFrameHost | n/a | absent | J6 skipped (A1-3) |

Every target uses **repo-controlled content**: synthetic notepad document,
synthetic explorer folder, in-process StalledFixture.

## J1. Notepad launch -> interact -> close round-trip

Product `launch` of absolute Notepad path with `--no-attach`, headless
`set-value` interact, graceful `close-app`.

**Launch envelope (shapes):**

- `ok: true`, `command: "launch"`
- data keys include `id`, `bounds`, `process_instance` (presence only)

**Interact:** `set-value` ok; independent WM_GETTEXT SHA-256 matched marker
(`utf16=12`; value not recorded).

**Close envelope:**

- `ok: true`, `command: "close-app"`
- `closed: true`, `requested: true`, `method: graceful`
- Independent process re-read: gone

**Verdict:** pass - full launch/interact/close round-trip with independent
hash and process-gone checks.

## J2. Explorer interact + protected-process close refusal

Product `launch explorer.exe` (attach-default) returned `AMBIGUOUS_TARGET`
(`not_delivered`) on this host - multiple shell `explorer.exe` rows; args are
not applied on the attach path. `--no-attach` returned `ACTION_FAILED` (already
running). Interact coverage used a harness-opened folder window; product
`snapshot` returned `ok: true` with `ref_count=88`.

**Close refusal envelope:**

- `ok: false`, `command: "close-app"`, `error.code: "INVALID_ARGS"`
- `disposition.delivery: "not_delivered"`, `retry: "safe"`
- suggestion present

**Verdict:** pass - interact observed via product snapshot; close of
`explorer.exe` refused before termination. Plan text named `PERM_DENIED`;
shipped code and unit tests pin `INVALID_ARGS` (U10 doc correction).

## J3. APP_UNRESPONSIVE / ProcessState::Unresponsive

No CLI `process-state` command exists (adapter-only). Evidence:

```
cargo test --locked -p agent-desktop-windows --lib
  system::process_state::tests::stalled_fixture_classifies_unresponsive
  -- --exact
```

**Result:** 1 test ran, ok (StalledFixture -> `ProcessState::Unresponsive`).

**Verdict:** pass - Unresponsive classification pinned live against
StalledFixture; CLI surface absent by design.

## J4. window_op resize / move / minimize / maximize / restore

Scratch Notepad window; each op judged by envelope flag **and** independent
Win32 placement re-read (A21-5 tolerance 8 px; `showCmd` 2/1/3/1).

| op | envelope flag | independent |
| --- | --- | --- |
| resize 640x480 | `resized: true` | width/height delta 0 |
| move 120,140 | `moved: true` | x/y delta 0 |
| minimize | `minimized: true` | `showCmd=2` |
| restore | `restored: true` | `showCmd=1` |
| maximize | `maximized: true` | `showCmd=3` |
| restore | `restored: true` | `showCmd=1` |

**Verdict:** pass - all six placements verified outside the command envelope.

## J5. headed `press --app` on real Notepad

After clearing SearchUI foreground stealer and product `focus-window`
(`fg_owned=true`):

**Envelope:**

- `ok: true`, `command: "press"`
- `action: "press_key"`
- `disposition.delivery: "delivered_unverified"`, `retry: "unsafe"`

**Independent:** WM_GETTEXT length grew `0 -> 1` after `press a --app
notepad.exe`; Assert-Foreground bracket clean.

**Verdict:** pass - headed app-targeted press delivered; edit buffer changed.

## J6. UWP lifecycle target

Census: `appx_candidate_count=0`, `application_frame_host_windows=0`.

**Verdict:** skipped - no reachable UWP/`ApplicationFrameHost` lifecycle
target on this Server 2019 host; gap recorded per A1-3.

## Residuals (owners for U10 / later)

| residual | owner | status |
| --- | --- | --- |
| Product `launch explorer.exe` attach-default -> `AMBIGUOUS_TARGET` when multiple shell explorer rows exist; `--no-attach` -> already-running `ACTION_FAILED`; folder args ignored on attach | §2.15 launcher/shell attach settlement (plan already names launcher-style child-pid / attach ambiguity) | recorded |
| Protected-process close ships `INVALID_ARGS` + `not_delivered`, not `PERM_DENIED` as U9 plan prose said | U10 - correct plan/phases wording to match `close_app.rs` / `close_tests.rs` | recorded |
| No CLI for `process_state` / `APP_UNRESPONSIVE` (lib + adapter only) | none for 2.9; optional later CLI surface | recorded |
| UWP/`ApplicationFrameHost` lifecycle unjudged on this host (A1-3) | environment / §2.14 AUMID launch when binding lands | recorded |
| Headed activation fails while SearchUI owns foreground; dogfood dismisses it first | harness hygiene; activation fail-closed is correct | recorded |

## Notes for U10 (do not implement here)

1. Align protected-process refusal code wording with shipped `INVALID_ARGS`
   (not `PERM_DENIED`) wherever §2.9 / skills still say otherwise.
2. Confirm §2.15 already carries explorer/shell attach + launcher-style
   `WINDOW_NOT_FOUND`/`AMBIGUOUS_TARGET` divergence; cite this dogfood if a
   row id is needed beside A21.
3. UWP gap remains A1-3 / §2.14 - no correction beyond confirming absence on
   Server 2019.

## Verification Contract result (U9 dogfood gate set)

| gate | result |
| --- | --- |
| run with repo-controlled content | yes - synthetic notepad, scratch explorer folder, StalledFixture |
| safety envelope enforced | yes - Assert-Foreground on headed press, hygiene restore, redaction gate passed |
| skips reasoned | yes - J6 skipped with A1-3 note |
| findings escalated | explorer launch attach residual + INVALID_ARGS vs PERM_DENIED for U10 |
| durable redaction-compliant report | this report + capture JSON |
| environment header + per-target matrix | above |
| judgements backed by quoted envelope shapes | J1-J6 above |

Release binary ~2.08 MiB (under 15 MiB cap). Runner exit code **0** (required
judgements pass; J6 skipped with reason). Interference rows: **0**.
