#Requires -Version 5.1

<#
    Reliability.ps1 - U10 approach item 5: remove-row then acting on the
    removed ref fails closed (STALE_REF), --wait-for-gone observes the
    removal, appear-later is observed by `wait --text` and by a post-action
    selector, and `wait enabled` on delayed-button.

    One correction, measured live against this fixture and binary and cited
    in the sub-phase report: the plan does not state a `--timeout-ms` for
    the stale-ref leg, and the default auto-wait budget matters here. A
    plain `click` on the pre-removal ref does not surface STALE_REF at all
    under the default budget - it returns generic `TIMEOUT` after ~5s,
    because the resolve loop treats "no live candidate found yet" as
    retryable and keeps polling until the deadline
    (crates/core/src/ref_action_poll.rs's `execute_poll_loop`/
    `handle_resolve_failure`). `--timeout-ms 0` forces exactly one resolve
    attempt and reliably reports `STALE_REF` - the same deterministic-form
    correction Acceptance.ps1's occlusion leg makes for the same reason.
#>

Set-StrictMode -Version 2.0

function Invoke-ReliabilityScenario {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$App)
    Register-Legs -Names @(
        'reliability-stale-ref-after-removal', 'reliability-wait-for-gone-observes-removal',
        'reliability-appear-later-wait-text', 'reliability-appear-later-post-action-selector',
        'reliability-wait-enabled-delayed-button'
    )

    Invoke-RemoveRowLegs -App $App
    Invoke-AppearLaterLegs -App $App
    Invoke-WaitEnabledLeg -App $App
}

function Invoke-RemoveRowLegs {
    <# removable-row is reset first (Visible = true) so this leg is
       falsifiable regardless of what an earlier run left behind - the same
       discipline NonInterference.ps1's check/clear legs use with their own
       Setup blocks. #>
    param([Parameter(Mandatory = $true)][string]$App)
    try {
        Enter-Stage -Lock DesktopLease -Body {
            $reset = Require-Target -Target (Find-Target -App $App -NativeId 'reset-removable' -TimeoutSeconds 10) -Description 'reset-removable'
            Invoke-Target -Target $reset -Action 'click' -RequireOk -Description 'reset-removable' | Out-Null

            $preRemoval = Require-Target -Target (Find-Target -App $App -NativeId 'removable-row' -TimeoutSeconds 10) -Description 'removable-row (pre-removal ref)'
            $removeRow = Require-Target -Target (Find-Target -App $App -NativeId 'remove-row' -TimeoutSeconds 10) -Description 'remove-row'

            $callArgs = @('click', $removeRow.RefId, '--snapshot', $removeRow.SnapshotId, '--wait-for-gone', 'button:Removable row')
            $envelope = Invoke-AgentDesktop -Arguments $callArgs -TimeoutSeconds 20
            Assert-Envelope -Envelope $envelope -Ok
            try {
                $stillThere = Find-Target -App $App -NativeId 'removable-row' -TimeoutSeconds 2
                if ($stillThere) { throw '--wait-for-gone returned ok but removable-row still resolves by --native-id' }
                Add-Pass -Leg 'reliability-wait-for-gone-observes-removal'
            } catch { Add-Fail -Leg 'reliability-wait-for-gone-observes-removal' -Reason $_.Exception.Message }

            try {
                $staleEnvelope = Invoke-AgentDesktop -Arguments @('click', $preRemoval.RefId, '--snapshot', $preRemoval.SnapshotId, '--timeout-ms', '0')
                Assert-Envelope -Envelope $staleEnvelope -ErrorCode 'STALE_REF'
                Add-Pass -Leg 'reliability-stale-ref-after-removal'
            } catch { Add-Fail -Leg 'reliability-stale-ref-after-removal' -Reason $_.Exception.Message }

            $resetAgain = Require-Target -Target (Find-Target -App $App -NativeId 'reset-removable' -TimeoutSeconds 10) -Description 'reset-removable'
            Invoke-Target -Target $resetAgain -Action 'click' -RequireOk -Description 'reset-removable' | Out-Null
        }
    } catch {
        Add-Fail -Leg 'reliability-wait-for-gone-observes-removal' -Reason $_.Exception.Message
        Add-Fail -Leg 'reliability-stale-ref-after-removal' -Reason $_.Exception.Message
    }
}

function Invoke-AppearLaterLegs {
    <# reset-appeared-text forces the known-false starting state first, the
       same Setup discipline NonInterference.ps1's check/clear legs use, so
       the wait below is provably watching a real transition. #>
    param([Parameter(Mandatory = $true)][string]$App)
    try {
        Enter-Stage -Lock DesktopLease -Body {
            $reset = Require-Target -Target (Find-Target -App $App -NativeId 'reset-appeared-text' -TimeoutSeconds 10) -Description 'reset-appeared-text'
            Invoke-Target -Target $reset -Action 'click' -RequireOk -Description 'reset-appeared-text' | Out-Null
            $appearLater = Require-Target -Target (Find-Target -App $App -NativeId 'appear-later' -TimeoutSeconds 10) -Description 'appear-later'
            Invoke-Target -Target $appearLater -Action 'click' -RequireOk -Description 'appear-later' | Out-Null
        }

        try {
            Enter-Stage -Lock DesktopLease -Body {
                $waitEnvelope = Invoke-AgentDesktop -Arguments @('wait', '--text', 'appeared', '--app', $App, '--timeout', '5000') -TimeoutSeconds 15
                Assert-Envelope -Envelope $waitEnvelope -Ok
            }
            Add-Pass -Leg 'reliability-appear-later-wait-text'
        } catch { Add-Fail -Leg 'reliability-appear-later-wait-text' -Reason $_.Exception.Message }

        try {
            Enter-Stage -Lock DesktopLease -Body {
                $appeared = Require-Target -Target (Find-Target -App $App -NativeId 'appeared-text' -TimeoutSeconds 10) -Description 'appeared-text'
                $value = Get-Target -Target $appeared -Property 'value'
                if ($value -ne 'appeared') { throw "appeared-text read '$value' via a fresh post-action find/get, expected 'appeared'" }
            }
            Add-Pass -Leg 'reliability-appear-later-post-action-selector'
        } catch { Add-Fail -Leg 'reliability-appear-later-post-action-selector' -Reason $_.Exception.Message }

        Enter-Stage -Lock DesktopLease -Body {
            $resetAgain = Require-Target -Target (Find-Target -App $App -NativeId 'reset-appeared-text' -TimeoutSeconds 10) -Description 'reset-appeared-text'
            Invoke-Target -Target $resetAgain -Action 'click' -RequireOk -Description 'reset-appeared-text' | Out-Null
        }
    } catch {
        Add-Fail -Leg 'reliability-appear-later-wait-text' -Reason $_.Exception.Message
        Add-Fail -Leg 'reliability-appear-later-post-action-selector' -Reason $_.Exception.Message
    }
}

function Invoke-WaitEnabledLeg {
    <# delayed-button starts disabled (reset-delayed-button forces that
       known-false state) and the 4s timer flips it enabled - `wait
       --predicate enabled` is the oracle, read independently of the click
       auto-wait legs Acceptance.ps1 already exercises against the same
       card. Wait-Target itself calls only Invoke-AgentDesktop (not
       Invoke-Target/Native), but it still touches the desktop, so it stays
       inside the same Enter-Stage body as the setup for R13's declared-lock
       discipline rather than only where rule09 strictly requires it. #>
    param([Parameter(Mandatory = $true)][string]$App)
    try {
        Enter-Stage -Lock DesktopLease -Body {
            $reset = Require-Target -Target (Find-Target -App $App -NativeId 'reset-delayed-button' -TimeoutSeconds 10) -Description 'reset-delayed-button'
            Invoke-Target -Target $reset -Action 'click' -RequireOk -Description 'reset-delayed-button' | Out-Null
            $enableLater = Require-Target -Target (Find-Target -App $App -NativeId 'enable-later' -TimeoutSeconds 10) -Description 'enable-later'
            Invoke-Target -Target $enableLater -Action 'click' -RequireOk -Description 'enable-later' | Out-Null
            $target = Require-Target -Target (Find-Target -App $App -NativeId 'delayed-button' -TimeoutSeconds 10) -Description 'delayed-button'
            $ok = Wait-Target -Target $target -Predicate 'enabled' -TimeoutMs 8000
            if (-not $ok) { throw 'wait --predicate enabled on delayed-button did not report found within 8000ms' }
        }
        Add-Pass -Leg 'reliability-wait-enabled-delayed-button'
    } catch { Add-Fail -Leg 'reliability-wait-enabled-delayed-button' -Reason $_.Exception.Message }
}
