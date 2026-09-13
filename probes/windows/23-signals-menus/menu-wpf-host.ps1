<#
.SYNOPSIS
    Standalone WPF host carrying a real Menu bar and a ContextMenu, for area
    23's per-stack menu detection leg.

.DESCRIPTION
    Deliberately does NOT dot-source common.ps1 or reuse ScratchWpf.ps1 -
    that fixture is shared by the AutomationId-census probes and the crate's
    own live tests, which assert its exact control set; this host is
    additive-only and area-23-private so nothing else can regress from it.

    Self-driving on a fixed DispatcherTimer schedule rather than reacting to
    external signals: WPF's Popup/ContextMenu state changes are synchronous
    with the dispatcher, so a fixed schedule is exact, and it avoids IPC
    entirely. Every transition is also written to -StateFile so the probe can
    confirm the fixture's own state independently of the predicate under test.

    Emits SCRATCHMENU23_PID=<pid> and SCRATCHMENU23_READY=1 on stdout.
#>
[CmdletBinding()]
param(
    [int]$Left = 40,
    [int]$Top = 40,
    [int]$MenuOpenAtMs = 1200,
    [int]$MenuOpenDurationMs = 2400,
    [int]$ContextOpenAtMs = 4200,
    [int]$ContextOpenDurationMs = 2400,
    [int]$ExitAtMs = 9000,
    [string]$StateFile = ''
)

$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName PresentationFramework
Add-Type -AssemblyName PresentationCore
Add-Type -AssemblyName WindowsBase

function Write-MenuState {
    param([string]$Value)
    if ($StateFile) {
        try { [IO.File]::WriteAllText($StateFile, $Value) } catch { }
    }
}

$xaml = @'
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
        Title="AgentDesktop Scratch Menu WPF" Width="360" Height="260"
        WindowStartupLocation="Manual" ShowActivated="False"
        AutomationProperties.AutomationId="wndScratchMenuWpf">
  <DockPanel>
    <Menu DockPanel.Dock="Top" x:Name="mnuBar" AutomationProperties.AutomationId="mnuBar">
      <MenuItem x:Name="mnuFile" Header="_File" AutomationProperties.AutomationId="mnuFile">
        <MenuItem Header="Open" AutomationProperties.AutomationId="mnuFileOpen"/>
        <MenuItem Header="Exit" AutomationProperties.AutomationId="mnuFileExit"/>
      </MenuItem>
    </Menu>
    <Border x:Name="ctxTarget" Background="White" AutomationProperties.AutomationId="ctxTarget">
      <Border.ContextMenu>
        <ContextMenu x:Name="ctxMenu" AutomationProperties.AutomationId="ctxMenu">
          <MenuItem Header="Copy" AutomationProperties.AutomationId="ctxCopy"/>
          <MenuItem Header="Paste" AutomationProperties.AutomationId="ctxPaste"/>
        </ContextMenu>
      </Border.ContextMenu>
      <TextBlock x:Name="lblState" Text="idle" Margin="8" AutomationProperties.AutomationId="lblState"/>
    </Border>
  </DockPanel>
</Window>
'@

$reader = New-Object System.Xml.XmlNodeReader ([xml]$xaml)
$window = [Windows.Markup.XamlReader]::Load($reader)
$window.Left = $Left
$window.Top = $Top

$mnuFile = $window.FindName('mnuFile')
$ctxTarget = $window.FindName('ctxTarget')
$ctxMenu = $ctxTarget.ContextMenu
$lblState = $window.FindName('lblState')

$window.Add_Closed({ $window.Dispatcher.InvokeShutdown() })
$window.Show()

Write-Output ('SCRATCHMENU23_PID=' + $PID)
Write-Output 'SCRATCHMENU23_READY=1'
Write-MenuState 'idle'

$openMenuTimer = New-Object System.Windows.Threading.DispatcherTimer
$openMenuTimer.Interval = [TimeSpan]::FromMilliseconds($MenuOpenAtMs)
$openMenuTimer.Add_Tick({
        $openMenuTimer.Stop()
        $mnuFile.IsSubmenuOpen = $true
        $lblState.Text = 'menu-open'
        Write-MenuState 'menu-open'
    })
$openMenuTimer.Start()

$closeMenuTimer = New-Object System.Windows.Threading.DispatcherTimer
$closeMenuTimer.Interval = [TimeSpan]::FromMilliseconds($MenuOpenAtMs + $MenuOpenDurationMs)
$closeMenuTimer.Add_Tick({
        $closeMenuTimer.Stop()
        $mnuFile.IsSubmenuOpen = $false
        $lblState.Text = 'menu-closed'
        Write-MenuState 'menu-closed'
    })
$closeMenuTimer.Start()

$openCtxTimer = New-Object System.Windows.Threading.DispatcherTimer
$openCtxTimer.Interval = [TimeSpan]::FromMilliseconds($ContextOpenAtMs)
$openCtxTimer.Add_Tick({
        $openCtxTimer.Stop()
        $ctxMenu.PlacementTarget = $ctxTarget
        $ctxMenu.IsOpen = $true
        $lblState.Text = 'context-open'
        Write-MenuState 'context-open'
    })
$openCtxTimer.Start()

$closeCtxTimer = New-Object System.Windows.Threading.DispatcherTimer
$closeCtxTimer.Interval = [TimeSpan]::FromMilliseconds($ContextOpenAtMs + $ContextOpenDurationMs)
$closeCtxTimer.Add_Tick({
        $closeCtxTimer.Stop()
        $ctxMenu.IsOpen = $false
        $lblState.Text = 'context-closed'
        Write-MenuState 'context-closed'
    })
$closeCtxTimer.Start()

if ($ExitAtMs -gt 0) {
    $exitTimer = New-Object System.Windows.Threading.DispatcherTimer
    $exitTimer.Interval = [TimeSpan]::FromMilliseconds($ExitAtMs)
    $exitTimer.Add_Tick({ $exitTimer.Stop(); $window.Close() })
    $exitTimer.Start()
}

[System.Windows.Threading.Dispatcher]::Run()
