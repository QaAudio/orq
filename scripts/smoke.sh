#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"
cargo build -q
ORQ="$ROOT/target/debug/porq"
DATA="$(mktemp -d)"
export ORQ_DATA_DIR="$DATA"
export ORQ_WORKSPACE=default
"$ORQ" init --json >/dev/null
"$ORQ" poi table create health --cols status:string:poi --json >/dev/null
"$ORQ" poi set health system '"ok"' --state ok --json >/dev/null
"$ORQ" poi table create computer --cols purpose:string:poi --json >/dev/null
"$ORQ" poi lock computer focus --holder holder-a --reason smoke-a --ttl 60 --json >/dev/null
if "$ORQ" poi lock computer focus --holder holder-b --reason smoke-b --ttl 60 --json >/dev/null 2>&1; then
  echo "lock without --wait should fail when held" >&2
  exit 1
fi
"$ORQ" poi unlock computer focus --holder holder-a --json >/dev/null
"$ORQ" poi lock computer focus --holder holder-b --reason smoke-wait --ttl 60 --wait --timeout-ms 5000 --json >/dev/null
"$ORQ" poi unlock computer focus --holder holder-b --json >/dev/null
"$ORQ" run --sync --name hi --json -- "echo smoke-ok" >/dev/null
"$ORQ" report --md >/dev/null
"$ORQ" integrate cursor --path "$DATA/host" --json >/dev/null
"$ORQ" dash snapshot --json >/dev/null
test -f "$DATA/dash/data.json"

WEB="$ROOT/web"
if ! command -v node >/dev/null 2>&1; then
  echo "node not found — install Node.js, then: cd web && npm ci && npx playwright install chromium" >&2
  exit 1
fi
if [[ ! -d "$WEB/node_modules/@playwright/test" ]]; then
  echo "Playwright not installed — run: cd web && npm ci && npx playwright install chromium" >&2
  exit 1
fi
(
  cd "$WEB"
  export ORQ_BIN="$ORQ"
  npm run test:e2e
)

echo "SMOKE OK"
rm -rf "$DATA"
