param(
  [string]$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path,
  [string]$ApiBaseUrl = 'http://localhost:8080',
  [string]$Source = 'futures',
  [int]$MinSetupCount = 1,
  [int]$FamilyLimit = 5000,
  [int]$MaxSetups = 1000,
  [int]$MaxForwardBars = 2000,
  [int]$Concurrency = 4,
  [switch]$DebugBuild,
  [string]$LogDir = (Join-Path $RepoRoot 'logs')
)

$ErrorActionPreference = 'Stop'

$abcdDir = Join-Path $RepoRoot 'rust_sr\abcd'
$buildProfile = if ($DebugBuild) { 'debug' } else { 'release' }
$optimizerExe = Join-Path $abcdDir "target\$buildProfile\run_phase1_optimizer.exe"
$timestamp = Get-Date -Format 'yyyyMMdd_HHmmss'
$summaryLog = Join-Path $LogDir "phase1-all-$timestamp.log"
$currentFile = Join-Path $LogDir 'phase1-all-current.txt'
$familyLogDir = Join-Path $LogDir "phase1-family-runs-$timestamp"
$Concurrency = [Math]::Max(1, [Math]::Min($Concurrency, 8))

New-Item -ItemType Directory -Force -Path $LogDir | Out-Null
New-Item -ItemType Directory -Force -Path $familyLogDir | Out-Null

function Write-Summary {
  param([string]$Message)
  $line = "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') $Message"
  Add-Content -Path $summaryLog -Value $line
  Set-Content -Path $currentFile -Value $line
  Write-Host $line
}

if (-not (Test-Path $optimizerExe)) {
  Write-Summary "$buildProfile optimizer exe missing; building run_phase1_optimizer"
  Push-Location $abcdDir
  if ($DebugBuild) {
    cargo build --bin run_phase1_optimizer 2>&1 | Tee-Object -FilePath $summaryLog -Append
  } else {
    cargo build --release --bin run_phase1_optimizer 2>&1 | Tee-Object -FilePath $summaryLog -Append
  }
  Pop-Location
}

$body = @{
  source_scope = $Source
  year = $null
  limit = $FamilyLimit
  min_setup_count = $MinSetupCount
} | ConvertTo-Json

$response = Invoke-RestMethod `
  -Uri "$ApiBaseUrl/pattern-families" `
  -Method Post `
  -ContentType 'application/json' `
  -Body $body

$families = @($response | ForEach-Object { $_ } | Where-Object { $_.family_key })
$total = $families.Count
if ($total -le 0) {
  Write-Summary "no families returned for source=$Source min_setup_count=$MinSetupCount"
  exit 1
}

Write-Summary "started Phase 1 all-family sweep: families=$total source=$Source max_setups=$MaxSetups max_forward_bars=$MaxForwardBars concurrency=$Concurrency build=$buildProfile"

$started = Get-Date
$completed = 0
$failed = 0
$nextIndex = 0
$active = @()

function Start-FamilyRun {
  param(
    [int]$Index,
    [object]$Family
  )

  $familyKey = [string]$Family.family_key
  $setupCount = [int]$Family.setup_count
  $ordinal = $Index + 1
  $stdoutLog = Join-Path $familyLogDir ("{0:D4}-{1}.out.log" -f $ordinal, $familyKey)
  $stderrLog = Join-Path $familyLogDir ("{0:D4}-{1}.err.log" -f $ordinal, $familyKey)
  $args = @(
    '--family', $familyKey,
    '--source', $Source,
    '--max-setups', [string]$MaxSetups,
    '--max-forward-bars', [string]$MaxForwardBars
  )

  Write-Summary "family $ordinal/$total start family=$familyKey setups=$setupCount"
  $process = Start-Process `
    -FilePath $optimizerExe `
    -ArgumentList $args `
    -WorkingDirectory $abcdDir `
    -WindowStyle Hidden `
    -RedirectStandardOutput $stdoutLog `
    -RedirectStandardError $stderrLog `
    -PassThru

  [pscustomobject]@{
    Index = $Index
    FamilyKey = $familyKey
    SetupCount = $setupCount
    StartedAt = Get-Date
    Process = $process
    StdoutLog = $stdoutLog
    StderrLog = $stderrLog
  }
}

Push-Location $abcdDir
try {
  while ($nextIndex -lt $total -or $active.Count -gt 0) {
    while ($nextIndex -lt $total -and $active.Count -lt $Concurrency) {
      $active += Start-FamilyRun -Index $nextIndex -Family $families[$nextIndex]
      $nextIndex += 1
    }

    Start-Sleep -Seconds 2
    $stillActive = @()

    foreach ($run in $active) {
      $run.Process.Refresh()
      if (-not $run.Process.HasExited) {
        $stillActive += $run
        continue
      }

      $run.Process.WaitForExit()
      $exitCode = $run.Process.ExitCode
      if ($null -eq $exitCode -and (Select-String -Path $run.StdoutLog -Pattern 'Stored .* Phase 1 route results' -Quiet)) {
        $exitCode = 0
      }
      $familyElapsed = (Get-Date) - $run.StartedAt
      if ($exitCode -eq 0) {
        $completed += 1
        $status = 'done'
      } else {
        $failed += 1
        $status = "failed exit=$exitCode"
      }

      $done = $completed + $failed
      $elapsed = (Get-Date) - $started
      $avgSeconds = [Math]::Max(1.0, $elapsed.TotalSeconds / [Math]::Max(1, $done))
      $remainingSeconds = [Math]::Round($avgSeconds * ($total - $done))
      $eta = (Get-Date).AddSeconds($remainingSeconds)

      Write-Summary (
        "family $done/$total $status family=$($run.FamilyKey) elapsed_family={0:n1}s completed=$completed failed=$failed active=$($stillActive.Count) eta={1}" -f `
          $familyElapsed.TotalSeconds, `
          $eta.ToString('yyyy-MM-dd HH:mm:ss')
      )
    }

    $active = $stillActive
  }
}
finally {
  Pop-Location
}

$totalElapsed = (Get-Date) - $started
Write-Summary ("finished Phase 1 all-family sweep: completed=$completed failed=$failed elapsed={0:n1}m" -f $totalElapsed.TotalMinutes)
