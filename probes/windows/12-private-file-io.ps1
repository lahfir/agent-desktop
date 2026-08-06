[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
. "$PSScriptRoot\common.ps1"

$Probe = '12-private-file-io'
$script:GENERIC_READ = [uint32]2147483648
$script:GENERIC_READ_WRITE = [uint32]3221225472
$script:MOVEFILE_REPLACE_EXISTING = [uint32]1
$script:TrustedSids = @('S-1-5-18', 'S-1-5-19', 'S-1-5-20', 'S-1-5-32-544', 'S-1-3-0')
$script:WriteishMask = [int]([System.Security.AccessControl.FileSystemRights]'CreateFiles,CreateDirectories,WriteData,AppendData,Delete,DeleteSubdirectoriesAndFiles,WriteAttributes,WriteExtendedAttributes,ChangePermissions,TakeOwnership')
$script:AccountCache = @{}
$script:HeldHandle = [long](-1)
$script:PrivateFile = $null

function New-CleanDirectory {
    param([Parameter(Mandatory = $true)][string]$Path)
    if ([IO.Directory]::Exists($Path)) { [IO.Directory]::Delete($Path, $true) }
    [void][IO.Directory]::CreateDirectory($Path)
}

function Remove-DirectoryTree {
    param([Parameter(Mandatory = $true)][string]$Path)
    if ([IO.Directory]::Exists($Path)) {
        try { [IO.Directory]::Delete($Path, $true) } catch { Write-ProbeLog -Message ('cleanup failed for ' + $Path + ': ' + $_.Exception.Message) -Level 'warn' }
    }
}

function Resolve-AccountName {
    param([Parameter(Mandatory = $true)][string]$Sid)
    if ($script:AccountCache.ContainsKey($Sid)) { return $script:AccountCache[$Sid] }
    $name = '<untranslatable>'
    try { $name = (New-Object System.Security.Principal.SecurityIdentifier($Sid)).Translate([System.Security.Principal.NTAccount]).Value } catch { $name = '<untranslatable>' }
    $script:AccountCache[$Sid] = $name
    return $name
}

function Get-CscToolchain {
    $csc = Join-Path $env:WINDIR 'Microsoft.NET\Framework64\v4.0.30319\csc.exe'
    if (-not [IO.File]::Exists($csc)) { throw ('PROBE-HARNESS: in-box compiler not found at ' + $csc) }
    return [pscustomobject]@{ Path = $csc; FileVersion = (Get-Item -LiteralPath $csc).VersionInfo.FileVersion }
}

function Import-PrivateFileProbe {
    param([Parameter(Mandatory = $true)][string]$CscPath, [Parameter(Mandatory = $true)][string]$WorkDir)
    $source = Join-Path $PSScriptRoot 'scratch\PrivateFileProbe.cs'
    if (-not [IO.File]::Exists($source)) { throw ('PROBE-HARNESS: helper source not found at ' + $source) }
    $dll = Join-Path $WorkDir 'PrivateFileProbe.dll'
    $compilerOutput = & $CscPath /nologo /target:library /r:System.dll ('/out:' + $dll) $source
    if ($LASTEXITCODE -ne 0) { throw ('PROBE-HARNESS: csc.exe exit ' + $LASTEXITCODE + ': ' + ($compilerOutput -join ' ')) }
    $assembly = [System.Reflection.Assembly]::Load([IO.File]::ReadAllBytes($dll))
    [IO.File]::Delete($dll)
    $type = $assembly.GetType('AgentDesktopProbe.PrivateFile')
    if ($null -eq $type) { throw 'PROBE-HARNESS: AgentDesktopProbe.PrivateFile not found in compiled assembly' }
    return $type
}

function Invoke-ShareModeCase {
    param(
        [Parameter(Mandatory = $true)][string]$Operation,
        [Parameter(Mandatory = $true)][string]$HandleOn,
        [Parameter(Mandatory = $true)][hashtable]$Access,
        [Parameter(Mandatory = $true)][hashtable]$Share,
        [Parameter(Mandatory = $true)][string]$WorkDir
    )
    New-CleanDirectory -Path $WorkDir
    $target = Join-Path $WorkDir 'target.txt'
    $source = Join-Path $WorkDir 'source.txt'
    [IO.File]::WriteAllText($target, 'TARGET')
    [IO.File]::WriteAllText($source, 'SOURCE')

    $handle = [long](-1)
    $openStatus = 'not-opened'
    $openError = 0
    $openErrorName = 'n/a'
    if ($HandleOn -ne 'none') {
        $handlePath = $target
        if ($HandleOn -eq 'source') { $handlePath = $source }
        $opened = $script:PrivateFile::OpenHandle($handlePath, [uint32]$Access.Mask, [uint32]$Share.Mask, [uint32]0)
        $openError = $opened.LastError
        $openErrorName = $opened.ErrorName
        if ($opened.Success) { $handle = $opened.Handle; $script:HeldHandle = $handle; $openStatus = 'opened' } else { $openStatus = 'open-failed' }
    }

    if ($Operation -eq 'MoveFileEx') {
        $operationResult = $script:PrivateFile::MoveOver($source, $target, $script:MOVEFILE_REPLACE_EXISTING)
    } else {
        $operationResult = $script:PrivateFile::ReplaceOver($target, $source, [uint32]0)
    }

    $handleReadStatus = 'n/a'
    $handleReadContent = ''
    if ($handle -ne -1) {
        $readBack = $script:PrivateFile::ReadFromHandle($handle, 16)
        $handleReadStatus = $readBack.ErrorName
        $handleReadContent = $readBack.Detail
        $null = $script:PrivateFile::CloseFileHandle($handle)
        $script:HeldHandle = [long](-1)
    }
    $targetContent = '<absent>'
    if ([IO.File]::Exists($target)) { $targetContent = [IO.File]::ReadAllText($target) }

    $verdict = 'succeeded'
    if (-not $operationResult.Success) {
        if ($operationResult.LastError -eq 32) { $verdict = 'sharing-violation-32' } else { $verdict = 'failed-' + $operationResult.LastError }
    }

    return [pscustomobject]@{
        Operation           = $Operation
        HandleOn            = $HandleOn
        Access              = $Access.Name
        AccessMask          = ('0x' + ([uint32]$Access.Mask).ToString('X8'))
        Share               = $Share.Name
        ShareMask           = ('0x' + ([uint32]$Share.Mask).ToString('X8'))
        HandleShareDelete   = [bool]($Share.Mask -band 4)
        OpenStatus          = $openStatus
        OpenError           = $openError
        OpenErrorName       = $openErrorName
        OperationSucceeded  = $operationResult.Success
        OperationError      = $operationResult.LastError
        OperationErrorName  = $operationResult.ErrorName
        OperationMessage    = $operationResult.Message
        TargetContentAfter  = $targetContent
        SourceExistsAfter   = [IO.File]::Exists($source)
        HeldHandleReadAfter = $handleReadStatus
        HeldHandleContent   = $handleReadContent
        Verdict             = $verdict
    }
}

function Get-OwnerSource {
    param([Parameter(Mandatory = $true)][AllowNull()]$Record)
    if ($null -eq $Record -or -not $Record.FileOwnerSid) { return 'not-measured' }
    if ($Record.OwnerMatchesTokenOwner -and -not $Record.OwnerMatchesTokenUser) { return 'TokenOwner' }
    if ($Record.OwnerMatchesTokenUser -and -not $Record.OwnerMatchesTokenOwner) { return 'TokenUser' }
    if ($Record.OwnerMatchesTokenUser -and $Record.OwnerMatchesTokenOwner) { return 'TokenUser==TokenOwner' }
    return 'neither'
}

function Get-AclRecord {
    param([Parameter(Mandatory = $true)][string]$Path, [bool]$IsDirectory = $false)
    $acl = Get-Acl -LiteralPath $Path
    $ownerSid = $acl.GetOwner([System.Security.Principal.SecurityIdentifier]).Value
    $rules = @($acl.GetAccessRules($true, $true, [System.Security.Principal.SecurityIdentifier]))
    $aces = New-Object System.Collections.ArrayList
    $untrustedWriters = New-Object System.Collections.ArrayList
    foreach ($rule in $rules) {
        $sid = $rule.IdentityReference.Value
        [void]$aces.Add([pscustomobject]@{
                IdentitySid      = $sid
                IdentityAccount  = (Resolve-AccountName -Sid $sid)
                Rights           = $rule.FileSystemRights.ToString()
                RightsMask       = ('0x' + ([int]$rule.FileSystemRights).ToString('X8'))
                AccessType       = $rule.AccessControlType.ToString()
                InheritedAce     = $rule.IsInherited
                InheritanceFlags = $rule.InheritanceFlags.ToString()
                PropagationFlags = $rule.PropagationFlags.ToString()
            })
        $isWriteish = ((([int]$rule.FileSystemRights) -band $script:WriteishMask) -ne 0)
        if ($rule.AccessControlType.ToString() -eq 'Allow' -and $isWriteish -and ($script:TrustedSids -notcontains $sid) -and ($sid -ne $script:TokenUserSid)) {
            [void]$untrustedWriters.Add((Resolve-AccountName -Sid $sid))
        }
    }
    $inherited = @($aces | Where-Object { $_.InheritedAce })
    return [pscustomobject]@{
        Path                     = $Path
        IsDirectory              = $IsDirectory
        OwnerSid                 = $ownerSid
        OwnerAccount             = (Resolve-AccountName -Sid $ownerSid)
        InheritanceProtected     = $acl.AreAccessRulesProtected
        AceCount                 = $aces.Count
        InheritedAceCount        = $inherited.Count
        ExplicitAceCount         = ($aces.Count - $inherited.Count)
        UntrustedWriteGrants     = @($untrustedWriters | Select-Object -Unique)
        Sddl                     = $acl.GetSecurityDescriptorSddlForm('Owner,Group,Access')
        Aces                     = @($aces)
    }
}

function Get-AncestorChain {
    param([Parameter(Mandatory = $true)][string]$LeafPath)
    $chain = New-Object System.Collections.ArrayList
    $current = Split-Path -Parent $LeafPath
    while ($current) {
        [void]$chain.Add((Get-AclRecord -Path $current -IsDirectory $true))
        $next = Split-Path -Parent $current
        if ($next -eq $current) { break }
        $current = $next
    }
    return @($chain)
}

function Get-OwnershipRecord {
    param([Parameter(Mandatory = $true)][string]$Stage, [Parameter(Mandatory = $true)][string]$FilePath, [Parameter(Mandatory = $true)][int]$OwningProcessId)
    $identity = [System.Security.Principal.WindowsIdentity]::GetCurrent()
    $acl = Get-Acl -LiteralPath $FilePath
    $ownerSid = $acl.GetOwner([System.Security.Principal.SecurityIdentifier]).Value
    return [pscustomobject]@{
        Stage                  = $Stage
        ProcessId              = $OwningProcessId
        IntegritySid           = [AgentDesktopProbe.Native]::GetIntegritySid($OwningProcessId)
        TokenUserSid           = $identity.User.Value
        TokenOwnerSid          = $identity.Owner.Value
        TokenAccount           = $identity.Name
        FileOwnerSid           = $ownerSid
        FileOwnerAccount       = (Resolve-AccountName -Sid $ownerSid)
        OwnerMatchesTokenUser  = ($ownerSid -eq $identity.User.Value)
        OwnerMatchesTokenOwner = ($ownerSid -eq $identity.Owner.Value)
        Icacls                 = @(& icacls $FilePath | ForEach-Object { "$_" })
        Sddl                   = $acl.GetSecurityDescriptorSddlForm('Owner,Group,Access')
    }
}

$status = 'ok'
$resultMessage = ''
$summary = $null
$scratchRoot = Join-Path $env:TEMP ('agent-desktop-probe-12-' + $PID)
$mediumOutDir = Join-Path $env:TEMP ('agent-desktop-probe-12-medium-' + $PID)
$mediumProcessId = 0

try {
    Initialize-ProbeNative
    $staleError = Join-Path (Get-CaptureDir -Probe $Probe) 'probe-error.txt'
    foreach ($stale in @($staleError, ($staleError + '.normalized'))) {
        if ([IO.File]::Exists($stale)) { [IO.File]::Delete($stale) }
    }
    New-CleanDirectory -Path $scratchRoot
    $script:TokenUserSid = [System.Security.Principal.WindowsIdentity]::GetCurrent().User.Value
    $toolchain = Get-CscToolchain
    $script:PrivateFile = Import-PrivateFileProbe -CscPath $toolchain.Path -WorkDir $scratchRoot
    Write-ProbeLog -Message ('compiled PrivateFileProbe.cs with csc ' + $toolchain.FileVersion)

    $accessModes = @(
        @{ Name = 'GENERIC_READ'; Mask = $script:GENERIC_READ },
        @{ Name = 'GENERIC_READ|GENERIC_WRITE'; Mask = $script:GENERIC_READ_WRITE }
    )
    $shareModes = @(
        @{ Name = 'NONE'; Mask = [uint32]0 },
        @{ Name = 'FILE_SHARE_READ'; Mask = [uint32]1 },
        @{ Name = 'FILE_SHARE_READ|FILE_SHARE_WRITE'; Mask = [uint32]3 },
        @{ Name = 'FILE_SHARE_DELETE'; Mask = [uint32]4 },
        @{ Name = 'FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE'; Mask = [uint32]7 }
    )
    $shareCaseDir = Join-Path $scratchRoot 'sharemode'
    $shareRows = New-Object System.Collections.ArrayList
    foreach ($operation in @('MoveFileEx', 'ReplaceFile')) {
        [void]$shareRows.Add((Invoke-ShareModeCase -Operation $operation -HandleOn 'none' -Access @{ Name = 'n/a'; Mask = [uint32]0 } -Share @{ Name = 'n/a'; Mask = [uint32]0 } -WorkDir $shareCaseDir))
        foreach ($handleOn in @('target', 'source')) {
            foreach ($access in $accessModes) {
                foreach ($share in $shareModes) {
                    [void]$shareRows.Add((Invoke-ShareModeCase -Operation $operation -HandleOn $handleOn -Access $access -Share $share -WorkDir $shareCaseDir))
                }
            }
        }
    }
    $handleCases = @($shareRows | Where-Object { $_.HandleOn -ne 'none' })
    $handleOpenFailures = @($handleCases | Where-Object { $_.OpenStatus -ne 'opened' })
    if ($handleOpenFailures.Count -gt 0) {
        $first = $handleOpenFailures[0]
        throw ('PROBE-HARNESS: refusing to record a share-mode verdict without a concurrently open handle - ' +
            $handleOpenFailures.Count + ' of ' + $handleCases.Count + ' handle cases failed to open, first ' +
            $first.Operation + '/' + $first.HandleOn + '/' + $first.Access + '/' + $first.Share + ' ' + $first.OpenErrorName)
    }
    $unexpectedSuccess = @($shareRows | Where-Object { $_.OpenStatus -eq 'opened' -and $_.Verdict -eq 'succeeded' -and (-not $_.HandleShareDelete) })
    [void](Write-ProbeJson -Probe $Probe -Name 'share-mode-rename.json' -InputObject ([pscustomobject]@{
                Experiment          = 'share-mode rename over a concurrently open validation handle'
                Question            = 'does atomic rename over a concurrently open handle require FILE_SHARE_DELETE'
                Toolchain           = ('csc.exe ' + $toolchain.FileVersion)
                MoveFileExFlags     = 'MOVEFILE_REPLACE_EXISTING (0x1)'
                ReplaceFileFlags    = '0x0'
                CaseCount           = $shareRows.Count
                CasesHoldingAnOpenHandle = $handleCases.Count
                SharingViolations   = @($shareRows | Where-Object { $_.Verdict -eq 'sharing-violation-32' }).Count
                Successes           = @($shareRows | Where-Object { $_.Verdict -eq 'succeeded' }).Count
                SuccessWithoutShareDelete = @($unexpectedSuccess | ForEach-Object { $_.Operation + '/' + $_.HandleOn + '/' + $_.Access + '/' + $_.Share })
                Cases               = @($shareRows)
            }))

    $highDir = Join-Path $scratchRoot 'ownership-high'
    New-CleanDirectory -Path $highDir
    $highFile = Join-Path $highDir 'created-by-high.txt'
    [IO.File]::WriteAllText($highFile, 'HIGH')
    $highRecord = Get-OwnershipRecord -Stage 'high-integrity-parent' -FilePath $highFile -OwningProcessId $PID

    $childScript = Join-Path $scratchRoot 'medium-owner-child.ps1'
    $childBody = @'
param([string]$OutDir)
$ErrorActionPreference = 'Stop'
$fallback = Join-Path $env:TEMP 'agent-desktop-probe-12-medium-fallback.json'
$data = $null
try {
    [void][IO.Directory]::CreateDirectory($OutDir)
    $file = Join-Path $OutDir 'created-by-medium.txt'
    [IO.File]::WriteAllText($file, 'MEDIUM')
    $identity = [System.Security.Principal.WindowsIdentity]::GetCurrent()
    $acl = Get-Acl -LiteralPath $file
    $ownerSid = $acl.GetOwner([System.Security.Principal.SecurityIdentifier]).Value
    $ownerAccount = '<untranslatable>'
    try { $ownerAccount = $acl.GetOwner([System.Security.Principal.NTAccount]).Value } catch { $ownerAccount = '<untranslatable>' }
    $data = [pscustomobject]@{
        Stage                  = 'medium-integrity-child'
        ProcessId              = $PID
        TokenUserSid           = $identity.User.Value
        TokenOwnerSid          = $identity.Owner.Value
        TokenAccount           = $identity.Name
        FileOwnerSid           = $ownerSid
        FileOwnerAccount       = $ownerAccount
        OwnerMatchesTokenUser  = ($ownerSid -eq $identity.User.Value)
        OwnerMatchesTokenOwner = ($ownerSid -eq $identity.Owner.Value)
        Icacls                 = @(& icacls $file | ForEach-Object { "$_" })
        Sddl                   = $acl.GetSecurityDescriptorSddlForm('Owner,Group,Access')
        ChildError             = ''
    }
} catch {
    $data = [pscustomobject]@{ Stage = 'medium-integrity-child'; ProcessId = $PID; ChildError = $_.Exception.Message }
}
$json = ConvertTo-Json -InputObject $data -Depth 10
$out = Join-Path $OutDir 'medium-result.json'
try { [IO.File]::WriteAllText($out, $json) } catch { [IO.File]::WriteAllText($fallback, $json) }
Start-Sleep -Seconds 4
'@
    [IO.File]::WriteAllText($childScript, $childBody, (New-Object System.Text.UTF8Encoding $true))

    $powershellExe = Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe'
    $mediumArgs = @('-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-File', ('"' + $childScript + '"'), ('"' + $mediumOutDir + '"'))
    $medium = Start-MediumIntegrityProcess -FilePath $powershellExe -ArgumentList $mediumArgs
    $mediumProcessId = $medium.ProcessId
    Write-ProbeLog -Message ('medium-integrity child pid ' + $mediumProcessId + ' label ' + $medium.IntegritySid)

    $mediumResultPath = Join-Path $mediumOutDir 'medium-result.json'
    $fallbackPath = Join-Path $env:TEMP 'agent-desktop-probe-12-medium-fallback.json'
    $deadline = (Get-Date).AddSeconds(60)
    while ((Get-Date) -lt $deadline) {
        if ([IO.File]::Exists($mediumResultPath) -or [IO.File]::Exists($fallbackPath)) { break }
        Start-Sleep -Milliseconds 250
    }
    $mediumRecord = $null
    $readPath = $mediumResultPath
    if (-not [IO.File]::Exists($readPath)) { $readPath = $fallbackPath }
    if ([IO.File]::Exists($readPath)) {
        $mediumRecord = ([IO.File]::ReadAllText($readPath) | ConvertFrom-Json)
    } else {
        throw 'PROBE-HARNESS: medium-integrity child produced no result file'
    }
    if ($mediumRecord.ChildError) {
        throw ('PROBE-HARNESS: refusing to record an ownership verdict - the medium-integrity child measured nothing: ' + $mediumRecord.ChildError)
    }
    if (-not $mediumRecord.FileOwnerSid) {
        throw 'PROBE-HARNESS: refusing to record an ownership verdict - the medium-integrity child reported no file owner'
    }
    $mediumRecord | Add-Member -NotePropertyName 'IntegritySid' -NotePropertyValue $medium.IntegritySid -Force

    [void](Write-ProbeJson -Probe $Probe -Name 'ownership-under-elevation.json' -InputObject ([pscustomobject]@{
                Experiment       = 'owner of a newly created file, High-integrity parent vs Medium-integrity child'
                Question         = 'does a new object land as TokenUser or as TokenOwner (BUILTIN\Administrators)'
                MediumLaunch     = 'Start-MediumIntegrityProcess (DuplicateTokenEx + SetTokenInformation(TokenIntegrityLevel) + CreateProcessAsUser), label asserted S-1-16-8192'
                HighIntegrity    = $highRecord
                MediumIntegrity  = $mediumRecord
                OwnerSourceHigh  = (Get-OwnerSource -Record $highRecord)
                OwnerSourceMedium = (Get-OwnerSource -Record $mediumRecord)
                EnvironmentLimit = 'single built-in Administrator account on this VM; a non-admin CI account could not be exercised here'
            }))

    Stop-ScratchProcess -ProcessId $mediumProcessId
    $mediumProcessId = 0

    $localityFile = Join-Path $scratchRoot 'locality-target.txt'
    [IO.File]::WriteAllText($localityFile, 'LOCALITY')
    $localityTargets = @(
        @{ Label = 'local-ntfs-temp-file'; Path = $localityFile; Root = 'C:\'; IsDirectory = $false },
        @{ Label = 'local-ntfs-temp-dir'; Path = $scratchRoot; Root = 'C:\'; IsDirectory = $true },
        @{ Label = 'local-ntfs-system-file'; Path = (Join-Path $env:WINDIR 'System32\notepad.exe'); Root = 'C:\'; IsDirectory = $false },
        @{ Label = 'local-ntfs-volume-root'; Path = 'C:\'; Root = 'C:\'; IsDirectory = $true },
        @{ Label = 'local-udf-cdrom-root'; Path = 'D:\'; Root = 'D:\'; IsDirectory = $true },
        @{ Label = 'local-udf-cdrom-file'; Path = 'D:\autorun.inf'; Root = 'D:\'; IsDirectory = $false },
        @{ Label = 'unc-loopback-share-root'; Path = '\\localhost\C$\'; Root = '\\localhost\C$\'; IsDirectory = $true },
        @{ Label = 'unc-loopback-file'; Path = '\\localhost\C$\Windows\System32\notepad.exe'; Root = '\\localhost\C$\'; IsDirectory = $false },
        @{ Label = 'unc-ip-loopback-file'; Path = '\\127.0.0.1\C$\Windows\System32\notepad.exe'; Root = '\\127.0.0.1\C$\'; IsDirectory = $false }
    )
    $localityRows = New-Object System.Collections.ArrayList
    foreach ($t in $localityTargets) {
        $row = $script:PrivateFile::QueryLocality([string]$t.Path, [string]$t.Root, [bool]$t.IsDirectory)
        [void]$localityRows.Add([pscustomobject]@{
                Label                = $t.Label
                Target               = $row.Target
                DriveTypeName        = $row.DriveTypeName
                DriveTypeValue       = $row.DriveType
                PathIsNetworkPath    = $row.PathIsNetworkPath
                OpenSucceeded        = $row.OpenSuccess
                OpenError            = $row.OpenError
                OpenErrorName        = $row.OpenErrorName
                ControlCallSucceeded = $row.ControlCallSuccess
                ControlErrorName     = $row.ControlErrorName
                ControlDetail        = $row.ControlDetail
                InfoClass            = $row.InfoClass
                OutOfRangeClassSucceeded = $row.OutOfRangeClassCallSuccess
                OutOfRangeClassError = $row.OutOfRangeClassLastError
                OutOfRangeClassErrorName = $row.OutOfRangeClassErrorName
                RemoteCallSucceeded  = $row.CallSuccess
                RemoteError          = $row.LastError
                RemoteErrorName      = $row.ErrorName
                RemoteMessage        = $row.Message
                StructureVersion     = $row.StructureVersion
                StructureSize        = $row.StructureSize
                Protocol             = $row.Protocol
                ProtocolName         = $row.ProtocolName
                ProtocolMajor        = $row.ProtocolMajorVersion
                ProtocolMinor        = $row.ProtocolMinorVersion
                ProtocolRevision     = $row.ProtocolRevision
                RemoteFlags          = $row.Flags
                RemoteFlagNames      = $row.FlagNames
            })
    }
    $localOk = @($localityRows | Where-Object { $_.Label -like 'local-*' -and $_.RemoteCallSucceeded })
    $remoteOk = @($localityRows | Where-Object { $_.Label -like 'unc-*' -and $_.RemoteCallSucceeded })
    [void](Write-ProbeJson -Probe $Probe -Name 'locality-classification.json' -InputObject ([pscustomobject]@{
                Experiment              = 'GetFileInformationByHandleEx(FileRemoteProtocolInfo, FILE_INFO_BY_HANDLE_CLASS 13) locality classification'
                Question                = 'can FileRemoteProtocolInfo distinguish local from remote volumes on this build'
                ControlClass            = 'FileBasicInfo (class 0) on the same handle, proving the call plumbing'
                OutOfRangeControlClass  = 'class 55 (past MaximumFileInfoByHandleClass) on the same handle, testing whether ERROR_INVALID_PARAMETER is specific to locality'
                BufferSize              = 'sizeof(FILE_REMOTE_PROTOCOL_INFO) = 116 bytes'
                LocalTargetsSucceeding  = $localOk.Count
                RemoteTargetsSucceeding = $remoteOk.Count
                DistinguishesLocalFromRemote = ($localOk.Count -eq 0 -and $remoteOk.Count -gt 0)
                ErrorCodeIsAmbiguous    = @($localityRows | Where-Object { (-not $_.RemoteCallSucceeded) -and $_.RemoteError -eq $_.OutOfRangeClassError }).Count
                Targets                 = @($localityRows)
            }))

    $plainFile = Join-Path $scratchRoot 'plain-leaf.txt'
    [IO.File]::WriteAllText($plainFile, 'PLAIN')
    $plainRecord = Get-AclRecord -Path $plainFile
    $plainChain = Get-AncestorChain -LeafPath $plainFile

    $protectedDir = Join-Path $scratchRoot 'protected-parent'
    New-CleanDirectory -Path $protectedDir
    $protectedAcl = Get-Acl -LiteralPath $protectedDir
    $protectedAcl.SetAccessRuleProtection($true, $false)
    $protectedAcl.AddAccessRule((New-Object System.Security.AccessControl.FileSystemAccessRule(
                (New-Object System.Security.Principal.SecurityIdentifier($script:TokenUserSid)),
                [System.Security.AccessControl.FileSystemRights]::FullControl,
                ([System.Security.AccessControl.InheritanceFlags]::ContainerInherit -bor [System.Security.AccessControl.InheritanceFlags]::ObjectInherit),
                [System.Security.AccessControl.PropagationFlags]::None,
                [System.Security.AccessControl.AccessControlType]::Allow)))
    Set-Acl -LiteralPath $protectedDir -AclObject $protectedAcl
    if (-not (Get-Acl -LiteralPath $protectedDir).AreAccessRulesProtected) {
        throw ('PROBE-HARNESS: the protected-parent arm requires an inheritance-protected parent, but ' + $protectedDir + ' reads back AreAccessRulesProtected false after Set-Acl')
    }
    $protectedFile = Join-Path $protectedDir 'protected-leaf.txt'
    [IO.File]::WriteAllText($protectedFile, 'PROTECTED')
    $protectedDirRecord = Get-AclRecord -Path $protectedDir -IsDirectory $true
    $protectedFileRecord = Get-AclRecord -Path $protectedFile

    [void](Write-ProbeJson -Probe $Probe -Name 'acl-validation-contract.json' -InputObject ([pscustomobject]@{
                Experiment                 = 'DACL of a plain leaf vs a leaf under a protected parent, plus the full ancestor chain'
                Question                   = 'does the leaf alone or the ancestor chain carry the meaningful restriction'
                PlainLeaf                  = $plainRecord
                PlainLeafAllAcesInherited  = ($plainRecord.ExplicitAceCount -eq 0)
                PlainAncestorChain         = $plainChain
                PermissiveAncestors        = @($plainChain | Where-Object { $_.UntrustedWriteGrants.Count -gt 0 } | ForEach-Object { $_.Path + ' -> ' + ($_.UntrustedWriteGrants -join ', ') })
                ProtectedParent            = $protectedDirRecord
                ProtectedLeaf              = $protectedFileRecord
                ProtectedLeafAllAcesInherited = ($protectedFileRecord.ExplicitAceCount -eq 0)
            }))

    $summary = [pscustomobject]@{
        ShareModeCases            = $shareRows.Count
        ShareModeSharingViolations = @($shareRows | Where-Object { $_.Verdict -eq 'sharing-violation-32' }).Count
        ShareModeSuccessWithoutShareDelete = $unexpectedSuccess.Count
        OwnerSourceHigh           = (Get-OwnerSource -Record $highRecord)
        OwnerSourceMedium         = (Get-OwnerSource -Record $mediumRecord)
        LocalityLocalSucceeding   = $localOk.Count
        LocalityRemoteSucceeding  = $remoteOk.Count
        PlainLeafAceCount         = $plainRecord.AceCount
        PlainLeafInheritedAces    = $plainRecord.InheritedAceCount
        ProtectedLeafAceCount     = $protectedFileRecord.AceCount
        ProtectedLeafInheritedAces = $protectedFileRecord.InheritedAceCount
    }
    $resultMessage = 'four private-file experiments captured'
} catch {
    $status = 'fail'
    $resultMessage = $_.Exception.Message
    [void](Write-ProbeCapture -Probe $Probe -Name 'probe-error.txt' -Content (($_.Exception.Message + [Environment]::NewLine + $_.ScriptStackTrace)))
} finally {
    if ($script:HeldHandle -and $script:HeldHandle -ne -1 -and $script:PrivateFile) {
        $null = $script:PrivateFile::CloseFileHandle($script:HeldHandle)
        $script:HeldHandle = [long](-1)
    }
    if ($mediumProcessId -ne 0) {
        try { Stop-ScratchProcess -ProcessId $mediumProcessId } catch { Write-ProbeLog -Message ('medium child cleanup: ' + $_.Exception.Message) -Level 'warn' }
    }
    Remove-DirectoryTree -Path $scratchRoot
    Remove-DirectoryTree -Path $mediumOutDir
    $strayFallback = Join-Path $env:TEMP 'agent-desktop-probe-12-medium-fallback.json'
    if ([IO.File]::Exists($strayFallback)) { [IO.File]::Delete($strayFallback) }
}

Write-ProbeResult -Probe $Probe -Status $status -Message $resultMessage -Data $summary
if ($status -eq 'fail') { exit 1 }
exit 0
