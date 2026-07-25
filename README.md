<div align="center">
  <img src="docs/img/porq-icon.svg" alt="porq pig icon" width="96" height="96" />
  <h1>porq</h1>
  <p><strong>Progressive orchestration for multi-agent work.</strong></p>
  <p>Run tasks. Share state. Prevent collisions. Add only what you need.</p>

  <p>
    <a href="https://qaaudio.github.io/porq/demo/">Live demo</a> ·
    <a href="#quick-start">Quick start</a> ·
    <a href="recipes/">Recipes</a> ·
    <a href="docs/canvas-authoring.md">Canvas guide</a>
  </p>
  <p>
    <img src="https://img.shields.io/badge/Rust-2021-e8a838?style=flat-square" alt="Rust 2021" />
    <img src="https://img.shields.io/badge/storage-SQLite%20%7C%20libSQL-3db8a0?style=flat-square" alt="SQLite or libSQL storage" />
    <img src="https://img.shields.io/badge/license-MIT-f2ebe3?style=flat-square" alt="MIT license" />
  </p>
</div>

<br />

<p align="center">
  <a href="https://qaaudio.github.io/porq/demo/">
    <img src="docs/img/dashboard.gif" alt="porq dashboard switching between Canvases and Details" width="920" />
  </a>
</p>
<p align="center"><sub>Canvases for people; structured state for agents. <a href="https://qaaudio.github.io/porq/demo/">Open the observe-only demo →</a></sub></p>

## Why porq?

Multiple agents working in one repository need more than a task runner. They
need a shared view of progress, safe ownership of files and resources, and a
record of what happened.

porq provides that in one local-first binary:

- **Supervise** commands with logs, retries, cancellation, and timeouts.
- **Coordinate** through versioned POIs, leases, and path claims.
- **React** with guarded triggers and human approval gates.
- **Route** work across models with affinities, races, or Mixture-of-Agents.
- **Observe** everything in a live dashboard and append-only event log.

Start with `porq run`. The other layers remain optional.

## Quick start

Requires a recent Rust toolchain.

```bash
git clone https://github.com/QaAudio/porq.git
cd porq
cargo build --release

# Run and supervise a command inline
./target/release/porq run --sync --name tests -- "cargo test"

# Inspect the workspace
./target/release/porq status --json
```

`--sync` needs no daemon. Background runs start the local daemon automatically.
The first command creates a default workspace.

## Grow by layer

| Layer | Add | Gain |
|---|---|---|
| **Run** | `porq run` | Supervision, logs, retry, await, cancel, kill |
| **Remember** | POI tables | Versioned JSON cells, CAS, sessions, canvases |
| **Coordinate** | Leases + claims | Time-boxed ownership of records and paths |
| **React** | Triggers | Guarded automation, vetoes, remediation budgets |
| **Route** | Models + affinities | `single`, `race`, and `moa` strategies |
| **Share** | `--remote` | The same workflow on Turso/libSQL |

```text
workspace
├── POIs       versioned state + leases
├── tasks      commands + path claims
├── triggers   reactions + guardrails
├── jobs       model routing + outcomes
├── canvases   human-readable status
└── events     append-only history
```

## Common workflows

### Share state

POIs are versioned JSON cells. Use them for tickets, approvals, handoffs, or any
small piece of agent-readable state.

```bash
porq poi table create board --cols body:string:poi
porq poi set board T-12 '{"title":"Refactor parser"}' --state todo
porq poi get board T-12 --json
```

Every write increments a version. Pass `--if-version` for compare-and-swap.

### Prevent edit collisions

```bash
porq poi lock board T-12 --holder agent-a --ttl 300
porq run --sync --claim "src/parser/**" --name refactor -- "cargo test"
```

Locks are expiring leases. Path claims use the same mechanism and last for the
task's lifetime.

### Publish a canvas

```bash
porq canvas set plan --md ./plan.md
porq canvas set render --image ./frame.png --title "Latest render"
porq dash serve --port 9847
# http://127.0.0.1:9847/
```

Canvases support markdown (including Mermaid), images, URLs, and sandboxed HTML.
See the [authoring guide](docs/canvas-authoring.md).

### Integrate with Cursor

```bash
porq integrate cursor --path /path/to/your/repo
```

This installs a porq skill and workspace guidance so agents coordinate through
POIs and claims instead of ad-hoc lock files.

### Route across models

```bash
porq model add fast --cli "echo FAST:{cmd}" --capability code
porq model add strong --cli "echo STRONG:{cmd}" --capability code
porq affinity set code.edit strong --score 0.9
porq run --sync --class code.edit --strategy moa --moa-k 2 --name edit -- "propose"
```

Affinities learn from outcomes. `race` takes the first successful result;
`moa` gathers proposals and sends them to an aggregator.

## Dashboard

The Vue dashboard has two focused views:

- **Canvases** — status, plans, diagrams, images, and operator controls.
- **Details** — tasks, POIs, leases, triggers, models, jobs, and events.

Both views share a Grafana-style **12-column reactive grid**: drag a panel by
its title to move it, resize from the edges, and neighbors push/compact so
tiles never overlap. Layout prefs stay browser-local (`porq.dash.layout.canvases`
/ `porq.dash.layout.details`). Dark and light themes and archived filters are
supported. Local mutation is deliberately narrow; roadmap and git actions remain
CLI-only.

```bash
porq dash snapshot
porq dash serve --port 9847
```

[Try the live demo](https://qaaudio.github.io/porq/demo/) or read the
[td-rs autonomous loop case study](docs/usecases/td-rs-autonomous-loop.md).

## Remote storage

Local SQLite is the default. To share a workspace across machines, configure
Turso or another libSQL server and opt in with `--remote`.

```bash
# .env — see .env.example
ORQ_DB_URL=libsql://YOUR_DB.turso.io
ORQ_DB_TOKEN=YOUR_TOKEN

porq --remote init
porq --remote poi set board hello '{"msg":"shared"}'
```

A present `.env` never enables remote mode by itself.

## Recipes

Copyable patterns live in [`recipes/`](recipes/):

- `central-committer` — serialize git writes
- `computer-focus` — lease exclusive desktop capture
- `review-gate` / `preland-gate` — human and automated vetoes
- `queue-drain` — parallel work with single-flight apply
- `linear-sync` — mirror an external roadmap with CAS
- `model-routing` / `moa-merge` — select and combine models

## Development

```bash
# Full smoke test
./scripts/smoke.sh
# Windows: powershell -ExecutionPolicy Bypass -File scripts/smoke.ps1

# Dashboard
cd web
npm ci
npm run test:e2e
```

Dashboard sources live in [`web/src/`](web/src/). Regenerate the README capture
with `npm run capture:readme`, or the GitHub Pages demo with
`npm run publish:demo`.

Environment variables retain the `ORQ_*` prefix for compatibility.

## License

MIT. Contributions welcome.
