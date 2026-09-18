function Remove-SuiteRoot {
    param([string]$Path)
    Remove-Item -LiteralPath $Path -Recurse -Force
}
