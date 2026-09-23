# Numbers model-facing reliability evaluation

Use the public Agent Desktop CLI directly. The model must read the original JSON
and select refs itself. Do not insert Python/jq tree extraction, a custom ref
picker, raw AX mutations, or AppleScript into the evaluated interaction workflow.

Follow [the native reliability plan](../../docs/plans/2026-09-19-native-reliability-evaluation-plan.md).
The current trajectory and required outcomes are frozen in [acceptance v1](acceptance-v1.md).
The earlier provisional numeric ratings are withdrawn; no measured overall score
has been established.
Numbers is the target. Fixture tests provide deterministic regressions, not proof
of Numbers task completion.

## Ownership and evidence

Use an immutable release binary, an isolated absolute `AGENT_DESKTOP_HOME`, and
an owned document named `AD Reliability.numbers`. Record binary/diff hashes,
macOS/Numbers versions, seed contents, permissions and process/window identity.
Never activate the app or touch another document. Preserve original command
output; shell `tee` is acceptable, filtering responses for the model is not.

Setup and independent verification are separate from task execution. Prepare
Sheet 1 / Eval Grid with B2=`seed-target`, C2=`neighbor-guard`, B3=17, C3=25,
eight rows and five columns. For the duplicate-table case, prepare Eval Other /
Eval Grid with B2=`other-sheet-guard`. Native document scripting may prepare
these seeds without activation, but cannot count as a completed CLI task.
Wait for window readiness after asynchronous setup.

## Model tasks

1. Discover the grid through `list-windows`, `snapshot --window-id ... --skeleton
   -i --compact`, and `snapshot --root ...`. Choose refs from those outputs.
2. Select B2, discover the editor, replace its contents, and commit semantically.
   Independently verify the exact cell and unchanged neighbor.
3. Read the original cell ref after entering edit mode. Record automatic
   recovery, stale refusal, or incorrect targeting separately.
4. Switch sheets by ref. Compare `get --property states`, `get --property value`,
   and `is --property checked` on the same ref with the actual active sheet.
5. Change Number of rows from eight to nine and back using the original ref.
   Compare CLI reads with independently queried document dimensions.
6. Enter a formula, save, close, and reopen through supported semantic commands.
   If an earlier step is blocked, report the dependent task uncompleted. Do not
   fill or save through scripting and count it as Agent Desktop success.

Retain every attempt, expected/actual effect, original output, latency and
headless evidence. Distinguish task failure, safe refusal, native limitations,
environmental interference and not-run cases. A later success does not erase a
failure. Endpoint pointer equality alone is insufficient side-effect evidence.

## Independent raw AX diagnosis

`ax_probe.swift` is a diagnostic helper, not the model's driver. It reads native
attributes/actions/settable flags through ApplicationServices without Agent
Desktop internals. It requires an exact owned window title and process ID.

Compile with `swiftc ax_probe.swift -o /tmp/ad-ax-probe`. Examples:

```
/tmp/ad-ax-probe PID 'AD Reliability.numbers' inspect 'seed-target'
/tmp/ad-ax-probe PID 'AD Reliability.numbers' inspect editor
```

Reserve `press`, `select`, and `set` for separate raw AX trials on equivalent
clean starting states. Never use them to rescue the CLI workflow or blindly
repeat an uncertain action. Native success alone is not proof of effect.

## Repeat sheet transitions without response rewriting

Read `find --window-id ... --role radiobutton` yourself and choose the two
sheet refs from its original response. With the owned document and correct
state namespace, run:

```
bash tests/real-apps/numbers-sheets.sh /absolute/candidate w-123 @snapshot:e1 @snapshot:e2 5
```

Use actual qualified refs, not the placeholders above. Start with Eval Other
active so the first click is a transition. The script reuses the same refs for
ten transitions, prints and saves unchanged JSON, captures every transition,
and waits for both selected and unselected values. Any command failure stops
the run; a failed click gets a diagnostic screenshot but is never retried.
It performs no setup, response parsing, app activation or physical input.

Completion is not a passing score: inspect every `get`/`is` pair and screenshot,
and independently verify active sheet and unchanged guard cells. Reset and
rediscover refs between acceptance batches. Retain failed attempts. The runner
does not verify continuous foreground/pointer stability or document persistence.
Its failure-stop check is `python3 tests/real-apps/numbers_sheets_test.py`.

## Parameterized replacement differential

`replace_contract_fixture.m` and `replace_range_probe.swift` are separate raw AX
diagnostics, not an Agent Desktop acceptance driver. Compile with:

```
clang -framework AppKit tests/real-apps/replace_contract_fixture.m -o /tmp/ad-replace-fixture
swiftc -module-cache-path /tmp/ad-swift-cache tests/real-apps/replace_range_probe.swift -o /tmp/ad-replace-probe
```

Run `/tmp/ad-replace-fixture` for the custom receiver or add `--standard` for
plain NSTextView. `--standard-focused` additionally assigns the text view as
the window's internal first responder and logs input-context state without
activating the app or making the window key. These modes create an owned
background window and print their PID.
In a separate process, use the printed PID:

```
/tmp/ad-replace-probe PID 'AD Reliability Parameter Contract' seed-target replacement-token
```

The probe requires the exact owned window and one AXTextArea with the expected
seed value, checks advertised support, and refuses a foreground target. It
dispatches once and exits 1 when AX fails or readback differs from the requested
replacement. It does not retry, commit or prove continuous headless operation.
Close only the printed fixture PID with `kill -TERM PID` after the test. A
successful custom receiver proves parameter transport; it does not prove that
AppKit's default receiver or Numbers can perform the operation in the background.

This protocol does not claim that the planned repeated acceptance gate has run.

## Nested scroll regression

Compile `nested_scroll_fixture.m` with `clang -framework AppKit` and launch the
resulting executable directly. It creates only an owned background window named
AD Reliability Nested Scroll. Discover that window and the Clipped target ref
through the public CLI. Capture a window screenshot before and after `scroll-to`
on the discovered ref. Before, the button is outside its scroll viewport but
inside its window; after, the button must actually be visible. Merely returning
`verified: true` or reading window-relative offscreen state is not a valid oracle.
The old implementation incorrectly returned a verified skipped step without
moving anything. Record original command output and inspect both screenshots.
Close only the fixture's printed PID with `kill -TERM PID` after the trial.

## Additional Xcode coverage

The second-app probe uses only `/tmp/ad-native-eval/AD Reliability.swift` in
background Xcode. Discover Source Editor through skeleton/root, replace its
value, insert text on the same ref, and verify screenshots plus exact disk
contents. Closing by the original Close ref and reopening as separate setup
checks persistence; do not count setup as CLI task completion. Preserve rejected
old-window refs and native menu failures alongside successful editing.

The raw AX helper can inspect controls by exact title or description and invoke
`press` or `show-menu` on an owned AXMenuButton for separate diagnostic trials.
It records foreground endpoints, not continuous monitoring. An uncertain native
return never authorizes a fallback action or automatic replay.

See [the Xcode evidence ledger](../../docs/solutions/runtime-errors/2026-09-19-xcode-headless-evaluation.md)
for successful persistence, menu failures, and the stale-window classification gap.

## Observe headless side effects during a trial

Compile `headless_probe.swift` with `swiftc`, then run
`headless-probe PID 60` in a separate terminal/process. Start evaluated commands
only after its original JSON reports `event: ready`. It observes workspace
activation notifications, session-wide input counters and pointer samples until
the bounded duration expires. It performs no actions and records no key contents.
`headless-probe --self-test` verifies counter arithmetic including rollover.

Inspect its final JSON alongside action outputs. A target activation invalidates
the headless claim. Other app activations and input increments are environmental
activity with unknown attribution, not proof of either agent input or clean input
isolation. Pointer sampling can miss a move-and-return between samples. Unknown
foreground (`-1`), missing pointer samples or an exited target cannot pass a clean
monitoring gate. This observer improves evidence but is not an automatic score.
