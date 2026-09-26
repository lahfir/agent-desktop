#Requires -Version 5.1
<#
.SYNOPSIS
    WinForms fixture toolchain end-to-end probe (area 24, sub-phase 2.12).

.DESCRIPTION
    phases.md 2.12 assumes csc.exe + .NET 4.8 is sufficient to build a fixture
    app whose element identity is visible to UIA. Presence of csc.exe at
    C:\Windows\Microsoft.NET\Framework64\v4.0.30319\csc.exe is not that
    measurement. This probe compiles a minimal WinForms window (one Button,
    one TextBox, one CheckBox) with that exact compiler, launches it without
    stealing the foreground, and reads it back through the managed
    System.Windows.Automation client stack that every other probe in this
    corpus already uses.

    WinForms has no AutomationProperties.AutomationId setter the way WPF
    does. The mechanism under test is Control.Name: Microsoft documents that,
    since .NET Framework 4.7.1, the stock WinForms UIA provider surfaces
    Control.Name as AutomationId, but only once the AppContextSwitchOverrides
    Switch.UseLegacyAccessibilityFeatures* switches are turned off via the
    app's .exe.config - a documented behavior scratch/build-scratch.ps1
    already leans on for the more elaborate ScratchForms.exe fixture, but
    which this repo has not independently measured until this probe. This
    probe isolates that variable directly:

        arm "no_app_config"                   - compiled and run with no
                                                 .exe.config next to the
                                                 binary at all (the naive
                                                 "csc.exe + .NET 4.8 is
                                                 enough" assumption, tested
                                                 literally)
        arm "legacy_accessibility_switches_off" - the identical binary, run
                                                 with build-scratch.ps1's
                                                 proven .exe.config sitting
                                                 next to it

    A separate leg sweeps a representative /langversion: value set against
    the same csc.exe to report the accepted C# language-version ceiling,
    since that constrains what any fixture source under this toolchain can
    be written in.

    Captures under captures\ as fixture-toolchain-{devbox,ci}.json (+
    .normalized twin). Corpus safety: the fixture is probe-owned scratch
    code with no user content; all AutomationId values reported are
    constants this probe itself defines, not observed user data. No window
    titles beyond the fixture's own literal title, no file paths, no pids,
    no machine or user names leave this script - compiler diagnostic text is
    never captured verbatim, only structured CS#### codes and exit codes
    extracted from it, and everything that is written still passes through
    Protect-ProbeText/Test-CaptureRedaction as a backstop.
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

$script:Probe = '24-fixture-e2e-01-fixture-toolchain'
$script:ProbeDir = Split-Path -Parent $PSCommandPath
$script:CaptureDir = Join-Path $script:ProbeDir 'captures'
if (-not (Test-Path -LiteralPath $script:CaptureDir)) {
    New-Item -ItemType Directory -Path $script:CaptureDir -Force | Out-Null
}
$script:WorkDir = $null
$script:Spawned = New-Object System.Collections.ArrayList

$script:FixtureButtonId = 'btnProbeButton'
$script:FixtureTextBoxId = 'txtProbeInput'
$script:FixtureCheckBoxId = 'chkProbeOption'

Register-MandatoryCapture -Name @("fixture-toolchain-$Label.json")

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

function Register-SpawnedPid {
    param([int]$ProcessId)
    if ($ProcessId -gt 0 -and -not $script:Spawned.Contains($ProcessId)) { [void]$script:Spawned.Add($ProcessId) }
}

function Stop-AllSpawned {
    foreach ($id in @($script:Spawned)) {
        try { Stop-ScratchProcess -ProcessId $id } catch { }
    }
    $script:Spawned.Clear()
}

<#
    Every CS#### token found in raw csc.exe diagnostic text, deduplicated and
    order-preserved, tagged as an error or a warning by the line it came
    from. This is the structured substitute for capturing the diagnostic text
    itself: a per-file compiler error embeds the full source path being
    compiled, which under $env:TEMP carries the operator's profile directory.
    Extracting only the CS#### code answers "did it compile clean and with
    what diagnostic" without ever writing that path to a capture.
#>
function Get-DiagnosticCodes {
    param([string[]]$Lines)
    $errorCodes = New-Object System.Collections.ArrayList
    $warningCodes = New-Object System.Collections.ArrayList
    foreach ($line in $Lines) {
        $text = [string]$line
        foreach ($m in [regex]::Matches($text, 'CS\d{4}')) {
            if ($text -match 'error') {
                if (-not $errorCodes.Contains($m.Value)) { [void]$errorCodes.Add($m.Value) }
            } elseif ($text -match 'warning') {
                if (-not $warningCodes.Contains($m.Value)) { [void]$warningCodes.Add($m.Value) }
            }
        }
    }
    return [ordered]@{ error_codes = @($errorCodes); warning_codes = @($warningCodes) }
}

<#
    Sweeps a representative bracket of /langversion values against a
    throwaway one-line class through the exact csc.exe this probe targets.
    The legacy Framework64\v4.0.30319\csc.exe is a pre-Roslyn compiler that
    is expected to reject anything past C# 5 with CS1617 ("invalid option
    ... for /langversion"), which is itself the answer to "what can a
    fixture under this toolchain be written in" - measured, not assumed.
#>
function Measure-LangVersionAcceptance {
    param([Parameter(Mandatory = $true)][string]$Csc)
    $candidates = @('ISO-1', '3', '5', '6', '7', '7.3', '8', 'Default', 'Latest')
    $rows = New-Object System.Collections.ArrayList
    $work = Join-Path $env:TEMP ('agent-desktop-probe24-langver-' + [guid]::NewGuid().ToString('N'))
    New-Item -ItemType Directory -Path $work -Force | Out-Null
    try {
        $srcPath = Join-Path $work 'LangProbe.cs'
        [IO.File]::WriteAllText($srcPath, 'public class LangProbe24 { public int Value = 1; }', (New-Object System.Text.UTF8Encoding $false))
        foreach ($candidate in $candidates) {
            $safeTag = ($candidate -replace '[^A-Za-z0-9]', '')
            $dllPath = Join-Path $work ('out-' + $safeTag + '.dll')
            $cscArgs = @('/nologo', '/target:library', ('/langversion:' + $candidate), '/platform:anycpu', ('/out:' + $dllPath), $srcPath)
            $output = @(& $Csc $cscArgs 2>&1 | ForEach-Object { "$_" })
            $exit = $LASTEXITCODE
            $diagnostics = Get-DiagnosticCodes -Lines $output
            [void]$rows.Add([ordered]@{
                    requested_langversion = $candidate
                    accepted               = ($exit -eq 0 -and (Test-Path -LiteralPath $dllPath))
                    exit_code              = $exit
                    error_codes            = $diagnostics.error_codes
                })
            Remove-Item -LiteralPath $dllPath -Force -ErrorAction SilentlyContinue
        }
    } finally {
        Remove-Item -LiteralPath $work -Recurse -Force -ErrorAction SilentlyContinue
    }
    return [ordered]@{
        measurable             = $true
        branch                 = 'sweep_completed'
        compiler_file_version  = (Get-Item -LiteralPath $Csc).VersionInfo.FileVersion
        candidates_sampled     = @($candidates)
        rows                   = @($rows)
    }
}

function Get-RowField {
    param($Row, [Parameter(Mandatory = $true)][string]$Name)
    if ($null -eq $Row) { return $null }
    if ($Row -is [System.Collections.IDictionary]) {
        if ($Row.Contains($Name)) { return $Row[$Name] }
        return $null
    }
    if ($Row.PSObject.Properties.Name -contains $Name) { return $Row.$Name }
    return $null
}

function Test-LangVersionAccepted {
    param(
        [Parameter(Mandatory = $false)]$Rows,
        [Parameter(Mandatory = $true)][string]$Requested
    )
    foreach ($row in @($Rows)) {
        if ((Get-RowField -Row $row -Name 'requested_langversion') -ne $Requested) { continue }
        return [bool](Get-RowField -Row $row -Name 'accepted')
    }
    return $false
}

function Get-HighestAcceptedNumericLangVersion {
    param($Rows)
    foreach ($v in @('7.3', '7', '6', '5', '3')) {
        if (Test-LangVersionAccepted -Rows $Rows -Requested $v) { return $v }
    }
    return 'none_of_the_sampled_numeric_versions_accepted'
}

function Build-FixtureExe {
    param(
        [Parameter(Mandatory = $true)][string]$Csc,
        [Parameter(Mandatory = $true)][string]$OutPath,
        [Parameter(Mandatory = $true)][string]$SrcPath
    )
    $cscArgs = @(
        '/nologo', '/target:winexe', '/langversion:5', '/platform:anycpu', ('/out:' + $OutPath),
        '/reference:System.dll', '/reference:System.Drawing.dll', '/reference:System.Windows.Forms.dll', $SrcPath
    )
    $output = @(& $Csc $cscArgs 2>&1 | ForEach-Object { "$_" })
    $exit = $LASTEXITCODE
    $diagnostics = Get-DiagnosticCodes -Lines $output
    return [ordered]@{
        langversion_used = '5'
        exit_code        = $exit
        compiled_ok       = ($exit -eq 0 -and (Test-Path -LiteralPath $OutPath))
        error_codes       = $diagnostics.error_codes
        warning_codes     = $diagnostics.warning_codes
    }
}

<#
    "Read back through UIA" means reachable from the desktop root by
    enumeration, not merely bridgeable by a known HWND -
    AutomationElement.FromHandle can bind a window that a real snapshot walk
    (RootElement -> Children by pid) would never reach. Mirrors probe 23's
    Get-UiaMenuFamilyPresence root-child check so this leg answers the same
    question every other probe in the corpus already answers for menus.
#>
function Get-RootWalkPresence {
    param([Parameter(Mandatory = $true)][int]$ProcessId)
    $root = [System.Windows.Automation.AutomationElement]::RootElement
    $cond = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ProcessIdProperty, $ProcessId)
    $children = $null
    try { $children = $root.FindAll([System.Windows.Automation.TreeScope]::Children, $cond) } catch { }
    $count = 0
    $controlTypes = New-Object System.Collections.ArrayList
    if ($null -ne $children) {
        $count = $children.Count
        foreach ($c in $children) {
            try {
                $ct = $c.Current.ControlType.ProgrammaticName -replace '^ControlType\.', ''
                if (-not $controlTypes.Contains($ct)) { [void]$controlTypes.Add($ct) }
            } catch { }
        }
    }
    return [ordered]@{
        window_found_in_root_walk = ($count -gt 0)
        root_children_for_pid     = $count
        root_child_control_types  = @($controlTypes)
    }
}

<#
    A bounded ControlView dump of the fixture's own subtree (probe-owned
    controls only, so control_type/automation_id are reported verbatim - the
    same reasoning probe 04's AutomationIdSample relies on). This is what
    makes a "not found" result in Get-ControlReadback diagnosable: "the
    element is not in the tree at all" and "the element is in the tree but
    its AutomationId is empty" call for different 2.12 fixture designs, and
    a bare found:false cannot tell them apart.
#>
function Get-FixtureSubtreeDump {
    param($Root, [int]$MaxNodes = 40, [int]$MaxDepth = 6)
    $walker = [System.Windows.Automation.TreeWalker]::ControlViewWalker
    $records = New-Object System.Collections.ArrayList
    $stack = New-Object System.Collections.Stack
    $stack.Push(@{ Element = $Root; Depth = 0 })
    while ($stack.Count -gt 0 -and $records.Count -lt $MaxNodes) {
        $item = $stack.Pop()
        $element = $item.Element
        $cur = $null
        try { $cur = $element.Current } catch { }
        if ($null -eq $cur) { continue }
        $controlType = '<unavailable>'
        try { $controlType = $cur.ControlType.ProgrammaticName -replace '^ControlType\.', '' } catch { }
        $automationId = ''
        try { $automationId = [string]$cur.AutomationId } catch { }
        [void]$records.Add([ordered]@{
                depth         = $item.Depth
                control_type  = $controlType
                automation_id = $automationId
            })
        if ($item.Depth -ge $MaxDepth) { continue }
        $kids = New-Object System.Collections.ArrayList
        try {
            $k = $walker.GetFirstChild($element)
            while ($null -ne $k) { [void]$kids.Add($k); $k = $walker.GetNextSibling($k) }
        } catch { }
        for ($i = $kids.Count - 1; $i -ge 0; $i--) {
            $stack.Push(@{ Element = $kids[$i]; Depth = ($item.Depth + 1) })
        }
    }
    return @($records)
}

function Get-ControlReadback {
    param($Root, [Parameter(Mandatory = $true)][string]$AutomationId)
    $condition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::AutomationIdProperty, $AutomationId)
    $element = $null
    try { $element = $Root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition) } catch { }
    if ($null -eq $element) {
        return [ordered]@{
            automation_id           = $AutomationId
            found                   = $false
            control_type            = $null
            automation_id_readback  = $null
            automation_id_matches   = $false
        }
    }
    $controlType = '<unavailable>'
    try { $controlType = $element.Current.ControlType.ProgrammaticName -replace '^ControlType\.', '' } catch { }
    $readbackId = ''
    try { $readbackId = [string]$element.Current.AutomationId } catch { }
    return [ordered]@{
        automation_id          = $AutomationId
        found                  = $true
        control_type           = $controlType
        automation_id_readback = $readbackId
        automation_id_matches  = ($readbackId -eq $AutomationId)
    }
}

<#
    Launches one built fixture arm without activating it (SW_SHOWNOACTIVATE,
    via common.ps1's Start-ScratchProcess -NoActivate - the same technique
    every other probe in this corpus uses so a scratch window never steals
    the foreground), reads the three named controls back through
    AutomationElement.FromHandle, and reports what was found. An honest
    negative at any stage (exe missing, window never appears, UIA throws) is
    its own branch rather than a silent empty result.
#>
function Invoke-FixtureArm {
    param(
        [Parameter(Mandatory = $true)][string]$ArmName,
        [Parameter(Mandatory = $true)][string]$ExePath,
        [Parameter(Mandatory = $true)][bool]$AppConfigApplied
    )
    if (-not (Test-Path -LiteralPath $ExePath)) {
        return [ordered]@{ measurable = $false; branch = 'exe_missing'; arm = $ArmName; app_config_applied = $AppConfigApplied }
    }
    $launched = $null
    try {
        $launched = Start-ScratchProcess -FilePath $ExePath -NoActivate -TimeoutSec 20
    } catch {
        return [ordered]@{ measurable = $false; branch = 'launch_threw'; arm = $ArmName; app_config_applied = $AppConfigApplied; error_class = $_.Exception.GetType().Name }
    }
    if ($null -ne $launched -and $launched.ProcessId -gt 0) { Register-SpawnedPid -ProcessId $launched.ProcessId }
    if ($null -eq $launched -or $launched.MainWindowHandle -eq [IntPtr]::Zero) {
        if ($null -ne $launched -and $launched.ProcessId -gt 0) { try { Stop-ScratchProcess -ProcessId $launched.ProcessId } catch { } }
        return [ordered]@{ measurable = $false; branch = 'window_never_appeared'; arm = $ArmName; app_config_applied = $AppConfigApplied }
    }
    Start-Sleep -Milliseconds 500
    $result = [ordered]@{ measurable = $false; branch = 'uia_readback_exception'; arm = $ArmName; app_config_applied = $AppConfigApplied }
    try {
        $rootWalk = Get-RootWalkPresence -ProcessId $launched.ProcessId
        $root = [System.Windows.Automation.AutomationElement]::FromHandle($launched.MainWindowHandle)
        $rootControlType = '<unavailable>'
        try { $rootControlType = $root.Current.ControlType.ProgrammaticName -replace '^ControlType\.', '' } catch { }
        $button = Get-ControlReadback -Root $root -AutomationId $script:FixtureButtonId
        $textBox = Get-ControlReadback -Root $root -AutomationId $script:FixtureTextBoxId
        $checkBox = Get-ControlReadback -Root $root -AutomationId $script:FixtureCheckBoxId
        $allFound = ($button.found -and $textBox.found -and $checkBox.found)
        $allMatch = ($button.automation_id_matches -and $textBox.automation_id_matches -and $checkBox.automation_id_matches)
        $subtree = Get-FixtureSubtreeDump -Root $root
        $result = [ordered]@{
            measurable                     = $true
            branch                         = 'window_read_via_managed_uia'
            arm                            = $ArmName
            app_config_applied             = $AppConfigApplied
            root_found_by_handle           = $true
            root_control_type              = $rootControlType
            root_walk                      = $rootWalk
            controls                       = [ordered]@{
                button    = $button
                text_box  = $textBox
                check_box = $checkBox
            }
            all_three_controls_found       = $allFound
            all_three_automation_ids_match = $allMatch
            subtree_dump                   = $subtree
            subtree_dump_note              = 'bounded ControlView walk of the fixture window itself (probe-owned controls, so control_type/automation_id are verbatim); disambiguates found:false above between "not in the tree" and "in the tree with an empty AutomationId"'
        }
    } catch {
        $result['error_class'] = $_.Exception.GetType().Name
    } finally {
        try { Stop-ScratchProcess -ProcessId $launched.ProcessId } catch { }
    }
    return $result
}

$fixtureSource = @'
using System;
using System.Drawing;
using System.Windows.Forms;

namespace AgentDesktopProbe24 {
    public class FixtureForm : Form {
        public FixtureForm() {
            this.Name = "frmProbe24Fixture";
            this.Text = "AgentDesktop Probe24 Fixture";
            this.Width = 420;
            this.Height = 220;
            this.StartPosition = FormStartPosition.Manual;
            this.Location = new Point(80, 80);

            Button probeButton = new Button();
            probeButton.Name = "btnProbeButton";
            probeButton.Text = "Probe Button";
            probeButton.Location = new Point(20, 20);
            probeButton.Width = 160;
            this.Controls.Add(probeButton);

            TextBox probeTextBox = new TextBox();
            probeTextBox.Name = "txtProbeInput";
            probeTextBox.Location = new Point(20, 60);
            probeTextBox.Width = 220;
            this.Controls.Add(probeTextBox);

            CheckBox probeCheckBox = new CheckBox();
            probeCheckBox.Name = "chkProbeOption";
            probeCheckBox.Text = "Probe Option";
            probeCheckBox.Location = new Point(20, 100);
            this.Controls.Add(probeCheckBox);
        }

        [STAThread]
        public static void Main() {
            Application.Run(new FixtureForm());
        }
    }
}
'@

$appConfigText = @'
<?xml version="1.0" encoding="utf-8"?>
<configuration>
  <startup>
    <supportedRuntime version="v4.0" sku=".NETFramework,Version=v4.8" />
  </startup>
  <runtime>
    <AppContextSwitchOverrides value="Switch.UseLegacyAccessibilityFeatures=false;Switch.UseLegacyAccessibilityFeatures.2=false;Switch.UseLegacyAccessibilityFeatures.3=false" />
  </runtime>
</configuration>
'@

$result = $null

try {
    Initialize-ProbeNative

    $csc = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
    if (-not (Test-Path -LiteralPath $csc)) {
        $result = [ordered]@{
            probe      = $script:Probe
            question   = 'does the full WinForms fixture toolchain (csc.exe + .NET 4.8) work end to end: does it compile clean, does the window appear in a UIA walk, is AutomationId readable on Button/TextBox/CheckBox, what ControlType does each map to, and what C# language version does csc.exe accept'
            measurable = $false
            branch     = 'csc_not_found'
            csc_path_checked = $csc
        }
    } else {
        $script:WorkDir = Join-Path $env:TEMP ('agent-desktop-probe24-fixture-' + [guid]::NewGuid().ToString('N'))
        New-Item -ItemType Directory -Path $script:WorkDir -Force | Out-Null

        $langSweep = Measure-LangVersionAcceptance -Csc $csc

        $srcPath = Join-Path $script:WorkDir 'FixtureForm.cs'
        [IO.File]::WriteAllText($srcPath, $fixtureSource, (New-Object System.Text.UTF8Encoding $false))

        $exeNoConfig = Join-Path $script:WorkDir 'FixtureNoConfig.exe'
        $buildNoConfig = Build-FixtureExe -Csc $csc -OutPath $exeNoConfig -SrcPath $srcPath

        $exeConfig = Join-Path $script:WorkDir 'FixtureConfig.exe'
        $buildConfigResult = $null
        if ($buildNoConfig.compiled_ok) {
            Copy-Item -LiteralPath $exeNoConfig -Destination $exeConfig -Force
            [IO.File]::WriteAllText(($exeConfig + '.config'), $appConfigText, (New-Object System.Text.UTF8Encoding $false))
            $buildConfigResult = [ordered]@{
                compiled_ok = $true
                branch      = 'copied_from_no_config_binary_plus_exe_config'
            }
        } else {
            $buildConfigResult = [ordered]@{ compiled_ok = $false; branch = 'skipped_source_compile_failed' }
        }

        if ($buildNoConfig.compiled_ok) {
            $armNoConfig = Invoke-FixtureArm -ArmName 'no_app_config' -ExePath $exeNoConfig -AppConfigApplied $false
        } else {
            $armNoConfig = [ordered]@{ measurable = $false; branch = 'compile_failed'; arm = 'no_app_config'; app_config_applied = $false }
        }

        if ($buildConfigResult.compiled_ok) {
            $armConfig = Invoke-FixtureArm -ArmName 'legacy_accessibility_switches_off' -ExePath $exeConfig -AppConfigApplied $true
        } else {
            $armConfig = [ordered]@{ measurable = $false; branch = 'compile_failed'; arm = 'legacy_accessibility_switches_off'; app_config_applied = $true }
        }

        $readableNoConfig = ($armNoConfig.measurable -and $armNoConfig.all_three_automation_ids_match)
        $readableWithConfig = ($armConfig.measurable -and $armConfig.all_three_automation_ids_match)

        $result = [ordered]@{
            probe                    = $script:Probe
            question                 = 'does the full WinForms fixture toolchain (csc.exe + .NET 4.8) work end to end: does it compile clean, does the window appear in a UIA walk, is AutomationId readable on Button/TextBox/CheckBox, what ControlType does each map to, and what C# language version does csc.exe accept'
            measurable               = $true
            branch                   = 'toolchain_exercised'
            csc_path                 = $csc
            mechanism_hypothesis     = 'WinForms has no AutomationProperties.AutomationId setter; the hypothesis under test is that Control.Name is surfaced as AutomationId by the stock provider only once Switch.UseLegacyAccessibilityFeatures* is turned off via an .exe.config - the two arms below isolate exactly that variable, and readback_no_app_config vs readback_with_app_config below is the measurement, not this note'
            fixture_automation_ids   = [ordered]@{ button = $script:FixtureButtonId; text_box = $script:FixtureTextBoxId; check_box = $script:FixtureCheckBoxId }
            langversion_acceptance   = $langSweep
            compile_no_app_config    = $buildNoConfig
            compile_with_app_config  = $buildConfigResult
            readback_no_app_config   = $armNoConfig
            readback_with_app_config = $armConfig
            summary                  = [ordered]@{
                csc_compiled_clean                         = [bool]$buildNoConfig.compiled_ok
                automationid_readable_without_app_config    = [bool]$readableNoConfig
                automationid_readable_with_app_config       = [bool]$readableWithConfig
                app_config_required_for_automationid        = ((-not $readableNoConfig) -and $readableWithConfig)
                highest_accepted_sampled_langversion        = (Get-HighestAcceptedNumericLangVersion -Rows $langSweep.rows)
                latest_alias_accepted                       = [bool](Test-LangVersionAccepted -Rows $langSweep.rows -Requested 'Latest')
            }
        }
    }
} catch {
    $result = [ordered]@{
        probe        = $script:Probe
        measurable   = $false
        branch       = 'probe_threw'
        error_class  = $_.Exception.GetType().Name
        error_line   = [int]$_.InvocationInfo.ScriptLineNumber
        error_source = (Protect-ProbeText -Text ([string]$_.InvocationInfo.Line).Trim())
    }
} finally {
    Stop-AllSpawned
    if ($script:WorkDir -and (Test-Path -LiteralPath $script:WorkDir)) {
        Remove-Item -LiteralPath $script:WorkDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}

$capturePath = Write-A24Capture -Name "fixture-toolchain-$Label.json" -Content (ConvertTo-Json -InputObject $result -Depth 14)
Register-MandatoryPass -Capture $capturePath -Result $result

Assert-MandatoryMeasurement -Probe $script:Probe -Label $Label

Write-ProbeResult -Probe $script:Probe -Status 'ok' -Message 'WinForms fixture toolchain end-to-end probe captured' -Data @{
    capture = Split-Path -Leaf $capturePath
}
exit 0
