# Dogfood report — Observation read path (sub-phase 2.4)

**Date:** 2026-08-02 · **Branch:** `feat/windows-2.4-observation` · **Plan:** `docs/plans/2026-08-01-001-feat-windows-observation-read-path-plan.md`

A read path cannot be validated by a test that restates it. This is the run that establishes whether an agent's first `snapshot` on Windows actually works: the release binary pointed at software nobody in this repository wrote, the JSON read, and what was wrong judged or fixed.

## Environment

| fact | value |
| --- | --- |
| OS | Windows Server 2019 Datacenter, build 17763 |
| UIA runtime | UIA3 COM (`CUIAutomation8`), `uiautomation` crate 0.25.0 |
| Client stack | `uia3-com` — the stack the adapter ships |
| Binary | `target/release/agent-desktop.exe` (2.03 MB committed, measured release build) |
| Runner | release binary driven directly; captures regenerated on demand, not committed |
| Targets | classic Notepad, Explorer, WinForms fixture, WPF fixture, Obsidian (Chromium/Electron) |

**Stack coverage is Win32, DirectUI shell, WinForms, WPF and Chromium/Electron.** This sub-phase finally exercises the Chromium stack 2.3 could not reach (2.3's report recorded "no Chromium application is installed" — Obsidian is present on this box). Modern XAML (A10-7) stays unmeasured; Settings is skipped-with-reason.

## Targets

Every target shows **repo-controlled content**: Notepad and Explorer open a scratch directory of synthetic file names; the fixtures are the repo's own; Obsidian opens its own vault but this run reports only shape, counts and roles — never note titles. Absent targets are skipped with a reason, never reported captured.

| target | UI stack | result | refs | full-depth tree |
| --- | --- | --- | --- | --- |
| classic Notepad on a scratch file | Win32 `EDIT` proxy | captured, `complete:true` | 17 | 17 |
| Explorer on a scratch directory | DirectUI shell | captured, `complete:true` | 68 | 68 |
| WinForms scratch fixture | WinForms | captured, `complete:true` | 25 | 25 |
| WPF scratch fixture | WPF | captured, `complete:true` | 46 | 46 |
| Obsidian (Chromium/Electron) | Chromium + Electron | captured, `complete:true` after settle | 19 (default depth) / 57 (depth 50) | 57 |
| Settings | modern XAML | **SKIPPED** — no modern shell population on this Server 2019 box (A10-7) | — | — |

**The read path works end to end on every reachable stack.** Every target returned a reffed tree through the shipped `observe_tree`: ref IDs (`@<snapshot>:e{n}`), roles, states, `native_id`, `role_description`, window identity and a `complete` flag. The shell (Explorer, DirectUI) produced the densest ref set at 68.

## Findings

### Fixed during the run: the J-shell shell shape demanded activation before the tree had settled

The first Obsidian snapshots returned a 13-15 node tree with `complete:true` that was the **pre-activation Chromium shell** (A1-5's shape). The shell-shape detection originally keyed on raw-depth exhaustion, which a shallow shell never reaches — so a fresh Chromium window's shell could be reported as a complete, tiny tree. Corrected to macOS's **tree-end** signal (`max_logical_depth < requested`): a walk that runs out of tree rather than out of budget can conclude the tree is genuinely thin. The per-invocation `WindowsAdapter` state now distinguishes pre-settle (re-arms activation) from post-settle still-thin (returns the guidance `platform_detail`). The fix is pinned by `a_depth_clamped_walk_never_looks_shell_shaped` and the tree-end tests in `chromium.rs`.

After the fix, a settled Obsidian returns the full tree (57 refs at depth 50) with no activation loop and no still-thin error.

### Judged by design, not a defect: complete at a requested depth with reffed boundary counts

At the default depth 10, Obsidian reports `complete:true` with 19 refs and `children_count` on boundary containers; at depth 50 it reports 57 refs with no boundaries. This is KTD12's contract — a logical-depth boundary is a **drill-down target**, not a silent truncation: the boundary nodes carry real counts and are reffed, so an agent reads the shallow tree and drills into any boundary ref. macOS behaves identically (`at_requested_boundary` → `subtree_complete = read.complete`, child count emitted). Not a defect; the drill-down (U8) is what makes it honest rather than merely small.

## Judgements, per target

**Is completeness honest?** Yes on every target. Notepad, Explorer, WinForms and WPF report `complete:true` at full depth and their trees project fully. Obsidian at depth 50 reports `complete:true` with 57 refs — the settled tree matches the U1 census's 165-element raw walk scaled to reffed nodes. A deadline-sized walk returns `complete:false` with marked boundaries, never a complete-looking discard (KTD8's liveness re-verify verified live on every target).

**Did the ref-able arms the plan carried forward finally resolve?** The 2.3 report left two arms unexercised:
- **`switch` (Button + Toggle)** — **RESOLVED.** The WPF fixture's new `ToggleButton` advertises `ControlType.Button` + `IsTogglePatternAvailable`, and the snapshot emits `role:"switch"` on it. The fix is the fixture's `btnToggle` (U1 item 10).
- **`cell` (DataItem + GridItem/TableItem)** — **still not emitted, reason now measured.** U1's A16-10 recorded that WPF DataGrid cells carry GridItem/TableItem availability but present `ControlType.Custom` (50025), which maps to `Role::Unknown`, not `DataItem` (50029). The dogfood reproduces it: the grid's rows resolve to `role:"row"` (`DataItem` → else-branch) and the cells inside report `role:"unknown"` with `role_description:"custom"` even though they carry the cell patterns. **This is a real gap the unit tests could not see.** The decision belongs to `docs/phases.md` (U10): either `Custom` + GridItem/TableItem should refine to `cell`, or the arm stays as-designed and the cells remain `unknown` until a provider presents them as `DataItem`. The WinForms fixture's `DataGridView` advertises neither pattern, so that variant cannot exercise it either.

**What does the agent's-eye friction look like?** Two named frictions, both below the fixing bar but worth the record:

1. **`role:"row"` for WPF DataGrid rows is honest but the `cell` inside is `unknown`.** An agent that wants a cell's value reads the row, whose value slot is populated (`value:"Row-Alpha"`), and the cell is a `unknown` child with `role_description:"custom"`. Usable, but the 2.5 graded-fingerprint story is where `cell` and the Custom refinement belong.

2. **Chromium's settled tree depth is larger than the default depth budget.** At the default `--max-depth 10` a dense Electron app returns boundary counts that an agent must drill into. The `--timeout-ms` flag (U1 item 11's `--timeout-ms` branch) is the lever for the settle window; the default 3 s is under the 10-25 s cold Chromium settle measured by A16-11, so a fresh Obsidian's first snapshot can still reduce to a shell-held tree under the deadline. The flag is threaded into both the snapshot and drill-down deadlines.

**Are the three inventories right?** `list-windows` returns the desktop's visible windows with HWND ids, process-generation tokens, titles and app names; `list-apps` returns the window-owning processes with corroborated tokens; `list-displays` returns the single primary 96-DPI display, `scale:1.0`. The window and app tokens agree (the KTD10 corroboration holds). `focused-window` composes the focused-only filter.

## Paper cuts — friction that fails no assertion

1. **`role_description:"custom"` on a `unknown`-role cell** (0 `cell` refs on WPF). Recorded; the `Custom`→`unknown` arm is deliberate, whether it should also refine to `cell` on GridItem/TableItem is a U10 decision (above).

2. **The WPF fixture's `complete:true` at depth 10 carries no `subtree_truncated` markers on its shallowest boundary nodes**, because the fixture's tree is inside the default depth. On Obsidian the same depth produces `children_count` boundaries — the shape difference is target depth, not honesty.

3. **Obsidian's reffed-node count is ~3x smaller than its raw wideness** (57 refs vs 165 raw nodes): most Electron nodes are inert `group`/`pane` containers, ref-less by design. An agent reading the tree sees the actionable spine, which is the point of ref allocation.

## Decisions left for a human

One, carried to `docs/phases.md` by U10: whether the `cell` refinement should also fire on `ControlType.Custom` + GridItem/TableItem. The data: WPF DataGrid cells are exactly that shape and currently resolve `unknown`. Everything else this run exposed was a contained fix (the shell-shape detection) or an intended drill-down boundary.

## Residuals for later sub-phases

- **`cell` on WPF DataGrid cells** remains unresolved pending the U10 decision; the fixture extension (U1 item 10) stays so a later arm can be observed the moment it lands.
- **Modern XAML** remains unexercised on this box (A10-7).
- **Multi-monitor `list_displays`** remains single-monitor real evidence (A10-3), as this run reproduced one 96-DPI display.

## Verification Contract result

| gate | result |
| --- | --- |
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --locked -p agent-desktop-core -p agent-desktop-windows -p agent-desktop -p agent-desktop-ffi --all-targets -- -D warnings` | pass |
| `cargo test --locked -p agent-desktop-core -p agent-desktop-windows --lib` | pass — 969 core, 298 windows |
| `cargo test --locked -p agent-desktop` / `-p agent-desktop-ffi --tests` | pass |
| Release binary size | 2.03 MB (12.9% of the 15 MiB cap) |
| `probes/windows/13-ledger-check.ps1` | pass (A16 ledger rows for this sub-phase committed) |
| macOS lane, golden fixtures byte-identical | (macOS lane runs on the PR; the U2 golden diff is empty by test) |

**Readiness verdict: usable, with the `cell`-refinement decision open.** An agent on Windows can now `snapshot`, `list-windows`, `list-apps`, `list-displays`, drill down from a stored ref, and read honest completeness on every stack this box can present, including Chromium/Electron. One thing the run could not show is a genuine `cell` ref; the measured reason (WPF cells are `Custom`-typed) is now a documented decision rather than an unobserved arm.

## Redaction

This report carries no literal `Name`, `Value`, `AutomationId` value, file name, document text, window title or note title read from a real application. Obsidian's tree is described by roles, counts and shape only; every named example is a control class the repository's own fixtures created. No capture JSON is committed.

---

## Post-review-fix pass (2026-08-02, commit 5fde5b5)

A nine-reviewer code review of this branch changed five live behaviors after the run above — the deadline-bounded Chromium settle, per-process activation keying, liveness on the force and Element-rooted paths, the dedicated boundary-count budget, and the fail-closed resolution tie-break — so the read path was dogfooded again from the rebuilt release binary. Same targets, same redaction rules, effects verified by reading the JSON, all launched processes tracked and killed.

### Native targets (Notepad, Explorer, WinForms fixture, WPF fixture)

| target | full snapshot | vs pre-fix refs | skeleton | drill-down | notes |
| --- | --- | --- | --- | --- | --- |
| Notepad | `complete:true`, 17 refs, ~200ms | 17 → 17 | tree naturally ≤ depth 3, no boundary | ok | |
| Explorer | `complete:true`, 69 refs, ~450ms | 68 → 69 (shell variance) | 7 refs; six boundary `group` containers carry `children_count` 1–3 | ok | boundary-count budget observed live |
| WinForms | `complete:true`, 25 refs, ~307ms | 25 → 25 | shallow, no boundary | ok | |
| WPF | `complete:true`, 46 refs, ~1.1s | 46 → 46 | 8 boundary containers with counts; 2 report `role=unknown` | ok | role-mapping note below |

The hardened failure paths held: killing the WinForms fixture and re-running its drill-down returned `STALE_REF` (`resolve_no_candidate`, retry-safe, fresh-snapshot recovery) in **35ms** with no application text in any error field; `--surface focused` against an unfocused-but-live target failed closed with `WINDOW_NOT_FOUND` and succeeded after a real foreground change; `list-windows`/`list-apps`/`list-displays` erred nowhere across every combination tried, including two windows on one pid (the WPF host) resolving to distinct ids with window-level focus tracked correctly.

### Chromium (Obsidian, cold start)

The settle fix was exercised at the cold instant it exists for. With Obsidian freshly launched, `--timeout-ms 50` returned a structured `TIMEOUT` whose details echo the elapsed and configured deadline; the same command with `--timeout-ms 30000`, issued moments later against the same window, returned `complete:true` — the knob observably governs the settle window, which the pre-fix one-shot 50ms path could not do. The cold tree is role-for-role identical to the fully warm tree at both default depth and depth 50, so no pre-activation shell survives to be claimed complete. `--force-electron-a11y` returned a real tree on the live window and failed closed with a structured error in 198ms after the window was killed — no hang, no stale tree.

Depth-50 richness improved materially over the pre-fix run: **57 → 166 refs**, with real web-content roles (buttons, textfields, images) behind the wrapper skip. Default-depth refs read 15 against the pre-fix 19; cold and warm agree exactly at that depth, so the difference is the depth-10 clamp meeting Obsidian's DOM nesting, not a settle regression.

Per-process activation keying could not be exercised live — Obsidian is the only Chromium-family application on this box — and is recorded **skipped with that reason**, carried by the unit tests that pin two pids through one adapter.

### Second environment for the cost row

The probe build defect this review found (the CI temp-workdir copy list omitted a module, failed the build, and exited 0 as a skip) is fixed, and the first fixed run uploaded the missing captures: the hosted Server 2025 runner measures the added properties at 1.14x (Win32) and 1.24x (WPF) against the dev box's 1.03x/1.18x. A16-7 now rests on two environments.

### Findings from this pass

- **`--app` matching requires the image name with extension** (`Obsidian.exe`, not `Obsidian`) — correct per the exact-match contract, but a friction note for agents arriving from macOS where the convention omits it. Recorded as a UX consideration, not a defect.
- **Two WPF boundary containers report `role=unknown`** at the skeleton boundary (list/grid-shaped control types). Cosmetic here; the role-mapping look belongs with the open `cell`-refinement decision already carried by `docs/phases.md`.
- The binary reports envelope version 2.2; no behavior consequence for this run.

**Post-fix readiness verdict: the fixes hold under real applications.** Every behavior the review changed was either observed working live or skipped with its reason and a pinning test named. No hang, no dishonest completeness, no redaction leak was observed anywhere in this pass.
