param(
  [Parameter(Mandatory=$true)][string]$JsonPath,
  [Parameter(Mandatory=$true)][string]$BowtiesSrc,
  [Parameter(Mandatory=$true)][string]$LccProSrc
)

$data = Get-Content $JsonPath -Raw | ConvertFrom-Json
$rows = $data.groups
Write-Host "Total memory-config groups: $($rows.Count)"
Write-Host ""

function Stats([double[]]$vals) {
  if ($vals.Count -eq 0) { return "(no data)" }
  $sorted = $vals | Sort-Object
  $min  = $sorted[0]
  $max  = $sorted[-1]
  $mean = [math]::Round(($vals | Measure-Object -Average).Average, 1)
  $p50  = $sorted[[math]::Floor($sorted.Count * 0.5)]
  $p95  = $sorted[[math]::Floor($sorted.Count * 0.95)]
  return ("n={0,3}  min={1,3}  p50={2,3}  mean={3,5}  p95={4,4}  max={5,4}" -f $vals.Count, $min, $p50, $mean, $p95, $max)
}

function AnalyzeClient([object[]]$items, [string]$label) {
  Write-Host "=== $label ==="
  Write-Host ("  memory-config interactions : {0}" -f $items.Count)

  $ok  = @($items | Where-Object { $_.addressMatched -eq $true }).Count
  $nak = @($items | Where-Object { $_.addressMatched -eq $false }).Count
  $na  = @($items | Where-Object { -not ($_.PSObject.Properties.Name -contains 'addressMatched') }).Count
  Write-Host ("  addressMatched=true        : {0}" -f $ok)
  Write-Host ("  addressMatched=false       : {0}  (unpaired - no matching reply)" -f $nak)
  Write-Host ("  addressMatched (n/a)       : {0}  (non-Read/Write)" -f $na)

  $incomplete = @($items | Where-Object { $_.complete -eq $false }).Count
  Write-Host ("  incomplete rows            : {0}" -f $incomplete)

  # Read vs Write split (by summary text)
  $reads  = @($items | Where-Object { $_.summary -match '^Read' }).Count
  $writes = @($items | Where-Object { $_.summary -match '^Write' }).Count
  $other  = $items.Count - $reads - $writes
  Write-Host ("  Read / Write / Other       : {0} / {1} / {2}" -f $reads, $writes, $other)

  # Address space breakdown
  $spaces = $items | Group-Object -Property { if ($_.summary -match 'space=(0x[0-9A-Fa-f]+)') { $matches[1] } else { '?' } }
  Write-Host "  by address space:"
  foreach ($sp in ($spaces | Sort-Object Name)) {
    Write-Host ("    {0} : {1}" -f $sp.Name, $sp.Count)
  }

  # Timing aggregates from paired rows
  $paired = @($items | Where-Object { $_.addressMatched -eq $true -and $_.timing })
  [double[]]$r2a = @($paired | ForEach-Object { $_.timing.requestToAckMs } | Where-Object { $_ -ne $null })
  [double[]]$a2r = @($paired | ForEach-Object { $_.timing.ackToReplyMs   } | Where-Object { $_ -ne $null })
  [double[]]$r2ak = @($paired | ForEach-Object { $_.timing.replyToAckMs   } | Where-Object { $_ -ne $null })
  [double[]]$gap = @($paired | ForEach-Object { $_.timing.gapToNextMs   } | Where-Object { $_ -ne $null })

  Write-Host "  timings (ms):"
  Write-Host ("    requestToAckMs : {0}" -f (Stats $r2a))
  Write-Host ("    ackToReplyMs   : {0}" -f (Stats $a2r))
  Write-Host ("    replyToAckMs   : {0}" -f (Stats $r2ak))
  Write-Host ("    gapToNextMs    : {0}" -f (Stats $gap))

  # Approximate wall clock: sum per-interaction (r2a+a2r+r2ak) + inter-interaction gap
  $wallMs = 0.0
  foreach ($it in $paired) {
    $t = $it.timing
    $v1 = if ($null -ne $t.requestToAckMs) { [double]$t.requestToAckMs } else { 0.0 }
    $v2 = if ($null -ne $t.ackToReplyMs  ) { [double]$t.ackToReplyMs   } else { 0.0 }
    $v3 = if ($null -ne $t.replyToAckMs  ) { [double]$t.replyToAckMs   } else { 0.0 }
    $v4 = if ($null -ne $t.gapToNextMs   ) { [double]$t.gapToNextMs    } else { 0.0 }
    $wallMs += $v1 + $v2 + $v3 + $v4
  }
  Write-Host ("  approx wall-clock (sum of per-interaction spans + inter-gaps): {0} ms  (~{1:N1} s)" -f [int]$wallMs, ($wallMs / 1000.0))
  Write-Host ""
}

$bowties = @($rows | Where-Object { $_.src -eq $BowtiesSrc })
$lccpro  = @($rows | Where-Object { $_.src -eq $LccProSrc  })

AnalyzeClient $bowties "Bowties ($BowtiesSrc)"
AnalyzeClient $lccpro  "LccPro  ($LccProSrc)"

# Aggregate throughput comparison: reads/second based on approximate wall clock
$totalOther = $rows.Count - $bowties.Count - $lccpro.Count
if ($totalOther -gt 0) {
  Write-Host "Note: $totalOther memory-config rows from other sources not analyzed."
}
