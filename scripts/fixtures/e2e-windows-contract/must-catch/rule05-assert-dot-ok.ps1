function Invoke-Leg {
    $r = Invoke-Target -Target $script:FakeTarget -Action 'click'
    Assert-True ($r.ok) 'the action should have reported ok'
}
