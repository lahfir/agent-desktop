function Test-ExitCodeAndParams {
    param($App)
    $root = $PSScriptRoot
    $bound = $PSBoundParameters.Count
    $exitCode = $LASTEXITCODE
    return "$root/$bound/$exitCode"
}
