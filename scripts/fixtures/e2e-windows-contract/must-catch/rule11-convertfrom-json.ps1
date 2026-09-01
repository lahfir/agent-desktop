function Read-Envelope {
    param([string]$Json)
    return $Json | ConvertFrom-Json
}
