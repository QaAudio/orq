# orq — multi-agent task orchestration CLI

Local-first Rust CLI for coordinating multi-agent workspaces with:

- **Workspaces** (lazy `default`, optional `--session` ephemeral scope)
- **Points of interest (POIs)** — lockable, versioned state cells in mixed tables (incl. `paths`)
- **Tasks** — supervised oneshot/service processes with await/retry/cancel/kill/interrupt and path **claims**
- **Triggers** — declarative rules with blocking hooks, cascade guards, and budgets
- **Models / affinities / jobs** — seeded routing (`single|race|moa`) with EMA learning
- **Visualization** — `--json`, ASCII tables, `report --md|--html`, `watch` TUI, **HTTP dashboard** (`orq dash serve`)

Inspired by (but independent from) td-rs Loop Meta and prior art: Pueue, Temporal, Restate, Taskwarrior, Hatchet.

## Build

```bash
cargo build --release
# binary: target/release/orq
```

Optional data dir: `ORQ_DATA_DIR` (default: platform local data dir `/orq`).

## Quick start

```bash
orq status --json --limit 20
orq poi table create notes --cols body:string:poi
orq poi set notes hello '"world"' --tier ephemeral
orq run --sync --name hi -- "echo hello from orq"
orq report --md
```

## Agent adoption

```bash
orq integrate cursor --path /path/to/host/repo
```

Emits `.cursor/skills/orq/SKILL.md` and an `AGENTS.md` snippet.

## Recipes

See [`recipes/`](recipes/) for executable patterns:

| Recipe | Purpose |
|--------|---------|
| `linear-sync` | Bidirectional roadmap via CAS + poller service task |
| `central-committer` | Serialized git commits via write lease |
| `review-gate` | Human approval POI unblocks a task |
| `preland-gate` | Fan-out verify + blocking veto |
| `queue-drain` | Parallel drain + single-flight apply |
| `model-routing` | Affinity-based single-model route |
| `moa-merge` | Mixture-of-Agents propose + aggregate |

Run smoke tests: `scripts/smoke.ps1` (or `scripts/smoke.sh`). Smoke also runs the dashboard Playwright gate when Node + Playwright deps are present under `web/`.

## Live dashboard

First-class UI lives in [`web/dashboard/`](web/dashboard/) and is served over localhost HTTP (not `file://`).

```bash
orq dash snapshot                 # atomic JSON → $ORQ_DATA_DIR/dash/data.json
orq dash serve --port 9847        # serves web/dashboard + /data.json (refreshes every 1s)
# open http://127.0.0.1:9847/
```

Override static root with `--root` or `ORQ_DASH_ROOT`. The old `%TEMP%/orq-live-*/dashboard.html` demo path is obsolete — do not use it as source of truth.

### Dashboard E2E (no LLM)

```bash
cargo build -p orq
cd web
npm ci
npx playwright install chromium
npm run test:e2e
```

Seed data is CLI-only (`echo` stubs): board POIs, affinities, a routed task, and one MoA job.

## Model routing quickstart

```bash
orq model add fast --cli "echo FAST:{cmd}" --capability code
orq model add strong --cli "echo STRONG:{cmd}" --capability code
orq affinity set code.edit strong --score 0.9
orq run --sync --class code.edit --strategy moa --moa-k 2 --moa-aggregator strong --seed 42 --name edit -- "propose"
orq job report <id>
```

## Storage backends

The store has two backends behind one API (`orq-core/src/db.rs`):

| Mode | Backend | Selected by |
|---|---|---|
| Local (default) | SQLite file `<data_dir>/orq.db` via rusqlite (WAL, busy_timeout) | nothing to do |
| Remote | Turso / any libSQL server over HTTP (hrana) via `libsql` | `ORQ_DB_URL` (+ `ORQ_DB_TOKEN`) |

Remote is **opt-in**: pass `--remote` (loads a `.env` found from the current directory
up) or export `ORQ_DB_URL` yourself. Without either, everything stays local — a stray
`.env` never silently redirects local runs to the cloud.

```bash
# .env (gitignored) next to the repo or exported in the shell:
#   ORQ_DB_URL=libsql://<db>.turso.io
#   ORQ_DB_TOKEN=<token>
orq --remote init
orq --remote poi set board hello '{"msg":"shared"}'
orq --remote workspace drop old-ws --yes   # cloud hygiene (destructive)
```

Notes:

- All multi-statement mutations (poi set + event, lease acquire/release, task/job
  insert, affinity updates, `workspace drop`) run inside a single transaction
  (`BEGIN IMMEDIATE` locally, `BEGIN` on remote).
- The live E2E test (`cargo test -p orq-core --test turso_e2e`) reads `tools/orq/.env`
  and exercises CAS, leases and the event log against the real Turso instance; it
  self-skips when no credentials are present.
- Embedded-replica mode (local reads, synced writes) needs the libsql C core
  (`sync` feature + CMake) and is a planned follow-up.
- Lease expiry still compares against the client clock; with several writers, keep
  clocks NTP-synced (server-side time is a follow-up).

## Design notes

- Daemon-optional: `orq run --sync` needs no daemon; unsupervised `run` auto-spawns `orq daemon run` (TCP localhost + port file).
- Event log is append-only; every mutation emits an event.
- Locks are **leases** (TTL + holder); use `poi steal` for recovery.
- Claims (`--claim "src/**"`) acquire `paths` table write leases for the task lifetime.
