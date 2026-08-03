<#
.SYNOPSIS
    WPF scratch window with an explicit AutomationProperties.AutomationId on every
    interactive control.

.DESCRIPTION
    Standalone by design: does NOT dot-source common.ps1. Built from inline XAML via
    [Windows.Markup.XamlReader]::Load so no compiler is involved. Exists so U4 can
    compare AutomationId coverage and identity stability across the WPF stack versus
    WinForms versus Win32.

    The window is shown with ShowActivated=False, so launching it does not steal
    foreground. The process blocks in a dispatcher loop; terminate it by pid
    (Stop-Process -Id <pid>) or close the window. -TimeoutSeconds adds a self-close
    watchdog for unattended runs.

    Emits three machine-readable stdout lines before blocking:
        SCRATCHWPF_PID=<pid>
        SCRATCHWPF_TITLE=<window title>
        SCRATCHWPF_READY=1
#>
[CmdletBinding()]
param(
    [string]$Tag = 'wpf',
    [switch]$MutateList,
    [int]$Left = 500,
    [int]$Top = 100,
    [int]$TimeoutSeconds = 0,
    [string]$SecretMarker = 'zzvocabsecretzz'
)

$ErrorActionPreference = 'Stop'

Add-Type -AssemblyName PresentationFramework
Add-Type -AssemblyName PresentationCore
Add-Type -AssemblyName WindowsBase

$xaml = @'
<Window xmlns="http://schemas.microsoft.com/winfx/2006/xaml/presentation"
        xmlns:x="http://schemas.microsoft.com/winfx/2006/xaml"
        Title="AgentDesktop Scratch WPF" Width="460" Height="760"
        WindowStartupLocation="Manual" ShowActivated="False"
        AutomationProperties.AutomationId="wndScratchWpf">
  <Grid Margin="12">
    <Grid.RowDefinitions>
      <RowDefinition Height="Auto"/>
      <RowDefinition Height="Auto"/>
      <RowDefinition Height="Auto"/>
      <RowDefinition Height="Auto"/>
      <RowDefinition Height="*"/>
      <RowDefinition Height="Auto"/>
      <RowDefinition Height="Auto"/>
      <RowDefinition Height="Auto"/>
      <RowDefinition Height="Auto"/>
      <RowDefinition Height="160"/>
      <RowDefinition Height="150"/>
    </Grid.RowDefinitions>
    <CheckBox x:Name="chkToggle" Grid.Row="0" Margin="0,4"
              AutomationProperties.AutomationId="chkToggle" Content="Enable feature"/>
    <TextBox x:Name="txtValue" Grid.Row="1" Margin="0,4" Height="24"
             AutomationProperties.AutomationId="txtValue" Text="seed-value"/>
    <ComboBox x:Name="cboChoice" Grid.Row="2" Margin="0,4" Height="24"
              AutomationProperties.AutomationId="cboChoice" SelectedIndex="0">
      <ComboBoxItem AutomationProperties.AutomationId="cboItem0" Content="Choice-One"/>
      <ComboBoxItem AutomationProperties.AutomationId="cboItem1" Content="Choice-Two"/>
      <ComboBoxItem AutomationProperties.AutomationId="cboItem2" Content="Choice-Three"/>
      <ComboBoxItem AutomationProperties.AutomationId="cboItem3" Content="Choice-Four"/>
    </ComboBox>
    <StackPanel Grid.Row="3" Orientation="Horizontal" Margin="0,4">
      <Button x:Name="btnAction" AutomationProperties.AutomationId="btnAction"
              Content="Do Action" Width="110" Height="26" Margin="0,0,8,0"/>
      <Button x:Name="btnMutateList" AutomationProperties.AutomationId="btnMutateList"
              Content="Mutate List" Width="110" Height="26"/>
    </StackPanel>
    <ListBox x:Name="lstItems" Grid.Row="4" Margin="0,4"
             AutomationProperties.AutomationId="lstItems"/>
    <TextBox x:Name="txtStatusMirror" Grid.Row="5" Margin="0,4" Height="24" IsReadOnly="True"
             AutomationProperties.AutomationId="txtStatusMirror" Text="status:ready"/>
    <TextBlock x:Name="lblStatus" Grid.Row="6" Margin="0,4"
               AutomationProperties.AutomationId="lblStatus" Text="status:ready"/>
    <StackPanel Grid.Row="7" Orientation="Horizontal" Margin="0,4">
      <TextBlock x:Name="lblFieldName" AutomationProperties.AutomationId="lblFieldName"
                 Text="Field label" VerticalAlignment="Center" Margin="0,0,8,0"/>
      <TextBox x:Name="txtLabelled" AutomationProperties.AutomationId="txtLabelled"
               AutomationProperties.LabeledBy="{Binding ElementName=lblFieldName}"
               Width="140" Height="24"/>
    </StackPanel>
    <StackPanel Grid.Row="8" Orientation="Horizontal" Margin="0,4">
      <PasswordBox x:Name="pwdSecret" AutomationProperties.AutomationId="pwdSecret"
                   Width="120" Height="24" Margin="0,0,8,0"/>
      <TextBox x:Name="txtLabelledBySecret" AutomationProperties.AutomationId="txtLabelledBySecret"
               AutomationProperties.LabeledBy="{Binding ElementName=pwdSecret}"
               Width="140" Height="24"/>
    </StackPanel>
    <TabControl x:Name="tabMain" Grid.Row="9" Margin="0,4"
                AutomationProperties.AutomationId="tabMain">
      <TabItem x:Name="tabAlpha" Header="Tab-Alpha" AutomationProperties.AutomationId="tabAlpha"/>
      <TabItem x:Name="tabBravo" Header="Tab-Bravo" AutomationProperties.AutomationId="tabBravo"/>
      <TabItem x:Name="tabCharlie" Header="Tab-Charlie" AutomationProperties.AutomationId="tabCharlie"/>
    </TabControl>
    <StackPanel Grid.Row="10" Orientation="Horizontal" Margin="0,4">
      <ToggleButton x:Name="btnToggle" AutomationProperties.AutomationId="btnToggle"
                    Content="Toggle Mode" Width="120" Height="26" Margin="0,0,8,0"/>
      <DataGrid x:Name="dgvRows" Margin="0,0,0,0" Width="380" Height="120"
                AutomationProperties.AutomationId="dgvRows"
                AutoGenerateColumns="False" CanUserAddRows="False" HeadersVisibility="Column">
        <DataGrid.Columns>
          <DataGridTextColumn Header="Column-Label" Binding="{Binding Label}"/>
          <DataGridTextColumn Header="Column-Value" Binding="{Binding Value}"/>
        </DataGrid.Columns>
      </DataGrid>
    </StackPanel>
  </Grid>
</Window>
'@

$reader = New-Object System.Xml.XmlNodeReader ([xml]$xaml)
$window = [Windows.Markup.XamlReader]::Load($reader)

$window.Title = 'AgentDesktop Scratch WPF [' + $Tag + ']'
$window.Left = $Left
$window.Top = $Top

$chkToggle = $window.FindName('chkToggle')
$txtValue = $window.FindName('txtValue')
$cboChoice = $window.FindName('cboChoice')
$btnAction = $window.FindName('btnAction')
$btnMutateList = $window.FindName('btnMutateList')
$lstItems = $window.FindName('lstItems')
$lblStatus = $window.FindName('lblStatus')
$txtStatusMirror = $window.FindName('txtStatusMirror')
$dgvRows = $window.FindName('dgvRows')

$script:BaselineItems = @('Item-Alpha', 'Item-Bravo', 'Item-Charlie', 'Item-Delta', 'Item-Echo')
$script:MutatedItems = @('Item-Alpha', 'Item-Charlie', 'Item-Delta', 'Item-Echo', 'Item-Foxtrot', 'Item-Golf')
$script:ListMutated = [bool]$MutateList
$script:ActionCount = 0

function Set-ScratchStatus {
    param([string]$Value)
    $lblStatus.Text = $Value
    $txtStatusMirror.Text = $Value
}

function Set-ScratchList {
    param([bool]$Mutated)
    $source = if ($Mutated) { $script:MutatedItems } else { $script:BaselineItems }
    $lstItems.Items.Clear()
    foreach ($item in $source) {
        $entry = New-Object System.Windows.Controls.ListBoxItem
        $entry.Content = $item
        [System.Windows.Automation.AutomationProperties]::SetAutomationId($entry, ('lstItem-' + $item))
        $lstItems.Items.Add($entry) | Out-Null
    }
    $script:ListMutated = $Mutated
}

$btnAction.Add_Click({
        $script:ActionCount = $script:ActionCount + 1
        Set-ScratchStatus ('action:' + $script:ActionCount)
    })

$btnMutateList.Add_Click({
        Set-ScratchList (-not $script:ListMutated)
        if ($script:ListMutated) { Set-ScratchStatus 'list:mutated' } else { Set-ScratchStatus 'list:baseline' }
    })

$chkToggle.Add_Click({
        if ($chkToggle.IsChecked) { Set-ScratchStatus 'check:on' } else { Set-ScratchStatus 'check:off' }
    })

$cboChoice.Add_SelectionChanged({
        if ($cboChoice.SelectedItem -ne $null) {
            Set-ScratchStatus ('combo:' + $cboChoice.SelectedItem.Content)
        }
    })

$lstItems.Add_SelectionChanged({
        if ($lstItems.SelectedItem -ne $null) {
            Set-ScratchStatus ('list-sel:' + $lstItems.SelectedItem.Content)
        }
    })

$window.Add_Closed({ $window.Dispatcher.InvokeShutdown() })

Set-ScratchList $script:ListMutated
Set-ScratchStatus 'status:ready'
$window.FindName('pwdSecret').Password = $SecretMarker

$dgvRows.ItemsSource = @(
    [pscustomobject]@{ Label = 'Row-Alpha'; Value = 'Value-Alpha' },
    [pscustomobject]@{ Label = 'Row-Bravo'; Value = 'Value-Bravo' }
)

if ($TimeoutSeconds -gt 0) {
    $timer = New-Object System.Windows.Threading.DispatcherTimer
    $timer.Interval = [TimeSpan]::FromSeconds($TimeoutSeconds)
    $timer.Add_Tick({ $timer.Stop(); $window.Close() })
    $timer.Start()
}

$window.Show()

Write-Output ('SCRATCHWPF_PID=' + $PID)
Write-Output ('SCRATCHWPF_TITLE=' + $window.Title)
Write-Output 'SCRATCHWPF_READY=1'

[System.Windows.Threading.Dispatcher]::Run()
