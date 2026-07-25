
## porq (progressive orchestration)

When coordinating multiple agents or shared lockable state, prefer the `porq` CLI
(skill: `.cursor/skills/porq/SKILL.md` if installed via `porq integrate cursor`).

- Cheap surfaces first: `porq status|poi ls|canvas ls|events` with `--json --limit`.
- Prefer `--session` + ephemeral POI tier for short-lived agent work; `porq gc` when done.
- Path edits: declare `--claim "glob/**"` so the scheduler serializes overlapping work.
- Do not invent parallel lock files; use `porq poi lock` / claims.
- Share status on the dashboard with `porq canvas set` (primary view); Details holds ops tables.
- Canvas shape: H1 + state word + next command + freshness; tables over JSON dumps (see porq `docs/canvas-authoring.md`).
- Multi-model work: register models, set affinities, prefer `--strategy single|race|moa --sync`.
- Host Loop Meta / roadmap cards own vetoes; porq does not invent them.
