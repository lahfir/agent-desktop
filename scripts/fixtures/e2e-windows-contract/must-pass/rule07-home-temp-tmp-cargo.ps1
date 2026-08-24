function Enter-IsolatedEnvironment {
    param([string]$Root)
    $env:HOME = Join-Path $Root 'home'
    $env:TEMP = Join-Path $Root 'tmp'
    $env:TMP = Join-Path $Root 'tmp'
    $env:CARGO_TARGET_DIR = Join-Path $Root 'cargo-target'
}
