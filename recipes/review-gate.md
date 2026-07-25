# Recipe: review-gate

Human-in-the-loop: task stays blocked until an `approved` POI is flipped.

```bash
porq poi table create review --cols approved:bool:poi
porq poi set review land '{"approved":false}' --state pending
porq trigger add wait-approve --on poi.changed --where-cond "key==land" --do-action 'spawn:echo approved-continue'
# human:
porq poi set review land '{"approved":true}' --state approved
```

Pair with `porq run --needs-poi review/land` so the worker will not start while blocked.
