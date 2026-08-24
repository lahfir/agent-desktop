param(
    [switch]$SelfTestSeedFailure
)

$ok = Write-Verdict
if ($ok) { exit 0 } else { exit 1 }
