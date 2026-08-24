function Invoke-MediumReadLeg {
    Start-StagedIntegrityProcess -FilePath $AgentDesktopBinary -ArgumentList @('snapshot')
}
