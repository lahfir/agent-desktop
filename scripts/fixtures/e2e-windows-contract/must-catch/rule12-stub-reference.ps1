function Set-StubTarget {
    param([string]$SelftestDir)
    Set-TargetBinary -FilePath 'powershell.exe' -PrefixArgs @('-File', (Join-Path $SelftestDir 'Stub-AgentDesktop.ps1'))
}
