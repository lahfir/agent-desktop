function Enter-IsolatedEnvironment {
    param([string]$Root)
    $env:LOCALAPPDATA = Join-Path $Root 'appdata'
}
