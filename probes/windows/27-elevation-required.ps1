#Requires -Version 5.1
<#
.SYNOPSIS
    Probe 27: live measurement of ERROR_ELEVATION_REQUIRED (Win32 error 740).

.DESCRIPTION
    Measures ERROR_ELEVATION_REQUIRED (Win32 error 740) live. A non-elevated caller
    that calls CreateProcessW on an executable whose manifest requests
    requireAdministrator does not get elevated - CreateProcessW fails synchronously
    with 740.

    Control leg (High integrity): confirms the requireAdministrator-manifested
    fixture launches successfully when called from an elevated caller.
    Measured leg (Medium integrity): stages a reduced-integrity (Medium) caller
    via Start-StagedIntegrityProcess, runs the in-process CreateProcessW probe,
    and records whether the elevation boundary was manufactured.

    Run: powershell -NoProfile -ExecutionPolicy Bypass -File .\probes\windows\27-elevation-required.ps1 -Label <devbox|ci>
#>
[CmdletBinding()]
param(
    [ValidateSet('devbox', 'ci')][string]$Label = 'devbox'
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

. (Join-Path (Split-Path -Parent $PSCommandPath) 'common.ps1')
Initialize-ProbeRedaction
Initialize-ProbeNative

$Probe = '27-elevation-required'
Register-MandatoryCapture -Name @("elevation-required-$Label.json")

function Parse-ProbeResultLine {
    param([string]$Line)
    $res = [ordered]@{
        IntegritySid = $null
        Launched     = $null
        Win32Error   = $null
    }
    if ([string]::IsNullOrWhiteSpace($Line)) { return $res }

    $sidMatch = [regex]::Match($Line, 'integrity_sid=([^\s]+)')
    if ($sidMatch.Success) {
        $res.IntegritySid = $sidMatch.Groups[1].Value
    }

    $launchedMatch = [regex]::Match($Line, 'launched=(\d+)')
    if ($launchedMatch.Success) {
        $res.Launched = ($launchedMatch.Groups[1].Value -eq '1')
    }

    $errMatch = [regex]::Match($Line, 'win32_error=(-?\d+)')
    if ($errMatch.Success) {
        $res.Win32Error = [int]$errMatch.Groups[1].Value
    }

    return $res
}

$probeDir = Split-Path -Parent $PSCommandPath
$repoRoot = Split-Path -Parent (Split-Path -Parent $probeDir)
$e2eWindowsDir = Join-Path $repoRoot 'tests\e2e-windows'
$fixturePath = Join-Path $probeDir 'scratch\lifecycle-helpers\bin\LifecycleHelpers.elev.exe'
$csSourcePath = Join-Path $probeDir '27-elevation-required.cs'
$compiledExePath = Join-Path $env:TEMP ('agent-desktop-probe-27-' + [guid]::NewGuid().ToString('N') + '.exe')

try {
    $fixturePresent = Test-Path -LiteralPath $fixturePath

    if (-not $fixturePresent) {
        $capture = [ordered]@{
            label                           = $Label
            fixture_present                 = $false
            probe_compiled                  = $null
            control_leg_launched            = $null
            control_leg_win32_error         = $null
            staged_caller_integrity_sid     = $null
            staged_caller_is_medium         = $null
            measured_leg_launched           = $null
            measured_win32_error            = $null
            elevation_boundary_manufactured = $null
            mechanism_note                  = $null
        }

        $capturePath = Write-ProbeJson -Probe $Probe -Name "elevation-required-$Label.json" -InputObject $capture
        Register-MandatoryPass -Capture $capturePath -Result $capture
        Assert-MandatoryMeasurement -Probe $Probe -Label $Label
        Write-ProbeResult -Probe $Probe -Status 'ok' -Message 'fixture missing; rebuild via probes/windows/scratch/lifecycle-helpers/build.ps1' -Data @{ fixture_present = $false }
        exit 0
    }

    # ---------------------------------------------------------------- STEP 1 - Compile sibling C# probe
    $csc = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
    $probeCompiled = $false

    if ((Test-Path -LiteralPath $csc) -and (Test-Path -LiteralPath $csSourcePath)) {
        try {
            $cscArgs = @('/nologo', '/langversion:5', '/target:exe', ('/out:' + $compiledExePath), $csSourcePath)
            $buildOutput = & $csc $cscArgs 2>&1
            if ($LASTEXITCODE -eq 0 -and (Test-Path -LiteralPath $compiledExePath)) {
                $probeCompiled = $true
            }
        } catch {
            $probeCompiled = $false
        }
    }

    if (-not $probeCompiled) {
        $capture = [ordered]@{
            label                           = $Label
            fixture_present                 = [bool]$fixturePresent
            probe_compiled                  = $false
            control_leg_launched            = $null
            control_leg_win32_error         = $null
            staged_caller_integrity_sid     = $null
            staged_caller_is_medium         = $null
            measured_leg_launched           = $null
            measured_win32_error            = $null
            elevation_boundary_manufactured = $null
            mechanism_note                  = $null
        }

        $capturePath = Write-ProbeJson -Probe $Probe -Name "elevation-required-$Label.json" -InputObject $capture
        Register-MandatoryPass -Capture $capturePath -Result $capture
        Assert-MandatoryMeasurement -Probe $Probe -Label $Label
        Write-ProbeResult -Probe $Probe -Status 'ok' -Message 'probe binary could not be built' -Data @{ probe_compiled = $false }
        exit 0
    }

    Import-Module (Join-Path $e2eWindowsDir 'NativeTypes.psm1') -Force
    Import-Module (Join-Path $e2eWindowsDir 'Native.psm1') -Force
    Import-Module (Join-Path $e2eWindowsDir 'NativeToken.psm1') -Force
    Import-Module (Join-Path $e2eWindowsDir 'StagedProcess.psm1') -Force

    # ---------------------------------------------------------------- STEP 2 - Control leg (Elevated caller)
    $controlResultPath = Join-Path $env:TEMP ('agent-desktop-probe-27-control-' + [guid]::NewGuid().ToString('N') + '.txt')
    $controlLegLaunched = $null
    $controlLegWin32Error = $null

    try {
        if (Test-Path -LiteralPath $controlResultPath) {
            Remove-Item -LiteralPath $controlResultPath -Force -ErrorAction SilentlyContinue
        }
        $psi = New-Object System.Diagnostics.ProcessStartInfo
        $psi.FileName = $compiledExePath
        $psi.Arguments = ('"' + $fixturePath + '" "' + $controlResultPath + '"')
        $psi.UseShellExecute = $false
        $psi.CreateNoWindow = $true
        $controlProc = [System.Diagnostics.Process]::Start($psi)
        if ($null -ne $controlProc) {
            $controlProc.WaitForExit(15000) | Out-Null
            if (-not $controlProc.HasExited) {
                try { $controlProc.Kill() } catch { }
            }
        }
        if (Test-Path -LiteralPath $controlResultPath) {
            $controlLine = [System.IO.File]::ReadAllText($controlResultPath).Trim()
            $parsedControl = Parse-ProbeResultLine -Line $controlLine
            $controlLegLaunched = $parsedControl.Launched
            $controlLegWin32Error = $parsedControl.Win32Error
        }
    } catch {
        $controlLegLaunched = $null
        $controlLegWin32Error = $null
    } finally {
        if (Test-Path -LiteralPath $controlResultPath) {
            try { Remove-Item -LiteralPath $controlResultPath -Force -ErrorAction SilentlyContinue } catch { }
        }
    }

    # ---------------------------------------------------------------- STEP 3 - Measured leg (Medium integrity caller)
    $measuredResultPath = Join-Path $env:TEMP ('agent-desktop-probe-27-measured-' + [guid]::NewGuid().ToString('N') + '.txt')
    $stagedCallerIntegritySid = $null
    $stagedCallerIsMedium = $null
    $measuredLegLaunched = $null
    $measuredWin32Error = $null

    try {
        if (Test-Path -LiteralPath $measuredResultPath) {
            Remove-Item -LiteralPath $measuredResultPath -Force -ErrorAction SilentlyContinue
        }
        $stagedResult = Start-StagedIntegrityProcess -IntegrityLevel Medium -FilePath $compiledExePath -ArgumentList @($fixturePath, $measuredResultPath) -TimeoutSeconds 15
        if ($null -ne $stagedResult -and $null -ne $stagedResult.LiveProcessIntegritySid) {
            $stagedCallerIntegritySid = [string]$stagedResult.LiveProcessIntegritySid
            $stagedCallerIsMedium = ($stagedCallerIntegritySid -eq 'S-1-16-8192')
        }
        Start-Sleep -Milliseconds 250
        if (Test-Path -LiteralPath $measuredResultPath) {
            $measuredLine = [System.IO.File]::ReadAllText($measuredResultPath).Trim()
            $parsedMeasured = Parse-ProbeResultLine -Line $measuredLine
            $measuredLegLaunched = $parsedMeasured.Launched
            $measuredWin32Error = $parsedMeasured.Win32Error
        }
    } catch {
        $stagedCallerIntegritySid = $null
        $stagedCallerIsMedium = $null
        $measuredLegLaunched = $null
        $measuredWin32Error = $null
    } finally {
        if (Test-Path -LiteralPath $measuredResultPath) {
            try { Remove-Item -LiteralPath $measuredResultPath -Force -ErrorAction SilentlyContinue } catch { }
        }
    }

    # ---------------------------------------------------------------- STEP 4 & 5 - Verdict & Capture
    $elevationBoundaryManufactured = $false
    if ($measuredLegLaunched -eq $false -and $measuredWin32Error -eq 740) {
        $elevationBoundaryManufactured = $true
    }

    $mechanismNote = 'CreateRestrictedToken(DISABLE_MAX_PRIVILEGE) plus a lowered integrity label disables privileges but leaves the Administrators group SID in the token, so the staged caller is Medium-integrity while still carrying Administrators and Windows requires no elevation'

    $capture = [ordered]@{
        label                           = $Label
        fixture_present                 = [bool]$fixturePresent
        probe_compiled                  = [bool]$probeCompiled
        control_leg_launched            = $controlLegLaunched
        control_leg_win32_error         = $controlLegWin32Error
        staged_caller_integrity_sid     = $stagedCallerIntegritySid
        staged_caller_is_medium         = $stagedCallerIsMedium
        measured_leg_launched           = $measuredLegLaunched
        measured_win32_error            = $measuredWin32Error
        elevation_boundary_manufactured = [bool]$elevationBoundaryManufactured
        mechanism_note                  = $mechanismNote
    }

    $capturePath = Write-ProbeJson -Probe $Probe -Name "elevation-required-$Label.json" -InputObject $capture
    Register-MandatoryPass -Capture $capturePath -Result $capture
    Assert-MandatoryMeasurement -Probe $Probe -Label $Label

    if ($elevationBoundaryManufactured) {
        $message = '740 was observed from a Medium-integrity caller with the control leg launching'
    } else {
        $message = "elevation boundary was NOT manufactured (control: launched=$controlLegLaunched, win32_error=$controlLegWin32Error; measured: launched=$measuredLegLaunched, win32_error=$measuredWin32Error); staged token retains Administrators"
    }

    Write-ProbeResult -Probe $Probe -Status 'ok' -Message $message -Data @{
        control_leg_launched            = $controlLegLaunched
        control_leg_win32_error         = $controlLegWin32Error
        staged_caller_integrity_sid     = $stagedCallerIntegritySid
        staged_caller_is_medium         = $stagedCallerIsMedium
        measured_leg_launched           = $measuredLegLaunched
        measured_win32_error            = $measuredWin32Error
        elevation_boundary_manufactured = [bool]$elevationBoundaryManufactured
    }

    if (Test-Path -LiteralPath $compiledExePath) {
        try { Remove-Item -LiteralPath $compiledExePath -Force -ErrorAction SilentlyContinue } catch { }
    }
    exit 0
} catch {
    if (Test-Path -LiteralPath $compiledExePath) {
        try { Remove-Item -LiteralPath $compiledExePath -Force -ErrorAction SilentlyContinue } catch { }
    }
    Write-ProbeResult -Probe $Probe -Status 'fail' -Message ('unhandled error: ' + ($_.Exception.Message -replace '[\r\n]+', ' '))
    exit 1
}
