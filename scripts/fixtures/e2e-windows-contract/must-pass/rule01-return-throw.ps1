function Invoke-Leg {
    param([bool]$Ok)
    if (-not $Ok) {
        throw 'leg failed'
    }
    return $true
}
