function Get-Status {
    param($Target)
    Invoke-AgentDesktop -Arguments @('get', $Target.RefId, '--property', 'text')
}
