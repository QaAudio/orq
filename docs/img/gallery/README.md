# Dashboard image gallery

Thumbnails for the README theme + UI-scale grids. Regen from `web/`:

```bash
npm run capture:readme
```

| File | Shows |
|------|--------|
| `theme-dark.png` | Canvases · **dark** theme (SDK default) |
| `theme-light.png` | Canvases · **light** theme |
| `scale-100.png` | dark theme @ **100%** zoom |
| `scale-125.png` | dark theme @ **125%** zoom |
| `scale-150.png` | dark theme @ **150%** zoom |

Viewport for all stills: 1440×900. Feature showcase board (mermaid + markdown + HTML + image). Consumer loop screenshots stay in [`../usecases/`](../usecases/).

Aliases `default`/`dracula` → dark and `system` → light are CLI/env only — not separate gallery files.
