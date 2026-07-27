$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\common.ps1"

$Probe = '00-environment'
$AclSamplePath = Join-Path $env:TEMP 'agent-desktop-probe-00-acl-sample.tmp'

function Get-RegistryValueOrUnset {
    param([string]$Path, [string]$Name)
    $item = Get-ItemProperty -LiteralPath $Path -ErrorAction SilentlyContinue
    if ($null -eq $item) { return '<key-absent>' }
    if (-not ($item.PSObject.Properties.Name -contains $Name)) { return '<unset>' }
    $value = $item.$Name
    if ($null -eq $value) { return '<unset>' }
    return $value
}

function Get-IntegrityLevelName {
    param([string]$Sid)
    switch ($Sid) {
        'S-1-16-0'     { return 'Untrusted' }
        'S-1-16-4096'  { return 'Low' }
        'S-1-16-8192'  { return 'Medium' }
        'S-1-16-8448'  { return 'Medium Plus' }
        'S-1-16-12288' { return 'High' }
        'S-1-16-16384' { return 'System' }
        default        { return 'Unknown' }
    }
}

function Get-SidAccountOrUnmapped {
    param([string]$Sid)
    try {
        $s = New-Object System.Security.Principal.SecurityIdentifier($Sid)
        return $s.Translate([System.Security.Principal.NTAccount]).Value
    } catch {
        return '<unmapped>'
    }
}

function Get-QuerySessionLines {
    $lines = New-Object System.Collections.ArrayList
    $previous = $ErrorActionPreference
    try {
        $ErrorActionPreference = 'Continue'
        $raw = & (Join-Path $env:WINDIR 'System32\query.exe') session 2>$null
        foreach ($line in $raw) {
            $text = ([string]$line).TrimEnd()
            if ($text) { [void]$lines.Add($text) }
        }
    } catch {
        [void]$lines.Add('<query session unavailable: ' + $_.Exception.Message + '>')
    } finally {
        $ErrorActionPreference = $previous
    }
    return @($lines)
}

function Get-AssemblyRecord {
    param([string]$Name)
    $assembly = $null
    try {
        Add-Type -AssemblyName $Name -ErrorAction Stop
        $assembly = @([AppDomain]::CurrentDomain.GetAssemblies() |
            Where-Object { $_.GetName().Name -eq $Name })[0]
    } catch { $assembly = $null }
    if ($null -eq $assembly) {
        return [ordered]@{ Name = $Name; Loaded = $false; FullName = '<load-failed>'; Location = '<load-failed>' }
    }
    return [ordered]@{
        Name     = $Name
        Loaded   = $true
        FullName = $assembly.FullName
        Location = $assembly.Location
    }
}

function Get-ComServerRecord {
    param([string]$Label, [string]$Clsid)
    $base = 'HKLM:\SOFTWARE\Classes\CLSID\' + $Clsid
    return [ordered]@{
        Label           = $Label
        Clsid           = $Clsid
        Registered      = (Test-Path -LiteralPath $base)
        FriendlyName    = (Get-RegistryValueOrUnset -Path $base -Name '(default)')
        InprocServer32  = (Get-RegistryValueOrUnset -Path ($base + '\InprocServer32') -Name '(default)')
        ThreadingModel  = (Get-RegistryValueOrUnset -Path ($base + '\InprocServer32') -Name 'ThreadingModel')
    }
}

function Get-FileVersionRecord {
    param([string]$Label, [string]$Path)
    if (-not (Test-Path -LiteralPath $Path)) {
        return [ordered]@{ Label = $Label; Path = $Path; Present = $false; FileVersion = '<absent>'; ProductVersion = '<absent>' }
    }
    $info = (Get-Item -LiteralPath $Path).VersionInfo
    return [ordered]@{
        Label          = $Label
        Path           = $Path
        Present        = $true
        FileVersion    = $info.FileVersion
        ProductVersion = $info.ProductVersion
    }
}

try {
    $capturedAt = (Get-Date).ToUniversalTime().ToString('yyyy-MM-ddTHH:mm:ssZ')

    $cv = Get-ItemProperty -LiteralPath 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
    $os = [ordered]@{
        ProductName        = $cv.ProductName
        EditionId          = $cv.EditionID
        InstallationType   = $cv.InstallationType
        CurrentBuild       = $cv.CurrentBuild
        Ubr                = (Get-RegistryValueOrUnset -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion' -Name 'UBR')
        ReleaseId          = (Get-RegistryValueOrUnset -Path 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion' -Name 'ReleaseId')
        BuildLabEx         = $cv.BuildLabEx
        OsVersionString    = [Environment]::OSVersion.VersionString
        Is64BitOs          = [Environment]::Is64BitOperatingSystem
        Is64BitProcess     = [Environment]::Is64BitProcess
        CurrentCulture     = [System.Globalization.CultureInfo]::CurrentCulture.Name
        InstalledUiCulture = [System.Globalization.CultureInfo]::InstalledUICulture.Name
        AnsiCodePage       = (Get-RegistryValueOrUnset -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Nls\CodePage' -Name 'ACP')
        OemCodePage        = (Get-RegistryValueOrUnset -Path 'HKLM:\SYSTEM\CurrentControlSet\Control\Nls\CodePage' -Name 'OEMCP')
        DotNetDefaultEncodingCodePage = [System.Text.Encoding]::Default.CodePage
    }

    $session = [ordered]@{
        SessionNameEnv  = $env:SESSIONNAME
        SessionId       = [System.Diagnostics.Process]::GetCurrentProcess().SessionId
        UserInteractive = [Environment]::UserInteractive
        QuerySession    = (Get-QuerySessionLines)
    }

    $identity = [System.Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object System.Security.Principal.WindowsPrincipal($identity)
    $userSid = $identity.User.Value
    $rid = $userSid.Substring($userSid.LastIndexOf('-') + 1)
    $groups = New-Object System.Collections.ArrayList
    foreach ($group in ($identity.Groups | Sort-Object -Property Value)) {
        $sid = $group.Value
        [void]$groups.Add([ordered]@{
            Sid       = $sid
            WellKnown = (-not $sid.StartsWith('S-1-5-21-'))
            Account   = (Get-SidAccountOrUnmapped -Sid $sid)
        })
    }
    $identityShape = [ordered]@{
        AccountNameShape  = '<machine>\<user>'
        AccountName       = $identity.Name
        UserSidShape      = 'S-1-5-21-<redacted>-' + $rid
        UserSid           = $userSid
        SidAuthority      = 'S-1-5-21 (local machine authority)'
        WellKnownRid      = $rid
        RidMeaning        = 'RID 500 is the built-in Administrator account'
        TokenOwnerSid     = $identity.Owner.Value
        TokenOwnerAccount = (Get-SidAccountOrUnmapped -Sid $identity.Owner.Value)
        IsAuthenticated   = $identity.IsAuthenticated
        IsSystem          = $identity.IsSystem
        InAdministratorsRole = $principal.IsInRole([System.Security.Principal.WindowsBuiltInRole]::Administrator)
        AuthenticationType   = $identity.AuthenticationType
        GroupCount        = $groups.Count
        Groups            = @($groups)
    }

    Initialize-ProbeNative
    $integritySid = [AgentDesktopProbe.Native]::GetIntegritySid($PID)
    $policyPath = 'HKLM:\SOFTWARE\Microsoft\Windows\CurrentVersion\Policies\System'
    $integrity = [ordered]@{
        ProcessId                 = $PID
        MandatoryLabelSid         = $integritySid
        MandatoryLabelName        = (Get-IntegrityLevelName -Sid $integritySid)
        EnableLua                 = (Get-RegistryValueOrUnset -Path $policyPath -Name 'EnableLUA')
        FilterAdministratorToken  = (Get-RegistryValueOrUnset -Path $policyPath -Name 'FilterAdministratorToken')
        ConsentPromptBehaviorAdmin = (Get-RegistryValueOrUnset -Path $policyPath -Name 'ConsentPromptBehaviorAdmin')
        AdminApprovalModeActive   = 'no — built-in Administrator (RID 500) with FilterAdministratorToken unset runs a full High-IL token; Start-Process -Verb RunAs yields High-vs-High and no integrity boundary'
    }

    Set-Content -LiteralPath $AclSamplePath -Value 'agent-desktop probe acl sample' -Encoding ASCII
    $acl = Get-Acl -LiteralPath $AclSamplePath
    $ownerSid = $acl.GetOwner([System.Security.Principal.SecurityIdentifier]).Value
    $ownerClass = 'other'
    if ($ownerSid -eq $userSid) { $ownerClass = 'user (TokenUser)' }
    elseif ($ownerSid -eq 'S-1-5-32-544') { $ownerClass = 'group (BUILTIN\Administrators, TokenOwner)' }
    $aces = New-Object System.Collections.ArrayList
    foreach ($ace in $acl.Access) {
        [void]$aces.Add([ordered]@{
            Identity    = $ace.IdentityReference.Value
            Rights      = $ace.FileSystemRights.ToString()
            Type        = $ace.AccessControlType.ToString()
            IsInherited = $ace.IsInherited
            Inheritance = $ace.InheritanceFlags.ToString()
            Propagation = $ace.PropagationFlags.ToString()
        })
    }
    $inheritedCount = @($acl.Access | Where-Object { $_.IsInherited }).Count
    $defaultAcl = [ordered]@{
        SamplePath           = $AclSamplePath
        OwnerAccount         = $acl.Owner
        OwnerSid             = $ownerSid
        OwnerClass           = $ownerClass
        OwnerMatchesTokenOwner = ($ownerSid -eq $identity.Owner.Value)
        OwnerMatchesTokenUser  = ($ownerSid -eq $userSid)
        AreAccessRulesProtected = $acl.AreAccessRulesProtected
        AceCount             = $aces.Count
        InheritedAceCount    = $inheritedCount
        Aces                 = @($aces)
    }

    Add-Type -AssemblyName System.Windows.Forms | Out-Null
    Add-Type -AssemblyName System.Drawing | Out-Null
    $graphics = [System.Drawing.Graphics]::FromHwnd([IntPtr]::Zero)
    $dpiX = $graphics.DpiX
    $dpiY = $graphics.DpiY
    $graphics.Dispose()
    $screens = New-Object System.Collections.ArrayList
    foreach ($screen in [System.Windows.Forms.Screen]::AllScreens) {
        [void]$screens.Add([ordered]@{
            DeviceName   = $screen.DeviceName
            Primary      = $screen.Primary
            BitsPerPixel = $screen.BitsPerPixel
            X            = $screen.Bounds.X
            Y            = $screen.Bounds.Y
            Width        = $screen.Bounds.Width
            Height       = $screen.Bounds.Height
        })
    }
    $virtual = [System.Windows.Forms.SystemInformation]::VirtualScreen
    $display = [ordered]@{
        ScreenCount        = $screens.Count
        Screens            = @($screens)
        VirtualScreenX     = $virtual.X
        VirtualScreenY     = $virtual.Y
        VirtualScreenWidth = $virtual.Width
        VirtualScreenHeight = $virtual.Height
        ReportedDpiX       = $dpiX
        ReportedDpiY       = $dpiY
        ScaleFactor        = ([double]$dpiX / 96.0)
        LogPixelsRegistry  = (Get-RegistryValueOrUnset -Path 'HKCU:\Control Panel\Desktop' -Name 'LogPixels')
        DpiAwarenessNote   = 'PowerShell 5.1 is DPI-unaware, so these values are the virtualized system DPI; per-monitor DPI behavior is measured in the DPI/multi-monitor probe, not here'
        TopologyNote       = 'Bounds come from the VM console framebuffer, which resizes with the console viewer. A normalized diff confined to Screens/VirtualScreen bounds is viewer resize, not platform drift; ScreenCount, Primary, and ScaleFactor are the load-bearing topology facts.'
    }

    $uiaClient = Get-AssemblyRecord -Name 'UIAutomationClient'
    $uiaTypes = @()
    $uiaNamespaces = @()
    $hasIUIAutomation = $false
    $hasCUIAutomation = $false
    $hasAutomationElement = $false
    if ($uiaClient.Loaded) {
        $assembly = @([AppDomain]::CurrentDomain.GetAssemblies() |
            Where-Object { $_.GetName().Name -eq 'UIAutomationClient' })[0]
        $uiaTypes = @($assembly.GetExportedTypes())
        $uiaNamespaces = @($uiaTypes | Group-Object -Property Namespace | Sort-Object -Property Name |
            ForEach-Object { [ordered]@{ Namespace = $_.Name; TypeCount = $_.Count } })
        $hasIUIAutomation = (@($uiaTypes | Where-Object { $_.Name -eq 'IUIAutomation' }).Count -gt 0)
        $hasCUIAutomation = (@($uiaTypes | Where-Object { $_.Name -like 'CUIAutomation*' }).Count -gt 0)
        $hasAutomationElement = (@($uiaTypes | Where-Object { $_.FullName -eq 'System.Windows.Automation.AutomationElement' }).Count -gt 0)
    }
    $uia3Evidence = [ordered]@{
        Question                    = 'Does the GAC UIAutomationClient assembly expose the UIA3 COM types (IUIAutomation / CUIAutomation)?'
        Answer                      = 'no — it is the managed System.Windows.Automation stack; UIA3 COM requires a csc-compiled interop shim against the registered COM server'
        AssemblyFullName            = $uiaClient.FullName
        AssemblyLocation            = $uiaClient.Location
        ExportedTypeCount           = $uiaTypes.Count
        ExportedNamespaces          = @($uiaNamespaces)
        ExposesIUIAutomation        = $hasIUIAutomation
        ExposesCUIAutomation        = $hasCUIAutomation
        ExposesAutomationElement    = $hasAutomationElement
        ComServers                  = @(
            (Get-ComServerRecord -Label 'CUIAutomation' -Clsid '{ff48dba4-60ef-4201-aa87-54103eef594e}'),
            (Get-ComServerRecord -Label 'CUIAutomation8' -Clsid '{e22ad333-b25f-460c-83d0-0581107395c9}')
        )
    }

    $frameworkDir = Join-Path $env:WINDIR 'Microsoft.NET\Framework64'
    $tools = [ordered]@{
        PowerShellVersion     = $PSVersionTable.PSVersion.ToString()
        PowerShellEdition     = 'Desktop (Windows PowerShell 5.1; pwsh absent)'
        ClrVersion            = $PSVersionTable.CLRVersion.ToString()
        Csc                   = (Get-FileVersionRecord -Label 'csc.exe' -Path (Join-Path $frameworkDir 'v4.0.30319\csc.exe'))
        CscLanguageCeiling    = 'C# 5 (pre-Roslyn .NET Framework compiler)'
        UiAutomationCore      = (Get-FileVersionRecord -Label 'UIAutomationCore.dll' -Path (Join-Path $env:WINDIR 'System32\UIAutomationCore.dll'))
        DotNetFullVersion     = (Get-RegistryValueOrUnset -Path 'HKLM:\SOFTWARE\Microsoft\NET Framework Setup\NDP\v4\Full' -Name 'Version')
        DotNetFullRelease     = (Get-RegistryValueOrUnset -Path 'HKLM:\SOFTWARE\Microsoft\NET Framework Setup\NDP\v4\Full' -Name 'Release')
        FrameworkDirectories  = @(Get-ChildItem -LiteralPath $frameworkDir -Directory | ForEach-Object { $_.Name } | Sort-Object)
        GacAssemblies         = @(
            $uiaClient,
            (Get-AssemblyRecord -Name 'UIAutomationTypes'),
            (Get-AssemblyRecord -Name 'UIAutomationClientsideProviders'),
            (Get-AssemblyRecord -Name 'WindowsBase'),
            (Get-AssemblyRecord -Name 'System.Windows.Forms'),
            (Get-AssemblyRecord -Name 'System.Drawing')
        )
        Uia3ComExposure       = $uia3Evidence
    }

    $environment = [ordered]@{
        Probe        = $Probe
        CapturedAtUtc = $capturedAt
        Purpose      = 'machine facts every sub-phase 2.0 ledger row inherits'
        Os           = $os
        Session      = $session
        Identity     = $identityShape
        Integrity    = $integrity
        DefaultAcl   = $defaultAcl
        Display      = $display
        Tools        = $tools
    }
    $environmentPath = Write-ProbeJson -Probe $Probe -Name 'environment.json' -InputObject $environment

    $deferred = [ordered]@{
        Probe         = $Probe
        CapturedAtUtc = $capturedAt
        Area          = 'session and DPI/multi-monitor behavior (plan area 10): RDP session-transition behavior'
        Facet         = 'How UIA tree reads, window handles, foreground/focus, and input synthesis behave across an RDP connect/disconnect/reconnect session transition, and how a disconnected session differs from an attached console session.'
        Verdict       = 'DEFERRED'
        ObservedHere  = [ordered]@{
            SessionNameEnv  = $env:SESSIONNAME
            SessionId       = [System.Diagnostics.Process]::GetCurrentProcess().SessionId
            UserInteractive = [Environment]::UserInteractive
        }
        Reason        = 'This VM runs on the physical console: SESSIONNAME=Console, UserInteractive=True, one interactive session. An RDP session transition cannot be produced here without disconnecting the very session the probe corpus runs in, so no honest observation of remote-session behavior is available from this environment (KTD3).'
        ClosurePoint  = 'Sub-phase 2.1 runner registration. Registering the Windows CI runner in 2.1 creates the second, non-console session environment; the RDP session-transition facet is measured there and this row closes against that runner. Until then no Windows adapter behavior may assume console-session semantics.'
        PhasesAction  = 'phases.md must not claim RDP/remote-session support for the Windows adapter until 2.1 runner evidence exists.'
    }
    $deferredPath = Write-ProbeJson -Probe $Probe -Name 'deferred-rdp.json' -InputObject $deferred

    $summary = [ordered]@{
        Build             = $os.CurrentBuild
        SessionName       = $session.SessionNameEnv
        UserInteractive   = $session.UserInteractive
        MandatoryLabelSid = $integrity.MandatoryLabelSid
        OwnerClass        = $defaultAcl.OwnerClass
        InheritedAceCount = $defaultAcl.InheritedAceCount
        ScreenCount       = $display.ScreenCount
        Captures          = 2
    }
    Write-ProbeLog -Message ('wrote ' + (Split-Path -Leaf $environmentPath) + ' and ' + (Split-Path -Leaf $deferredPath))
    Write-ProbeResult -Probe $Probe -Status 'ok' -Message 'environment, session, identity shape, integrity, default ACL, display, and tool inventory captured' -Data $summary
    exit 0
} catch {
    Write-ProbeResult -Probe $Probe -Status 'fail' -Message ('unhandled error: ' + $_.Exception.Message)
    exit 1
} finally {
    if (Test-Path -LiteralPath $AclSamplePath) {
        Remove-Item -LiteralPath $AclSamplePath -Force -ErrorAction SilentlyContinue
    }
}
