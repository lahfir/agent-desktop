function Remove-SuiteRoot {
    param([string]$Path)
    Remove-ItemRecoverable -Path $Path
    Remove-Item Env:\AGENT_DESKTOP_SESSION -ErrorAction SilentlyContinue
}
