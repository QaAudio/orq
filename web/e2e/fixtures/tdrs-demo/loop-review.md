# Loop Review

**Gate:** `none` | **Unit:** scratch-demo | **Session:** demo-static | **Iteration:** n/a
**State:** BLOCKED | **Generated:** 2026-07-25T12:00:00Z

**Verdict:** `reject` — land veto path demonstrated.

Review found a forced red on the scratch land unit so `land-*` never starts
when `veto-land` fires. Observe on the board; CLI / harness remains control.

**Next human command:** none — demo shows veto working; do not force-land.

| Field | Value |
|---|---|
| Task | `exec-demo-scratch` |
| Trigger | `veto-land` (blocking `task.pre-exec` on `land-*`) |
| Result | land blocked |

_Demo fixture — red review proving land-veto._
