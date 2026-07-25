# Recipe: review-agent

Spawn a read-only review task after successful `exec-*` tasks. Use a blocking
`task.pre-exec` hook on `land-*` to fail closed if the review POI is missing/red.

Provider-neutral: the review command should be a launch-profile wrapper that
reads `ORQ_EVENT_*` / `{id}` / `{name}` and writes a schema-valid result POI.

```bash
porq trigger add review-on-exec \
  --on task.done \
  --where-cond "name^=exec-" \
  --do-action "spawn:porq run --sync --name review-{name} -- ./scripts/review_agent.sh {id}" \
  --max-fires-per-hour 30

porq trigger add veto-land-without-review \
  --on task.pre-exec \
  --where-cond "name^=land-" \
  --blocking \
  --do-action "hook:./scripts/require_review_green.sh {name}"
```

The review wrapper must be read-only (no git writes). Land stays blocked until
the review POI is green.
