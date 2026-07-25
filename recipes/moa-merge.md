# Recipe: moa-merge

Together-style Mixture-of-Agents using stub CLI models (no API keys).

```bash
orq model add p1 --cli "echo proposer1:{cmd}" --capability code
orq model add p2 --cli "echo proposer2:{cmd}" --capability code
orq model add p3 --cli "echo proposer3:{cmd}" --capability code
orq model add synth --cli "echo AGGREGATE:{cmd}" --capability code
orq affinity set code.edit p1 --score 0.7
orq affinity set code.edit p2 --score 0.6
orq affinity set code.edit p3 --score 0.5
orq affinity set code.edit synth --score 0.8

orq run --sync --class code.edit --strategy moa --moa-k 3 --moa-layers 1 \
  --moa-aggregator synth --seed 7 --name moa-demo -- "design API"

orq job list --json
orq job report <job-id>
orq poi ls moa --json
```

Proposers write logs → POI `moa/{job}/layer0/{model}`; aggregator reconciles; affinities update from outcomes.
