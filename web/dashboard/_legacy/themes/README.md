# Dashboard themes

Opt-in polish for `porq dash serve`. Zero flags → **default** (current warm dark).

## Packs

| Name | File | Notes |
|------|------|-------|
| `default` | `default.css` | Warm dark — production look |
| `dracula` | `dracula.css` | Classic Dracula mapped to semantic tokens |
| `system` | `system.css` | Light + `@media (prefers-color-scheme: dark)` |

Structure lives in `base.css` (no hardcoded brand colors — only `var(--*)`).

## Selection precedence

1. `?theme=` query
2. `--theme-file` / `ORQ_DASH_THEME_FILE` (custom CSS of the same variables)
3. `--theme` / `ORQ_DASH_THEME`
4. `localStorage` key `porq.dash.theme`
5. `default`

## Variable catalog

| Token | Role |
|-------|------|
| `--bg` `--bg-wash` | Page background |
| `--panel` `--panel-elev` | Cards / elevated surfaces |
| `--border` `--border-soft` | Borders |
| `--text` `--muted` | Body / secondary text |
| `--accent` `--accent-dim` | Brand / focus / primary chrome |
| `--ok` `--ok-dim` | Success / done |
| `--warn` `--warn-dim` | Waiting caution / blocked |
| `--danger` `--danger-dim` | Failed / error |
| `--info` `--info-dim` | Informational / links |
| `--proposed` `--proposed-dim` | Proposed state |
| `--approved` `--approved-dim` | Approved / live-ok |
| `--font-display` `--font-mono` | Type |
| `--radius` `--gap` | Density |
| `--scroll-thumb` `--scroll-thumb-hover` | Scrollbars |

Status pills map to these (`status-blocked` → warn, `status-approved` → approved, etc.).

## Add a pack

1. Copy `default.css` → `mytheme.css` and edit tokens.
2. Register the name in the dash theme allowlist (`dash_serve` curated list + picker options).
3. Or skip registration: `porq dash serve --theme-file ./mytheme.css`.

Hot-edit: files under this folder are served from disk when present; embedded `include_str!` is fallback only.
