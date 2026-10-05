function Get-Win32ErrorOrThrow {
    param($Ok)
    if (-not $Ok) {
        $error = [System.Runtime.InteropServices.Marshal]::GetLastWin32Error()
        throw "failed: $error"
    }
}
