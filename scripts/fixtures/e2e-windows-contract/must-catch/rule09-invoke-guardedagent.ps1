function Invoke-VersionLeg {
    Invoke-GuardedAgent -FilePath $AgentDesktopBinary -ArgumentList @('version')
}
