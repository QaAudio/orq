# Use-case screenshots (capture after visual acceptance)

Placeholder directory for the [td-rs autonomous loop](../../usecases/td-rs-autonomous-loop.md)
showcase. **Do not invent fake binary screenshots** — drop real captures here
only after the dashboard + loop canvases look accepted in UI E2E.

## Capture list

| File (suggested) | What to show |
|------------------|--------------|
| `loop-canvases.png` | Canvases view: `loop-health`, `loop-roadmap` (program stop / `Active: none` OK), `loop-preflight` |
| `loop-details-pulse.png` or short GIF | Details tab: board/tasks/events pulse with a claimed `exec-*` task |
| `loop-checks.png` | `loop-checks` matrix (at least one green row; optional forced-red dry-run) |
| `loop-drift.png` | `loop-drift` / drift-report canvas with classification + next human command |
| `loop-proposals.png` | `loop-proposals` pending proposal + mirrored approval state (no real secrets/receipts) |
| `loop-freshness.gif` (optional) | Fresh → stale canvas age / status polish before/after |

Prefer regenerating via the dashboard capture path under `tools/orq/web` when
available; hand-frame only what the story needs. Captions in the use-case doc
should name the **porq feature** shown (canvas, lease, veto, POI), not td-rs
internals.
