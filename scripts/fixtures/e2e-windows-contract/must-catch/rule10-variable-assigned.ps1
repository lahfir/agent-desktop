function Invoke-VersionLeg {
    $binary = 'target\release\agent-desktop.exe'
    & $binary version
}
