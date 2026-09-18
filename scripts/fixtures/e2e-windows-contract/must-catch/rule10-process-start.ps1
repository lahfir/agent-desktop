function Invoke-VersionLeg {
    [System.Diagnostics.Process]::Start('agent-desktop.exe', 'version')
}
