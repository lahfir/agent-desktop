#Requires -Version 5.1

<#
    Area 26 shared plumbing: compiles and binds shell-probe.cs (the UIA3 COM
    shim built exactly like probes/windows/08-uia3-com.cs - hand-declared
    [ComImport] interfaces bound to CUIAutomation8, never the GAC managed
    client), runs its modes as child processes, and writes area-26 captures
    with the corpus's redact-normalize-verify pipeline.

    Capture-safety rules every caller inherits:
      - no window title, element Name, pid number, or untagged AutomationId
        ever crosses from the helper into a capture;
      - this lib only renames what the helper already emits.
#>

Set-StrictMode -Version 2.0

$script:Shell26Dir = Split-Path -Parent $MyInvocation.MyCommand.Path
$script:Shell26CaptureDir = Join-Path $script:Shell26Dir 'captures'
if (-not (Test-Path -LiteralPath $script:Shell26CaptureDir)) {
    New-Item -ItemType Directory -Path $script:Shell26CaptureDir -Force | Out-Null
}
$script:ShellProbeExe = $null

function Initialize-ShellProbe {
    if ($script:ShellProbeExe -and (Test-Path -LiteralPath $script:ShellProbeExe)) { return $script:ShellProbeExe }
    $csc = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
    if (-not (Test-Path -LiteralPath $csc)) { throw ('csc.exe not found at ' + $csc) }
    $buildDir = Join-Path $env:TEMP 'agent-desktop-shell26'
    New-Item -ItemType Directory -Path $buildDir -Force | Out-Null
    $exe = Join-Path $buildDir 'shell-probe.exe'
    $src = Join-Path $script:Shell26Dir 'shell-probe.cs'
    if (-not (Test-Path -LiteralPath $src)) { throw ('shell-probe.cs missing at ' + $src) }
    $needRebuild = -not (Test-Path -LiteralPath $exe)
    if (-not $needRebuild) {
        $needRebuild = ((Get-Item -LiteralPath $src).LastWriteTimeUtc -gt (Get-Item -LiteralPath $exe).LastWriteTimeUtc)
    }
    if ($needRebuild) {
        $compilerOutput = (& $csc /nologo /target:exe /langversion:5 /platform:anycpu ('/out:' + $exe) '/reference:System.dll' $src 2>&1 | Out-String).Trim()
        if ($LASTEXITCODE -ne 0) { throw ('csc.exe failed (' + $LASTEXITCODE + '): ' + $compilerOutput) }
    }
    $script:ShellProbeExe = $exe
    return $exe
}

function Invoke-ShellProbe {
    param(
        [Parameter(Mandatory = $true)][string[]]$Arguments,
        [int]$TimeoutSec = 300
    )
    $exe = Initialize-ShellProbe
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $exe
    $psi.Arguments = ($Arguments | ForEach-Object {
            if ($_ -match '\s') { '"' + $_ + '"' } else { $_ }
        }) -join ' '
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.CreateNoWindow = $true
    $proc = [System.Diagnostics.Process]::Start($psi)
    Register-ScratchProcessId -ProcessId $proc.Id
    $stdout = $proc.StandardOutput.ReadToEnd()
    $stderr = $proc.StandardError.ReadToEnd()
    if (-not $proc.WaitForExit($TimeoutSec * 1000)) {
        try { $proc.Kill() } catch { }
        throw ('shell-probe mode ' + $Arguments[0] + ' exceeded ' + $TimeoutSec + 's')
    }
    if ($proc.ExitCode -ne 0) {
        throw ('shell-probe mode ' + $Arguments[0] + ' exited ' + $proc.ExitCode + ': ' + $stderr.Trim())
    }
    return ($stdout.Trim() | ConvertFrom-Json)
}

function Write-Shell26Capture {
    param(
        [Parameter(Mandatory = $true)][string]$Name,
        [Parameter(Mandatory = $true)][AllowEmptyString()][string]$Content
    )
    if (-not (Test-Path -LiteralPath $script:Shell26CaptureDir)) {
        New-Item -ItemType Directory -Path $script:Shell26CaptureDir -Force | Out-Null
    }
    $redacted = Protect-ProbeText -Text $Content
    $path = Join-Path $script:Shell26CaptureDir $Name
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
    The clean-baseline rule for accelerator-raised surfaces: nothing another
    leg left on screen may stand in for the surface about to be raised. Sends
    ESC, and for a shell CoreWindow that ignores ESC (the empty-center state
    does not take keyboard dismissal) posts WM_CLOSE to the specific handle,
    verifying through the reach mechanism after each step that no uncloaked
    shell CoreWindow remains; returns $true when clean, $false otherwise.
#>
function Reset-ShellSurfaceBaseline {
    param([int]$Attempts = 4)
    Initialize-ShellProbe
    $openHandles = New-Object System.Collections.ArrayList
    try {
        for ($i = 0; $i -lt $Attempts; $i++) {
            Invoke-ShellProbe -Arguments @('key', '--seq', 'esc') | Out-Null
            Start-Sleep -Milliseconds 400
            [void]$openHandles.Clear()
            $scan = Invoke-ShellProbe -Arguments @('reachscan')
            foreach ($c in $scan.children) {
                if ($c.ac_candidate -and $c.nativewindowhandle -ne 0) {
                    $cloaked = $true
                    try {
                        $pred = Invoke-ShellProbe -Arguments @('predicate', '--hwnd', ([string]$c.nativewindowhandle))
                        $cloaked = ($pred.cloak_state -ne 'none')
                    } catch { }
                    if (-not $cloaked) { [void]$openHandles.Add([string]$c.nativewindowhandle) }
                }
            }
            if (@($openHandles).Count -eq 0) { return $true }
            foreach ($h in @($openHandles)) {
                Invoke-ShellProbe -Arguments @('closewindow', '--hwnd', ([string]$h)) | Out-Null
            }
            Start-Sleep -Milliseconds 700
        }
        return $false
    } finally {
    }
}
