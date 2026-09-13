function Invoke-VersionLeg {
    Start-Process -FilePath 'target\release\agent-desktop.exe' -ArgumentList @('version')
}
