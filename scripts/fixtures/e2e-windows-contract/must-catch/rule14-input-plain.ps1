function Invoke-Leg {
    param($App)
    $input = Require-Target -Target (Find-Target -App $App -NativeId 'text-input') -Description 'text-input'
    Invoke-Target -Target $input -Action 'click'
}
