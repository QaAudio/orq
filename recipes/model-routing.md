# Recipe: model-routing

Eval → affinity → single-model route (semi-deterministic).

```bash
orq model add fast --cli "echo FAST:{cmd}" --capability code
orq model add strong --cli "echo STRONG:{cmd}" --capability code
orq affinity set code.edit strong --score 0.85
orq affinity set code.edit fast --score 0.4
orq eval show --name edit -- "implement refactor in src"
orq run --sync --class code.edit --strategy single --policy sticky --seed 42 --name edit -- "implement fix"
orq affinity ls --class code.edit --json
```

Same seed + epoch ⇒ same model pick. `orq affinity epoch bump` starts a new generation.
