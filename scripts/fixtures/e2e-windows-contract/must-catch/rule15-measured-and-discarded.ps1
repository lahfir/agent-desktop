<# MUST-CATCH rule15: the probe is measured, printed, and never gated on -
   the shape four shipped legs had. #>
function Invoke-MeasuredAndDiscardedLeg {
    param([Parameter(Mandatory = $true)][string]$App)
    Register-Legs -Names @('measured-and-discarded')
    Enter-Stage -Lock DesktopLease -Body {
        $target = Require-Target -Target (Find-Target -App $App -NativeId 'probe' -TimeoutSeconds 10) -Description 'probe'
        $observed = Test-Target -Target $target -Property 'enabled'
        Write-Host "observed=$observed"
    }
    Add-Pass -Leg 'measured-and-discarded'
}
