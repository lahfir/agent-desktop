function Invoke-Leg {
    $r = Invoke-Target -Target $script:FakeTarget -Action 'click'
    return $r.data.ok
}
