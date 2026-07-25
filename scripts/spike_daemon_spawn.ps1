# Daemon → trigger → child spawn spike
#
# Proves: porq run --sync → task.done → spawn child reaches terminal status.
# Exit 0 = trigger-spawn path is usable for G5; non-zero = fall back to
# explicit supervised `porq run --sync review-*` (plan G1 no-go fallback).

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $PSScriptRoot
$Porq = Join-Path $Root "target\release\porq.exe"
if (-not (Test-Path $Porq)) {
    $Porq = Join-Path $Root "target\debug\porq.exe"
}
if (-not (Test-Path $Porq)) {
    Write-Error "porq binary not found; build with cargo build --release -p orq"
    exit 2
}

$Data = Join-Path $env:TEMP ("porq-spike-" + [guid]::NewGuid().ToString("n"))
New-Item -ItemType Directory -Path $Data | Out-Null
$env:ORQ_DATA_DIR = $Data
$Ws = "spike-spawn"

try {
    & $Porq init --workspace $Ws | Out-Null
    & $Porq --workspace $Ws daemon run | Out-Null
    Start-Sleep -Seconds 1

    $marker = Join-Path $Data "child-ok.txt"
    $childCmd = "powershell -NoProfile -Command `"Set-Content -Path '$marker' -Value ok`""
    & $Porq --workspace $Ws trigger add spike-child `
        --on task.done `
        --where-cond "name^=spike-parent-" `
        --do-action "spawn:porq run --sync --name spike-child-{id} -- $childCmd" `
        --max-fires-per-hour 60 | Out-Null

    & $Porq --workspace $Ws run --sync --name spike-parent-1 -- "powershell -NoProfile -Command `"exit 0`"" | Out-Null

    $deadline = (Get-Date).AddSeconds(45)
    $childSeen = $false
    while ((Get-Date) -lt $deadline) {
        if (Test-Path $marker) {
            $childSeen = $true
            break
        }
        $tasks = & $Porq --workspace $Ws status --json --limit 20 | ConvertFrom-Json
        $child = @($tasks.tasks) | Where-Object { $_.name -like "spike-child-*" }
        if ($child -and ($child.status -in @("succeeded", "failed", "cancelled", "timed_out"))) {
            $childSeen = $true
            break
        }
        Start-Sleep -Seconds 1
    }

    if (-not $childSeen) {
        Write-Host "SPIKE FAIL: triggered child did not reach terminal status within timeout"
        Write-Host "FALLBACK: use explicit supervised porq run --sync for review agents"
        exit 1
    }
    Write-Host "SPIKE OK: daemon trigger spawn reached terminal / marker"
    exit 0
}
finally {
    try { & $Porq --workspace $Ws daemon stop 2>$null | Out-Null } catch {}
}
