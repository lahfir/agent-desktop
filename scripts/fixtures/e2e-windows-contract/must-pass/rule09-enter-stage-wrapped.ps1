function Invoke-ClickLeg {
    param($Target)
    Enter-Stage -Lock 'DesktopLease' -Body {
        Invoke-Target -Target $Target -Action 'click'
        Get-NativeCursorPosition
    }
}
