@{
    <#
        R18b's committed per-leg p99 store. Empty on this PR: no positive
        Assert-Effect leg has been costed yet (scenario units land later),
        so Initialize-NoEffectWindow falls back to BootstrapP99Ms and
        Write-Verdict records that the bootstrap was used. U15's cost run
        (min-of-seven, warm-up discarded) rewrites Legs with one entry per
        positive Assert-Effect leg name, keyed exactly as the leg is
        registered with Register-Legs.
    #>
    BootstrapP99Ms = 2500
    Legs           = @{}
}
