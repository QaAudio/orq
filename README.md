# orq

> **o**rchestrate · **r**oute · **q**ueue
>
> The progressive orchestration system for multi-agent work.
> Start with one command. Stop wherever it stops hurting.

Most orchestrators ask you to move in: adopt the platform, learn the DSL,
deploy the control plane, *then* run your first task. **orq** works the other
way around. It's a single local binary that is useful at every layer of
adoption — and each layer is opt-in:

```text
run one task            →  orq run --sync -- "cargo test"
share state             →  + POIs (versioned JSON cells)
stop collisions         →  + leases & path claims
react to changes        →  + triggers & the daemon
route across models     →  + affinities, race / MoA jobs
share across machines   →  + --remote (Turso/libSQL)
```

You never rewrite what you built at the previous layer. The commands stay the
same; the system underneath grows with you — from "glorified task runner on my
laptop" to "shared cloud desk for a swarm of agents" without a single
Kubernetes manifest.

![orq live dashboard — board, tasks, jobs, event timeline](docs/img/dashboard.png)

<p align="center"><sub>The real UI — <code>orq dash serve</code> on localhost, refreshed every second.</sub></p>

---

## Why this exists

You have three agents editing the same repo. One is rewriting the API. One is
fixing tests. One just invented a new folder name. Without coordination you
get git roulette. orq is the shared desk: sticky notes that version, leases
that expire, tasks that claim files, triggers that react, and a dashboard that
proves something actually happened.

The trick is that you don't adopt "the desk" on day one. You adopt `orq run`.
The rest is sitting there when you need it.

---

## The layers

| Layer | You add | You get |
|-------|---------|---------|
| **0 — Run** | `orq run` | Supervised tasks: await, retry, cancel, kill, logs. No daemon needed with `--sync`. |
| **1 — Remember** | POI tables (+ `canvas`) | Versioned JSON cells (board cards, approvals, dashboard canvases). CAS, tiers, sessions. |
| **2 — Not collide** | Leases & claims | Time-boxed locks with holders; `--claim "src/**"` fences files for a task's lifetime. |
| **3 — React** | Triggers | "When X happens, do Y" with blocking vetoes, budgets, cascade guards. Daemon auto-spawns. |
| **4 — Route** | Models & jobs | Affinity scores that learn (EMA), `single` / `race` / `moa` scheduling. |
| **5 — Share** | `--remote` | Same CLI against Turso/libSQL. Two laptops, one desk, real transactions. |

Every mutation at every layer lands in one append-only **event log**, and the
**dashboard** renders it all live — including agent-published **canvases**
(markdown / image / url / html). Observability isn't a layer — it's the floor.

Inspired by (and cheerfully independent from) Pueue, Temporal, Restate,
Taskwarrior, Hatchet, and the Loop Meta vibe from td-rs.

---

## Install

```bash
git clone git@github.com:QaAudio/orq.git
cd orq
cargo build --release
# binary: target/release/orq
```

Optional: set `ORQ_DATA_DIR` (defaults to your platform's local data dir `/orq`).

Smoke everything once:

```bash
# Windows
powershell -ExecutionPolicy Bypass -File scripts/smoke.ps1
# Unix
./scripts/smoke.sh
```

---

## Quickstarts, one per layer

Expand, copy-paste, climb. Deeper plots live under
[Workflows](#workflows-that-emerge) and [`recipes/`](recipes/).

<details>
<summary><strong>Layer 0 — Run a task</strong> — no daemon, no ceremony</summary>

```bash
orq init
orq run --sync --name hi -- "echo hello from orq"
orq status --json
```

`--sync` blocks and supervises inline. Drop it and a daemon quietly takes over.

</details>

<details>
<summary><strong>Layer 1 — Pin shared state</strong> — POIs, the sticky notes that version</summary>

```bash
orq poi table create notes --cols body:string:poi
orq poi set notes hello '"world"' --tier ephemeral
orq poi get notes hello --json
```

Every `set` bumps a version; `--if-version` gives you compare-and-swap.

</details>

<details>
<summary><strong>Layer 2 — Stop collisions</strong> — leases and path claims</summary>

```bash
orq poi lock notes hello --holder agent-a --ttl 300
orq run --sync --name refactor --claim "src/**" -- "cargo test -p foo"
```

Overlapping claims wait or fail closed — leases, not vibes. `poi steal` for recovery.

</details>

<details>
<summary><strong>Layer 3 — React to changes</strong> — triggers with budgets</summary>

```bash
orq trigger add on-broken --on poi.changed --where-cond "state==broken" --do-action "spawn:./remediate.sh"
```

Blocking triggers can veto; budgets and cascade guards keep a flaky rule from
becoming a fork-bomb.

</details>

<details>
<summary><strong>Layer 4 — Route across models</strong> — affinity + race / MoA</summary>

```bash
orq model add fast --cli "echo FAST:{cmd}" --capability code
orq model add strong --cli "echo STRONG:{cmd}" --capability code
orq affinity set code.edit strong --score 0.9
orq run --sync --class code.edit --strategy moa --moa-k 2 --name edit -- "propose"
```

Affinities learn from outcomes (EMA). `race` for the first good answer, `moa`
for propose → aggregate.

</details>

<details>
<summary><strong>Layer 5 — Share across machines</strong> — same CLI, Turso/libSQL</summary>

```bash
cp .env.example .env   # fill ORQ_DB_URL + TOKEN
orq --remote init
orq --remote poi set board hello '{"msg":"shared"}'
```

Opt-in only: without `--remote`, you stay local. A stray `.env` never hijacks you.

</details>

<details>
<summary><strong>Bonus — Publish a canvas</strong> — markdown / image / url on the dashboard</summary>

```bash
orq canvas set plan --body "## Next\n- probe\n- ship"
orq canvas set shot --image ./out.png --title "Render"
orq dash serve --port 9847
```

Canvases are POIs in the reserved `canvas` table — versioned, lockable, live within 1s.

</details>

<details>
<summary><strong>Bonus — Watch the pulse</strong> — that's the screenshot above</summary>

```bash
orq dash snapshot
orq dash serve --port 9847
# → http://127.0.0.1:9847/
```

</details>

<details>
<summary><strong>Bonus — Teach Cursor about orq</strong> — skill + AGENTS snippet</summary>

```bash
orq integrate cursor --path /path/to/host/repo
```

Agents then speak POI / claim / trigger instead of inventing folklore.

</details>

---

## Workflows that emerge

orq is deliberately small as a *product* and very useful as a *protocol*.
Each workflow below is just layers composed — nothing here needed a new feature:

### Multi-agent coding desk (layers 1–3)

Several Cursor / CLI agents share one workspace.

1. A `board` POI table holds tickets (`todo` → `doing` → `done`).
2. Each agent **locks** a card (`orq poi lock board T-12 --holder agent-a`).
3. Tasks **claim** the paths they will touch.
4. A trigger cancels stale work when a card flips to `blocked`.
5. Dashboard on a second monitor so humans can interrupt without Slack archaeology.

### Human-in-the-loop gates (layers 1 + 3)

POIs as approval tokens:

```text
agent finishes PR  →  sets poi reviews/pr-42 = {ready:true}
human flips blocked → false
trigger unblocks   →  merge / deploy task runs
```

See `review-gate` and `preland-gate` in [`recipes/`](recipes/).

### Serialized "one committer" lane (layer 2)

Only one process may touch git at a time: write lease on `paths/repo`, queue of
proposed patches as POIs, a central committer task drains → commits → releases.
Chaos-free git even when five agents feel inspired.

### Roadmap sync, Linear ↔ local (layers 1 + 0)

A durable POI table mirrors external tickets; a **service** task polls and
CAS-writes. Agents never talk to Linear directly — they talk to orq. The
syncer is the diplomat.

### Verify fan-out before land (layer 3)

`preland-gate`: spawn parallel check tasks; a **blocking** trigger vetoes land
if any fail. Budgets keep a flaky check from spawning a fork-bomb.

### Model pit lane (layer 4)

Register several model recipes (CLI wrappers, API scripts, whatever). Affinity
EMA learns which model wins for `code.edit` vs `docs.summarize`.

### Local today, shared tomorrow (layer 5)

Prototype entirely offline (SQLite). When the team wants one desk, copy
`.env.example` → `.env`, pass `--remote`, keep the same commands. Transactions
wrap poi+event, leases, jobs — so two laptops don't half-write a board card.

---

## Recipes

Executable patterns live in [`recipes/`](recipes/):

| Recipe | Mood |
|--------|------|
| `linear-sync` | Bidirectional roadmap via CAS + poller |
| `central-committer` | One git pen, many authors |
| `review-gate` | Human approval unblocks a task |
| `preland-gate` | Fan-out verify + blocking veto |
| `queue-drain` | Parallel drain, single-flight apply |
| `model-routing` | Affinity picks a model |
| `moa-merge` | Mixture-of-Agents propose + aggregate |

---

## Live dashboard

That's the real UI at the top of this README. Source lives in
[`web/dashboard/`](web/dashboard/), served over **HTTP** (not `file://`).

```bash
orq dash snapshot                 # → $ORQ_DATA_DIR/dash/data.json
orq dash serve --port 9847        # static UI + /data.json (1s refresh)
```

Override static root with `--root` or `ORQ_DASH_ROOT`.

### Canvases (display protocol v1)

Agents publish arbitrary display surfaces as POIs in the reserved **`canvas`**
table. The dashboard polls them with everything else — no new transport.

| `kind` | Fields | Renders as |
|--------|--------|------------|
| `markdown` | `title`, `body` | Safe escape-first markdown subset |
| `image` | `title`, `src`, `alt?` | `<img>` |
| `url` | `title`, `src`, `height?` | Sandboxed iframe |
| `html` | `title`, `body`, `height?` | `srcdoc` iframe (`sandbox=""` — no scripts) |
| *(other)* | any | Pretty-printed JSON fallback |

`src` forms: `data:…` (inline), `canvas:<file>` → `$ORQ_DATA_DIR/canvas/` via
`GET /canvas/<file>`, or `http(s)://…`. Layout hints: `columns.order`,
`columns.span` (`1`|`2`). State pill: `live` / `done` / `archived`.

```bash
orq canvas set plan --md ./notes.md --order 1
orq canvas set shot --image ./frame.png --span 2
orq canvas set report --url https://example.com/status --height 480
orq canvas ls --json
orq canvas rm plan
```

### Dashboard E2E (no LLM)

```bash
cargo build -p orq
cd web && npm ci && npx playwright install chromium
npm run test:e2e
```

Seed data is CLI-only (`echo` stubs): board POIs, affinities, a routed task,
one MoA job. Regenerate the README screenshot with `npm run capture:readme`.

---

## Storage backends

Progressive here too: one store API (`crates/orq-core/src/db.rs`), two backends,
zero code changes to switch.

| Mode | Backend | How you get it |
|------|---------|----------------|
| **Local** (default) | SQLite file `<data_dir>/orq.db` | just run `orq` |
| **Remote** | Turso / any libSQL over HTTP | `orq --remote` + `ORQ_DB_URL` / `ORQ_DB_TOKEN` |

```bash
# .env (gitignored) — see .env.example
ORQ_DB_URL=libsql://YOUR_DB.turso.io
ORQ_DB_TOKEN=YOUR_TOKEN_HERE

orq --remote init
orq --remote workspace drop old-ws --yes   # destructive cloud hygiene
```

Notes worth knowing:

- Multi-statement mutations run in one transaction (`BEGIN IMMEDIATE` locally).
- Live Turso E2E: `cargo test -p orq-core --test turso_e2e` (skips if no creds).
- Embedded replica (local reads, synced writes) is a planned follow-up.
- Lease expiry uses client clocks — keep NTP honest when several writers share remote.

---

## Mental model (the short version)

```text
┌──────────── workspace ────────────┐
│  POIs (versioned cells + leases)  │
│  Tasks (claims, logs, await)      │
│  Triggers (react + budgets)       │
│  Jobs / models (route & learn)    │
│  Events (append-only pulse)       │
└───────────────────────────────────┘
         │
         ├── local SQLite
         └── optional Turso / libSQL
```

- Daemon-optional: `orq run --sync` needs none; unsupervised `run` auto-spawns
  `orq daemon run` (localhost TCP + port file).
- Every mutation emits an event. Follow the log like a black box recorder.
- Locks are **leases** (TTL + holder). Use `poi steal` for recovery theatre.
- Claims (`--claim "src/**"`) are write leases on the `paths` table for the task lifetime.
- Nothing above layer 0 is mandatory. That's the whole point.

---

## License

MIT — see the repo. Contributions welcome; chaos optional but documented.
