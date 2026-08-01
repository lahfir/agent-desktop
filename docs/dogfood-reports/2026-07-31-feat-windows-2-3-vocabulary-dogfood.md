# Dogfood report — Windows vocabulary (sub-phase 2.3)

**Date:** 2026-07-31 · **Branch:** `feat/windows-2.3-vocabulary` · **Plan:** `docs/plans/2026-07-31-001-feat-windows-vocabulary-roles-states-plan.md`

A role map cannot be validated by a test that restates it. This is the run that establishes whether the vocabulary is correct: the tool pointed at software nobody in this repository wrote, the output read, and what was wrong fixed.

## Environment

| fact | value |
| --- | --- |
| OS | Windows Server 2019 Datacenter, build 17763 |
| UIA runtime | UIA3 COM (`CUIAutomation8`), `uiautomation` crate 0.25.0 |
| Client stack | `uia3-com` — the stack the adapter ships, not the managed stack 2.0's dumps used |
| Walker | `RawViewWalker` (what 2.2 shipped), with the ControlView delta reported per target |
| Targets | classic Notepad, Explorer, WinForms fixture, WPF fixture |
| Captures | **not committed** — thousands of lines of JSON, regenerated on demand by the runner below. This report is the durable record |
| Runner | `probes/windows/scratch/run-dogfood.ps1` |

**Stack coverage is Win32, DirectUI shell, WinForms and WPF.** A10-7 records that this box carries no WinUI3 or MSIX population, and no Chromium application is installed. Modern XAML vocabulary stays unmeasured until 2.12, and this report does not imply the sweep was complete.

## Targets

Every target shows **repo-controlled content**. The run creates a scratch directory with synthetic file names and opens Notepad and Explorer on that, never on the developer's own documents — which is both the safety rule and the better measurement, since a folder of known contents is reproducible and has nothing to redact.

| target | UI stack | outcome | nodes | capture |
| --- | --- | --- | --- | --- |
| classic Notepad on a scratch file | Win32, client-side `EDIT` proxy | captured | 26 | `notepad.json` |
| Explorer on a scratch directory | DirectUI shell | captured | 113 | `explorer.json` |
| WinForms scratch fixture | WinForms | captured | 43 | `winforms.json` |
| WPF scratch fixture | WPF | captured | 82 | `wpf.json` |
| Chromium / Electron | — | **SKIPPED** — no window of class `Chrome_WidgetWin_1` exists on this box; none is installed | — | — |
| Settings | — | **SKIPPED** — `SystemSettings.exe` is not running and this Server 2019 image presents no modern Settings app (A10-7) | — | — |

A skipped target is not a green one. Two of six targets were not reachable, and the Chromium gap matters most: A7-1 measured Electron at **0% `AutomationId` coverage on 8 interactive elements**, so the stack most likely to stress the vocabulary is the one this run could not exercise.

The census walked to the same node count as the shipped walker on all four targets (26/26, 113/113, 43/43, 82/82), with zero enumeration failures and `structurally_complete` true everywhere — so the numbers below describe whole trees, not truncated ones.

## Findings

### Fixed: `invalid` was emitted on every node of every target

The defect the run existed to find. Microsoft's ARIA state table gives `IsDataValidForForm` as the source for `invalid`, and the producer read it as written: `false` → `invalid`.

Run against real software it emitted `invalid` on **26 of 26** Notepad nodes, **113 of 113** Explorer nodes, and every node of both fixtures — on static text, title bars, windows, scrollbars and menus alike.

`false` is that property's *default*, not an assertion: it means no form rule declares the element valid, which is true of everything that is not a form field. The neighbouring `IsRequiredForForm` shares the default and is safe only because `required` is emitted on `true`, a positive claim. `invalid` read the default as a claim.

The property cannot distinguish "not applicable" from "invalid" on any stack measured here, so it is no longer read at all and `invalid` is **unproduced on Windows** — which is what KTD6 requires of a token whose platform source turns out unusable.

- **Root cause:** a state emitted on a property's default value rather than on a positive claim.
- **Regression tests:** `a_default_false_form_validity_flag_produces_no_invalid_token` and `invalid_is_unproduced_whatever_the_read_set_says` in `states_tests.rs`. Both were **observed failing** against the reintroduced defect, with the message `a statictext was reported invalid because a form flag defaulted to false`. The first also asserts the sibling `required` arm still fires, so it cannot pass by disabling both.
- **Effect:** nodes carrying a state fell from 100% to 14/26, 49/113, 1/43 and 26/82. States now discriminate.

This is the same failure shape KTD4 anticipated for actions — a signal that is true of everything carries no information — reproduced for states, from a source the vendor's own table endorsed. It was invisible to every unit test, because a unit test asserts what its author already believed.

### Fixed: the census could stop at its depth limit without saying so

`census_truncated` was set by the deadline, the cycle guard and the sibling cap, but a walk that stopped at `--max-depth` returned silently. A capture that stopped early would read as one that covered everything. The depth cutoff now marks truncation. It did not fire on this run — every target's census matched the shipped walk's node count — which is why it needed fixing before it mattered rather than after.

## Judgements, per target

**Which `ControlType`s resolved to `unknown`?** One, on two targets: `Custom` (50025) — 1 node on Explorer, 6 on WPF. That is the settled arm, not a gap: `Custom` carries no semantics by definition and a guess would be worse than honesty. Every other control type observed on every target resolved to a canonical role. Notepad and WinForms resolved **100%**.

**Is each resolved role right?** The cases worth naming, each judged against what the control actually is:

- Notepad's edit surface reports `ControlType.Document` and resolves to **`textfield`** — A2-4's counterexample, which is the reason `ControlType` alone was ruled insufficient, coming out correct through the `Document` + `Value` refinement.
- Explorer's tab strip resolves to **`tablist`** and its pages to **`tab`** — the UIA/ARIA inversion, confirmed against a real provider rather than against Microsoft's table alone.
- Explorer's `SplitButton`s resolve to **`menubutton`**, and its `Button`s advertising ExpandCollapse also resolve to `menubutton` — the refinement firing where affordance disagrees with presentation.
- WPF's `DataItem` rows resolve to **`row`**, not `cell`, because they advertise neither `GridItem` nor `TableItem`. That is the refinement's else-branch behaving correctly for row-level elements; **the `cell` branch remains unobserved.**

**Which `INTERACTIVE_ROLES` appeared?** Across the four targets: `button`, `checkbox`, `combobox`, `incrementor`, `listbox`, `menubutton`, `menuitem`, `option`, `radiobutton`, `tab`, `textfield`, `treeitem`. `colorwell` and `dockitem` are asserted unproduced in the map itself — no Windows control type reaches either.

**Were the four never-observed ref-able types exercised?** Yes, all four, which was the point of extending the fixtures:

| `ControlType` | role | first observed on |
| --- | --- | --- |
| `Tab` 50018 | `tablist` | Explorer, WPF |
| `TabItem` 50019 | `tab` | Explorer, WPF |
| `Spinner` 50016 | `incrementor` | WinForms |
| `DataItem` 50029 | `row` | WPF |

`DataGrid` (50028 → `grid`) and `HeaderItem` (50035 → `column`) were observed for the first time too.

**Do the state tokens make sense?** After the `invalid` fix, yes, and they discriminate. `haspopup` appears on menu items and split buttons and nowhere else — A15-6 predicted exactly that from the MSAA bitmask, and this is the prediction landing. `disabled` appears on Notepad's greyed toolbar buttons. `expanded` and `selected` appear on Explorer tree items. `secure` appears on exactly one WPF node, the `PasswordBox`. `offscreen` appears on four WPF nodes and is never inherited by descendants.

**Does the action list distinguish the actionable from the inert?** Yes. 21 of 26 Notepad nodes, 98 of 113 Explorer nodes, 28 of 43 and 52 of 82 advertise an action — so the list is neither universally empty nor universally populated. Static text, images and groups come out with `[]`. **This is the KTD4 trap not springing at scale:** `IsLegacyIAccessiblePatternAvailable` is true on every element of every one of these targets, and gating on `DefaultAction` is what keeps 15 Notepad nodes and 15 Explorer nodes correctly inert.

**Non-blank `AutomationId` coverage, against A7-1:** Explorer **49/113 (43%)**, WPF **52/82 (63%)**, WinForms **40/43 (93%)**, Notepad **14/26 (54%)**. A7-1 measured Explorer at 97.6% and WPF at 100% *of interactive elements*; these are percentages of **all** nodes and are counted **non-blank**, so the two number families are not comparable and the capture records which rule it used. Restricted to nodes carrying an action, coverage is far closer to A7-1's. The counting-rule correction was made precisely so this comparison stops being made by accident.

**Is the name evidence usable?** Broadly yes: 22/26, 91/113, 35/43 and 45/82 nodes carry a non-blank name. Explorer's 91/113 is the strongest signal, since its names come from a real shell provider.

**The agent's-eye question — could you find the control you wanted and tell it apart from its siblings?** Mostly, with two frictions recorded below. Explorer's tree items and list options carry names, roles, actions and states that together disambiguate them. Notepad's menu bar is navigable by role and name. The WinForms fixture's 27 `Pane` nodes all resolving to `group` is the one place where an agent reading the tree would have to fall back on position.

## Paper cuts — friction that fails no assertion

These are findings, recorded rather than fixed, each with what it would take to settle it.

1. **`SetValue` appears on elements no agent would type into.** Notepad's title bar, Explorer's toolbars and progress bar advertise `SetValue`, because their providers implement `ValuePattern` and report it not-read-only. The action list is reporting the provider truthfully; whether the *product* should surface a value-setting action on a title bar is a question for 2.6/2.7, which owns invocation and can discover that the call fails. Not fixed here: suppressing it would mean the adapter second-guessing a provider's own advertisement, which is a policy decision, not a mapping fix.

2. **`readonly` appears on images, static text and groups on Explorer.** The token is correctly gated on `IsValuePatternAvailable`, so these elements really do advertise a read-only value. It is truthful and low-value. Same owner as the above.

3. **The WinForms fixture yields 27 `Pane` nodes, all `group`.** Container-heavy trees give an agent little to steer by. `Pane` → `group` is the honest default and the alternative (guessing a semantic role) is worse; what would actually help is 2.4's surface detection and the `subrole` field, which is where this belongs.

4. **`Custom` → `unknown` is correct but opaque.** Six WPF nodes and one Explorer node give an agent nothing. `AriaRole` is the natural refinement and is deliberately 2.4's, per KTD2.

## Decisions left for a human

None. The one defect found was a contained fix with a regression test that was observed failing. The paper cuts above are all assigned to a later sub-phase by the existing scope boundaries rather than needing a new decision.

## Residuals for later sub-phases

- **Chromium/Electron and modern XAML remain unexercised on this box.** 2.12 owns the self-hosted runner that would present them. A7-1's 0%-`AutomationId` Electron measurement means the vocabulary's behaviour on that stack is inferred, not observed.
- **`DataItem` → `cell`** (the `GridItem`/`TableItem` branch) has still never run. `docs/phases.md` §2.4 carries it.
- **`Switch`** is reachable only through `Button` + `Toggle` refinement, and no target advertised that combination, so the arm has not run.

## Verification Contract result

Run on the dev box after the last fix:

| gate | result |
| --- | --- |
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --locked -p agent-desktop-core -p agent-desktop-windows -p agent-desktop -p agent-desktop-ffi --all-targets -- -D warnings` | pass |
| `cargo test --locked -p agent-desktop-core -p agent-desktop-windows --lib` | pass — 953 core, 230 windows |
| `cargo test --locked -p agent-desktop` / `-p agent-desktop-ffi --tests` | pass |
| `cargo check --locked -p agent-desktop-windows --all-targets --target x86_64-unknown-linux-gnu` | pass |
| `probes/windows/13-ledger-check.ps1` | pass |
| macOS lane, golden fixtures byte-identical | pass — 490 macOS tests, no fixture changed |

**Readiness verdict: ready.** The vocabulary resolved every control type observed on four UI stacks except `Custom`, which is the deliberate arm; the one real defect it exposed is fixed and pinned by a regression test observed failing; and the two unreachable stacks are recorded as skipped with their reasons rather than reported green. A green suite alone would not have supported this verdict — the defect that mattered most passed every unit test in the sub-phase.

## Redaction

This report carries no literal `Name`, `HelpText`, `FullDescription`, `ItemStatus`, `AutomationId` value, file name, document text, or window title read from a real application. Findings are described by control type, role, state token, action name and shape, and every example is either a control class the run created itself or a description rather than a quotation.

**No census is committed**, so the strongest form of the leak risk — a real application's text living in the repository permanently — does not arise at all. The rule still governs the censuses the runner writes locally: every value-bearing property is recorded as presence and length only, and process ids, provider ids and user paths are substituted at render time. That rule lives in `crates/windows/examples/uia_tree_dump/render_slots.rs`, beside the code that applies it.
