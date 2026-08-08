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
        Title="AgentDesktop Scratch WPF" Width="480" Height="980"
        WindowStartupLocation="Manual" ShowActivated="False"
        AutomationProperties.AutomationId="wndScratchWpf">
  <ScrollViewer VerticalScrollBarVisibility="Auto" HorizontalScrollBarVisibility="Disabled">
  <Grid Margin="12">
    <Grid.RowDefinitions>
      <RowDefinition Height="Auto"/>
      <RowDefinition Height="Auto"/>
      <RowDefinition Height="Auto"/>
      <RowDefinition Height="Auto"/>
      <RowDefinition Height="Auto"/>
      <RowDefinition Height="Auto"/>
      <RowDefinition Height="Auto"/>
      <RowDefinition Height="Auto"/>
      <RowDefinition Height="Auto"/>
      <RowDefinition Height="Auto"/>
      <RowDefinition Height="Auto"/>
      <RowDefinition Height="Auto"/>
      <RowDefinition Height="Auto"/>
      <RowDefinition Height="Auto"/>
      <RowDefinition Height="160"/>
      <RowDefinition Height="150"/>
      <RowDefinition Height="Auto"/>
    </Grid.RowDefinitions>
    <StackPanel Grid.Row="0" Orientation="Horizontal" Margin="0,4">
      <CheckBox x:Name="chkToggle" Margin="0,0,16,0"
                AutomationProperties.AutomationId="chkToggle" Content="Enable feature"/>
      <CheckBox x:Name="chkTriState" IsThreeState="True" IsChecked="{x:Null}"
                AutomationProperties.AutomationId="chkTriState" Content="Tri-state"/>
    </StackPanel>
    <TextBox x:Name="txtValue" Grid.Row="1" Margin="0,4" Height="24"
             AutomationProperties.AutomationId="txtValue" Text="seed-value"/>
    <StackPanel Grid.Row="2" Orientation="Horizontal" Margin="0,4">
      <TextBox x:Name="txtReadOnly" Width="200" Height="24" IsReadOnly="True" Margin="0,0,8,0"
               AutomationProperties.AutomationId="txtReadOnly" Text="readonly-seed"/>
      <TextBox x:Name="txtDisabled" Width="160" Height="24" IsEnabled="False"
               AutomationProperties.AutomationId="txtDisabled" Text="disabled-seed"/>
    </StackPanel>
    <ComboBox x:Name="cboChoice" Grid.Row="3" Margin="0,4" Height="24"
              AutomationProperties.AutomationId="cboChoice" SelectedIndex="0">
      <ComboBoxItem AutomationProperties.AutomationId="cboItem0" Content="Choice-One"/>
      <ComboBoxItem AutomationProperties.AutomationId="cboItem1" Content="Choice-Two"/>
      <ComboBoxItem AutomationProperties.AutomationId="cboItem2" Content="Choice-Three"/>
      <ComboBoxItem AutomationProperties.AutomationId="cboItem3" Content="Choice-Four"/>
      <ComboBoxItem AutomationProperties.AutomationId="cboItem4" Content="Choice-Five"/>
      <ComboBoxItem AutomationProperties.AutomationId="cboItem5" Content="Choice-Six"/>
      <ComboBoxItem AutomationProperties.AutomationId="cboItem6" Content="Choice-Seven"/>
      <ComboBoxItem AutomationProperties.AutomationId="cboItem7" Content="Choice-Eight"/>
    </ComboBox>
    <StackPanel Grid.Row="4" Orientation="Horizontal" Margin="0,4">
      <Button x:Name="btnAction" AutomationProperties.AutomationId="btnAction"
              Content="Do Action" Width="110" Height="26" Margin="0,0,8,0"/>
      <Button x:Name="btnMutateList" AutomationProperties.AutomationId="btnMutateList"
              Content="Mutate List" Width="110" Height="26" Margin="0,0,8,0"/>
      <Button x:Name="btnDisabled" AutomationProperties.AutomationId="btnDisabled"
              Content="Disabled" Width="90" Height="26" IsEnabled="False" Margin="0,0,8,0"/>
      <Button x:Name="btnZeroSize" AutomationProperties.AutomationId="btnZeroSize"
              Content="zero" Width="0" Height="0"/>
    </StackPanel>
    <ListBox x:Name="lstItems" Grid.Row="5" Margin="0,4" Height="140"
             VirtualizingStackPanel.IsVirtualizing="True"
             VirtualizingStackPanel.VirtualizationMode="Recycling"
             ScrollViewer.CanContentScroll="True"
             AutomationProperties.AutomationId="lstItems"/>
    <Canvas x:Name="pnlOverlay" Grid.Row="6" Margin="0,4" Height="56"
            AutomationProperties.AutomationId="pnlOverlay">
      <Button x:Name="btnCovered" Canvas.Left="8" Canvas.Top="8" Width="120" Height="36"
              AutomationProperties.AutomationId="btnCovered" Content="Covered"/>
      <Button x:Name="btnOverlay" Canvas.Left="40" Canvas.Top="16" Width="120" Height="36"
              AutomationProperties.AutomationId="btnOverlay" Content="Overlay"/>
    </Canvas>
    <TextBox x:Name="txtStatusMirror" Grid.Row="7" Margin="0,4" Height="24" IsReadOnly="True"
             AutomationProperties.AutomationId="txtStatusMirror" Text="status:ready"/>
    <TextBlock x:Name="lblStatus" Grid.Row="8" Margin="0,4"
               AutomationProperties.AutomationId="lblStatus" Text="status:ready"/>
    <StackPanel Grid.Row="9" Orientation="Horizontal" Margin="0,4">
      <TextBlock x:Name="lblFieldName" AutomationProperties.AutomationId="lblFieldName"
                 Text="Field label" VerticalAlignment="Center" Margin="0,0,8,0"/>
      <TextBox x:Name="txtLabelled" AutomationProperties.AutomationId="txtLabelled"
               AutomationProperties.LabeledBy="{Binding ElementName=lblFieldName}"
               Width="140" Height="24"/>
    </StackPanel>
    <StackPanel Grid.Row="10" Orientation="Horizontal" Margin="0,4">
      <PasswordBox x:Name="pwdSecret" AutomationProperties.AutomationId="pwdSecret"
                   Width="120" Height="24" Margin="0,0,8,0"/>
      <TextBox x:Name="txtWritableBesidePwd" AutomationProperties.AutomationId="txtWritableBesidePwd"
               Width="140" Height="24" Text="writable-beside" Margin="0,0,8,0"/>
      <TextBox x:Name="txtLabelledBySecret" AutomationProperties.AutomationId="txtLabelledBySecret"
               AutomationProperties.LabeledBy="{Binding ElementName=pwdSecret}"
               Width="120" Height="24"/>
    </StackPanel>
    <StackPanel Grid.Row="11" Orientation="Horizontal" Margin="0,4">
      <Slider x:Name="tbSlider" AutomationProperties.AutomationId="tbSlider"
              Width="220" Minimum="0" Maximum="100" Value="25" TickFrequency="10"
              IsSnapToTickEnabled="True" Margin="0,0,12,0"/>
      <Slider x:Name="tbSliderDisabled" AutomationProperties.AutomationId="tbSliderDisabled"
              Width="160" Minimum="0" Maximum="100" Value="40" IsEnabled="False"/>
    </StackPanel>
    <TreeView x:Name="trvNodes" Grid.Row="12" Margin="0,4" Height="90"
              AutomationProperties.AutomationId="trvNodes">
      <TreeViewItem x:Name="trvParent" Header="Node-Parent"
                    AutomationProperties.AutomationId="trvParent" IsExpanded="True">
        <TreeViewItem x:Name="trvLeaf" Header="Node-Leaf"
                      AutomationProperties.AutomationId="trvLeaf"/>
      </TreeViewItem>
    </TreeView>
    <ScrollViewer x:Name="svOuter" Grid.Row="13" Margin="0,4" Height="110"
                  VerticalScrollBarVisibility="Auto" HorizontalScrollBarVisibility="Disabled"
                  AutomationProperties.AutomationId="svOuter">
      <StackPanel>
        <TextBlock Height="90" Text="outer-pad-top"/>
        <ScrollViewer x:Name="svInner" Height="70"
                      VerticalScrollBarVisibility="Auto" HorizontalScrollBarVisibility="Disabled"
                      AutomationProperties.AutomationId="svInner">
          <StackPanel>
            <TextBlock Height="90" Text="inner-pad-top"/>
            <Button x:Name="btnNestedDeep" AutomationProperties.AutomationId="btnNestedDeep"
                    Content="Nested Deep" Width="140" Height="28" HorizontalAlignment="Left"/>
            <TextBlock Height="40" Text="inner-pad-bottom"/>
          </StackPanel>
        </ScrollViewer>
        <TextBlock Height="40" Text="outer-pad-bottom"/>
      </StackPanel>
    </ScrollViewer>
    <TabControl x:Name="tabMain" Grid.Row="14" Margin="0,4"
                AutomationProperties.AutomationId="tabMain">
      <TabItem x:Name="tabAlpha" Header="Tab-Alpha" AutomationProperties.AutomationId="tabAlpha"/>
      <TabItem x:Name="tabBravo" Header="Tab-Bravo" AutomationProperties.AutomationId="tabBravo"/>
      <TabItem x:Name="tabCharlie" Header="Tab-Charlie" AutomationProperties.AutomationId="tabCharlie"/>
    </TabControl>
    <StackPanel Grid.Row="15" Orientation="Horizontal" Margin="0,4">
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
    <TextBlock Grid.Row="16" Margin="0,4" Text="a19-fixture-tail"/>
  </Grid>
  </ScrollViewer>
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

$script:BaselineItems = @(0..39 | ForEach-Object { 'Item-{0:D2}' -f $_ })
$script:MutatedItems = @(0..39 | ForEach-Object { 'Item-{0:D2}' -f (($_ + 3) % 40) })
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
$pwdSecret = $window.FindName('pwdSecret')
$pwdSecret.Password = $SecretMarker
$plantLen = $pwdSecret.Password.Length
if ($plantLen -ne $SecretMarker.Length) {
    throw 'PROBE-FIXTURE: WPF PasswordBox plant length mismatch before measurement'
}

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
Write-Output ('SCRATCHWPF_SECRET_PLANTED=1')
Write-Output ('SCRATCHWPF_SECRET_LEN=' + $plantLen)
Write-Output 'SCRATCHWPF_READY=1'

[System.Windows.Threading.Dispatcher]::Run()
