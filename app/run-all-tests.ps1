# Run all tests: Svelte/TypeScript (vitest) and Rust (cargo test)
param(
    [switch]$Verbose
)

$ErrorActionPreference = 'Continue'
$failedTests = @()

# Per-project counters
$results = [ordered]@{
    'vitest'    = @{ Passed = 0; Failed = 0 }
    'lcc-rs'    = @{ Passed = 0; Failed = 0 }
    'src-tauri' = @{ Passed = 0; Failed = 0 }
}

# --- Vitest (Svelte / TypeScript) ---
Write-Host "`n=== Svelte / TypeScript Tests (vitest) ===" -ForegroundColor Cyan
Push-Location $PSScriptRoot
try {
    $vitestRaw = npx vitest run 2>&1 | Out-String
    Write-Host $vitestRaw
    # Strip ANSI escape codes for reliable parsing
    $vitestOutput = $vitestRaw -replace '\x1b\[[0-9;]*m',''

    # Parse summary line: "Tests  200 passed (200)" or "Tests  3 failed | 197 passed (200)"
    if ($vitestOutput -match 'Tests\s+(?:(\d+)\s+failed\s+\|\s+)?(\d+)\s+passed') {
        $results['vitest'].Failed = if ($Matches[1]) { [int]$Matches[1] } else { 0 }
        $results['vitest'].Passed = [int]$Matches[2]
    }

    # Collect failed test file names (lines with FAIL or ×/✗)
    foreach ($line in $vitestOutput -split "`n") {
        if ($line -match '^\s*[×✗]\s+(.+?)\s+\(\d+\s+test') {
            $failedTests += "[vitest] $($Matches[1].Trim())"
        } elseif ($line -match 'FAIL\s+(.+?)\s+\(\d+\s+test') {
            $failedTests += "[vitest] $($Matches[1].Trim())"
        }
    }
} finally {
    Pop-Location
}

# --- Helper to run cargo test and parse output ---
function Invoke-CargoTest {
    param([string]$Label, [string]$ManifestPath)

    Write-Host "`n=== Rust Tests: $Label ===" -ForegroundColor Cyan
    $rawOutput = cargo test --manifest-path $ManifestPath 2>&1 | Out-String
    Write-Host $rawOutput
    # Strip ANSI escape codes for reliable parsing
    $output = $rawOutput -replace '\x1b\[[0-9;]*m',''

    # Parse all "test result:" summary lines
    foreach ($m in [regex]::Matches($output, 'test result: \w+\.\s+(\d+)\s+passed;\s+(\d+)\s+failed')) {
        $script:results[$Label].Passed += [int]$m.Groups[1].Value
        $script:results[$Label].Failed += [int]$m.Groups[2].Value
    }

    # Collect individual failed test names
    foreach ($line in $output -split "`n") {
        if ($line -match '^\s*test\s+(.+?)\s+\.\.\.\s+FAILED') {
            $script:failedTests += "[$Label] $($Matches[1].Trim())"
        }
    }
}

Invoke-CargoTest -Label 'lcc-rs'    -ManifestPath "$PSScriptRoot\..\lcc-rs\Cargo.toml"
Invoke-CargoTest -Label 'src-tauri' -ManifestPath "$PSScriptRoot\src-tauri\Cargo.toml"

# --- Summary ---
$totalPassed = ($results.Values | ForEach-Object { $_.Passed } | Measure-Object -Sum).Sum
$totalFailed = ($results.Values | ForEach-Object { $_.Failed } | Measure-Object -Sum).Sum

Write-Host "`n========================================" -ForegroundColor White
Write-Host "            TEST SUMMARY" -ForegroundColor White
Write-Host "========================================" -ForegroundColor White

if ($failedTests.Count -gt 0) {
    Write-Host "`nFailed tests:" -ForegroundColor Red
    foreach ($t in $failedTests) {
        Write-Host "  - $t" -ForegroundColor Red
    }
}

Write-Host ""
Write-Host ("  {0,-12} {1,8} {2,8}" -f 'Project', 'Passed', 'Failed')
Write-Host ("  {0,-12} {1,8} {2,8}" -f '-------', '------', '------')
foreach ($key in $results.Keys) {
    $p = $results[$key].Passed
    $f = $results[$key].Failed
    $color = if ($f -gt 0) { 'Red' } else { 'Green' }
    Write-Host ("  {0,-12} {1,8} {2,8}" -f $key, $p, $f) -ForegroundColor $color
}
Write-Host ("  {0,-12} {1,8} {2,8}" -f '-------', '------', '------')
Write-Host ("  {0,-12} {1,8} {2,8}" -f 'TOTAL', $totalPassed, $totalFailed) -ForegroundColor $(if ($totalFailed -gt 0) { 'Red' } else { 'Green' })
Write-Host ""

if ($totalFailed -gt 0) {
    exit 1
}
