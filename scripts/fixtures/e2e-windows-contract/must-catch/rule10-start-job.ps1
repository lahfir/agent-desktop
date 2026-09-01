function Invoke-VersionLeg {
    Start-Job -ScriptBlock { agent-desktop.exe version }
}
