# orq

> **o**rchestrate · **r**oute · **q**ueue
>
> A local-first CLI that turns a pile of agents, scripts, and half-finished ideas
> into a shared workspace with locks, memory, and a pulse.

You have three agents editing the same repo. One is rewriting the API. One is
fixing tests. One just invented a new folder name. Without coordination you get
git roulette. **orq** is the shared desk: sticky notes that version, leases that
expire, tasks that claim files, triggers that react, and a dashboard that proves
something actually happened.

No Kubernetes required. No “platform team.” One binary, one SQLite file by
default — or the same API pointed at Turso when you want the desk in the cloud.

![orq live dashboard — board, tasks, jobs, event timeline](docs/img/dashboard.png)

<p align="center"><sub>The real UI — <code>orq dash serve</code> on localhost, refreshed every second.</sub></p>

---

## What you get

| Piece | In one line |
|-------|-------------|
| **Workspaces** | Named sandboxes (lazy `default`). Optional `--session` for ephemeral chaos. |
| **POIs** | Versioned cells of state — board cards, paths, approvals, whatever JSON you invent. |
| **Leases** | Time-boxed locks (`write` / `read-block`). Steal when someone walks away. |
| **Tasks** | Oneshot or service processes with await, retry, cancel, kill, and **path claims**. |
| **Triggers** | “When X happens, do Y” with budgets and cascade guards. |
| **Models & jobs** | Affinity routing + `single` / `race` / `moa` scheduling that learns. |
| **Dashboard** | Live HTTP UI of the board, tasks, jobs, events — not a `file://` apology. |

Inspired by (and cheerfully independent from) Pueue, Temporal, Restate, Taskwarrior,
Hatchet, and the Loop Meta vibe from td-rs.

---

## Install

```bash
git clone git@github.com:QaAudio/orq.git
cd orq
cargo build --release
# binary: target/release/orq
```

Optional: set `ORQ_DATA_DIR` (defaults to your platform’s local data dir `/orq`).

Smoke everything once:

```bash
# Windows
powershell -ExecutionPolicy Bypass -File scripts/smoke.ps1
# Unix
./scripts/smoke.sh
```

---

## Quickstarts

Pick a lane, expand, copy-paste. Deeper plots live under [Possibilities](#possibilities--workflows) and [`recipes/`](recipes/).

<details>
<summary><strong>Desk in 30 seconds</strong> — init, pin a note, run a task</summary>

```bash
orq init
orq poi table create notes --cols body:string:poi
orq poi set notes hello '"world"' --tier ephemeral
orq run --sync --name hi -- "echo hello from orq"
```

</details>

<details>
<summary><strong>Watch the pulse</strong> — that’s the screenshot above</summary>

```bash
orq dash snapshot
orq dash serve --port 9847
# → http://127.0.0.1:9847/
```

</details>

<details>
<summary><strong>Teach Cursor about orq</strong> — skill + AGENTS snippet</summary>

```bash
orq integrate cursor --path /path/to/host/repo
```

Agents then speak POI / claim / trigger instead of inventing folklore.

</details>

<details>
<summary><strong>Shared cloud desk</strong> — same CLI, Turso/libSQL (opt-in)</summary>

```bash
cp .env.example .env   # fill ORQ_DB_URL + TOKEN
orq --remote init
orq --remote poi set board hello '{"msg":"shared"}'
```

Without `--remote`, you stay local. A stray `.env` never hijacks you.

</details>

<details>
<summary><strong>Route models like a pit crew</strong> — affinity + MoA</summary>

```bash
orq model add fast --cli "echo FAST:{cmd}" --capability code
orq model add strong --cli "echo STRONG:{cmd}" --capability code
orq affinity set code.edit strong --score 0.9
orq run --sync --class code.edit --strategy moa --moa-k 2 --name edit -- "propose"
```

</details>

<details>
<summary><strong>Don’t collide on disk</strong> — path claims for a task</summary>

```bash
orq run --sync --name refactor --claim "src/**" -- "cargo test -p foo"
```

Overlapping claims wait or fail closed — leases, not vibes.

</details>

---

## Possibilities & workflows

orq is deliberately unfinished as a *product* and very useful as a *protocol*.
Here are shapes people actually run:

### 1. Multi-agent coding desk

Several Cursor / CLI agents share one workspace.

1. Create a `board` POI table for tickets (`todo` → `doing` → `done`).
2. Each agent **locks** a card (`orq poi lock board T-12 --holder agent-a`).
3. Tasks **claim** the paths they will touch.
4. A trigger cancels stale work when a card flips to `blocked`.
5. Dashboard on a second monitor so humans can interrupt without Slack archaeology.

### 2. Human-in-the-loop gates

POIs as approval tokens.

```text
agent finishes PR  →  sets poi reviews/pr-42 = {ready:true}
human flips blocked → false
trigger unblocks   →  merge / deploy task runs
```

See recipe ideas in [`recipes/`](recipes/) (`review-gate`, `preland-gate`).

### 3. Serialized “one committer” lane

Only one process may touch git at a time:

- Write lease on `paths/repo`
- Queue of proposed patches as POIs
- Central committer task drains the queue, commits, releases

Chaos-free git even when five agents feel inspired.

### 4. Roadmap sync (Linear ↔ local)

A durable POI table mirrors external tickets; a **service** task polls and CAS-writes.
Agents never talk to Linear directly — they talk to orq. The syncer is the diplomat.

### 5. Verify fan-out before land

`preland-gate`: spawn parallel check tasks; a **blocking** trigger vetoes land if any fail.
Budgets keep a flaky check from spawning a fork-bomb.

### 6. Model pit lane

Register several model recipes (CLI wrappers, API scripts, whatever). Affinity EMA
learns which model wins for `code.edit` vs `docs.summarize`. Use `race` when you want
the first good answer; `moa` when you want propose → critique → merge.

### 7. Local today, shared tomorrow

Prototype entirely offline (SQLite). When the team wants one desk, copy `.env.example`
→ `.env`, pass `--remote`, keep the same commands. Transactions wrap poi+event,
leases, jobs — so two laptops don’t half-write a board card.

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

That’s the real UI at the top of this README. Source lives in [`web/dashboard/`](web/dashboard/), served over **HTTP** (not `file://`).

```bash
orq dash snapshot                 # → $ORQ_DATA_DIR/dash/data.json
orq dash serve --port 9847        # static UI + /data.json (1s refresh)
```

Override static root with `--root` or `ORQ_DASH_ROOT`.

### Dashboard E2E (no LLM)

```bash
cargo build -p orq
cd web && npm ci && npx playwright install chromium
npm run test:e2e
```

Seed data is CLI-only (`echo` stubs): board POIs, affinities, a routed task, one MoA job.

---

## Storage backends

One store API (`crates/orq-core/src/db.rs`), two backends:

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

---

## License

MIT — see the repo. Contributions welcome; chaos optional but documented.
