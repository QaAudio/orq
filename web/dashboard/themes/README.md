# Dashboard themes

The live dash is **Vue 3 + `@quantumaudio/ableton-extension-sdk`**. Theme switching uses SDK `data-qa-theme` (`dark` | `light`), not the old CSS pack files.

## Operator

```bash
porq dash serve --theme dark
porq dash serve --theme light
# aliases: default|dracula → dark, system → light
# optional override CSS: --theme-file ./extra.css  → /themes/custom.css
```

Browser picker persists `porq.dash.qa-theme`.

## Canvas HTML bridge

Layout CSS aliases porq authoring vars onto SDK tokens (`--text`, `--muted`, `--accent`, `--bg`, `--panel`, `--border`). See `docs/canvas-authoring.md`.

## Legacy packs

Pre-Vue packs live under `web/dashboard/_legacy/themes/` for reference only — they are not served by `dash serve`.
