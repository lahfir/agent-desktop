function Invoke-Leg {
    $r = Invoke-Target -Target $script:FakeTarget -Action 'click'
    $ready = $true
    if ($ready -and $r.ok) {
        return $true
    }
    return $false
}
