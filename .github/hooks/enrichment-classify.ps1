# Shared enrichment classifier (Workstream A) — single source of truth for the
# "production source changed but no doc/KB was updated" predicate.
#
# Consumed by both:
#   - .github/hooks/enrichment-gate.ps1   (VS Code Stop hook, working-tree changes)
#   - .githooks/pre-push -> enrichment-prepush.ps1 (git pre-push, commit-range changes)
#
# Dot-source this file, then call Get-EnrichmentVerdict / Get-EnrichmentReason.
# It defines functions only and has no side effects on import.

# Paths that count as production source. A non-test change under one of these,
# with a code extension, means behaviour may have changed and docs should follow.
$script:EnrichmentCodeRoots = @('app/src/', 'app/src-tauri/src/', 'lcc-rs/', 'bowties-core/')
$script:EnrichmentCodeExts  = @('.ts', '.tsx', '.js', '.mjs', '.svelte', '.rs')
$script:EnrichmentExcludeDirs = @('node_modules/', '.svelte-kit/', 'target/', 'dist/', 'build/')
$script:EnrichmentTestMarkers = @('.test.', '.spec.', '/tests/', '/test/')

# Paths that count as a doc / knowledge-base enrichment.
$script:EnrichmentDocRoots = @('aiwiki/', 'product/')
$script:EnrichmentDocFiles = @('specs/backlog.md')

function Get-EnrichmentVerdict {
    <#
    .SYNOPSIS
        Classify a set of changed paths into an enrichment-staleness verdict.
    .PARAMETER ChangedPaths
        Repo-relative paths (forward-slash) that changed.
    .PARAMETER ForceRequire
        Treat the change set as code-changed regardless of paths (honours a
        [kb-required] override tag).
    .OUTPUTS
        PSCustomObject with CodeChanged, DocsChanged, Stale, CodeFiles.
    #>
    param(
        [string[]]$ChangedPaths = @(),
        [switch]$ForceRequire
    )

    $codeFiles = New-Object System.Collections.Generic.List[string]
    $docsChanged = $false

    foreach ($path in $ChangedPaths) {
        if ([string]::IsNullOrWhiteSpace($path)) { continue }
        $norm = $path.Trim().Trim('"').Replace('\', '/')
        $lower = $norm.ToLowerInvariant()

        # Doc / KB enrichment signal.
        if ($script:EnrichmentDocFiles -contains $norm) { $docsChanged = $true }
        foreach ($root in $script:EnrichmentDocRoots) {
            if ($lower.StartsWith($root)) { $docsChanged = $true; break }
        }

        # Production source signal.
        $underCodeRoot = $false
        foreach ($root in $script:EnrichmentCodeRoots) {
            if ($lower.StartsWith($root)) { $underCodeRoot = $true; break }
        }
        if (-not $underCodeRoot) { continue }

        $isExcludedDir = $false
        foreach ($dir in $script:EnrichmentExcludeDirs) {
            if ($lower.Contains($dir)) { $isExcludedDir = $true; break }
        }
        if ($isExcludedDir) { continue }

        $ext = [System.IO.Path]::GetExtension($lower)
        if ($script:EnrichmentCodeExts -notcontains $ext) { continue }

        $isTest = $false
        foreach ($marker in $script:EnrichmentTestMarkers) {
            if ($lower.Contains($marker)) { $isTest = $true; break }
        }
        if ($isTest) { continue }

        $codeFiles.Add($norm)
    }

    $codeChanged = ($codeFiles.Count -gt 0) -or [bool]$ForceRequire

    [PSCustomObject]@{
        CodeChanged = $codeChanged
        DocsChanged = $docsChanged
        Stale       = ($codeChanged -and -not $docsChanged)
        CodeFiles   = $codeFiles.ToArray()
    }
}

function Get-EnrichmentReason {
    <#
    .SYNOPSIS
        Build the shared actionable guidance, with a surface-specific lead-in.
    .PARAMETER Action
        The verb phrase for the lead-in, e.g. 'finishing' or 'pushing'.
    #>
    param([string]$Action = 'finishing')

    @(
        "Enrichment gate: production source changed but no knowledge-base or product doc was updated.",
        "Before ${Action}, update at least one of:",
        "  - aiwiki/ (owners.md, flows.md, architecture-health.md) for modules/conventions/flows you touched",
        "  - product/ (durable behavior or architecture docs, incl. ADRs) if user-visible behavior or ownership changed",
        "  - specs/backlog.md if this work resolved, changed, or revealed future work",
        "If none genuinely applies, document why and proceed."
    ) -join "`n"
}
