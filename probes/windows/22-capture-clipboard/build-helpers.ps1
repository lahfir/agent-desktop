#Requires -Version 5.1
# Builds Capture22.dll and small console helpers that load it.
[CmdletBinding()]
param([switch]$Force)

$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
. (Join-Path $here 'native.ps1')

# Force rebuild path by touching when -Force
if ($Force) {
    $dll = Join-Path $here 'bin\Capture22.dll'
    if (Test-Path -LiteralPath $dll) { Remove-Item -LiteralPath $dll -Force }
}
Initialize-CaptureClipboardNative

$frameworkDir = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319'
$csc = Join-Path $frameworkDir 'csc.exe'
$outDir = Join-Path $here 'bin'
$dll = Join-Path $outDir 'Capture22.dll'

$holderCs = @'
using System;
using System.Reflection;
class Program {
  static int Main() {
    var asm = Assembly.LoadFrom(AppDomain.CurrentDomain.BaseDirectory + "Capture22.dll");
    var t = asm.GetType("AgentDesktopProbe.A22.Capture22");
    return (int)t.GetMethod("RunClipboardHolder").Invoke(null, null);
  }
}
'@
$delayCs = @'
using System;
using System.Reflection;
class Program {
  static int Main() {
    var asm = Assembly.LoadFrom(AppDomain.CurrentDomain.BaseDirectory + "Capture22.dll");
    var t = asm.GetType("AgentDesktopProbe.A22.Capture22");
    return (int)t.GetMethod("RunDelayedClipboardOwner").Invoke(null, null);
  }
}
'@

$holderSrc = Join-Path $outDir 'ClipboardHolder.cs'
$delaySrc = Join-Path $outDir 'DelayedOwner.cs'
$holderExe = Join-Path $outDir 'ClipboardHolder.exe'
$delayExe = Join-Path $outDir 'DelayedOwner.exe'
[IO.File]::WriteAllText($holderSrc, $holderCs)
[IO.File]::WriteAllText($delaySrc, $delayCs)

foreach ($pair in @(
    @{ Src = $holderSrc; Out = $holderExe },
    @{ Src = $delaySrc; Out = $delayExe }
)) {
    $need = $Force -or -not (Test-Path -LiteralPath $pair.Out)
    if (-not $need) {
        if ((Get-Item -LiteralPath $dll).LastWriteTimeUtc -gt (Get-Item -LiteralPath $pair.Out).LastWriteTimeUtc) {
            $need = $true
        }
    }
    if ($need) {
        $args = @(
            '/nologo', '/target:exe', '/langversion:5', '/platform:anycpu',
            "/out:$($pair.Out)",
            '/reference:System.dll',
            $pair.Src
        )
        $out = & $csc @args 2>&1
        if ($LASTEXITCODE -ne 0) { throw ("helper build failed: " + ($out -join '; ')) }
    }
}

Write-Host "helpers ready under $outDir"
