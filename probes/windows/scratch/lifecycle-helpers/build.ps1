#Requires -Version 5.1
<#
.SYNOPSIS
    Compiles LifecycleHelpers.cs (+ optional elevation-manifested twin).
#>
[CmdletBinding()]
param([switch]$Force)

$ErrorActionPreference = 'Stop'
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$source = Join-Path $here 'LifecycleHelpers.cs'
$outDir = Join-Path $here 'bin'
$exe = Join-Path $outDir 'LifecycleHelpers.exe'
$elevExe = Join-Path $outDir 'LifecycleHelpers.elev.exe'
$manifest = Join-Path $here 'requireAdmin.manifest'
$frameworkDir = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319'
$csc = Join-Path $frameworkDir 'csc.exe'

if (-not (Test-Path -LiteralPath $csc)) { throw "csc.exe missing at $csc" }
if (-not (Test-Path -LiteralPath $outDir)) { New-Item -ItemType Directory -Path $outDir | Out-Null }

$needBuild = $Force -or -not (Test-Path -LiteralPath $exe)
if (-not $needBuild) {
    if ((Get-Item -LiteralPath $source).LastWriteTimeUtc -gt (Get-Item -LiteralPath $exe).LastWriteTimeUtc) {
        $needBuild = $true
    }
}

if ($needBuild) {
    $args = @(
        '/nologo', '/target:winexe', '/langversion:5', '/platform:anycpu',
        "/out:$exe",
        '/reference:System.dll',
        '/reference:System.Drawing.dll',
        '/reference:System.Windows.Forms.dll',
        $source
    )
    $out = & $csc @args 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw ("LifecycleHelpers build failed: " + ($out -join '; '))
    }
}

if (-not (Test-Path -LiteralPath $manifest)) {
    $manifestBody = @'
<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>
'@
    $utf8 = New-Object System.Text.UTF8Encoding $true
    [IO.File]::WriteAllText($manifest, $manifestBody, $utf8)
}

$needElev = $Force -or -not (Test-Path -LiteralPath $elevExe)
if (-not $needElev) {
    if ((Get-Item -LiteralPath $source).LastWriteTimeUtc -gt (Get-Item -LiteralPath $elevExe).LastWriteTimeUtc) {
        $needElev = $true
    }
}
if ($needElev) {
    $args = @(
        '/nologo', '/target:winexe', '/langversion:5', '/platform:anycpu',
        "/out:$elevExe",
        "/win32manifest:$manifest",
        '/reference:System.dll',
        '/reference:System.Drawing.dll',
        '/reference:System.Windows.Forms.dll',
        $source
    )
    $out = & $csc @args 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw ("LifecycleHelpers.elev build failed: " + ($out -join '; '))
    }
}

Write-Output "ok: $exe"
Write-Output "ok: $elevExe"
