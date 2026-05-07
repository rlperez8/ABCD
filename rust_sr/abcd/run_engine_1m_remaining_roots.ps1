$ErrorActionPreference = 'Stop'

$roots = @(
  'NQ',
  'YM',
  'RTY',
  'EMD',
  'NKD',
  '6A',
  '6B',
  '6C',
  '6E',
  '6J',
  '6N',
  '6S',
  'CL',
  'QM',
  'NG',
  'QG',
  'HO',
  'RB',
  'GC',
  'SI',
  'HG',
  'PL',
  'PA',
  'ZC',
  'ZW',
  'ZS',
  'ZM',
  'ZL',
  'HE',
  'LE',
  'GF',
  'ZB',
  'ZN'
)

$stamp = Get-Date -Format 'yyyyMMdd_HHmmss'
$batchLog = Join-Path $PSScriptRoot "engine-1m-remaining-$stamp.batch.log"
$currentFile = Join-Path $PSScriptRoot 'engine-1m-remaining-current.txt'

function Write-BatchLog {
  param([string]$Message)
  $line = "$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss') $Message"
  $line | Tee-Object -FilePath $batchLog -Append
}

Write-BatchLog "Starting 1m engine batch for $($roots.Count) roots."

foreach ($root in $roots) {
  $rootStamp = Get-Date -Format 'yyyyMMdd_HHmmss'
  $outLog = Join-Path $PSScriptRoot "engine-$root-1m-$rootStamp.out.log"

  @(
    "root=$root",
    "out_log=$outLog",
    "batch_log=$batchLog",
    "started_at=$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')"
  ) | Set-Content -Path $currentFile

  Write-BatchLog "Starting root $root. Log: $outLog"

  $env:ABCD_CANDLE_SOURCE = 'futures_contracts'
  $env:ABCD_FUTURES_ROOT = $root
  $env:ABCD_FUTURES_TIMEFRAME = '1m'
  $env:ABCD_SCAN_CONCURRENCY = '1'
  $env:ABCD_WRITE_BATCH_SIZE = '1000'
  $env:ABCD_PROGRESS_EVERY = '10000'
  $env:ABCD_SKIP_PROP_FAMILY_SUMMARIES = '1'
  $env:ABCD_REFRESH_PROP_FAMILY_SUMMARIES = '0'
  Remove-Item Env:\ABCD_SYMBOL_OFFSET -ErrorAction SilentlyContinue
  Remove-Item Env:\ABCD_SYMBOL_LIMIT -ErrorAction SilentlyContinue

  $previousErrorActionPreference = $ErrorActionPreference
  $ErrorActionPreference = 'Continue'
  cargo run --bin abcd *> $outLog
  $exitCode = $LASTEXITCODE
  $ErrorActionPreference = $previousErrorActionPreference
  Write-BatchLog "Finished root $root with exit code $exitCode."

  if ($exitCode -ne 0) {
    Write-BatchLog "Stopping batch after failure on root $root."
    exit $exitCode
  }

  Write-BatchLog "Refreshing storage summary after root $root."
  $previousErrorActionPreference = $ErrorActionPreference
  $ErrorActionPreference = 'Continue'
  cargo run --bin refresh_storage_summary *>> $batchLog
  $refreshExitCode = $LASTEXITCODE
  $ErrorActionPreference = $previousErrorActionPreference
  Write-BatchLog "Storage summary refresh after root $root exited with code $refreshExitCode."

  if ($refreshExitCode -ne 0) {
    Write-BatchLog "Stopping batch after storage refresh failure on root $root."
    exit $refreshExitCode
  }
}

@(
  "root=complete",
  "out_log=",
  "batch_log=$batchLog",
  "finished_at=$(Get-Date -Format 'yyyy-MM-dd HH:mm:ss')"
) | Set-Content -Path $currentFile

Write-BatchLog "Finished all 1m engine roots."
