#Requires -Version 5.1
<#
.SYNOPSIS
    Cell ref-able role shape probe (area 24, sub-phase 2.12).

.DESCRIPTION
    phases.md's `cell` ref-able role arm expects the DataItem + GridItem/TableItem
    shape. A16-10 (2.4 dogfood) measured that neither stock control produces it:
    WPF DataGrid exposes GridItem/TableItem patterns but on ControlType.Custom
    cells, not ControlType.DataItem; WinForms DataGridView exposes neither
    pattern on its rows at all. Neither stock control can force the exact shape,
    so the open question for 2.12 is whether a *custom* UIA provider can - which
    would tell U9/U10 whether the fixture can regression-test the `cell` arm at
    all, or whether it must stay unexercised by design.

    Two legs, both always run so the report carries the full picture rather than
    stopping on the first negative:

    1. Reconfirm-stock: launches ScratchForms.exe in --host-providers mode (the
       apples-to-apples WinForms row per A16-10's FixtureModeNote - the fixture's
       own custom collapse-to-Pane provider is switched off so dgvRows exposes
       its real WinForms UIA automation peer) and reads ControlType plus
       GetSupportedPatterns/TryGetCurrentPattern(GridItem/TableItem) at the grid,
       row, and cell levels. This re-measures A16-10 on this box rather than
       trusting the prior finding blind.

    2. Custom-provider (decisive): compiles a minimal WinForms host with csc.exe
       4.8 whose one child control answers WM_GETOBJECT with a hand-written
       IRawElementProviderSimple that also implements IGridItemProvider and
       ITableItemProvider and reports ControlType.DataItem - the exact shape the
       `cell` arm wants. The same compiler and reference assemblies
       (UIAutomationProvider.dll / UIAutomationTypes.dll under the .NET 4.8 WPF
       reference directory) that already build ScratchForms.exe's own
       AutomationIdProvider are reused, so a compile failure here would be a
       genuine toolchain gap, not a probe bug. A UIA client in this same
       PowerShell process then reads the control back by AutomationId and calls
       TryGetCurrentPattern for both patterns. If that succeeds, the fixture CAN
       force the shape and 2.12 can decide whether Custom+GridItem should also
       refine to `cell`; if csc.exe cannot compile it, the exact compiler output
       is captured so the deferral is evidence-based rather than assumed.

    Corpus safety: only ControlType names, pattern names, booleans, and small
    integers (row/column indices from our own fixture code) ever reach the
    capture - no window titles, file paths, pids, machine names, user names, or
    message text. Every process this probe spawns (the ScratchForms.exe leg-1
    instance and the leg-2 compiled host) is killed in that leg's own `finally`
    and again swept at top level from the scratch-process ledger. Every
    temporary file (the leg-2 .cs/.exe pair) is removed in a `finally`.

    Run: powershell -NoProfile -ExecutionPolicy Bypass -File .\probes\windows\24-fixture-e2e\06-cell-role-provider.ps1 -Label <devbox|ci>
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) '..\common.ps1')
Initialize-ProbeRedaction
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes

$script:Probe = '24-fixture-e2e-06-cell-role-provider'
$script:ProbeDir = Split-Path -Parent $PSCommandPath
$script:CaptureDir = Join-Path $script:ProbeDir 'captures'
if (-not (Test-Path -LiteralPath $script:CaptureDir)) {
    New-Item -ItemType Directory -Path $script:CaptureDir -Force | Out-Null
}

$AE = [System.Windows.Automation.AutomationElement]
$TS = [System.Windows.Automation.TreeScope]
$ControlWalker = [System.Windows.Automation.TreeWalker]::ControlViewWalker

Register-MandatoryCapture -Name @("cell-role-provider-$Label.json")

function Write-A24Capture {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Content
    )
    $redacted = Protect-ProbeText -Text $Content
    $path = Join-Path $script:CaptureDir $Name
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    [IO.File]::WriteAllText($path, $redacted, $utf8NoBom)
    $normalized = Get-NormalizedCapture -Text $redacted
    [IO.File]::WriteAllText(($path + '.normalized'), $normalized, $utf8NoBom)
    if (-not (Test-CaptureRedaction -Path $path)) {
        throw "redaction residue in $path"
    }
    return $path
}

<#
    One reading of ControlType + pattern shape for a single element, reused at
    every level of both legs. GetSupportedPatterns() is recorded alongside an
    explicit TryGetCurrentPattern for GridItem/TableItem rather than trusted
    alone - 05-interactions.ps1's own finding is that the declared list and the
    actual acquisition can disagree, and GridItem/TableItem availability is
    exactly what this probe exists to settle.
#>
function Get-ElementRoleShape {
    param([Parameter(Mandatory = $true)]$Element)
    $controlType = 'unread'
    try { $controlType = ($Element.Current.ControlType.ProgrammaticName -replace '^ControlType\.', '') } catch { }
    $supported = @()
    try { $supported = @($Element.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName -replace 'PatternIdentifiers\.Pattern$', '' } | Sort-Object) } catch { }

    $gridItemObj = $null
    $hasGridItem = $false
    try { $hasGridItem = $Element.TryGetCurrentPattern([System.Windows.Automation.GridItemPattern]::Pattern, [ref]$gridItemObj) } catch { }
    $gridRow = $null
    $gridColumn = $null
    if ($hasGridItem -and $null -ne $gridItemObj) {
        try { $gridRow = $gridItemObj.Current.Row; $gridColumn = $gridItemObj.Current.Column } catch { }
    }

    $tableItemObj = $null
    $hasTableItem = $false
    try { $hasTableItem = $Element.TryGetCurrentPattern([System.Windows.Automation.TableItemPattern]::Pattern, [ref]$tableItemObj) } catch { }

    return [ordered]@{
        control_type               = $controlType
        supported_patterns         = @($supported)
        grid_item_pattern_try_get  = $hasGridItem
        grid_item_row              = $gridRow
        grid_item_column           = $gridColumn
        table_item_pattern_try_get = $hasTableItem
    }
}

# ---------------------------------------------------------------- leg 1: stock WinForms DataGridView

function Measure-StockDataGridView {
    $scratchExe = Join-Path (Get-ProbeRoot) 'scratch\bin\ScratchForms.exe'
    if (-not (Test-Path -LiteralPath $scratchExe)) {
        try {
            & powershell -NoProfile -ExecutionPolicy Bypass -File (Join-Path (Get-ProbeRoot) 'scratch\build-scratch.ps1') | Out-Null
        } catch { }
    }
    if (-not (Test-Path -LiteralPath $scratchExe)) {
        return [ordered]@{ measurable = $false; branch = 'scratch_forms_exe_unavailable' }
    }

    $procId = 0
    try {
        $started = Start-ScratchProcess -FilePath $scratchExe -ArgumentList @('--tag', 'a24u6-stock', '--pos', '0,0', '--host-providers') -NoActivate
        $procId = $started.ProcessId
        if ($started.MainWindowHandle -eq [IntPtr]::Zero) {
            return [ordered]@{ measurable = $false; branch = 'scratch_forms_window_not_found' }
        }
        Start-Sleep -Milliseconds 500

        $root = $null
        try { $root = $AE::FromHandle($started.MainWindowHandle) } catch { }
        if ($null -eq $root) {
            return [ordered]@{ measurable = $false; branch = 'uia_from_handle_failed' }
        }

        $cond = New-Object System.Windows.Automation.PropertyCondition($AE::AutomationIdProperty, 'dgvRows')
        $grid = $null
        try { $grid = $root.FindFirst($TS::Descendants, $cond) } catch { }
        if ($null -eq $grid) {
            return [ordered]@{ measurable = $false; branch = 'dgvRows_element_not_found' }
        }

        $gridInfo = Get-ElementRoleShape -Element $grid

        <#
            All direct children of the grid, not just the first: a .NET 4.8
            DataGridView's first ControlView child is typically the column
            header row, not a data row, and sampling only that would mislabel a
            header as `row` and could falsely read as "matches A16-10" even if
            data rows themselves carried GridItem/TableItem. Bounded at 12,
            far above what a 2-column/3-row grid plus a header row and
            scrollbars can realize.
        #>
        $rows = New-Object System.Collections.Generic.List[object]
        $child = $null
        try { $child = $ControlWalker.GetFirstChild($grid) } catch { }
        $visited = 0
        while ($null -ne $child -and $visited -lt 12) {
            $visited++
            $rowInfo = Get-ElementRoleShape -Element $child
            $cellInfo = $null
            $grandchild = $null
            try { $grandchild = $ControlWalker.GetFirstChild($child) } catch { }
            if ($null -ne $grandchild) { $cellInfo = Get-ElementRoleShape -Element $grandchild }
            [void]$rows.Add([ordered]@{ row = $rowInfo; cell = $cellInfo })
            try { $child = $ControlWalker.GetNextSibling($child) } catch { $child = $null }
        }

        $rowHasShape = $false
        $cellHasShape = $false
        foreach ($entry in $rows) {
            if ($null -ne $entry.row -and ($entry.row.grid_item_pattern_try_get -or $entry.row.table_item_pattern_try_get)) { $rowHasShape = $true }
            if ($null -ne $entry.cell -and $entry.cell.control_type -eq 'DataItem' -and ($entry.cell.grid_item_pattern_try_get -or $entry.cell.table_item_pattern_try_get)) { $cellHasShape = $true }
        }
        $matchesA1610 = (-not ($rowHasShape -or $cellHasShape))

        return [ordered]@{
            measurable                                 = $true
            branch                                      = if ($matchesA1610) { 'stock_dgv_matches_a16_10_no_griditem_tableitem' } else { 'stock_dgv_contradicts_a16_10_shape_present' }
            fixture_mode                                = 'host-providers: fixture custom collapse-to-Pane provider is switched off, dgvRows exposes its real WinForms UIA automation peer'
            grid_children_walked                        = $rows.Count
            matches_a16_10_no_griditem_tableitem_on_dgv = $matchesA1610
            grid                                        = $gridInfo
            rows                                        = @($rows)
        }
    } finally {
        if ($procId -gt 0) { try { Stop-ScratchProcess -ProcessId $procId } catch { } }
    }
}

# ---------------------------------------------------------------- leg 2: custom UIA provider (decisive)

<#
    A minimal WinForms host whose one child Panel answers WM_GETOBJECT with a
    hand-written IRawElementProviderSimple that ALSO implements
    IGridItemProvider and ITableItemProvider - the exact pattern combination the
    `cell` arm's DataItem + GridItem/TableItem shape needs. The WM_GETOBJECT
    hook itself (CellHostWindow) is the identical mechanism ScratchForms.cs's
    own AutomationIdProvider/AutomationIdWindow already uses in production in
    this corpus (03-pattern-census's winforms-default fixture), so any failure
    here isolates to the two new interfaces, not to the hook technique.
    ContainingGrid intentionally returns the cell's own provider rather than a
    distinct grid provider - a known simplification, reported via
    containing_grid_is_self so a reader does not mistake it for a full grid
    shape; the row/column values (2, 1) are fixture-authored constants, not
    read off any real data.
#>
$script:CellProviderSource = @'
using System;
using System.Drawing;
using System.Windows.Automation;
using System.Windows.Automation.Provider;
using System.Windows.Forms;

namespace AgentDesktopProbe.A24U6
{
    public class CellProvider : IRawElementProviderSimple, IGridItemProvider, ITableItemProvider
    {
        private readonly IntPtr handle;

        public CellProvider(IntPtr handle)
        {
            this.handle = handle;
        }

        public ProviderOptions ProviderOptions
        {
            get { return ProviderOptions.ServerSideProvider; }
        }

        public object GetPatternProvider(int patternId)
        {
            if (patternId == GridItemPatternIdentifiers.Pattern.Id) { return this; }
            if (patternId == TableItemPatternIdentifiers.Pattern.Id) { return this; }
            return null;
        }

        public object GetPropertyValue(int propertyId)
        {
            if (propertyId == AutomationElementIdentifiers.ControlTypeProperty.Id) { return ControlType.DataItem.Id; }
            if (propertyId == AutomationElementIdentifiers.AutomationIdProperty.Id) { return "probeCellProvider"; }
            if (propertyId == AutomationElementIdentifiers.NameProperty.Id) { return "probe-cell"; }
            if (propertyId == AutomationElementIdentifiers.IsGridItemPatternAvailableProperty.Id) { return true; }
            if (propertyId == AutomationElementIdentifiers.IsTableItemPatternAvailableProperty.Id) { return true; }
            return null;
        }

        public IRawElementProviderSimple HostRawElementProvider
        {
            get { return AutomationInteropProvider.HostProviderFromHandle(handle); }
        }

        public int Row { get { return 2; } }
        public int Column { get { return 1; } }
        public int RowSpan { get { return 1; } }
        public int ColumnSpan { get { return 1; } }
        public IRawElementProviderSimple ContainingGrid { get { return this; } }

        public IRawElementProviderSimple[] GetRowHeaderItems() { return new IRawElementProviderSimple[0]; }
        public IRawElementProviderSimple[] GetColumnHeaderItems() { return new IRawElementProviderSimple[0]; }
    }

    public class CellHostWindow : NativeWindow
    {
        private const int WmGetObject = 0x003D;
        private static readonly IntPtr UiaRootObjectId = new IntPtr(-25);
        private CellProvider provider;

        protected override void WndProc(ref Message m)
        {
            if (m.Msg == WmGetObject && m.LParam == UiaRootObjectId)
            {
                if (provider == null) { provider = new CellProvider(m.HWnd); }
                m.Result = AutomationInteropProvider.ReturnRawElementProvider(m.HWnd, m.WParam, m.LParam, provider);
                return;
            }
            base.WndProc(ref m);
        }
    }

    public class ProbeHostForm : Form
    {
        private readonly Panel cellPanel = new Panel();
        private readonly CellHostWindow cellWindow = new CellHostWindow();

        public ProbeHostForm()
        {
            Name = "frmA24U6ProbeHost";
            Text = "AgentDesktop Probe A24U6";
            ClientSize = new Size(200, 120);
            StartPosition = FormStartPosition.Manual;
            Location = new Point(0, 0);
            cellPanel.Name = "pnlCell";
            cellPanel.SetBounds(10, 10, 160, 80);
            Controls.Add(cellPanel);
        }

        protected override bool ShowWithoutActivation
        {
            get { return true; }
        }

        protected override void OnLoad(EventArgs e)
        {
            base.OnLoad(e);
            cellWindow.AssignHandle(cellPanel.Handle);
        }
    }

    public static class Program
    {
        [STAThread]
        public static void Main()
        {
            Application.Run(new ProbeHostForm());
        }
    }
}
'@

function Measure-CustomGridItemProvider {
    $workDir = Join-Path $env:TEMP ('a24u6-provider-' + [guid]::NewGuid().ToString('N'))
    $procId = 0
    try {
        New-Item -ItemType Directory -Path $workDir -Force | Out-Null
        $csPath = Join-Path $workDir 'CellProviderHost.cs'
        $exePath = Join-Path $workDir 'CellProviderHost.exe'
        $utf8NoBom = New-Object System.Text.UTF8Encoding $false
        [IO.File]::WriteAllText($csPath, $script:CellProviderSource, $utf8NoBom)

        $frameworkDir = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319'
        $csc = Join-Path $frameworkDir 'csc.exe'
        if (-not (Test-Path -LiteralPath $csc)) {
            return [ordered]@{ measurable = $false; branch = 'csc_exe_not_found'; csc_path_checked = $true }
        }
        $wpfDir = Join-Path $frameworkDir 'WPF'
        $providerDll = Join-Path $wpfDir 'UIAutomationProvider.dll'
        $typesDll = Join-Path $wpfDir 'UIAutomationTypes.dll'
        if (-not (Test-Path -LiteralPath $providerDll) -or -not (Test-Path -LiteralPath $typesDll)) {
            return [ordered]@{
                measurable        = $false
                branch            = 'uiautomationprovider_reference_missing'
                wpf_ref_dir_exists = (Test-Path -LiteralPath $wpfDir)
            }
        }

        $cscArgs = @(
            '/nologo', '/target:winexe', '/langversion:5', '/platform:anycpu',
            ('/out:' + $exePath),
            '/reference:System.dll', '/reference:System.Drawing.dll', '/reference:System.Windows.Forms.dll',
            ('/reference:' + $providerDll), ('/reference:' + $typesDll),
            $csPath
        )
        $buildOut = & $csc $cscArgs 2>&1 | ForEach-Object { "$_" }
        $buildExit = $LASTEXITCODE
        if ($buildExit -ne 0 -or -not (Test-Path -LiteralPath $exePath)) {
            return [ordered]@{
                measurable      = $false
                branch          = 'csc_compile_failed'
                compiler_exit   = $buildExit
                compiler_output = @($buildOut)
            }
        }

        $started = Start-ScratchProcess -FilePath $exePath -NoActivate
        $procId = $started.ProcessId
        if ($started.MainWindowHandle -eq [IntPtr]::Zero) {
            return [ordered]@{ measurable = $false; branch = 'provider_host_window_not_found'; compiler_exit = $buildExit }
        }
        Start-Sleep -Milliseconds 500

        $root = $null
        try { $root = $AE::FromHandle($started.MainWindowHandle) } catch { }
        if ($null -eq $root) {
            return [ordered]@{ measurable = $false; branch = 'uia_from_handle_failed_on_host' }
        }

        $cond = New-Object System.Windows.Automation.PropertyCondition($AE::AutomationIdProperty, 'probeCellProvider')
        $cellEl = $null
        try { $cellEl = $root.FindFirst($TS::Descendants, $cond) } catch { }
        if ($null -eq $cellEl) {
            return [ordered]@{ measurable = $false; branch = 'provider_element_not_found_by_client' }
        }

        $cellInfo = Get-ElementRoleShape -Element $cellEl
        $forcesShape = ($cellInfo.control_type -eq 'DataItem' -and $cellInfo.grid_item_pattern_try_get -and $cellInfo.table_item_pattern_try_get)

        return [ordered]@{
            measurable              = $true
            branch                   = if ($forcesShape) { 'custom_provider_forces_dataitem_griditem_tableitem_shape' } else { 'custom_provider_compiled_and_ran_but_shape_incomplete' }
            host_note                = 'WM_GETOBJECT hook on a Panel child, same mechanism as ScratchForms.cs AutomationIdProvider/AutomationIdWindow, extended with IGridItemProvider + ITableItemProvider'
            containing_grid_is_self  = $true
            cell                     = $cellInfo
        }
    } finally {
        if ($procId -gt 0) { try { Stop-ScratchProcess -ProcessId $procId } catch { } }
        if (Test-Path -LiteralPath $workDir) { Remove-Item -LiteralPath $workDir -Recurse -Force -ErrorAction SilentlyContinue }
    }
}

# ---------------------------------------------------------------- main

$question = 'can the fixture produce the DataItem + GridItem/TableItem shape the cell ref-able role arm needs: reconfirm the stock WinForms DataGridView shape on this box (A16-10), then determine whether a custom UIA provider (IRawElementProviderSimple + IGridItemProvider/ITableItemProvider) compiles with csc.exe 4.8 and reads back through a UIA client'

$stock = $null
try { $stock = Measure-StockDataGridView } catch {
    $stock = [ordered]@{ measurable = $false; branch = 'stock_leg_threw'; error_class = $_.Exception.GetType().Name }
}

$custom = $null
try { $custom = Measure-CustomGridItemProvider } catch {
    $custom = [ordered]@{ measurable = $false; branch = 'custom_provider_leg_threw'; error_class = $_.Exception.GetType().Name }
}

if ($custom.measurable -and $custom.branch -eq 'custom_provider_forces_dataitem_griditem_tableitem_shape') {
    $overall = [ordered]@{
        measurable = $true
        branch     = 'fixture_can_force_cell_shape_via_custom_provider'
        conclusion = 'a custom UIA provider compiles with the pinned csc.exe 4.8 + UIAutomationProvider/Types toolchain and a UIA client reads DataItem + GridItem/TableItem back off it, so 2.12 can decide whether the cell arm should be exercised this way'
    }
} elseif (-not $custom.measurable) {
    $overall = [ordered]@{
        measurable = $false
        branch     = 'custom_provider_not_measurable'
        conclusion = 'the decisive leg could not run on this box; see custom_provider.branch for the specific reason (csc.exe absent, reference assemblies missing, compile failure with compiler_output, or the compiled host not answering as expected)'
    }
} else {
    $overall = [ordered]@{
        measurable = $true
        branch     = 'custom_provider_compiled_but_shape_incomplete'
        conclusion = 'the custom provider host compiled and ran but the UIA client did not read back the full DataItem + GridItem + TableItem shape; see custom_provider.cell for exactly what was observed'
    }
}

$result = [ordered]@{
    probe           = $script:Probe
    question        = $question
    cites           = @('A16-10')
    overall         = $overall
    stock_dgv       = $stock
    custom_provider = $custom
}

$overallError = $null
$capturePath = $null
try {
    $capturePath = Write-A24Capture -Name "cell-role-provider-$Label.json" -Content (ConvertTo-Json -InputObject $result -Depth 18)
    Register-MandatoryPass -Capture $capturePath -Result $result
} catch {
    $overallError = $_.Exception.GetType().Name
} finally {
    foreach ($leftoverId in (Get-ScratchProcessIds)) {
        try { Stop-Process -Id $leftoverId -Force -ErrorAction SilentlyContinue } catch { }
    }
}

if ($null -ne $overallError) {
    Write-ProbeResult -Probe $script:Probe -Status 'fail' -Message ('probe threw while writing capture: ' + $overallError) -Data @{ error_class = $overallError }
    exit 1
}

Assert-MandatoryMeasurement -Probe $script:Probe -Label $Label

Write-ProbeResult -Probe $script:Probe -Status 'ok' -Message 'cell ref-able role provider probe captured' -Data @{
    capture = Split-Path -Leaf $capturePath
}
exit 0
