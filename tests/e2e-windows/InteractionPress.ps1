#Requires -Version 5.1

<#
    InteractionPress.ps1 - Invoke-PressLegs, dot-sourced by
    scenarios/Interaction.ps1. Split out purely to keep both files under
    the 400-line cap, and placed beside InteractionHeaded.ps1 rather than
    under scenarios/ for the reason that file's header gives: the contract
    gate scopes every scenarios/*.ps1 file as a top-level scenario required
    to call Register-Legs and declare its own locks. The caller registers
    the four legs below; this function takes both locks itself. $App is
    always passed explicitly; no caller-scope state is shared implicitly.

    press takes a combo and an optional --app, never a ref
    (src/cli_args/actions.rs), so every leg observes the effect on the
    control that held keyboard focus, never on the command's own ok. Key
    names come from crates/windows/src/input/keyboard/keyboard_map.rs: "delete" is an
    alias of Backspace there, so the legs say backspace. The blocked-combo
    refusal is POLICY_DENIED from core (crates/core/src/commands/combo.rs),
    raised before any app is resolved, so it needs neither --headed nor
    --app and delivers nothing.
#>

Set-StrictMode -Version 2.0

function Invoke-PressLegs {
    param([Parameter(Mandatory = $true)][string]$App)
    Enter-Stage -Lock DesktopLease -Body {
        Enter-Stage -Lock ForegroundStage -Body {
            try {
                $textInput = Require-Target -Target (Find-Target -App $App -NativeId 'text-input' -TimeoutSeconds 10) -Description 'text-input'
                Invoke-Target -Target $textInput -Action 'scroll-to' -RequireOk -Description 'text-input (scroll into view)' | Out-Null
                Invoke-Target -Target $textInput -Action 'set-value' -ActionArgs @('abc') -RequireOk -Description 'text-input (seed)' | Out-Null
                Invoke-Target -Target $textInput -Action 'focus' -Headed -RequireOk -Description 'text-input (keyboard focus)' | Out-Null
                if ((Get-Target -Target $textInput -Property 'value') -ne 'abc') { throw 'text-input did not hold the seed value before the press' }
                Assert-Envelope -Envelope (Invoke-AgentDesktop -Arguments @('--headed', 'press', 'ctrl+a', '--app', $App)) -Ok
                Assert-Envelope -Envelope (Invoke-AgentDesktop -Arguments @('--headed', 'press', 'backspace', '--app', $App)) -Ok
                $clock = [System.Diagnostics.Stopwatch]::StartNew()
                $observed = Get-Target -Target $textInput -Property 'value'
                while ($observed -ne '' -and $clock.Elapsed.TotalSeconds -lt 5) {
                    Start-Sleep -Milliseconds 150
                    $observed = Get-Target -Target $textInput -Property 'value'
                }
                if ($observed -ne '') { throw "ctrl+a then backspace left text-input at '$observed'; expected the whole value removed (a dropped ctrl would leave a partial value, an undelivered press leaves 'abc')" }
                Add-Pass -Leg 'press-ctrl-a-backspace-clears-text'
            } catch { Add-Fail -Leg 'press-ctrl-a-backspace-clears-text' -Reason $_.Exception.Message }

            try {
                $textInput = Require-Target -Target (Find-Target -App $App -NativeId 'text-input' -TimeoutSeconds 10) -Description 'text-input'
                Invoke-Target -Target $textInput -Action 'focus' -Headed -RequireOk -Description 'text-input (keyboard focus)' | Out-Null
                if (-not (Test-Target -Target $textInput -Property 'focused')) { throw 'text-input was not focused before shift+tab; the leg would prove nothing' }
                Assert-Envelope -Envelope (Invoke-AgentDesktop -Arguments @('--headed', 'press', 'shift+tab', '--app', $App)) -Ok
                $clock = [System.Diagnostics.Stopwatch]::StartNew()
                $stillFocused = Test-Target -Target $textInput -Property 'focused'
                while ($stillFocused -and $clock.Elapsed.TotalSeconds -lt 5) {
                    Start-Sleep -Milliseconds 150
                    $stillFocused = Test-Target -Target $textInput -Property 'focused'
                }
                if ($stillFocused) { throw 'text-input still reports focused after shift+tab' }
                Add-Pass -Leg 'press-shift-tab-moves-focus'
            } catch { Add-Fail -Leg 'press-shift-tab-moves-focus' -Reason $_.Exception.Message }

            try {
                $button = Require-Target -Target (Find-Target -App $App -NativeId 'primary-button' -TimeoutSeconds 10) -Description 'primary-button'
                Invoke-Target -Target $button -Action 'scroll-to' -RequireOk -Description 'primary-button (scroll into view)' | Out-Null
                Invoke-Target -Target $button -Action 'focus' -Headed -RequireOk -Description 'primary-button (keyboard focus)' | Out-Null
                $clickStatus = Require-Target -Target (Find-Target -App $App -NativeId 'click-status' -TimeoutSeconds 10) -Description 'click-status'
                $before = Get-Target -Target $clickStatus -Property 'value'
                Assert-Envelope -Envelope (Invoke-AgentDesktop -Arguments @('--headed', 'press', 'return', '--app', $App)) -Ok
                $clock = [System.Diagnostics.Stopwatch]::StartNew()
                $after = Get-Target -Target $clickStatus -Property 'value'
                while ($after -eq $before -and $clock.Elapsed.TotalSeconds -lt 5) {
                    Start-Sleep -Milliseconds 150
                    $after = Get-Target -Target $clickStatus -Property 'value'
                }
                if ($after -eq $before -or $after -notlike 'clicked#*') { throw "return on the focused primary-button left click-status at '$after' (was '$before')" }
                Add-Pass -Leg 'press-return-activates-focused-button'
            } catch { Add-Fail -Leg 'press-return-activates-focused-button' -Reason $_.Exception.Message }

            try {
                $foregroundBefore = Get-NativeForegroundWindowHandle
                $refused = Invoke-AgentDesktop -Arguments @('press', 'win+x')
                Assert-Envelope -Envelope $refused -ErrorCode 'POLICY_DENIED' -Delivery 'not_delivered'
                if (-not ([string]$refused['error']['message']).Contains('blocked for safety')) { throw "win+x was refused with an unexpected message: '$($refused['error']['message'])'" }
                $clock = [System.Diagnostics.Stopwatch]::StartNew()
                while ($clock.ElapsedMilliseconds -lt 1500) {
                    if ((Get-NativeForegroundWindowHandle) -ne $foregroundBefore) { throw 'foreground window changed after a refused win+x; the combo may have been delivered' }
                    Start-Sleep -Milliseconds 150
                }
                if (-not (Get-WindowId -Where { $_['title'] -eq 'AgentDeskFixture' } -TimeoutSeconds 5)) { throw 'the fixture window is no longer listed after the refused combo' }
                Add-Pass -Leg 'press-blocked-combo-refused'
            } catch { Add-Fail -Leg 'press-blocked-combo-refused' -Reason $_.Exception.Message }
        }
    }
}
