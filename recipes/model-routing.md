# Recipe: model-routing

Eval → affinity → single-model route (semi-deterministic).

```bash
porq model add fast --cli "echo FAST:{cmd}" --capability code
porq model add strong --cli "echo STRONG:{cmd}" --capability code
porq affinity set code.edit strong --score 0.85
porq affinity set code.edit fast --score 0.4
porq eval show --name edit -- "implement refactor in src"
porq run --sync --class code.edit --strategy single --policy sticky --seed 42 --name edit -- "implement fix"
porq affinity ls --class code.edit --json
```

Same seed + epoch ⇒ same model pick. `porq affinity epoch bump` starts a new generation.
