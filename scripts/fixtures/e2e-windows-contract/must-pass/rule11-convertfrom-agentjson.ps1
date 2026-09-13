function Read-Envelope {
    param([string]$Json)
    return ConvertFrom-AgentJson -Json $Json
}
