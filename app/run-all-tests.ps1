# Run all tests: Svelte/TypeScript (vitest) and Rust (cargo test)
param(
    [switch]$Verbose
)

$ErrorActionPreference = 'Continue'
$failedTests = @()

# Per-project counters
$results = [ordered]@{
    'vitest'       = @{ Passed = 0; Failed = 0; RunnerFailure = $false }
    'bowties-core' = @{ Passed = 0; Failed = 0; RunnerFailure = $false }
    'tauri-backend' = @{ Passed = 0; Failed = 0; RunnerFailure = $false }
    'lcc-rs'       = @{ Passed = 0; Failed = 0; RunnerFailure = $false }
}

function Invoke-SuiteCommand {
    param([string]$Directory, [string]$Command, [string[]]$Arguments)

    $ErrorActionPreference = 'Stop'
    $PSNativeCommandUseErrorActionPreference = $false
    $pushed = $false
    $output = @()
    $exitCode = $null
    $errorMessage = $null
    try {
        Push-Location -LiteralPath $Directory
        $pushed = $true
        $global:LASTEXITCODE = $null
        $output = @(& $Command @Arguments 2>&1)
        $exitCode = $LASTEXITCODE
    } catch {
        $errorMessage = $_.Exception.Message
    } finally {
        if ($pushed) { Pop-Location }
    }
    return @{ Output = ($output | Out-String); ExitCode = $exitCode; Error = $errorMessage }
}

function Set-RunnerFailure {
    param([string]$Label, [string]$Message)
    $script:results[$Label].RunnerFailure = $true
    Write-Host "[$Label] Runner failure: $Message" -ForegroundColor Red
}

function Test-SuiteInvocation {
    param([string]$Label, $Invocation, [bool]$HasSummary)
    if ($Invocation.Error) {
        Set-RunnerFailure $Label $Invocation.Error
    } elseif ($null -eq $Invocation.ExitCode) {
        Set-RunnerFailure $Label 'command did not report an exit code'
    } elseif ($Invocation.ExitCode -ne 0) {
        Set-RunnerFailure $Label "exit code $($Invocation.ExitCode)"
    }
    if (-not $HasSummary) {
        Set-RunnerFailure $Label 'no recognized completion summary'
    }
}

# --- Vitest (Svelte / TypeScript) ---
Write-Host "`n=== Svelte / TypeScript Tests (vitest) ===" -ForegroundColor Cyan
    $vitestInvocation = Invoke-SuiteCommand -Directory $PSScriptRoot -Command 'npx' -Arguments @('vitest', 'run')
    $vitestRaw = $vitestInvocation.Output
    Write-Host $vitestRaw
    # Strip ANSI escape codes for reliable parsing
    $vitestOutput = $vitestRaw -replace '\x1b\[[0-9;]*m',''

    # Parse passed-only, failed-only, and mixed summaries.
    $vitestHasSummary = $vitestOutput -match 'Tests\s+(?:(\d+)\s+failed(?:\s*\|\s*(\d+)\s+passed)?|(\d+)\s+passed)'
    if ($vitestHasSummary) {
        $results['vitest'].Failed = if ($Matches[1]) { [int]$Matches[1] } else { 0 }
        $results['vitest'].Passed = if ($Matches[2]) { [int]$Matches[2] } elseif ($Matches[3]) { [int]$Matches[3] } else { 0 }
    }
    Test-SuiteInvocation 'vitest' $vitestInvocation $vitestHasSummary

    # Collect failed test file names (lines with FAIL or ×/✗)
    foreach ($line in $vitestOutput -split "`n") {
        if ($line -match '^\s*[×✗]\s+(.+?)\s+\(\d+\s+test') {
            $failedTests += "[vitest] $($Matches[1].Trim())"
        } elseif ($line -match 'FAIL\s+(.+?)\s+\(\d+\s+test') {
            $failedTests += "[vitest] $($Matches[1].Trim())"
        }
    }

# --- Helper to run cargo test and parse output ---
function Invoke-CargoTest {
    param([string]$Label, [string]$Directory)

    Write-Host "`n=== Rust Tests: $Label ===" -ForegroundColor Cyan
    $manifestPath = Join-Path $Directory 'Cargo.toml'
    $invocation = Invoke-SuiteCommand -Directory $Directory -Command 'cargo' -Arguments @('test', '--locked', '--manifest-path', $manifestPath)
    $rawOutput = $invocation.Output
    Write-Host $rawOutput
    # Strip ANSI escape codes for reliable parsing
    $output = $rawOutput -replace '\x1b\[[0-9;]*m',''

    # Parse all "test result:" summary lines
    $summaries = [regex]::Matches($output, 'test result: \w+\.\s+(\d+)\s+passed;\s+(\d+)\s+failed')
    foreach ($m in $summaries) {
        $script:results[$Label].Passed += [int]$m.Groups[1].Value
        $script:results[$Label].Failed += [int]$m.Groups[2].Value
    }
    Test-SuiteInvocation $Label $invocation ($summaries.Count -gt 0)

    # Collect individual failed test names
    foreach ($line in $output -split "`n") {
        if ($line -match '^\s*test\s+(.+?)\s+\.\.\.\s+FAILED') {
            $script:failedTests += "[$Label] $($Matches[1].Trim())"
        }
    }
}

Invoke-CargoTest -Label 'bowties-core' -Directory ([IO.Path]::GetFullPath("$PSScriptRoot\..\bowties-core"))
Invoke-CargoTest -Label 'tauri-backend' -Directory ([IO.Path]::GetFullPath("$PSScriptRoot\src-tauri"))
Invoke-CargoTest -Label 'lcc-rs' -Directory ([IO.Path]::GetFullPath("$PSScriptRoot\..\lcc-rs"))

# --- Summary ---
$totalPassed = ($results.Values | ForEach-Object { $_.Passed } | Measure-Object -Sum).Sum
$totalFailed = ($results.Values | ForEach-Object { $_.Failed } | Measure-Object -Sum).Sum
$runnerFailed = @($results.Values | Where-Object { $_.RunnerFailure }).Count -gt 0

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
Write-Host ("  {0,-14} {1,8} {2,8}  {3}" -f 'Project', 'Passed', 'Failed', 'Status')
Write-Host ("  {0,-14} {1,8} {2,8}  {3}" -f '---------', '------', '------', '------')
foreach ($key in $results.Keys) {
    $p = $results[$key].Passed
    $f = $results[$key].Failed
    $status = if ($results[$key].RunnerFailure) { 'RUNNER FAILED' } elseif ($f -gt 0) { 'FAILED' } else { 'OK' }
    $color = if ($status -ne 'OK') { 'Red' } else { 'Green' }
    Write-Host ("  {0,-14} {1,8} {2,8}  {3}" -f $key, $p, $f, $status) -ForegroundColor $color
}
Write-Host ("  {0,-14} {1,8} {2,8}" -f '---------', '------', '------')
Write-Host ("  {0,-14} {1,8} {2,8}" -f 'TOTAL', $totalPassed, $totalFailed) -ForegroundColor $(if ($totalFailed -gt 0 -or $runnerFailed) { 'Red' } else { 'Green' })
Write-Host ""

if ($totalFailed -gt 0 -or $runnerFailed) {
    exit 1
}
exit 0
