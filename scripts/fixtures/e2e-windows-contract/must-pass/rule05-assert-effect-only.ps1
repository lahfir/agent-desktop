function Invoke-Leg {
    param($Target)
    Assert-Effect -Target $Target -Property 'value' -Expected 'done' -Action 'click'
    Assert-Envelope -Envelope (Invoke-Target -Target $Target -Action 'click') -Ok
}
