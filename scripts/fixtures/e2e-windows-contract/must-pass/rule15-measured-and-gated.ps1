<# MUST-PASS rule15: the same measurement, gated on. #>
function Invoke-MeasuredAndGatedLeg {
    param([Parameter(Mandatory = $true)][string]$App)
    Register-Legs -Names @('measured-and-gated')
    Enter-Stage -Lock DesktopLease -Body {
        $target = Require-Target -Target (Find-Target -App $App -NativeId 'probe' -TimeoutSeconds 10) -Description 'probe'
        $observed = Test-Target -Target $target -Property 'enabled'
        Write-Host "observed=$observed"
        if (-not $observed) {
            Add-Fail -Leg 'measured-and-gated' -Reason 'the probe read false'
            return
        }
    }
    Add-Pass -Leg 'measured-and-gated'
}
