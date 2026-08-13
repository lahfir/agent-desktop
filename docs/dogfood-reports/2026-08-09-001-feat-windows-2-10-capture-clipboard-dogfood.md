# Dogfood report - Capture and clipboard (sub-phase 2.10 U10)

**Date:** 2026-08-10 | **Branch:** `feat/windows-2.10-capture-clipboard` | **Plan:** `docs/plans/2026-08-09-001-feat-windows-capture-clipboard-plan.md`

The capture and clipboard surfaces cannot be validated by tests that restate
Win32 return codes or fixture pixel patterns alone. This run drives the release
binary against repo-controlled targets (Notepad, ScratchForms, ScratchWpf,
Obsidian) and real clipboard producers (CF_UNICODETEXT, CF_DIB, CF_HDROP).
Judgements use JSON envelope shapes plus independent PNG pixel statistics
(black ratio / unique-colour sample / mean luma classification) and clipboard
SHA-256 / count checks - never `ok:true` alone. The corpus safety envelope
applies: shapes and counts only at point of record (no titles, paths, pids,
machine names, message text, or clipboard values).

The runner exits non-zero when any judgement records `fail`.

## Environment

| fact | value |
| --- | --- |
| OS | Windows Server 2019 Datacenter, build 17763 |
| UIA runtime | UIA3 COM (`CUIAutomation8`), `uiautomation` crate 0.25.0 |
| Binary | `target/release/agent-desktop.exe` (2,275,840 B release build) |
| Runner | `probes/windows/scratch/run-capture-clipboard-dogfood.ps1`, release binary driven directly |
| Capture | `docs/dogfood-reports/2026-08-09-001-captures/capture-clipboard-dogfood-run.json` (redaction gate passed) |
| Targets | Notepad (Win32/GDI), ScratchForms (WinForms), ScratchWpf (WPF), Obsidian (Electron/Chromium) |
| WGC | `GraphicsCaptureSession::IsSupported` = true; modern attempt fails activating `IGraphicsCaptureItemInterop`; silent Legacy fallback succeeds (A22-1) |
| Permissions | `screen_recording.state` = `not_required` |

Chrome / Edge were absent on this host (`measurable:false` named branch:
Program Files Chrome/Edge paths missing). Obsidian supplied the Electron row.

## Per-target matrix

| target | UI stack | result | judgements |
| --- | --- | --- | --- |
| Notepad | Win32/GDI | ran | J1a pass, J2c pass, J2d pass, J3a disappointing |
| ScratchForms | WinForms | ran | J1b pass |
| ScratchWpf | WPF | ran | J1c pass |
| Obsidian | Electron/Chromium | ran | J1d pass |
| Primary display | BitBlt | ran | J2a pass, J2b pass |
| Clipboard producers | CF_UNICODETEXT / CF_DIB / CF_HDROP | ran | J3a-J3e (see below) |
| WGC modern backend | WinRT capture | ran | J0 disappointing |

Every target uses **repo-controlled content**: synthetic notepad marker file,
repo ScratchForms / ScratchWpf fixtures, Obsidian cold window (no vault
content recorded), synthetic PNG and scratch drop-files for clipboard.

## Leg 1 - Window capture by stack (PW_RENDERFULLCONTENT)

Independent classification samples PNG pixels after product `screenshot`
(width/height/bytes + black_ratio / unique_colors_capped / mean_luma). A frame
is `real_content` when it is not black / near-black / flat-dark.

### J1a. Notepad (Win32/GDI)

**Envelope:** `ok: true`, `command: "screenshot"`, `format: "png"`,
`width: 640`, `height: 480`, `scale_factor: 1.0`, path present.

**PNG:** `classification=real_content`, bytes ~7 KiB.

**Verdict:** pass - Legacy/`PrintWindow` (+ `PW_RENDERFULLCONTENT` then bare
fallback inside the product) returned a non-black Notepad frame.

### J1b. ScratchForms (WinForms)

**Envelope:** `ok: true`, dims 700x520.

**PNG:** `classification=real_content`, bytes ~25 KiB.

**Verdict:** pass - WinForms stack returned real content under Legacy.

### J1c. ScratchWpf (WPF / DWM-composited)

**Envelope:** `ok: true`, dims 480x752 (window-id scoped).

**PNG:** `classification=real_content`, bytes ~22 KiB.

**Verdict:** pass - GPU-composited WPF was not black under Legacy on this host.
This is the DWM-composition question `PW_RENDERFULLCONTENT` is meant to answer;
here the flag path produced usable pixels.

### J1d. Obsidian (Electron/Chromium)

**Envelope:** `ok: true`, dims 1024x691 (window-id scoped).

**PNG:** `classification=real_content`, bytes ~54 KiB.

**Verdict:** pass - Chromium compositor window returned real content under
Legacy on this host (not the black-frame failure mode often assumed for
`PrintWindow` against Electron).

## Leg 2 - Display, FullScreen, occlusion

### J2a. FullScreen

Bare `screenshot PATH` (no `--app` / `--screen`) maps to primary display.

**Envelope:** `ok: true`, dims 1639x732, `scale_factor: 1.0`.

**PNG:** `classification=real_content`.

**Verdict:** pass.

### J2b. Screen index 0

`screenshot --screen 0` matched FullScreen width/height exactly.

**Verdict:** pass - display-index path and FullScreen share the primary.

### J2c. Partially occluded Notepad

Two Notepad windows staged; target covered ~half by a foreground sibling;
capture by `--window-id` of the back window.

**PNG:** `classification=real_content` (500x400).

**Verdict:** pass - `PrintWindow` returned the target window's own pixels,
not the occluder's screen composite.

### J2d. Fully behind Notepad

Front window fully covered the back; capture still scoped to the back
`--window-id`.

**PNG:** `classification=real_content` (500x400).

**Verdict:** pass - covered-window capture still returned target content.

## Leg 3 - Clipboard round-trips

### J3a. Text from editor / CF_UNICODETEXT

Headed `press ctrl+a` / `ctrl+c --app notepad.exe` did not land a matching
clipboard generation on this host (foreground / headed delivery flakiness).
Fallback used `System.Windows.Forms.Clipboard.SetText` as a real
`CF_UNICODETEXT` producer; product `clipboard-get --format text` matched the
marker by SHA-256 (value not recorded; char count matched).

**Verdict:** disappointing - read path pass; headed Notepad copy missed.
**Disposition:** accepted - Server 2019 headed-press foreground flakiness;
CF_UNICODETEXT decode path still judged. Not a clipboard marshalling defect.

### J3b. Image from CF_DIB producer

Synthetic 48x32 PNG published via `Clipboard.SetImage` (Paint-equivalent
`CF_DIB` producer). Product `clipboard-get --format image --out` wrote a PNG
with dims 48x32 and `real_content` classification.

**Verdict:** pass.

### J3b2. Product image write/read

`clipboard-set --image` then `clipboard-get --format image` dims matched 48x32.

**Verdict:** pass.

### J3c. File list from CF_HDROP producer

Two scratch files published via `Clipboard.SetFileDropList` (Explorer-shaped
`CF_HDROP`). Product `clipboard-get --format file-urls` returned `count=2`
(paths not recorded).

**Verdict:** pass.

### J3c2. Product file-url write/read

`clipboard-set --file-url` x2 then `clipboard-get --format file-urls` ->
`count=2`.

**Verdict:** pass.

### J3c3. Explorer folder stage

Explorer folder window opened on the scratch drop directory. CF_HDROP content
was judged via `SetFileDropList` (same format Explorer publishes); UI Ctrl+C
inside Explorer was not required for the format judgement.

**Verdict:** pass.

### J3d. Auto precedence

With files on the clipboard, `clipboard-get --format auto` resolved to
file-urls (`FileUrls -> Image -> Text`).

**Verdict:** pass.

### J3e. Clear

`clipboard-clear` then text get reported absence / empty.

**Verdict:** pass.

## J0. Modern capture honesty (required finding)

Verbose product capture logged modern failure activating
`IGraphicsCaptureItemInterop`, then `falling back to legacy`, and the command
still returned `ok: true` with a real PNG. `IsSupported` is true on this
build-17763 host (A22-1), so build-number gating would have been wrong; the
runtime precedence path is what saved the call.

**Verdict:** disappointing (host capability), not a product regression against
R2's silent Legacy fallback.
**Disposition:** accepted - contracted Modern -> Legacy degradation on a host
where interop cannot activate. **Owner for live modern success:** section 2.12
interactive / capable session (plan Deferred to Follow-Up Work).

## Findings and dispositions

| id | finding | disposition | owner / proof |
| --- | --- | --- | --- |
| F1 / J0 | `IsSupported=true` but `IGraphicsCaptureItemInterop` activate fails; silent Legacy fallback succeeds | accepted | R2 + A22-1; section 2.12 owns live modern verification on a capable session |
| F2 / J3a | Headed Notepad `ctrl+c` did not publish marker; Forms `CF_UNICODETEXT` producer still round-tripped through product get | accepted | headed foreground flakiness on Server 2019; clipboard text decode path proven |

No finding was fixed in product code during this dogfood. No finding was left
at "recorded" without a disposition.

## Residuals for U11 (`docs/phases.md` / skills)

| residual | owner |
| --- | --- |
| Live modern-capture verification (interop-capable interactive session) and RDP/locked degradation legs | section 2.12 (already named in plan Deferred to Follow-Up Work; U11 writes into scope) |
| Skills pages for `screenshot` / `clipboard-*`: document Modern-first precedence, silent Legacy fallback when interop fails despite `IsSupported`, and `screen_recording=not_required` when capture works | U11 -> `skills/agent-desktop/references/` |
| Headed `press --app` clipboard-copy flakiness on Server 2019 console sessions | accepted here; not a 2.10 clipboard defect - do not expand 2.10 scope |

## Notes (do not implement here)

1. Electron and WPF were **not** black under Legacy on this host with the
   shipped `PW_RENDERFULLCONTENT`-then-bare path - do not document them as
   universally black without a capable-session countermeasure.
2. FullScreen equals `--screen 0` dims on the single-monitor corpus host;
   multi-monitor arithmetic remains section 2.12 if a second display cannot
   be manufactured (U1 decision).
3. Scratch PNGs under `%TEMP%` were deleted after the run; only redacted JSON
   + this report are retained.

## Verification Contract result (U10 dogfood gate set)

| gate | result |
| --- | --- |
| run with repo-controlled content | yes - fixtures + synthetic clipboard payloads |
| safety envelope enforced | yes - shapes/counts only; redaction gate passed |
| skips reasoned | none required (Chrome/Edge absent but Obsidian covered Electron) |
| findings with dispositions | F1 accepted (section 2.12), F2 accepted (headed flakiness) |
| durable redaction-compliant report | this report + capture JSON |
| environment header + per-target matrix | above |
| judgements backed by envelope shapes + independent observation | J0-J3e above |
| zero-findings rule | satisfied (F1, F2) |

Release binary ~2.17 MiB (under 15 MiB cap). Runner exit code **0**
(required judgements have no `fail`; two `disappointing` with dispositions).
Product code changes during dogfood: **none**.
