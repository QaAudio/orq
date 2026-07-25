# orq recipes

Executable documentation. Each recipe is a shell/PowerShell script that exercises a use case via the `orq` CLI. Prefer `ORQ_DATA_DIR` pointing at a temp directory.

| Recipe | Purpose |
|--------|---------|
| `model-routing` | Eval → affinity → single route |
| `moa-merge` | Parallel proposers + aggregator (Together-style) |

Run all: `../scripts/smoke.ps1`
