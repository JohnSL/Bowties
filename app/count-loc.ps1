param(
    [switch]$Json
)

$ErrorActionPreference = 'Stop'

function Get-RepoRoot {
    return (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
}

function Get-CodeFiles {
    param(
        [Parameter(Mandatory = $true)]
        [string]$BasePath,

        [Parameter(Mandatory = $true)]
        [string[]]$Extensions,

        [string]$NamePattern
    )

    if (-not (Test-Path -LiteralPath $BasePath)) {
        return @()
    }

    $files = Get-ChildItem -LiteralPath $BasePath -File -Recurse | Where-Object {
        $Extensions -contains $_.Extension.ToLowerInvariant()
    }

    if ($NamePattern) {
        $files = $files | Where-Object { $_.Name -match $NamePattern }
    }

    return @($files.FullName | Sort-Object -Unique)
}

function Invoke-Cloc {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Files
    )

    $uniqueFiles = @($Files | Where-Object { $_ } | Sort-Object -Unique)
    if ($uniqueFiles.Count -eq 0) {
        return [pscustomobject]@{
            Files   = 0
            Code    = 0
            Comment = 0
            Blank   = 0
        }
    }

    $listFile = [System.IO.Path]::GetTempFileName()
    try {
        Set-Content -LiteralPath $listFile -Value $uniqueFiles -Encoding UTF8
        $clocJson = cloc --json --list-file=$listFile 2>$null | Out-String
        if ($LASTEXITCODE -ne 0 -or -not $clocJson.Trim()) {
            throw 'cloc failed to produce JSON output.'
        }

        $result = $clocJson | ConvertFrom-Json
        return [pscustomobject]@{
            Files   = [int]$result.SUM.nFiles
            Code    = [int]$result.SUM.code
            Comment = [int]$result.SUM.comment
            Blank   = [int]$result.SUM.blank
        }
    }
    finally {
        Remove-Item -LiteralPath $listFile -ErrorAction SilentlyContinue
    }
}

function New-CountRow {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Name,

        [Parameter(Mandatory = $true)]
        [string[]]$Files
    )

    $counts = Invoke-Cloc -Files $Files
    return [pscustomobject]@{
        Area    = $Name
        Files   = $counts.Files
        Code    = $counts.Code
        Comment = $counts.Comment
        Blank   = $counts.Blank
    }
}

if (-not (Get-Command cloc -ErrorAction SilentlyContinue)) {
    throw 'cloc is required on PATH to run this script.'
}

$repoRoot = Get-RepoRoot
$frontendRoot = Join-Path $repoRoot 'app/src'
$backendRoot = Join-Path $repoRoot 'app/src-tauri/src'
$crateRoot = Join-Path $repoRoot 'lcc-rs/src'
$crateTestsRoot = Join-Path $repoRoot 'lcc-rs/tests'
$backendBuildFile = Join-Path $repoRoot 'app/src-tauri/build.rs'

$frontendAll = Get-CodeFiles -BasePath $frontendRoot -Extensions @('.ts', '.js', '.svelte', '.css', '.html')
$frontendTestFiles = @($frontendAll | Where-Object { [System.IO.Path]::GetFileName($_) -match '\.(test|spec)\.' })
$frontendSourceFiles = @($frontendAll | Where-Object { [System.IO.Path]::GetFileName($_) -notmatch '\.(test|spec)\.' })

$backendSourceFiles = Get-CodeFiles -BasePath $backendRoot -Extensions @('.rs')
if (Test-Path -LiteralPath $backendBuildFile) {
    $backendSourceFiles = @($backendSourceFiles + $backendBuildFile | Sort-Object -Unique)
}

$crateSourceFiles = Get-CodeFiles -BasePath $crateRoot -Extensions @('.rs')
$crateTestFiles = Get-CodeFiles -BasePath $crateTestsRoot -Extensions @('.rs')

$testOnlyFiles = @($frontendTestFiles + $crateTestFiles | Sort-Object -Unique)
$allCountedFiles = @($frontendSourceFiles + $backendSourceFiles + $crateSourceFiles + $testOnlyFiles | Sort-Object -Unique)

$summaryRows = @(
    (New-CountRow -Name 'Frontend' -Files $frontendSourceFiles),
    (New-CountRow -Name 'Backend' -Files $backendSourceFiles),
    (New-CountRow -Name 'Crate' -Files $crateSourceFiles),
    (New-CountRow -Name 'Tests' -Files $testOnlyFiles),
    (New-CountRow -Name 'Total' -Files $allCountedFiles)
)

$testRows = @(
    (New-CountRow -Name 'Frontend Tests' -Files $frontendTestFiles),
    (New-CountRow -Name 'Crate Tests' -Files $crateTestFiles),
    (New-CountRow -Name 'Total Tests' -Files $testOnlyFiles)
)

if ($Json) {
    [pscustomobject]@{
        Summary = $summaryRows
        Tests   = $testRows
    } | ConvertTo-Json -Depth 4
    exit 0
}

Write-Host ''
Write-Host 'Code Line Counts' -ForegroundColor Cyan
$summaryRows | Format-Table -AutoSize

Write-Host ''
Write-Host 'Test-Only File Counts' -ForegroundColor Cyan
$testRows | Format-Table -AutoSize

Write-Host ''
Write-Host 'External dependencies are excluded; only files in this repo are counted.' -ForegroundColor DarkGray