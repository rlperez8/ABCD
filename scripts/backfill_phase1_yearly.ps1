param(
  [string]$ApiBase = "http://127.0.0.1:8080",
  [string]$SourceScope = "futures",
  [int]$Limit = 75,
  [int]$MinTrades = 100,
  [string]$LogPath = "logs\phase1_yearly_backfill.log",
  [string]$ProgressPath = "logs\phase1_yearly_backfill.progress.json"
)

$ErrorActionPreference = "Stop"

function Write-ProgressFile {
  param(
    [string]$Status,
    [int]$Completed,
    [int]$Skipped,
    [int]$Failed,
    [int]$Total,
    [string]$CurrentFamily = "",
    [string]$CurrentRoute = "",
    [string]$Message = ""
  )

  [pscustomobject]@{
    status = $Status
    completed = $Completed
    skipped = $Skipped
    failed = $Failed
    total = $Total
    current_family = $CurrentFamily
    current_route = $CurrentRoute
    message = $Message
    updated_at = (Get-Date).ToString("o")
  } | ConvertTo-Json | Set-Content -LiteralPath $ProgressPath
}

New-Item -ItemType Directory -Force (Split-Path -Parent $LogPath) | Out-Null
New-Item -ItemType Directory -Force (Split-Path -Parent $ProgressPath) | Out-Null

$leaderboardBody = @{
  source_scope = $SourceScope
  limit = $Limit
  min_trade_count = $MinTrades
  min_setup_count = 1
  best_per_family = $true
} | ConvertTo-Json

$leaderboardResponse = Invoke-RestMethod -Uri "$ApiBase/phase1/leaderboard" -Method Post -Body $leaderboardBody -ContentType "application/json"
$routes = @($leaderboardResponse | ForEach-Object { $_ })
$completed = 0
$skipped = 0
$failed = 0
$total = $routes.Count

"[$(Get-Date -Format o)] Starting Phase 1 yearly backfill: source=$SourceScope limit=$Limit minTrades=$MinTrades total=$total" |
  Add-Content -LiteralPath $LogPath
Write-ProgressFile -Status "running" -Completed $completed -Skipped $skipped -Failed $failed -Total $total

foreach ($route in $routes) {
  $family = [string]$route.family_key
  $routeId = [string]$route.route_id
  $runId = [string]$route.run_id
  Write-ProgressFile -Status "running" -Completed $completed -Skipped $skipped -Failed $failed -Total $total -CurrentFamily $family -CurrentRoute $routeId

  try {
    $cacheBody = @{
      family_key = $family
      run_id = $runId
      route_id = $routeId
      cache_only = $true
    } | ConvertTo-Json
    $cached = Invoke-RestMethod -Uri "$ApiBase/phase1/yearly-breakdown" -Method Post -Body $cacheBody -ContentType "application/json"

    if (@($cached.years).Count -gt 0) {
      $skipped++
      "[$(Get-Date -Format o)] skip cached family=$family route=$routeId rows=$(@($cached.years).Count)" |
        Add-Content -LiteralPath $LogPath
      continue
    }

    $buildBody = @{
      family_key = $family
      run_id = $runId
      route_id = $routeId
      cache_only = $false
    } | ConvertTo-Json
    $started = Get-Date
    $built = Invoke-RestMethod -Uri "$ApiBase/phase1/yearly-breakdown" -Method Post -Body $buildBody -ContentType "application/json"
    $elapsed = [math]::Round(((Get-Date) - $started).TotalSeconds, 1)
    $completed++

    "[$(Get-Date -Format o)] built family=$family route=$routeId rows=$(@($built.years).Count) elapsed=${elapsed}s" |
      Add-Content -LiteralPath $LogPath
  } catch {
    $failed++
    "[$(Get-Date -Format o)] failed family=$family route=$routeId error=$($_.Exception.Message)" |
      Add-Content -LiteralPath $LogPath
  }
}

Write-ProgressFile -Status "done" -Completed $completed -Skipped $skipped -Failed $failed -Total $total -Message "Phase 1 yearly backfill complete"
"[$(Get-Date -Format o)] Done Phase 1 yearly backfill: completed=$completed skipped=$skipped failed=$failed total=$total" |
  Add-Content -LiteralPath $LogPath
