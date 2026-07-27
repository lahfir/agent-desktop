<#
.SYNOPSIS
    Compiles ScratchForms.cs into ScratchForms.exe with the stock .NET Framework csc.exe.

.DESCRIPTION
    Standalone by design: does NOT dot-source common.ps1. Pins the pre-Roslyn
    csc.exe shipped with .NET Framework 4.x and forces /langversion:5 so the
    C# 5 ceiling is machine-checked rather than trusted.

    Output lands at probes/windows/scratch/bin/ScratchForms.exe.
#>
[CmdletBinding()]
param(
    [switch]$Force
)

$ErrorActionPreference = 'Stop'

$scratchDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$sourcePath = Join-Path $scratchDir 'ScratchForms.cs'
$outputDir = Join-Path $scratchDir 'bin'
$outputPath = Join-Path $outputDir 'ScratchForms.exe'
$configPath = Join-Path $outputDir 'ScratchForms.exe.config'
$frameworkDir = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319'
$csc = Join-Path $frameworkDir 'csc.exe'
$wpfDir = Join-Path $frameworkDir 'WPF'

if (-not (Test-Path -LiteralPath $csc)) {
    throw "csc.exe not found at $csc"
}
if (-not (Test-Path -LiteralPath $wpfDir)) {
    throw "UIAutomation reference assemblies not found at $wpfDir"
}
if (-not (Test-Path -LiteralPath $sourcePath)) {
    throw "Source not found at $sourcePath"
}
if (-not (Test-Path -LiteralPath $outputDir)) {
    New-Item -ItemType Directory -Path $outputDir | Out-Null
}

if ((Test-Path -LiteralPath $outputPath) -and (-not $Force)) {
    $sourceStamp = (Get-Item -LiteralPath $sourcePath).LastWriteTimeUtc
    $outputStamp = (Get-Item -LiteralPath $outputPath).LastWriteTimeUtc
    if ($outputStamp -gt $sourceStamp) {
        Write-Output "up-to-date: $outputPath"
        exit 0
    }
}

$cscArgs = @(
    '/nologo'
    '/target:winexe'
    '/langversion:5'
    '/platform:anycpu'
    "/out:$outputPath"
    '/reference:System.dll'
    '/reference:System.Drawing.dll'
    '/reference:System.Windows.Forms.dll'
    ('/reference:' + (Join-Path $wpfDir 'UIAutomationProvider.dll'))
    ('/reference:' + (Join-Path $wpfDir 'UIAutomationTypes.dll'))
    $sourcePath
)

Write-Output ("compiler: " + (Get-Item -LiteralPath $csc).VersionInfo.FileVersion)
Write-Output ("command : " + $csc + ' ' + ($cscArgs -join ' '))

$compilerOutput = & $csc $cscArgs 2>&1
$compilerExit = $LASTEXITCODE

foreach ($line in $compilerOutput) {
    Write-Output ($line | Out-String).TrimEnd()
}

if ($compilerExit -ne 0) {
    Write-Error ("csc.exe failed with exit code " + $compilerExit + " - see output above")
    exit $compilerExit
}
if (-not (Test-Path -LiteralPath $outputPath)) {
    Write-Error "csc.exe reported success but $outputPath does not exist"
    exit 1
}

$appConfig = @'
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
[IO.File]::WriteAllText($configPath, $appConfig, (New-Object System.Text.UTF8Encoding $false))

Write-Output ("built   : " + $outputPath)
Write-Output ("size    : " + (Get-Item -LiteralPath $outputPath).Length + " bytes")
Write-Output ("config  : " + $configPath)
exit 0
