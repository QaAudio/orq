$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
Set-Location $Root

Write-Host "== build =="
cargo build -q
$Orq = Join-Path $Root "target\debug\orq.exe"
if (-not (Test-Path $Orq)) { throw "orq binary missing" }

$Data = Join-Path $env:TEMP ("orq-smoke-" + [guid]::NewGuid().ToString("n"))
New-Item -ItemType Directory -Path $Data | Out-Null
$env:ORQ_DATA_DIR = $Data
$env:ORQ_WORKSPACE = "default"

function Invoke-Orq {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$OrqArgs)
    & $Orq @OrqArgs
    if ($LASTEXITCODE -ne 0) { throw "orq failed: $($OrqArgs -join ' ')" }
}

Write-Host "== workspace / events =="
Invoke-Orq init --json | Out-Null
Invoke-Orq events --json --limit 5 | Out-Null

Write-Host "== poi =="
Invoke-Orq poi table create health --cols status:string:poi --json | Out-Null
Invoke-Orq poi set health system '"ok"' --state ok --tier ephemeral --json | Out-Null
Invoke-Orq poi set health system '"ok2"' --if-version 1 --json | Out-Null
try {
    Invoke-Orq poi set health system '"stale"' --if-version 1 --json | Out-Null
    throw "CAS should have failed"
} catch {
    Write-Host "CAS conflict ok"
}

Write-Host "== sync task =="
& $Orq run --sync --name hi --json -- echo smoke-ok | Out-Null
if ($LASTEXITCODE -ne 0) { throw "sync task failed" }

Write-Host "== trigger remediation =="
& $Orq run --sync --name worker --json -- echo worker | Out-Null
if ($LASTEXITCODE -ne 0) { throw "worker task failed" }
Invoke-Orq trigger add on-broken --on poi.changed --where-cond "state==broken" --do-action "cancel:name:worker" --json | Out-Null
Invoke-Orq trigger add remediate --on task.cancelled --do-action "spawn:echo remediate" --json | Out-Null

Write-Host "== report / snapshot / gc =="
Invoke-Orq report --md | Out-Null
Invoke-Orq snapshot --json | Out-Null
Invoke-Orq gc --json | Out-Null

Write-Host "== integrate cursor (temp host) =="
$HostDir = Join-Path $Data "host"
New-Item -ItemType Directory -Path $HostDir | Out-Null
Invoke-Orq integrate cursor --path $HostDir --json | Out-Null
if (-not (Test-Path (Join-Path $HostDir ".cursor\skills\orq\SKILL.md"))) {
    throw "skill not written"
}

Write-Host "== model routing / moa =="
Invoke-Orq model add p1 --cli "echo P1:{cmd}" --capability code --json | Out-Null
Invoke-Orq model add p2 --cli "echo P2:{cmd}" --capability code --json | Out-Null
Invoke-Orq model add synth --cli "echo AGG:{cmd}" --capability code --json | Out-Null
Invoke-Orq affinity set code.edit p1 --score 0.8 --json | Out-Null
Invoke-Orq affinity set code.edit p2 --score 0.6 --json | Out-Null
& $Orq eval show --name edit --json -- implement code refactor | Out-Null
if ($LASTEXITCODE -ne 0) { throw "eval show failed" }
& $Orq run --sync --class code.edit --strategy single --seed 42 --name route1 --json -- echo routed | Out-Null
if ($LASTEXITCODE -ne 0) { throw "single route failed" }
& $Orq run --sync --class code.edit --strategy moa --moa-k 2 --moa-aggregator synth --seed 7 --name moa1 --json -- echo moa | Out-Null
if ($LASTEXITCODE -ne 0) { throw "moa failed" }
& $Orq job list --json | Out-Null
if ($LASTEXITCODE -ne 0) { throw "job list failed" }

Write-Host "== recipes presence =="
@(
    "linear-sync.md",
    "central-committer.md",
    "review-gate.md",
    "preland-gate.md",
    "queue-drain.md",
    "model-routing.md",
    "moa-merge.md"
) | ForEach-Object {
    if (-not (Test-Path (Join-Path $Root "recipes\$_"))) { throw "missing recipe $_" }
}

Write-Host "== dash snapshot =="
Invoke-Orq dash snapshot --json | Out-Null
$DashData = Join-Path $Data "dash\data.json"
if (-not (Test-Path $DashData)) { throw "dash snapshot missing at $DashData" }

Write-Host "== dashboard playwright e2e =="
$Web = Join-Path $Root "web"
$Node = Get-Command node -ErrorAction SilentlyContinue
if (-not $Node) {
    throw "node not found. Install Node.js, then: cd web; npm ci; npx playwright install chromium"
}
if (-not (Test-Path (Join-Path $Web "node_modules\@playwright\test"))) {
    throw "Playwright not installed. Run: cd web; npm ci; npx playwright install chromium"
}
Push-Location $Web
try {
    $env:ORQ_BIN = $Orq
    npm run test:e2e
    if ($LASTEXITCODE -ne 0) { throw "dashboard e2e failed" }
} finally {
    Pop-Location
}

Write-Host "SMOKE OK data=$Data"
Remove-Item -Recurse -Force $Data -ErrorAction SilentlyContinue
