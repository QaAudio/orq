# Canvas authoring (dashboard cards)

Short guide for agents publishing `porq canvas set` markdown/html so operators can skim without dumping logs.

## Shape (every live card)

1. **H1** — what this surface is (one line).
2. **State word** — one of: `live`, `ok`, `warn`, `blocked`, `failed`, `done`, `waiting` (match host vocabulary when present).
3. **Next command** — the single cheapest CLI/action an operator or agent should run next.
4. **Freshness** — when this was last written (ISO time or relative).

Example:

```markdown
# Loop health

**State:** ok

**Next:** `porq status --json --limit 20`

Updated 2026-07-25T16:02:00Z
```

## Prefer tables over dumps

- Summarize POIs/tasks as markdown tables (id | state | note).
- Do not paste unbounded JSON, full event logs, or multi-KB stderr into the body.
- Link to a file or say which `porq … --limit` command reveals detail.

## Theme inheritance

HTML canvases should use dashboard CSS variables (`var(--text)`, `var(--accent)`, …) — **no hardcoded hex** for chrome colors — so `--theme` / picker packs still apply. Markdown cards are theme-neutral text; avoid inline color styles.

See [`web/dashboard/themes/README.md`](../web/dashboard/themes/README.md) for the token catalog.

## Status vocabulary

Use plain status words operators already understand (`ok` / `warn` / `blocked` / `failed`). Do not invent host-specific gate law inside a generic canvas; host Loop Meta cards own veto semantics.

## Cheap surfaces first

Before publishing a heavy canvas, check whether `porq status`, `poi ls`, or `events --limit` already answers the claim. Canvases are for **shared glance**, not a substitute for bounded CLI probes.
