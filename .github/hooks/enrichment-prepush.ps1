# Enrichment pre-push gate implementation (Workstream A).
#
# Invoked by .githooks/pre-push. Reads git's ref-update lines from stdin,
# computes the files changed across the pushed commits, and uses the shared
# classifier (.github/hooks/enrichment-classify.ps1) to decide whether to block.
#
# Override tags in any pushed commit message:
#   [kb-skip:reason] - skip the check for this push (reason is logged)
#   [kb-required]    - force the check even when no code paths matched
#
# Exit 0 = allow push; exit 1 = block push.
#
# Fail open: any internal error allows the push (the Stop hook is the primary gate).

$ErrorActionPreference = 'Stop'
$ZERO = '0000000000000000000000000000000000000000'

function Approve { exit 0 }
function Block([string]$Message) {
    Write-Host ''
    Write-Host '  ✖ Push blocked by enrichment gate' -ForegroundColor Red
    Write-Host ''
    Write-Host $Message
    Write-Host ''
    Write-Host '  Override (use sparingly): add [kb-skip:reason] to a commit message.' -ForegroundColor DarkGray
    Write-Host ''
    exit 1
}

try {
    . (Join-Path $PSScriptRoot 'enrichment-classify.ps1')

    $repoRoot = (& git rev-parse --show-toplevel 2>$null)
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($repoRoot)) { Approve }
    $repoRoot = $repoRoot.Trim()

    $refLines = @($input)
    if ($refLines.Count -eq 0) { Approve }

    $changed = New-Object System.Collections.Generic.HashSet[string]
    $messages = New-Object System.Collections.Generic.List[string]
    $haveRange = $false

    foreach ($line in $refLines) {
        if ([string]::IsNullOrWhiteSpace($line)) { continue }
        # "<local ref> <local oid> <remote ref> <remote oid>"
        $parts = $line.Trim() -split '\s+'
        if ($parts.Count -lt 4) { continue }
        $localOid = $parts[1]
        $remoteOid = $parts[3]

        if ($localOid -eq $ZERO) { continue }  # branch deletion — nothing to check

        if ($remoteOid -eq $ZERO) {
            # New branch: commits in local not already on any remote.
            $range = @($localOid, '--not', '--remotes')
        } else {
            $range = @("$remoteOid..$localOid")
        }

        $files = & git -C $repoRoot diff --name-only @range 2>$null
        if ($LASTEXITCODE -eq 0 -and $files) {
            foreach ($f in $files) { if ($f) { [void]$changed.Add($f) } }
            $haveRange = $true
        }

        $msgs = & git -C $repoRoot log --format='%B' @range 2>$null
        if ($LASTEXITCODE -eq 0 -and $msgs) { $messages.Add(($msgs -join "`n")) }
    }

    if (-not $haveRange) { Approve }

    $allMessages = ($messages -join "`n")

    # Override: skip entirely.
    if ($allMessages -match '\[kb-skip:([^\]]*)\]') {
        Write-Host "  Enrichment gate skipped via [kb-skip:$($Matches[1])]" -ForegroundColor Yellow
        Approve
    }

    $forceRequire = $allMessages -match '\[kb-required\]'

    $verdict = Get-EnrichmentVerdict -ChangedPaths @($changed) -ForceRequire:$forceRequire
    if ($verdict.Stale) {
        Block (Get-EnrichmentReason -Action 'pushing')
    }

    Approve
} catch {
    Approve
}
