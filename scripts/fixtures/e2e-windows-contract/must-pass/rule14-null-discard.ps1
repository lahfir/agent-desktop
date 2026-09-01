function Invoke-Quietly {
    param($Doc)
    $null = $Doc | ConvertFrom-Json
}
