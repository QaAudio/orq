# Recipe: review-gate

Human-in-the-loop: task stays blocked until an `approved` POI is flipped.

```bash
orq poi table create review --cols approved:bool:poi
orq poi set review land '{"approved":false}' --state pending
orq trigger add wait-approve --on poi.changed --where-cond "key==land" --do-action 'spawn:echo approved-continue'
# human:
orq poi set review land '{"approved":true}' --state approved
```

Pair with `orq run --needs-poi review/land` so the worker will not start while blocked.
