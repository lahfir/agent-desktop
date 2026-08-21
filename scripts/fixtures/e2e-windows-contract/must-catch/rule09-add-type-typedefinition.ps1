function Invoke-RawPInvokeLeg {
    Add-Type -Namespace Raw -Name Native -MemberDefinition '[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);'
}
