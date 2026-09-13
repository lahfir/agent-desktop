# Sub-phase 2.12 dogfood — fixture app and live e2e harness

Release binary `target/release/agent-desktop.exe`, 2,382,336 bytes, driven against
real software on the dev box (Windows Server 2019, build 17763, single 1639×732
display, High-integrity Administrator, console session 1).

A report with no findings is a failed dogfood. This one carries five, each with
exactly one of three dispositions: fixed here with an invert-verified test,
owned elsewhere with the receiving sub-phase's scope written into
`docs/phases.md` in this same PR, or accepted with a stated reason.

Every effect below is judged by **independent re-observation**, never by the
command's own `ok:true` — that is the harness contract this sub-phase ships, and
the dogfood is held to it too.

## Legs driven

| id | target | what was driven | outcome |
|---|---|---|---|
| J1 | Notepad | `list-apps` after launch | resolves with `pid` and `process_instance` |
| J2 | Notepad | `snapshot --app -i` | `ok`, 20 nodes, 17 refs, `complete: true`, depth 3 |
| J3 | Notepad | `set-value` on the `textfield` ref | `delivered_verified`; a **fresh snapshot** reads back `value: dogfood-2-12` |
| J4 | File Explorer | `snapshot --app -i` over a real folder | `ok`, 135 nodes, 103 refs, `complete: true`, depth 9 |
| J5 | Task Manager | `snapshot --app -i` against a High-integrity target | `ok`, 34 nodes, 28 refs, `complete: true` |
| J6 | Notepad | ref action after the owning process is killed | see F3 |

J3 also confirms the new Windows lease is live on the action path rather than
merely unit-tested: the envelope carries `auto_wait.lease_hold_ms: 25` and
`lease_contention_count: 0`.

## Cost baseline

Probe-corpus methodology — min-of-seven with the warm-up discarded, reported as
min with median and max beside it (`A15-13`, applied in `A18-7`). The macOS
`scripts/perf-baseline-compare.sh` is structurally macOS-bound and is not the
vehicle here. Capture: `probes/windows/24-fixture-e2e/captures/cost-baseline-devbox.json`.

| command | min | median | max |
|---|---|---|---|
| `snapshot --app notepad.exe -i` | 142.8 ms | 155.4 ms | 184.7 ms |
| `snapshot --app explorer.exe -i` | 62.9 ms | 65.4 ms | 82.2 ms |
| `list-apps` | 22.1 ms | 22.8 ms | 24.0 ms |
| `list-windows` | 59.6 ms | 65.3 ms | 76.4 ms |

## Findings

### F1 — a ref action against a dead process burns its whole budget and then blames the wrong thing

**Observed (J6).** A `textfield` ref is taken from a live Notepad, the process is
killed, and `set-value` is issued against that ref. It returns `TIMEOUT` after
**5,091 ms** at the default budget and **1,127 ms** at `--timeout-ms 1000` — the
full budget in both cases — with `suggestion: "The target application may be
busy or unresponsive"` and `recovery: null`.

The application is not busy. It does not exist. And the product already knows
that on a neighbouring path: `snapshot --app notepad.exe` against the very same
dead process returns `APP_NOT_FOUND` **immediately**.

**Why it matters.** The envelope is actively misleading rather than merely
imprecise. `TIMEOUT` plus "may be busy" plus `retry: safe` tells an agent to
wait and try again against a process that can never come back, and `recovery`
is `null`, so the one field that would carry a real strategy is empty. The
documented answer for a ref that cannot be resolved is `STALE_REF` with
`recovery.strategy: refresh_snapshot_then_retry_original` — which would send the
agent to re-snapshot, the only action that can actually make progress. The cost
is paid twice: the budget is spent, and then it is spent again on a retry the
envelope invited.

**Root cause.** `resolve_element_strict` wraps its attempt in
`retry_incomplete_until(deadline, …)` (`crates/windows/src/tree/resolve.rs`), so a
resolution failure classified as *incomplete* is retried until the deadline
rather than answered terminally. A vanished owning process is knowable on the
first attempt and is not an incomplete read.

**Disposition: owned elsewhere — §2.15**, written into `docs/phases.md` in this
PR. Not fixed here, for a stated reason rather than a convenient one: the change
is to the resolver's retryability classification, which is §2.5's contract and
is reached by *every* ref action on the adapter — click, set-value, toggle,
expand, select, scroll. §2.15 already owns the adjacent cluster (the resolver
error-payload promotion into core, and the `stale_ref` constructor being handed
a sentence where it expects a ref id), and this is the same family of defect in
the same code path. Landing a cross-cutting envelope change inside a
fixture-and-harness sub-phase would put it in front of reviewers reading a
harness diff.

### F2 — `APP_NOT_FOUND` still carries no suggestion, confirmed on a second surface

**Observed (J6).** `snapshot --app notepad.exe` against a dead process returns
`APP_NOT_FOUND` with `suggestion: None`.

**Why it matters.** This is the failure an agent meets first when it holds a
stale idea of what is running, and the envelope offers no route forward. It is
the same shape §2.11's dogfood recorded from the `wait` surface; seeing it again
from `snapshot` establishes it is a property of the shared lookup rather than of
one command.

**Disposition: owned elsewhere — §2.15**, which already carries the `--app`
resolution-envelope entry from §2.11's dogfood. This report adds the second
surface as corroboration rather than opening a duplicate item.

### F3 — `AppInfo.presentation` is absent on Windows

**Observed (J1).** Every `list-apps` row carries `name`, `pid` and
`process_instance`; none carries `presentation`, which the macOS adapter
populates.

**Why it matters.** Identical JSON across platforms is a product promise. An
agent written against macOS that branches on `presentation` silently takes the
absent path on Windows rather than failing loudly.

**Disposition: owned elsewhere — §2.15.** It sits with the `renderer` divergence
already recorded there: both are `LaunchResult`/`AppInfo` fields macOS populates
from bundle metadata that Windows has no equivalent source for, and deciding one
without the other would split a single contract question across two gates.

### F4 — snapshot cost is not proportional to tree size

**Observed.** Notepad's 20-node tree costs **142.8 ms**; Explorer's 135-node tree
costs **62.9 ms**. A tree 6.75× larger is read in 44% of the time — per-node cost
differs by roughly an order of magnitude between the two provider stacks.

**Why it matters.** It falsifies the intuitive cost model. Anyone sizing a
budget, a timeout default, or a traversal cap from node count will be wrong in
both directions, and the error is largest exactly where it matters — a small
tree on a slow provider is the case that looks cheap and is not.

**Disposition: accepted**, with the reason stated rather than implied. This is a
measurement, not a defect: both reads are correct and complete, and nothing in
the shipped contract promises proportionality. It is recorded here and in the
cost capture so the next sub-phase to set a budget starts from the measurement
instead of the intuition. Chasing the provider-side difference would be
performance work on Notepad's UIA implementation, which this project does not
own.

### F5 — the fixture's own toolchain assumption was documented wrong, and was corrected by measurement

**Observed (during grounding, before any code was written).**
`docs/phases.md` named WinForms `AutomationProperties.AutomationId` as the
fixture's identity mechanism. That API does not exist on WinForms — it is a WPF
attached property. The mechanism that does exist is `Control.Name`, surfaced as
UIA `AutomationId` by the stock provider, and it needs no `.exe.config` (the
pre-registered hypothesis that the legacy accessibility switches must be
disabled first was measured across both arms and disproven). The in-box compiler
is pre-Roslyn and accepts **C# 5** at most.

**Why it matters.** An implementer following the document would have looked for
an API that does not exist on the framework the same sentence told them to use,
and would then have written C# 6+ against a compiler that rejects it with
`CS1617` rather than a missing-feature diagnostic.

**Disposition: fixed here.** `docs/phases.md` is corrected in place at both the
scope bullet and the **Key APIs** line, citing `A24-1`/`A24-2`/`A24-3`. The
guard against regression is not a unit test but a gate:
`scripts/check-phases-ledger-citations.ps1` fails the build if a cited row does
not exist, if a `closure: 2.12` row is never cited, or if a retired stem
reappears. It was invert-verified by reintroducing a retired stem into a copy of
the document and watching rule (c) fail, and by citing a nonexistent row id and
watching rule (a) fail.

## Safety envelope

Every capture and every line above carries shapes and counts only — no window
titles, file paths, pids, machine names, user names or message text. The single
literal value quoted (`dogfood-2-12`) is a constant this report itself wrote
into a scratch buffer, not observed user content. Enforced by
`scripts/check-capture-redaction.ps1` rather than by author discipline.
