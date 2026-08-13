#Requires -Version 5.1
# Capture/clipboard Win32 bindings for Area 22. Compiles Capture22.cs via csc.

function Initialize-CaptureClipboardNative {
    if ('AgentDesktopProbe.A22.Capture22' -as [type]) { return }

    $here = Split-Path -Parent $PSCommandPath
    $source = Join-Path $here 'Capture22.cs'
    $outDir = Join-Path $here 'bin'
    $dll = Join-Path $outDir 'Capture22.dll'
    $frameworkDir = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319'
    $csc = Join-Path $frameworkDir 'csc.exe'

    if (-not (Test-Path -LiteralPath $csc)) {
        throw "csc.exe missing at $csc"
    }
    if (-not (Test-Path -LiteralPath $source)) {
        throw "Capture22.cs missing at $source"
    }
    if (-not (Test-Path -LiteralPath $outDir)) {
        New-Item -ItemType Directory -Path $outDir -Force | Out-Null
    }

    $needBuild = -not (Test-Path -LiteralPath $dll)
    if (-not $needBuild) {
        if ((Get-Item -LiteralPath $source).LastWriteTimeUtc -gt (Get-Item -LiteralPath $dll).LastWriteTimeUtc) {
            $needBuild = $true
        }
    }
    if ($needBuild) {
        $args = @(
            '/nologo', '/target:library', '/langversion:5', '/platform:anycpu',
            "/out:$dll",
            '/reference:System.dll',
            '/reference:System.Drawing.dll',
            '/reference:System.Windows.Forms.dll',
            $source
        )
        $out = & $csc @args 2>&1
        if ($LASTEXITCODE -ne 0) {
            throw ('Capture22 build failed: ' + ($out -join '; '))
        }
    }

    [void][Reflection.Assembly]::LoadFrom($dll)
}
