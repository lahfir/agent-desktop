function Start-Fixture {
    Invoke-Guarded -FilePath 'fixture.exe' -ArgumentList @()
}
