---
date: 2026-04-24
topic: phase2-windows-crossplatform
---

# Phase 2 Windows + Cross-Platform Parity Requirements

## Problem Frame

agent-desktop is shipping as a strong macOS + FFI tool, but it is not yet a credible cross-platform automation runtime. Phase 2 must bring Windows online without weakening the core contract: agents observe and act through structured refs, not through foregrounded UI poking.

The next phase is larger than "add Windows." It also closes the parity gaps exposed by the FFI release: unstable element identity, polling-based waits, missing text-range operations, slower screenshots, incomplete permission reporting, and command-surface duplication risk. The goal is one coherent v0.2.0 release that makes Windows first-class while strengthening macOS in the same primitives.

The authoritative implementation plan already exists at `docs/plans/2026-04-18-001-feat-phase2-windows-crossplatform-plan.md`. This brief captures the product and architecture decisions that should guide execution.

## Release Gates

Phase 2 remains one v0.2.0 GA milestone, but execution should use internal gates so Windows value does not get buried under unrelated breadth:

- G1. Windows existing-command parity: the current macOS command surface works on Windows for Explorer, Notepad, Settings, VS Code, and Edge.
- G2. Headless contract: non-mouse commands on both macOS and Windows either use accessibility/headless APIs or return structured unsupported/action errors with guidance. Cursor or focus fallbacks are reserved for explicit mouse/window-focus commands.
- G3. Cross-platform primitives: stable selectors, event-backed waits, text ranges, modern screenshots, richer permission/error reporting, and command registration parity are implemented where the execution-prep spikes validate the API path.
- G4. Release readiness: Windows CI, Windows x86_64 and ARM64 artifacts, npm platform selection, FFI ABI handshaking, README, skills, and roadmap sync are complete.

G1 is the Windows-first MVP gate. G2-G4 decide whether the work is ready to be called Phase 2 GA rather than an internal checkpoint or prerelease.

## Requirements

- R1. Windows must support the existing command surface with the same JSON envelope and error contract as macOS.
- R2. Cross-platform parity is structural, not byte-identical: roles, refs, available actions, and stable identifiers must be comparable across equivalent apps while preserving platform-specific truth.
- R3. Headless-first behavior is non-negotiable: non-mouse commands must avoid focus steal, visible activation, and physical cursor movement. Existing macOS focus/cursor fallbacks must be migrated, gated behind explicit mouse/focus commands, or replaced with structured unsupported/action errors.
- R4. Windows skeleton traversal and ref-rooted drill-down must preserve the existing progressive traversal contract, including depth-3 skeletons, `children_count`, scoped invalidation, and the refmap size guard.
- R5. Stable selector fields must be added and used to reduce `STALE_REF` churn, especially in Electron, WebView2, localized, and custom-rendered apps.
- R6. Push-based event watching must complement polling waits through the public CLI shape `wait --event <kind> --ref @e... --timeout <ms>`. `watch_element` is the adapter primitive behind that command surface, not a separate Phase 2 CLI command.
- R7. Text range primitives must support selection, caret reads, range reads, and insert-at-caret on both macOS and Windows.
- R8. Modern per-window screenshot APIs must become the default on both platforms, with legacy paths retained as explicit fallbacks.
- R9. New action coverage must include `SelectRange` and `InsertAtCaret` for text workflows, plus `ShowMenu`, `WindowRaise`, `Cancel`, `DeliverFiles`, `LongPress`, and `ForceClick` only where the unit can prove a headless or platform-native path. If a variant has no native path on a platform, it must return a structured unsupported/action error rather than violating R3.
- R10. Missing surfaces and OS integration points must be promoted only when tied to Phase 2 workflows: Toolbar on both platforms for browser/editor automation; Windows SystemTray and notification parity for OS-level agent workflows; Spotlight, Dock, and MenuBarExtras on macOS as parity backfills only if their unit has a concrete target workflow and acceptance test.
- R11. Permission and error reporting must distinguish accessibility denial, permission revocation, screen-recording denial, automation denial, resource exhaustion, and AX messaging timeouts.
- R12. Command registration and FFI parity must use deterministic `build.rs` filesystem enumeration, not `inventory` or `linkme`, so one command file remains the durable source of truth across CLI, FFI, and future MCP.
- R13. Phase 2 must ship as v0.2.0 with Windows CI, Windows release artifacts, npm platform selection, FFI ABI handshaking, and updated skills/docs.

## Success Criteria

- `snapshot --app Explorer`, Notepad, Settings, VS Code, and Edge returns usable trees with refs on Windows.
- All existing commands run through Windows CI, and core still has zero platform crate leakage.
- Non-mouse command integration tests assert no focus change and no cursor movement; any fallback that would move focus/cursor returns a structured error with recovery guidance instead.
- Stable selectors measurably reduce stale-ref rate versus the Phase 1 resolver baseline.
- `wait --event value-changed --ref @e... --timeout 3000` receives events within 500 ms on macOS and Windows after the event-threading spike validates the path.
- Modern screenshot becomes the default after backend-specific benchmarks validate the threshold; legacy remains selectable. Do not hard-code a sub-50ms cold target before the screenshot spike because current ScreenCaptureKit crate docs report broader first-frame latency ranges.
- R9 action variants each have a target workflow, native/headless implementation path, or documented structured unsupported result before their unit opens.
- R10 surfaces each have a target workflow and acceptance test before their unit opens.
- Adding a new command requires one core command file plus type-level marshaling where necessary; it does not require hand-maintained per-transport command lists.
- v0.2.0 release artifacts include Windows x86_64 and ARM64 deliverables, with npm install working on Windows.

## Scope Boundaries

- Linux adapter remains Phase 3.
- MCP server mode remains Phase 4.
- Daemon, sessions, audit log, policy engine, OCR fallback, and package-manager distribution remain later production-readiness work.
- UI recording, macro replay, and GUI/TUI surfaces remain non-goals.
- Platform divergence is acceptable when the OS genuinely differs, but the JSON envelope, error semantics, and command names must stay consistent.

## Key Decisions

- Treat `docs/plans/2026-04-18-001-feat-phase2-windows-crossplatform-plan.md` as authoritative where earlier brainstorm or roadmap wording conflicts.
- Ship Phase 2 as one coherent release, not a 2a/2b split. Manage risk with reviewable units, not with hidden deferrals.
- Preserve headless-first as the phase's top architectural constraint. Any approach requiring foreground activation or physical cursor movement for ordinary commands is rejected.
- Use Windows UIA-first behavior and SendInput only as a fallback. UIA patterns are the reliability baseline.
- Use `ControlViewWalker` for Windows tree traversal, fresh cache requests per drill-down, and no cross-apartment UIA element caching.
- Reject `inventory` and `linkme`; use deterministic `build.rs` enumeration of one-command-per-file sources.
- Rename the old `FileDrop` concept to `DeliverFiles` because macOS drag sessions violate headless-first.
- Keep public APIs synchronous for CLI/FFI. Async or streaming behavior belongs to MCP/daemon phases.
- Keep `WindowInfo.pid` as `i32`; narrow Windows PIDs at the adapter boundary with explicit failure if reality ever exceeds the representation.
- Add FFI ABI handshaking before the breaking v0.2.0 shape changes so consumers fail closed.
- Do not refactor existing flat `AccessibilityNode` fields just to satisfy aesthetic field-count pressure; nest only the new selector group.
- Update docs and skills as a release gate, not as cleanup afterthoughts.

## External Research Notes

Checked on 2026-04-25:

- Microsoft UI Automation guidance says desktop-wide UIA clients should make UIA calls from a separate COM MTA thread that owns no windows, add/remove event handlers on that same non-UI thread, and handle COM apartment-affinity invalidation. This supports the Windows event-threading and no-cross-apartment-caching decisions.
- Microsoft `SendInput` documentation says input injection is subject to UIPI and only works into equal-or-lower integrity processes; it also does not reliably identify UIPI as the failure reason. This supports UIA-first behavior and structured fallback errors.
- Microsoft `IGraphicsCaptureItemInterop::CreateForWindow` targets a single `HWND` and requires Windows 10 version 1903 / build 18362 or later. Phase 2 should make that the minimum for modern per-window Windows screenshots, with legacy fallback for unsupported environments.
- Apple documents `SCScreenshotManager.captureImage(contentFilter:configuration:)` for single-frame capture through ScreenCaptureKit. The current `screencapturekit` crate docs list version 1.5.4 and report typical Apple Silicon first-frame latency around 30-100ms at 1080p, so the screenshot unit must benchmark before pinning a colder threshold.
- `uiautomation` 0.24.4 currently depends on `windows` 0.62.2, and the `windows` crate's current docs list 0.62.2. The Phase 2 dependency pins should be rechecked in the unit, but the existing direction is still plausible.
- GitHub's hosted-runner reference now lists `windows-11-arm` for ARM64 Windows in public and private repositories. Phase 2 should try ARM64 CI on that runner; keep build-only fallback only if repository, minutes, or toolchain constraints block reliable CI.

## Dependencies / Assumptions

- Windows support targets interactive desktop sessions, not Session 0 or Server Core.
- Modern Windows screenshot support requires Windows Graphics Capture availability, with `CreateForWindow` requiring Windows 10 version 1903 / build 18362 or later.
- Some Chromium/Electron apps still require renderer accessibility to expose useful trees; Phase 2 should detect and guide rather than pretend otherwise.
- ARM64 Windows should use GitHub's `windows-11-arm` runner for at least CI smoke coverage if reliable in this repository; fall back to build-only only if runner/toolchain constraints block it.
- v0.2.0 may break JSON and FFI ABI in documented ways because the project is still pre-1.0.

## Outstanding Questions

### Resolve Before Planning

- None for product direction. The existing Phase 2 plan can proceed through Unit 1 / Unit 2 prep work.

### Execution-Prep Gates

- [Affects R3][Technical] Audit existing macOS non-mouse action fallbacks and decide which become headless-only, which move behind explicit mouse/focus commands, and which return structured unsupported/action errors.
- [Affects R6][Technical] Validate macOS `AXObserver` threading and teardown against real apps before opening the final event-backed wait implementation.
- [Affects R7][Technical] Confirm exact Windows UIA text-range behavior in Notepad, Settings, and WebView2 controls, including nil/empty/error cases.
- [Affects R8][Needs research] Verify ScreenCaptureKit and `windows-capture` API pins and benchmark cold/warm screenshot latency before setting final thresholds.
- [Affects R9][Technical] For each new action variant, record the target workflow and native/headless path or structured unsupported result before implementation.
- [Affects R10][Technical] Confirm Windows notification and tray UIA trees across current Windows 11 builds and tie every macOS-only surface backfill to a target workflow before implementation.

### Deferred to Implementation

- [Affects R12][Technical] Keep checking generated wrapper/header drift in CI as command registry codegen lands.

## Next Steps

Use `docs/plans/2026-04-18-001-feat-phase2-windows-crossplatform-plan.md` for execution planning, but apply the gates above before opening affected implementation units. If moving directly into work, start with the v0.1.14 FFI prep / Unit 1 sequence before opening Windows adapter implementation.
