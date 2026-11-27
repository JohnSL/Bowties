$ErrorActionPreference = 'Stop'
$root = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$runner = Join-Path $PSScriptRoot 'run-all-tests.ps1'
$pwsh = (Get-Process -Id $PID).Path
$scratch = Join-Path $root "temp\run-all-tests-$([guid]::NewGuid().ToString('N'))"

function Assert-True($condition, [string]$message) {
    if (-not $condition) { throw $message }
}

function Invoke-Scenario([string]$name, [string]$npx, [string]$cargo, [string]$workingDirectory = $root, [switch]$InheritedFailure) {
    $dir = Join-Path $scratch $name
    [IO.Directory]::CreateDirectory($dir) | Out-Null
    if ($null -ne $npx) {
        [IO.File]::WriteAllText((Join-Path $dir 'npx.cmd'), $npx)
    }
    if ($null -ne $cargo) {
        [IO.File]::WriteAllText((Join-Path $dir 'cargo.cmd'), $cargo)
    }
    $log = Join-Path $dir 'invocations.log'
    $start = [Diagnostics.ProcessStartInfo]::new()
    $start.FileName = $pwsh
    $start.WorkingDirectory = $workingDirectory
    $start.UseShellExecute = $false
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.ArgumentList.Add('-NoProfile')
    if ($InheritedFailure) {
        $start.ArgumentList.Add('-Command')
        $quotedRunner = $runner.Replace("'", "''")
        $start.ArgumentList.Add("`$global:LASTEXITCODE = 23; `$ErrorActionPreference = 'Stop'; `$PSNativeCommandUseErrorActionPreference = `$true; & '$quotedRunner'")
    } else {
        $start.ArgumentList.Add('-File')
        $start.ArgumentList.Add($runner)
    }
    $start.Environment['PATH'] = "$dir;$env:SystemRoot\System32"
    $start.Environment['FIXTURE_LOG'] = $log
    $process = [Diagnostics.Process]::Start($start)
    $stdout = $process.StandardOutput.ReadToEnd()
    $stderr = $process.StandardError.ReadToEnd()
    $process.WaitForExit()
    return @{
        ExitCode = $process.ExitCode
        Output = $stdout + $stderr
        Calls = if (Test-Path $log) { @(Get-Content $log) } else { @() }
    }
}

try {
    [IO.Directory]::CreateDirectory($scratch) | Out-Null

    # Behavior 1: a successful run covers each suite in its own directory.
    $npxSuccess = "@echo off`r`necho %CD%^|%*>>`"%FIXTURE_LOG%`"`r`necho Tests  4 passed (4)`r`nexit /b 0`r`n"
    $cargoSuccess = "@echo off`r`necho %CD%^|%*>>`"%FIXTURE_LOG%`"`r`necho test result: ok. 3 passed; 0 failed; 0 ignored`r`nexit /b 0`r`n"
    $result = Invoke-Scenario 'success' $npxSuccess $cargoSuccess (Join-Path $root 'lcc-rs')
    Assert-True ($result.ExitCode -eq 0) "success: exit code should be 0; got $($result.ExitCode)"
    foreach ($row in @('vitest', 'bowties-core', 'tauri-backend', 'lcc-rs')) {
        Assert-True ($result.Output -match "(?m)^\s*$row\s+\d+\s+\d+") "success: missing summary row $row"
    }
    Assert-True ($result.Calls.Count -eq 4) "success: expected four invocations; got $($result.Calls.Count)"
    Assert-True ($result.Calls[0] -eq "$(Join-Path $root 'app')|vitest run") "success: wrong vitest invocation: $($result.Calls[0])"
    $crates = @(
        @{ Dir = Join-Path $root 'bowties-core' },
        @{ Dir = Join-Path $root 'app\src-tauri' },
        @{ Dir = Join-Path $root 'lcc-rs' }
    )
    for ($i = 0; $i -lt $crates.Count; $i++) {
        $dir = $crates[$i].Dir
        $expected = "$dir|test --locked --manifest-path $(Join-Path $dir 'Cargo.toml')"
        Assert-True ($result.Calls[$i + 1] -eq $expected) "success: wrong cargo invocation: $($result.Calls[$i + 1]); expected $expected"
    }
    Write-Host 'PASS: four-suite success'

    # Behavior 2: runner errors are visible, preserve counts, and never stop other suites.
    $core = Join-Path $root 'bowties-core'
    $backend = Join-Path $root 'app\src-tauri'
    $cargoProblems = @"
@echo off
echo %CD%^|%*>>"%FIXTURE_LOG%"
if /I "%CD%"=="$core" (
  echo test result: ok. 2 passed; 0 failed; 0 ignored
  echo compiler failed 1>&2
  exit /b 7
)
if /I "%CD%"=="$backend" (
  echo could not load tests 1>&2
  exit /b 0
)
echo compiler progress 1>&2
echo test result: ok. 3 passed; 0 failed; 0 ignored
exit /b 0
"@
    $problems = Invoke-Scenario 'runner-problems' $npxSuccess $cargoProblems
    Assert-True ($problems.ExitCode -eq 1) 'runner problems: expected exit 1 despite zero failed tests'
    Assert-True ($problems.Calls.Count -eq 4) 'runner problems: all independent suites must run'
    Assert-True ($problems.Output -match '(?m)^\s*bowties-core\s+2\s+0\s+RUNNER FAILED') 'runner problems: preserve partial counts and flag nonzero exit'
    Assert-True ($problems.Output -match '(?m)^\s*tauri-backend\s+0\s+0\s+RUNNER FAILED') 'runner problems: missing summary must be flagged without manufacturing counts'
    Assert-True ($problems.Output -match '(?m)^\s*lcc-rs\s+3\s+0\s+OK') 'runner problems: successful stderr must not fail the suite'
    Assert-True ($problems.Output -match 'exit code 7') 'runner problems: nonzero exit diagnostic missing'
    Assert-True ($problems.Output -match 'summary') 'runner problems: missing-summary diagnostic missing'

    $missing = Invoke-Scenario 'missing-npx' $null $cargoSuccess
    Assert-True ($missing.ExitCode -eq 1) 'missing command: expected exit 1'
    Assert-True ($missing.Calls.Count -eq 3) 'missing command: all cargo suites must still run'
    Assert-True ($missing.Output -match '(?m)^\s*vitest\s+0\s+0\s+RUNNER FAILED') 'missing command: vitest must be flagged'
    Assert-True ($missing.Output -match '(?i)npx') 'missing command: diagnostic must name the command'
    Write-Host 'PASS: runner failure isolation'

    # Behavior 3: parse legitimate summaries and isolate command status per project.
    $allFailed = "@echo off`r`necho %CD%^|%*>>`"%FIXTURE_LOG%`"`r`necho Tests  3 failed (3)`r`nexit /b 1`r`n"
    $failed = Invoke-Scenario 'all-failed' $allFailed $cargoSuccess
    Assert-True ($failed.ExitCode -eq 1) 'all failed: expected exit 1'
    Assert-True ($failed.Output -match '(?m)^\s*vitest\s+0\s+3\s+') 'all failed: vitest failed count must be 3'
    Assert-True ($failed.Output -notmatch '\[vitest\] Runner failure: no recognized completion summary') 'all failed: summary must be recognized'
    Assert-True ($failed.Calls.Count -eq 4) 'all failed: all suites must run'

    $npxZero = "@echo off`r`necho %CD%^|%*>>`"%FIXTURE_LOG%`"`r`necho Tests  0 passed (0)`r`nexit /b 0`r`n"
    $cargoZero = "@echo off`r`necho %CD%^|%*>>`"%FIXTURE_LOG%`"`r`necho progress 1>&2`r`necho test result: ok. 0 passed; 0 failed; 0 ignored`r`nexit /b 0`r`n"
    $zero = Invoke-Scenario 'zero-tests' $npxZero $cargoZero $root -InheritedFailure
    Assert-True ($zero.ExitCode -eq 0) 'zero tests: valid summaries and inherited failure must still succeed'
    Assert-True ($zero.Calls.Count -eq 4) 'zero tests: all suites must run'
    foreach ($row in @('vitest', 'bowties-core', 'tauri-backend', 'lcc-rs')) {
        Assert-True ($zero.Output -match "(?m)^\s*$row\s+0\s+0\s+OK") "zero tests: $row must be OK"
    }
    $missingCargo = Invoke-Scenario 'missing-cargo' $npxSuccess $null
    Assert-True ($missingCargo.ExitCode -eq 1) 'missing cargo: expected exit 1 after successful vitest'
    Assert-True ($missingCargo.Output -match '(?m)^\s*vitest\s+4\s+0\s+OK') 'missing cargo: vitest must stay successful'
    foreach ($row in @('bowties-core', 'tauri-backend', 'lcc-rs')) {
        Assert-True ($missingCargo.Output -match "(?m)^\s*$row\s+0\s+0\s+RUNNER FAILED") "missing cargo: $row must fail independently"
    }

    $mixedVitest = "@echo off`r`necho %CD%^|%*>>`"%FIXTURE_LOG%`"`r`necho FAIL example.test.ts (3 tests)`r`necho Tests  3 failed ^| 2 passed (5)`r`nexit /b 1`r`n"
    $multipleCargo = "@echo off`r`necho %CD%^|%*>>`"%FIXTURE_LOG%`"`r`necho test example::failed ... FAILED`r`necho test result: FAILED. 2 passed; 1 failed; 0 ignored`r`necho test result: ok. 3 passed; 0 failed; 0 ignored`r`nexit /b 1`r`n"
    $mixed = Invoke-Scenario 'mixed' $mixedVitest $multipleCargo
    Assert-True ($mixed.ExitCode -eq 1) 'mixed: expected exit 1'
    Assert-True ($mixed.Output -match '(?m)^\s*vitest\s+2\s+3\s+') 'mixed: vitest counts must be preserved'
    foreach ($row in @('bowties-core', 'tauri-backend', 'lcc-rs')) {
        Assert-True ($mixed.Output -match "(?m)^\s*$row\s+5\s+1\s+") "mixed: $row cargo summaries must be summed"
    }
    Assert-True ($mixed.Output -match '(?m)^\s*TOTAL\s+17\s+6') 'mixed: total counts must be preserved'
    Assert-True ($mixed.Output -match '\[vitest\] example\.test\.ts') 'mixed: vitest failure name must be retained'
    Assert-True ($mixed.Output -match '\[bowties-core\] example::failed') 'mixed: cargo failure name must be retained'

    $noSummary = "@echo off`r`necho %CD%^|%*>>`"%FIXTURE_LOG%`"`r`necho test runner stopped without a summary`r`nexit /b 0`r`n"
    $missingVitestSummary = Invoke-Scenario 'missing-vitest-summary' $noSummary $cargoSuccess
    Assert-True ($missingVitestSummary.ExitCode -eq 1) 'missing vitest summary: expected exit 1'
    Assert-True ($missingVitestSummary.Output -match '(?m)^\s*vitest\s+0\s+0\s+RUNNER FAILED') 'missing vitest summary: runner must fail without invented counts'
    Assert-True ($missingVitestSummary.Calls.Count -eq 4) 'missing vitest summary: other suites must run'
    Write-Host 'PASS: parsing and status isolation'
} finally {
    if (Test-Path $scratch) { Remove-Item -LiteralPath $scratch -Recurse -Force }
}
