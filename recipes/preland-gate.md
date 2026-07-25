# Recipe: preland-gate

Fan-out verify tasks; blocking hook vetoes land until green.

```bash
orq run --sync --name lint -- "echo lint-ok"
orq run --sync --name test -- "echo test-ok"
orq trigger add veto-land --on task.pre-exec --where-cond "name==land" --blocking \
  --do-action "hook:exit 1"
# after verifies pass, disable veto or replace hook with exit 0
orq trigger disable <id>
orq run --sync --name land -- "echo land"
```
