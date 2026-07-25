---
name: porq
description: Progressive multi-agent orchestration via the porq CLI — workspaces, POIs, canvases, supervised tasks, triggers, model affinities, and MoA merge jobs. Use when coordinating agents, locking paths/state, publishing dashboard canvases, routing models, or multi-model reconciliation.
---

# porq — progressive orchestration

## When to use
- Multi-agent workflows that need shared lockable state (POIs)
- Path claims to avoid edit conflicts
- Dashboard canvases (markdown / image / url / html) as the primary shared view
- Triggers that cancel/spawn remediation tasks
- Model routing / affinities / Mixture-of-Agents merge
- Ephemeral session work (prefer tmp tier + `--session`)

## Cheap surfaces (SoT — token discipline)
Prefer the cheapest surface that answers the claim. Never dump unbounded logs or full JSON tables.

1. `porq status --json --limit 20` — task board snapshot
2. `porq poi ls <table> --json --limit 20` — POI table
3. `porq canvas ls --json` — dashboard canvases
4. `porq events --json --limit 30` — what happened
5. `porq job report <id>` / `porq report --md` — timelines

Always pass `--limit` / `--fields` on list commands.

## Dashboard canvases
Publish status for operators with `porq canvas set` (Canvases is the primary view; Details holds ops tables).

Authoring rules (H1 + state word + next command + freshness; tables over dumps; inherit theme tokens): see porq `docs/canvas-authoring.md` (shipped with the porq repo).

```bash
porq canvas set plan --md ./plan.md
porq canvas set shot --image ./out.png
porq dash serve --port 9847
# optional theme: --theme dracula | --theme-file ./my.css  (env ORQ_DASH_THEME*)
```

## Loop / Meta routing
When a host repo defines Loop Meta / roadmap / doctor flows, follow **that** host's cards and skills for vetoes and program order. Porq is the substrate (POIs, tasks, canvases, triggers) — do not invent host-specific gates from this skill alone.

## Zero-setup
First command auto-creates the `default` workspace. Daemon auto-spawns on supervised runs.
Prefer ephemeral:

```bash
porq --session "$ORQ_SESSION" poi table create notes --cols "body:string:poi"
porq --session "$ORQ_SESSION" run --sync --name work -- "echo hello"
porq --session "$ORQ_SESSION" gc
```

## Core primitives
- **Workspace** — sandbox (`--workspace`, default `default`)
- **POI** — lockable record in a table (`poi get/set/lock/steal`)
- **Canvas** — display POI in reserved `canvas` table (`canvas set/ls/rm`; kinds: markdown/image/url/html)
- **Task** — supervised agent/process (`run/await/cancel/kill`)
- **Trigger** — declarative rules (`trigger add` …)
- **Model / Affinity / Job** — eval → seeded route → single|race|moa

## Model routing + MoA
```bash
porq model add fast --cli "echo FAST:{cmd}" --capability code
porq model add strong --cli "echo STRONG:{cmd}" --capability code
porq affinity set code.edit strong --score 0.9
porq run --sync --class code.edit --strategy moa --moa-k 2 --moa-aggregator strong --seed 42 --name edit -- "propose fix"
porq job report <id>
```

## CAS + claims
```bash
porq poi set roadmap item-1 '{"title":"x"}' --if-version 3
porq run --claim "src/engine/**" --name edit -- "…"
```

## Recipes
See `recipes/`: linear-sync, central-committer, review-gate, preland-gate, queue-drain, model-routing, moa-merge, review-agent, roadmap-sanity.

Env vars still use the `ORQ_*` prefix (compat); binary is `porq`.
