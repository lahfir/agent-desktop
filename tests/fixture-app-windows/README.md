# AgentDeskFixture (Windows)

A WinForms application built and controlled by the Windows live E2E harness
(`tests/e2e-windows/`). It exists so the harness can drive the real
`agent-desktop.exe` binary against a real window and verify every effect by
independent re-observation of the fixture, rather than by the command's own
`ok:true`.

## Build

```powershell
.\build.ps1 [-OutputDir <path>]
```

`csc.exe` is resolved by absolute path
(`%WINDIR%\Microsoft.NET\Framework64\v4.0.30319\csc.exe`), never through
`PATH`, so the build cannot silently pick up a different in-box compiler.
`-OutputDir` defaults to a `build` directory next to this script; the
harness passes its own isolated suite root.

The build is idempotent: it compiles to a temporary file and only replaces
the previous `AgentDeskFixture.exe` once the compile has fully succeeded.

## Toolchain constraint: C# 5 only

The pinned compiler (`csc.exe`, version `4.8.3761.0`, "NET48REL1") is the
legacy pre-Roslyn .NET Framework compiler. It accepts `/langversion:5` at
most; `6` and above fail with `CS1617`. Every `.cs` file in this directory
must therefore avoid string interpolation, expression-bodied members,
`nameof`, and null-conditional operators - none of those constructs compile
under this toolchain regardless of `/langversion`.

## Identity

Every interactive control's `AutomationId` comes from WinForms
`Control.Name`, assigned through `FixtureIdentity.Assign` rather than set
directly, so every control the fixture creates is accounted for. No
`.exe.config` is shipped or required: `Control.Name` surfaces as UIA
`AutomationId` on this toolchain with no `Switch.UseLegacyAccessibilityFeatures`
override needed.

`FixtureIdentity.WriteManifest` writes every id assigned so far, deduplicated
and sorted, to `AgentDeskFixture.ids.txt` beside the running exe on every
launch. The harness diffs this file against its own declared inventory as a
set-equality check in both directions, so a control removed from the fixture
without a matching removal from the harness's inventory (or vice versa)
fails the build instead of silently shrinking coverage.

`WriteManifest` runs exactly once, from `Main`, immediately after the main
form's constructor returns. Every identity-bearing control must therefore be
**constructed** (and so `Assign`ed) by that point, even if it is not yet
**shown** - a control whose construction is deferred until a later user
action (a lazily-built modal dialog, a context menu's items, a duplicate
window opened on demand) will be missing from the manifest even though it
carries a real, working `AutomationId` once it exists. Build every card's
controls eagerly in the form's own constructor and reveal them later,
rather than constructing them on demand.

## Status readouts

`FixtureStatus` is the one primitive every later effect assertion rests on:
a read-only `TextBox` (`ControlType.Edit`, carrying `ValuePattern`) whose
`AutomationId` is its identity and whose live `.Text` is its value - two
separate slots, so a value change can never be mistaken for an identity
change. Every status starts at the sentinel `idle`; every write through
`FixtureStatus.SetValue` appends a monotonically increasing per-status
counter to the assigned value, so no two writes to the same status ever
produce the same text and no leg can pass on a value an earlier leg left
behind.

Read a status through `agent-desktop get --property value`, never through
`--property text` or `--property name` - `value` is the property a
`ValuePattern`-bearing `Edit` answers and a `Label` cannot, which is what
makes the read itself discriminate a correct implementation from the
one-slot `Label` shape it must not regress to.

Not every `TextBox` readout goes through `FixtureStatus.SetValue`'s counter.
Two kinds exist, and a scenario author has to know which is which before
writing an equality assertion against one:

- **Counter-suffixed markers** (`click-status`, `text-status`, `menu-status`,
  every other `*-status` id): an opaque, monotonically-numbered marker
  (`"clicked#1"`, `"changed#2"`) that proves *an* effect landed, never the
  literal content of the action. A leg asserts "changed from what it was
  before", never a specific string.
- **Literal-value readouts** (`delayed-text`, `appeared-text`,
  `scroll-offset`): built through `FixtureStatus.Create` for the identity
  shape but written to directly (`.Text = "ready"`), bypassing the counter,
  because the exact string *is* the thing under test - the auto-wait leg
  polls `delayed-text` for the literal value `ready`, not for "any value
  different from before".

## Non-Control identity does not surface as AutomationId

`Control.Name` surfaces as UIA `AutomationId` with no `.exe.config` (A24-2),
but that is a property of `Control`, not of every WinForms item type.
Measured live against a running build of this fixture (`agent-desktop find
--native-id <id>`, zero matches in every case below) before it shipped:

- **`ToolStripItem.Name` does not surface as `AutomationId`.** A
  `ToolStripSplitButton` carrying a `Name` for `menu-disclosure` and a
  top-level `ToolStripMenuItem` carrying a `Name` for `menu-fire-item` were
  both unresolvable by `--native-id`, even though the *container*
  `ToolStrip`/`MenuStrip` (a real `Control`) resolved fine.
- **`TreeNode.Name` does not surface as `AutomationId`** either, for the same
  reason - a `TreeNode` is not a `Control`. A stock `TreeView`'s own
  `AutomationId` (a real `Control`) resolved fine; its child nodes did not.

`FixtureIdentity.Assign(ToolStripItem, string)` exists and does assign
`.Name`, matching the shape the stock provider would need if this ever
changes on a future toolchain, but nothing in this fixture relies on it for
resolvability today. Every target that needs to resolve by native id and
needed a `ControlType` no stock control produces is instead a real
`Control` (a `Button`, `Label` or `Panel`) wearing a `WM_GETOBJECT`
override provider (`UiaProviderHost` and its subclasses in
`FixtureCards.cs`) - `menu-disclosure`, `menu-fire-item`, `outline-tree`,
`outline-parent`, `outline-child-a`, `outline-child-b`. `outline-tree` is
therefore not a real `System.Windows.Forms.TreeView`: it is a `Panel`
holding three `Label`s arranged with indentation, each independently
overriding its own `ControlType`.

A registered custom raw provider also does **not** transparently inherit
the host's `InvokePattern` availability the way bounds/enabled/offscreen
merge through `HostRawElementProvider` - measured live against
`menu-fire-item` before this fixture shipped: with no explicit
`IInvokeProvider`, `InvokePattern.Invoke` came back `skipped` and the click
chain's `LegacyIAccessible.DoDefaultAction` fallback reported success
without the Click handler ever running. `ControlTypeOverrideHost` therefore
takes an optional invoke callback and implements `IInvokeProvider` itself
whenever a wrapped target must stay clickable through the CLI.

## Non-activating launch

Set `AGENT_DESKTOP_FIXTURE_NO_ACTIVATE=1` in the launching environment to
have the main window shown via `SW_SHOWNOACTIVATE` instead of stealing the
foreground - the same non-activating launch every probe in this corpus
already uses. Unset (or any value other than `1`), the window activates
normally.

## Process lifetime

`Main()` calls `Application.Run(form)` with the main `AgentDeskFixture`
instance rather than the parameterless overload, so the message loop - and
the process - exits when that specific form closes, regardless of how many
secondary windows (duplicate-title targets, modal dialogs, overlays) happen
to be open at the time.
