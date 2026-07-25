# porq recipes

Runnable patterns. Point `ORQ_DATA_DIR` at a temp folder and follow the CLI
sequences (`porq …`). Prefer `--json` when an agent reads the output.

| Recipe | What it demonstrates |
|--------|----------------------|
| [`linear-sync`](linear-sync.md) | External tracker ↔ local POIs via CAS + a poller service |
| [`central-committer`](central-committer.md) | One write lease owns `git commit`; others queue |
| [`computer-focus`](computer-focus.md) | Exclusive desktop capture / focus lease (`computer/focus`) |
| [`review-gate`](review-gate.md) | Human flips a blocked POI → task continues |
| [`preland-gate`](preland-gate.md) | Parallel verifies; blocking trigger vetoes land |
| [`queue-drain`](queue-drain.md) | Many workers drain; single-flight apply |
| [`model-routing`](model-routing.md) | Affinity picks a model for a class |
| [`moa-merge`](moa-merge.md) | Propose in parallel, aggregate (Mixture-of-Agents) |
| [`review-agent`](review-agent.md) | Post-`exec-*` review + `land-*` pre-exec veto |
| [`roadmap-sanity`](roadmap-sanity.md) | Advisory proposal review (never writes SoT) |

Full check: `../scripts/smoke.ps1` (or `../scripts/smoke.sh`).
