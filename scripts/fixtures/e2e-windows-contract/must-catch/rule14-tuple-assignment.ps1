function Get-WindowOwner {
    param($Handle)
    $name, $pid = 'window', 4242
    return "$name/$pid"
}
