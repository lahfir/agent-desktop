function Get-Status {
    param($Target)
    Invoke-AgentDesktop -Arguments @('get', $Target.RefId, '--snapshot', $Target.SnapshotId, '--property', 'text')
}
