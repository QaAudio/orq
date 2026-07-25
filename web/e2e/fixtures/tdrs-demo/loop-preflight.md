# Loop Preflight

**Gate:** `none` | **Unit:** n/a | **Session:** n/a | **Iteration:** n/a
**State:** HANDOFF | **Generated:** 2026-07-25T12:00:00Z

**Verdict:** `not-ready` (exit 1) — profile `default`

| Field | Value |
|---|---|
| Autonomy | `blocked` |
| Pending | `0` |
| Batch | `5` |
| Batches Needed | `0` |
| Outer Max | `20` |
| Budget Math | `ok` |

With Active=`none`, **not-ready** is the expected, correct verdict — not a bug.

**Next human command:** author the next gate in `docs/roadmap/STATE.json`, then re-run preflight (see loop-roadmap).

_Demo fixture — program-stop / HANDOFF story._
