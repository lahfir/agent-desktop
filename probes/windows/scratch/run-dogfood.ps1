#Requires -Version 5.1
<#
.SYNOPSIS
    Sub-phase 2.3 vocabulary dogfood capture runner.

.DESCRIPTION
    Orchestrates the sub-phase 2.3 dogfood run: launches a fixed set of
    repo-controlled targets - classic Notepad, Windows Explorer, the WinForms
    and WPF scratch fixtures, and, when present on the box, a Chromium/Electron
    app and Settings - and captures each one's UI Automation tree with
    crates\windows\examples\uia_tree_dump, the same census tool the product
    ships. Every target shows only repo-controlled, obviously-synthetic
    content, never the operator's own documents.

    A target that is not present on this box is recorded SKIPPED with a
    reason and is never reported as captured. The run summary is written to
    <OutDir>\dogfood-run.json, redacted through the same corpus redaction gate
    probes\windows\15-vocabulary\probe.ps1 uses (Initialize-ProbeRedaction /
    Protect-ProbeText / Test-CaptureRedaction from probes\windows\common.ps1).

    Every process this script starts is tracked by pid and killed in a
    `finally` block; it never touches a process it did not start. Explorer is
    a special case - "explorer.exe <dir>" commonly hands the request to an
    already-running shell process rather than starting a new one, so its
    launcher pid is tracked and killed the same way as everything else, and
    nothing else with that image name is ever touched.

.PARAMETER OutDir
    Directory the per-target capture JSON and the run summary are written to.
    Defaults to docs\plans\2026-07-31-001-captures under the repo root.
#>
[CmdletBinding()]
param(
    [string]$OutDir = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

$script:ScratchDir = Split-Path -Parent $PSCommandPath
$script:RepoRoot = (Resolve-Path (Join-Path $script:ScratchDir '..\..\..')).ProviderPath

. (Join-Path $script:RepoRoot 'probes\windows\common.ps1')
Initialize-ProbeRedaction

if (-not $OutDir) {
    $OutDir = Join-Path $script:RepoRoot 'docs\plans\2026-07-31-001-captures'
}
New-Item -ItemType Directory -Path $OutDir -Force | Out-Null
$OutDir = (Resolve-Path -LiteralPath $OutDir).ProviderPath

if (-not ('AgentDesktopDogfood.Native' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
namespace AgentDesktopDogfood {
    public static class Native {
        [DllImport("user32.dll")]
        public static extern int GetClassName(IntPtr hWnd, StringBuilder text, int count);
    }
}
'@
}

function Get-DogfoodClassName {
    param([Parameter(Mandatory = $true)][IntPtr]$Handle)
    $sb = New-Object System.Text.StringBuilder 256
    [void][AgentDesktopDogfood.Native]::GetClassName($Handle, $sb, 256)
    return $sb.ToString()
}

$script:LaunchedPids = New-Object System.Collections.Generic.List[int]

function Start-DogfoodProcess {
    param(
        [Parameter(Mandatory = $true)][string]$FilePath,
        [string[]]$ArgumentList = @(),
        [string]$WindowStyle = 'Normal'
    )
    if ($ArgumentList.Count -gt 0) {
        $proc = Start-Process -FilePath $FilePath -ArgumentList $ArgumentList -WindowStyle $WindowStyle -PassThru
    } else {
        $proc = Start-Process -FilePath $FilePath -WindowStyle $WindowStyle -PassThru
    }
    [void]$script:LaunchedPids.Add($proc.Id)
    return $proc
}

function Wait-DogfoodWindow {
    param(
        [Parameter(Mandatory = $true)]$Process,
        [int]$TimeoutSec = 20
    )
    $deadline = (Get-Date).AddSeconds($TimeoutSec)
    while ((Get-Date) -lt $deadline) {
        $Process.Refresh()
        if ($Process.HasExited) { return [IntPtr]::Zero }
        if ($Process.MainWindowHandle -ne [IntPtr]::Zero) { return $Process.MainWindowHandle }
        Start-Sleep -Milliseconds 200
    }
    return [IntPtr]::Zero
}

function Invoke-DogfoodCensus {
    param(
        [Parameter(Mandatory = $true)][string]$ClassName,
        [Parameter(Mandatory = $true)][string]$Variant,
        [Parameter(Mandatory = $true)][string]$OutFile
    )
    $stdoutFile = [IO.Path]::GetTempFileName()
    $stderrFile = [IO.Path]::GetTempFileName()
    try {
        Push-Location $script:RepoRoot
        try {
            # Local to this function only: cargo always writes routine progress
            # ("Compiling", "Finished", "Running") to stderr, even on success.
            # With the script-wide $ErrorActionPreference = 'Stop', redirecting
            # that stream to a file still wraps each line in a NativeCommandError
            # and aborts this statement before the example ever runs. Relaxing
            # the preference for just this native invocation lets the census
            # tool's own exit code be the only thing that decides success.
            $ErrorActionPreference = 'Continue'
            & cargo run -p agent-desktop-windows --example uia_tree_dump -- `
                --class $ClassName --variant $Variant --out $OutFile 1> $stdoutFile 2> $stderrFile
            $exitCode = $LASTEXITCODE
        } finally {
            Pop-Location
        }
        $stdout = ''
        $stderr = ''
        if (Test-Path -LiteralPath $stdoutFile) { $stdout = [string](Get-Content -LiteralPath $stdoutFile -Raw -ErrorAction SilentlyContinue) }
        if (Test-Path -LiteralPath $stderrFile) { $stderr = [string](Get-Content -LiteralPath $stderrFile -Raw -ErrorAction SilentlyContinue) }
        return [ordered]@{ ExitCode = $exitCode; StdOut = $stdout; StdErr = $stderr }
    } finally {
        Remove-Item -LiteralPath $stdoutFile -Force -ErrorAction SilentlyContinue
        Remove-Item -LiteralPath $stderrFile -Force -ErrorAction SilentlyContinue
    }
}

$script:Results = New-Object System.Collections.Generic.List[object]

function New-DogfoodTarget {
    param(
        [string]$Name, [string]$UiStack, [bool]$Captured, [string]$Reason,
        [string]$CaptureFile, [string]$ClassName, [string]$Variant
    )
    return [ordered]@{
        name          = $Name
        ui_stack      = $UiStack
        class_name    = $ClassName
        variant       = $Variant
        captured      = $Captured
        reason        = $Reason
        capture_file  = $CaptureFile
    }
}

function Add-DogfoodCapture {
    param([string]$Name, [string]$UiStack, [string]$ClassName, [string]$Variant, [string]$OutFileName)
    Write-Host ('dogfood: capturing ' + $Name + ' (class=' + $ClassName + ')')
    $outPath = Join-Path $OutDir $OutFileName
    $capture = Invoke-DogfoodCensus -ClassName $ClassName -Variant $Variant -OutFile $outPath
    if ($capture.ExitCode -eq 0 -and (Test-Path -LiteralPath $outPath)) {
        Write-Host ('dogfood: ' + $Name + ' captured -> ' + $outPath)
        $script:Results.Add((New-DogfoodTarget -Name $Name -UiStack $UiStack -Captured $true `
                    -Reason $null -CaptureFile $OutFileName -ClassName $ClassName -Variant $Variant))
    } else {
        # The tool prints a one-line {"status":"skipped","reason":"..."} JSON
        # payload on its own recognised absence path (render_slots::skipped).
        # Prefer that clean reason over the raw stderr, which PowerShell's
        # native-command redirection wraps in noisy ErrorRecord formatting.
        $combined = ($capture.StdErr + ' ' + $capture.StdOut)
        $jsonMatch = [regex]::Match($combined, '"reason"\s*:\s*"((?:[^"\\]|\\.)*)"')
        if ($jsonMatch.Success) {
            $reason = $jsonMatch.Groups[1].Value
        } else {
            $reason = $combined.Trim()
        }
        if (-not $reason) { $reason = 'uia_tree_dump exited with code ' + $capture.ExitCode }
        Write-Host ('dogfood: ' + $Name + ' SKIPPED - ' + $reason)
        $script:Results.Add((New-DogfoodTarget -Name $Name -UiStack $UiStack -Captured $false `
                    -Reason $reason -CaptureFile $null -ClassName $ClassName -Variant $Variant))
    }
}

function Add-DogfoodSkip {
    param([string]$Name, [string]$UiStack, [string]$ClassName, [string]$Variant, [string]$Reason)
    Write-Host ('dogfood: ' + $Name + ' SKIPPED - ' + $Reason)
    $script:Results.Add((New-DogfoodTarget -Name $Name -UiStack $UiStack -Captured $false `
                -Reason $Reason -CaptureFile $null -ClassName $ClassName -Variant $Variant))
}

$dogfoodTempDir = Join-Path $env:TEMP ('agent-desktop-dogfood-' + [guid]::NewGuid())

try {
    New-Item -ItemType Directory -Path $dogfoodTempDir -Force | Out-Null
    $utf8NoBom = New-Object System.Text.UTF8Encoding $false
    $files = [ordered]@{
        'alpha.txt'   = "alpha synthetic dogfood line one`r`nalpha synthetic dogfood line two`r`n"
        'bravo.txt'   = "bravo synthetic dogfood line one`r`nbravo synthetic dogfood line two`r`n"
        'charlie.txt' = "charlie synthetic dogfood line one`r`ncharlie synthetic dogfood line two`r`n"
        'delta.txt'   = "delta synthetic dogfood line one`r`ndelta synthetic dogfood line two`r`n"
    }
    foreach ($name in $files.Keys) {
        [IO.File]::WriteAllText((Join-Path $dogfoodTempDir $name), $files[$name], $utf8NoBom)
    }
    $alphaPath = Join-Path $dogfoodTempDir 'alpha.txt'

    # 1. classic Notepad on a scratch file
    Write-Host 'dogfood: launching classic Notepad'
    try {
        $notepad = Start-DogfoodProcess -FilePath 'notepad.exe' -ArgumentList @($alphaPath)
        $notepadHandle = Wait-DogfoodWindow -Process $notepad -TimeoutSec 15
        if ($notepadHandle -eq [IntPtr]::Zero) { throw 'notepad.exe never reported a top-level window' }
        Add-DogfoodCapture -Name 'notepad' -UiStack 'win32' -ClassName 'Notepad' `
            -Variant 'classic Win32 Notepad on a synthetic scratch file (dogfood run)' -OutFileName 'notepad.json'
    } catch {
        Add-DogfoodSkip -Name 'notepad' -UiStack 'win32' -ClassName 'Notepad' `
            -Variant 'classic Win32 Notepad on a synthetic scratch file (dogfood run)' -Reason $_.Exception.Message
    }

    # 2. Explorer on the scratch directory
    Write-Host 'dogfood: launching Explorer on the scratch directory'
    try {
        [void](Start-DogfoodProcess -FilePath 'explorer.exe' -ArgumentList @($dogfoodTempDir))
        Write-Host 'dogfood: waiting 22s for Explorer to reflect the filesystem change (A7-4)'
        Start-Sleep -Seconds 22
        Add-DogfoodCapture -Name 'explorer' -UiStack 'shell (Win32 CabinetWClass)' -ClassName 'CabinetWClass' `
            -Variant 'Windows Explorer on a synthetic scratch directory (dogfood run)' -OutFileName 'explorer.json'
    } catch {
        Add-DogfoodSkip -Name 'explorer' -UiStack 'shell (Win32 CabinetWClass)' -ClassName 'CabinetWClass' `
            -Variant 'Windows Explorer on a synthetic scratch directory (dogfood run)' -Reason $_.Exception.Message
    }

    # 3. WinForms scratch fixture
    Write-Host 'dogfood: building/launching the WinForms scratch fixture'
    try {
        & (Join-Path $script:ScratchDir 'build-scratch.ps1') | Out-Null
        $scratchExe = Join-Path $script:ScratchDir 'bin\ScratchForms.exe'
        if (-not (Test-Path -LiteralPath $scratchExe)) { throw "ScratchForms.exe was not found at $scratchExe" }
        $winforms = Start-DogfoodProcess -FilePath $scratchExe -ArgumentList @('--tag', 'dogfood')
        $winformsHandle = Wait-DogfoodWindow -Process $winforms -TimeoutSec 20
        if ($winformsHandle -eq [IntPtr]::Zero) { throw 'ScratchForms.exe never reported a top-level window' }
        $winformsClass = Get-DogfoodClassName -Handle $winformsHandle
        Add-DogfoodCapture -Name 'winforms' -UiStack 'winforms' -ClassName $winformsClass `
            -Variant 'AgentDesktop WinForms scratch fixture with Tab/Spinner/DataGridView (dogfood run)' -OutFileName 'winforms.json'
    } catch {
        Add-DogfoodSkip -Name 'winforms' -UiStack 'winforms' -ClassName '' `
            -Variant 'AgentDesktop WinForms scratch fixture (dogfood run)' -Reason $_.Exception.Message
    }

    # 4. WPF scratch fixture
    Write-Host 'dogfood: launching the WPF scratch fixture'
    try {
        $wpfScript = Join-Path $script:ScratchDir 'ScratchWpf.ps1'
        # -WindowStyle Hidden hides the launcher powershell.exe's own console
        # host window, so MainWindowHandle below resolves to the WPF window
        # the script creates (ShowActivated=False, but still visible - not
        # hidden) instead of racing the console host for it.
        $wpf = Start-DogfoodProcess -FilePath 'powershell.exe' -WindowStyle 'Hidden' -ArgumentList @(
            '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $wpfScript,
            '-Tag', 'dogfood', '-TimeoutSeconds', '180'
        )
        $wpfHandle = Wait-DogfoodWindow -Process $wpf -TimeoutSec 25
        if ($wpfHandle -eq [IntPtr]::Zero) { throw 'ScratchWpf.ps1 never reported a top-level window' }
        Start-Sleep -Seconds 2
        $wpfClass = Get-DogfoodClassName -Handle $wpfHandle
        if ($wpfClass -eq 'ConsoleWindowClass') { throw 'resolved the launcher console window instead of the WPF window' }
        Add-DogfoodCapture -Name 'wpf' -UiStack 'wpf' -ClassName $wpfClass `
            -Variant 'AgentDesktop WPF scratch fixture with TabControl/DataGrid (dogfood run)' -OutFileName 'wpf.json'
    } catch {
        Add-DogfoodSkip -Name 'wpf' -UiStack 'wpf' -ClassName '' `
            -Variant 'AgentDesktop WPF scratch fixture (dogfood run)' -Reason $_.Exception.Message
    }

    # 5. Chromium/Electron, if present
    Write-Host 'dogfood: probing for a Chromium/Electron window'
    try {
        Add-DogfoodCapture -Name 'chromium-electron' -UiStack 'chromium/electron' -ClassName 'Chrome_WidgetWin_1' `
            -Variant 'first visible Chromium/Electron top-level window found on the box (dogfood run)' -OutFileName 'chromium-electron.json'
    } catch {
        Add-DogfoodSkip -Name 'chromium-electron' -UiStack 'chromium/electron' -ClassName 'Chrome_WidgetWin_1' `
            -Variant 'first visible Chromium/Electron top-level window found on the box (dogfood run)' -Reason $_.Exception.Message
    }

    # 6. Settings, if present
    Write-Host 'dogfood: probing for Settings'
    try {
        $settingsProc = Get-Process -Name 'SystemSettings' -ErrorAction SilentlyContinue
        if (-not $settingsProc) { throw 'SystemSettings.exe is not running on this box' }
        Add-DogfoodCapture -Name 'settings' -UiStack 'uwp (ApplicationFrameHost)' -ClassName 'ApplicationFrameWindow' `
            -Variant 'Settings app frame window, SystemSettings.exe confirmed running (dogfood run)' -OutFileName 'settings.json'
    } catch {
        Add-DogfoodSkip -Name 'settings' -UiStack 'uwp (ApplicationFrameHost)' -ClassName 'ApplicationFrameWindow' `
            -Variant 'Settings app frame window (dogfood run)' -Reason $_.Exception.Message
    }

    $summaryPath = Join-Path $OutDir 'dogfood-run.json'
    $summaryJson = ConvertTo-Json -InputObject ([ordered]@{
            generated = (Get-Date).ToString('o')
            out_dir   = $OutDir
            targets   = $script:Results
        }) -Depth 10
    $redacted = Protect-ProbeText -Text $summaryJson
    $utf8NoBomOut = New-Object System.Text.UTF8Encoding $false
    [IO.File]::WriteAllText($summaryPath, $redacted, $utf8NoBomOut)
    if (-not (Test-CaptureRedaction -Path $summaryPath)) {
        throw "redaction residue in $summaryPath"
    }
    Write-Host ('dogfood: wrote ' + $summaryPath)
} finally {
    foreach ($launchedPid in $script:LaunchedPids) {
        try {
            $proc = Get-Process -Id $launchedPid -ErrorAction SilentlyContinue
            if ($proc) { Stop-Process -Id $launchedPid -Force -ErrorAction SilentlyContinue }
        } catch { }
    }
    if (Test-Path -LiteralPath $dogfoodTempDir) {
        Remove-Item -LiteralPath $dogfoodTempDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}
