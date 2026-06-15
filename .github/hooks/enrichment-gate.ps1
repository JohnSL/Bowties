# Enrichment Stop-gate (Workstream A) — deterministic doc/KB staleness check.
#
# Wired as a workspace-level VS Code Stop hook via enrichment-gate.json. When the
# agent tries to finish, this script blocks completion if production source
# changed in the working tree but no knowledge-base / product / backlog doc was
# updated alongside it. The block reason is fed back to the agent so it completes
# the bookkeeping instead of stopping.
#
# The change-classification predicate lives in enrichment-classify.ps1 and is
# shared with the git pre-push twin (.githooks/pre-push).
#
# Contract (VS Code agent hooks, Preview):
#   stdin  : Stop-hook JSON (common fields + stop_hook_active)
#   stdout : { hookSpecificOutput: { hookEventName, decision: "block", reason } } to block,
#            or { continue: $true } to allow stopping
#   exit 0 : stdout parsed as JSON
#
# Design constraints:
#   - Deterministic file diff only. No LLM calls, no quality re-litigation.
#   - Honour stop_hook_active to avoid nag loops (never block twice in a row).
#   - Fail open: any internal error allows the agent to stop.

$ErrorActionPreference = 'Stop'

function Write-Allow {
    # Allow the agent to stop. Empty-but-valid JSON keeps the contract simple.
    Write-Output '{ "continue": true }'
    exit 0
}

function Write-Block([string]$Reason) {
    $payload = [ordered]@{
        hookSpecificOutput = [ordered]@{
            hookEventName = 'Stop'
            decision      = 'block'
            reason        = $Reason
        }
    }
    Write-Output ($payload | ConvertTo-Json -Depth 5 -Compress)
    exit 0
}

try {
    . (Join-Path $PSScriptRoot 'enrichment-classify.ps1')

    # --- Parse stdin -------------------------------------------------------
    $raw = [Console]::In.ReadToEnd()
    $stopHookActive = $false
    if (-not [string]::IsNullOrWhiteSpace($raw)) {
        try {
            $payload = $raw | ConvertFrom-Json
            if ($null -ne $payload.stop_hook_active) {
                $stopHookActive = [bool]$payload.stop_hook_active
            }
        } catch {
            # Malformed input: treat as not-active and continue evaluating.
        }
    }

    # Loop guard: if we already blocked once and the agent is continuing, do
    # not block again. Lets the agent finish after one nudge.
    if ($stopHookActive) { Write-Allow }

    # --- Locate the repo ---------------------------------------------------
    $repoRoot = (& git rev-parse --show-toplevel 2>$null)
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($repoRoot)) { Write-Allow }
    $repoRoot = $repoRoot.Trim()

    # --- Collect working-tree changes -------------------------------------
    # Porcelain v1 covers staged, unstaged, and untracked files.
    $status = & git -C $repoRoot status --porcelain --untracked-files=all 2>$null
    if ($LASTEXITCODE -ne 0) { Write-Allow }
    if (-not $status) { Write-Allow }

    $changed = New-Object System.Collections.Generic.List[string]
    foreach ($line in $status) {
        if ([string]::IsNullOrWhiteSpace($line)) { continue }
        # Format: "XY <path>" or "XY <old> -> <new>" for renames.
        $path = $line.Substring(3).Trim()
        if ($path -match ' -> ') { $path = ($path -split ' -> ')[-1] }
        $changed.Add($path)
    }

    # --- Classify and decide ----------------------------------------------
    $verdict = Get-EnrichmentVerdict -ChangedPaths $changed.ToArray()
    if ($verdict.Stale) {
        Write-Block (Get-EnrichmentReason -Action 'finishing')
    }

    Write-Allow
} catch {
    # Fail open: never trap the agent on an internal error.
    Write-Allow
}
