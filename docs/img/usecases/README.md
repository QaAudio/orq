# Use-case screenshots

Captured from the live `tdrs-loop` dashboard (Vue + Ableton Extension SDK look):

| File | Shows |
|------|--------|
| `porq-demo-canvases.png` | **Canvases** — loop-health, roadmap, checks, review (dark theme) |
| `porq-demo-details.png` | **Details** — pulse strip, docked board/tasks |

Regen consumer stills: serve `porq -w tdrs-loop dash serve --port 9847`, refresh mirrors via
`porq_loop_doctor.ps1` / `sync_roadmap_to_porq.py`, then capture at ~1440×900.

Feature-showcase / theme / UI-scale gallery (not consumer): `cd web && npm run capture:readme`
(see [`../gallery/`](../gallery/) and `docs/img/dashboard.png`).
