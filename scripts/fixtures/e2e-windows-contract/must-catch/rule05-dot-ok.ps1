function Invoke-Leg {
    $r = Invoke-Target -Target $script:FakeTarget -Action 'click'
    if ($r.ok) {
        return $true
    }
    return $false
}
