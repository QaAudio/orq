# orq recipes

Small, runnable patterns. Point `ORQ_DATA_DIR` at a temp folder, smash buttons,
steal the plot for your own agents.

| Recipe | What it demonstrates |
|--------|----------------------|
| [`linear-sync`](linear-sync.md) | External tracker ↔ local POIs via CAS + a poller service |
| [`central-committer`](central-committer.md) | One write lease owns `git commit`; everyone else queues |
| [`review-gate`](review-gate.md) | Human flips a blocked POI → task continues |
| [`preland-gate`](preland-gate.md) | Parallel verifies; blocking trigger vetoes land |
| [`queue-drain`](queue-drain.md) | Many workers drain; single-flight apply |
| [`model-routing`](model-routing.md) | Affinity picks a model for a class |
| [`moa-merge`](moa-merge.md) | Propose in parallel, aggregate like Mixture-of-Agents |

Full kitchen-sink check: `../scripts/smoke.ps1` (or `../scripts/smoke.sh`).

Each recipe is documentation first — copy the CLI sequence into your own
scripts. Prefer `--json` when an agent is reading the output.
