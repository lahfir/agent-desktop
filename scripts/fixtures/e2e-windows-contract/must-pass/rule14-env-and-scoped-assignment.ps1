function Set-IsolatedState {
    param($Root)
    $env:CARGO_TARGET_DIR = Join-Path $Root 'cargo-target'
    $script:error = 'a script-scoped variable, not the automatic $Error'
}
