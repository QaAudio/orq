# Recipe: roadmap-sanity

Advisory review of human-owned roadmap proposals. Sanity never writes the
authoritative state file; it only publishes findings. Apply remains an
out-of-band human approval + deterministic harness step.

```bash
# Fire only on newly proposed rows; write findings to a separate table.
porq trigger add sanity-on-proposal \
  --on poi.changed \
  --where-cond "table==roadmap-proposals && state==proposed" \
  --do-action "spawn:./scripts/roadmap_sanity_agent.sh {key}" \
  --max-fires-per-hour 20

porq canvas set loop-sanity --md ./sanity-report.md --order 5
```

Do **not** let the sanity agent flip `state==approved`. Approval is a human
receipt outside the agent session; porq POIs only display status.
