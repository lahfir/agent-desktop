function Invoke-VersionLeg {
    param($StagedBinary)
    Invoke-GuardedAgent -FilePath $StagedBinary -ArgumentList @('version')
}
