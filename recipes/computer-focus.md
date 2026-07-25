# Recipe: computer-focus

Serialize OS screenshot / foreground-stealing work across agents (and humans)
via a Write lease on `computer/focus` in workspace `tdrs-loop`. Soft collision
rails alone are not enough — hold the lease before `td_ui` focus/capture/batch.

## Setup (once)

```bash
# Prefer porq_loop_bootstrap.ps1 — it creates the table + idle row + canvas.
porq -w tdrs-loop poi table create computer \
  --cols purpose:string:poi --cols holder_kind:string --cols session:string \
  --cols yield_requested:bool --cols yield_by:string --cols note:string
porq -w tdrs-loop poi set computer focus \
  '{"v":1,"purpose":"","holder_kind":"","session":"","yield_requested":false,"yield_by":null,"note":""}' \
  --state idle
```

## Acquire → capture → release

```bash
HOLDER="agent-$(hostname)-$$"
PURPOSE="td_ui scrape session"

porq -w tdrs-loop poi lock computer focus \
  --holder "$HOLDER" --reason "$PURPOSE" --ttl 300 \
  --wait --timeout-ms 120000

porq -w tdrs-loop poi set computer focus \
  "{\"v\":1,\"purpose\":\"$PURPOSE\",\"holder_kind\":\"agent\",\"session\":\"$HOLDER\",\"yield_requested\":false,\"yield_by\":null,\"note\":\"\"}" \
  --state held

porq -w tdrs-loop canvas set computer-focus --md ./computer-focus-held.md

# … td_ui focus / capture / batch …

# Honor yield: if value.yield_requested is true, unlock ASAP after this batch.
porq -w tdrs-loop poi set computer focus \
  '{"v":1,"purpose":"","holder_kind":"","session":"","yield_requested":false,"yield_by":null,"note":""}' \
  --state idle
porq -w tdrs-loop poi unlock computer focus --holder "$HOLDER"
porq -w tdrs-loop canvas set computer-focus --md ./computer-focus-idle.md
```

## Human wait (CLI, Wave 1)

```bash
porq -w tdrs-loop poi lock computer focus \
  --holder user --reason "HITL desktop" --ttl 600 \
  --wait --timeout-ms 300000
```

Request yield without stealing (agents must check POI value):

```bash
porq -w tdrs-loop poi set computer focus \
  '{"v":1,"purpose":"…","holder_kind":"agent","session":"…","yield_requested":true,"yield_by":"user","note":"please release"}' \
  --state held
```

Emergency recovery only: `porq -w tdrs-loop poi steal computer focus --holder user --reason recovery`.

## Dashboard

Canvas slot `computer-focus` is the status view. The dash Canvases panel exposes
**Take ownership (wait)** / **Request yield** / **Release** / **Steal** via
`POST /api/v1/poi/{lock,unlock,yield-request,steal}` (scoped to `computer/focus`).
CLI remains the full control plane.
