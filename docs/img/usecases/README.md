# Use-case screenshots

Captured from the live `tdrs-loop` dashboard after the compact/professional polish:

| File | Shows |
|------|--------|
| `porq-demo-canvases.png` | **Canvases** — loop-health, roadmap HANDOFF, checks, review veto (default theme) |
| `porq-demo-details.png` | **Details** — pulse strip, loop ops, board/tasks |
| `porq-demo-dracula.png` | Seeded Canvases under **dracula** theme (from `npm run capture:readme`) |

Regen default consumer stills: serve `porq -w tdrs-loop dash serve --port 9847`, refresh mirrors via
`porq_loop_doctor.ps1` / `sync_roadmap_to_porq.py`, then capture at ~1440×900.

Regen theme / UI-scale gallery + dracula still: `cd web && npm run capture:readme`
(see [`../gallery/`](../gallery/)).
