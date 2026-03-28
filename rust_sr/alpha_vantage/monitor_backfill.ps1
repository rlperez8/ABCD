param(
    [string]$Workdir = (Split-Path -Parent $MyInvocation.MyCommand.Path),
    [string]$StdoutLog = "backfill_half_stdout.log",
    [string]$StderrLog = "backfill_half_stderr.log",
    [string]$StatusFile = "backfill_half_status.txt",
    [int]$IntervalSeconds = 15
)

$stdoutPath = Join-Path $Workdir $StdoutLog
$stderrPath = Join-Path $Workdir $StderrLog
$statusPath = Join-Path $Workdir $StatusFile

while ($true) {
    $timestamp = Get-Date -Format "yyyy-MM-dd HH:mm:ss"
    $process = Get-Process -Name "alpha_vantage" -ErrorAction SilentlyContinue | Select-Object -First 1
    $isRunning = $null -ne $process

    $batchLine = $null
    $insertedCount = 0
    $buggedCount = 0

    if (Test-Path $stdoutPath) {
        $batchMatches = Select-String -Path $stdoutPath -Pattern '^\[batch (\d+)/(\d+) \| global (\d+)/(\d+)\] Symbol (.+)$'
        if ($batchMatches.Count -gt 0) {
            $batchLine = $batchMatches[-1]
        }

        $insertedCount = (Select-String -Path $stdoutPath -Pattern '^Inserted ' | Measure-Object).Count
    }

    if (Test-Path $stderrPath) {
        $buggedCount = (
            Select-String -Path $stderrPath -Pattern 'Insufficient history|Permanent API error|No candles returned' |
            Measure-Object
        ).Count
    }

    $summaryLines = @()
    $summaryLines += "Updated: $timestamp"
    $summaryLines += "Process running: $isRunning"

    if ($isRunning) {
        $summaryLines += "PID: $($process.Id)"
        $summaryLines += "CPU seconds: $([math]::Round($process.CPU, 2))"
        $summaryLines += "Started: $($process.StartTime.ToString('yyyy-MM-dd HH:mm:ss'))"
    }

    if ($null -ne $batchLine) {
        $batchCurrent = [int]$batchLine.Matches[0].Groups[1].Value
        $batchTotal = [int]$batchLine.Matches[0].Groups[2].Value
        $globalCurrent = [int]$batchLine.Matches[0].Groups[3].Value
        $globalTotal = [int]$batchLine.Matches[0].Groups[4].Value
        $symbol = $batchLine.Matches[0].Groups[5].Value
        $remaining = $batchTotal - $batchCurrent

        $summaryLines += "Current symbol: $symbol"
        $summaryLines += "Batch progress: $batchCurrent / $batchTotal"
        $summaryLines += "Batch remaining: $remaining"
        $summaryLines += "Global progress: $globalCurrent / $globalTotal"
    } else {
        $summaryLines += "Current symbol: unknown"
        $summaryLines += "Batch progress: unknown"
        $summaryLines += "Batch remaining: unknown"
        $summaryLines += "Global progress: unknown"
    }

    $summaryLines += "Inserted symbols: $insertedCount"
    $summaryLines += "Bugged or failed symbols: $buggedCount"

    if (Test-Path $stdoutPath) {
        $summaryLines += ""
        $summaryLines += "Recent stdout:"
        $summaryLines += Get-Content $stdoutPath -Tail 8
    }

    if (Test-Path $stderrPath) {
        $summaryLines += ""
        $summaryLines += "Recent stderr:"
        $summaryLines += Get-Content $stderrPath -Tail 8
    }

    Set-Content -Path $statusPath -Value $summaryLines
    Start-Sleep -Seconds $IntervalSeconds
}
