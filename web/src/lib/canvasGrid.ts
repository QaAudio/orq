export type CanvasGridItem = {
  x: number;
  y: number;
  w: number;
  h: number;
};

export type CanvasGridLayout = {
  v: 1;
  cols: number;
  rowHeight: number;
  items: Record<string, CanvasGridItem>;
};

export const CANVAS_GRID_COLS = 12;
export const CANVAS_GRID_ROW_HEIGHT = 48;
export const CANVAS_GRID_MIN_W = 3;
export const CANVAS_GRID_MIN_H = 3;

export function defaultCanvasGridLayout(): CanvasGridLayout {
  return {
    v: 1,
    cols: CANVAS_GRID_COLS,
    rowHeight: CANVAS_GRID_ROW_HEIGHT,
    items: {},
  };
}

function clampItem(item: CanvasGridItem, cols: number): CanvasGridItem {
  const w = Math.max(CANVAS_GRID_MIN_W, Math.min(cols, Math.round(item.w)));
  const h = Math.max(CANVAS_GRID_MIN_H, Math.round(item.h));
  const x = Math.max(0, Math.min(cols - w, Math.round(item.x)));
  const y = Math.max(0, Math.round(item.y));
  return { x, y, w, h };
}

function overlaps(a: CanvasGridItem, b: CanvasGridItem): boolean {
  return a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y;
}

/** Find first free slot for w×h scanning left-to-right, top-to-bottom. */
export function findFreeSlot(
  items: Record<string, CanvasGridItem>,
  w: number,
  h: number,
  cols: number,
  skipKey?: string
): { x: number; y: number } {
  const placed = Object.entries(items)
    .filter(([k]) => k !== skipKey)
    .map(([, v]) => v);
  let y = 0;
  for (;;) {
    for (let x = 0; x <= cols - w; x += 1) {
      const cand = { x, y, w, h };
      if (!placed.some((p) => overlaps(cand, p))) return { x, y };
    }
    y += 1;
    if (y > 500) return { x: 0, y: 0 };
  }
}

export function preferFullWidth(key: string, span2?: boolean): boolean {
  if (span2) return true;
  return key === "loop-roadmap";
}

export function defaultItemForKey(key: string, span2?: boolean): Omit<CanvasGridItem, "x" | "y"> {
  return preferFullWidth(key, span2) ? { w: 12, h: 8 } : { w: 6, h: 6 };
}

export function normalizeCanvasGridLayout(raw: unknown): CanvasGridLayout {
  const base = defaultCanvasGridLayout();
  if (!raw || typeof raw !== "object") return base;
  const o = raw as Record<string, unknown>;
  const cols = typeof o.cols === "number" && o.cols > 0 ? Math.round(o.cols) : CANVAS_GRID_COLS;
  const rowHeight =
    typeof o.rowHeight === "number" && o.rowHeight >= 24
      ? Math.round(o.rowHeight)
      : CANVAS_GRID_ROW_HEIGHT;
  const items: Record<string, CanvasGridItem> = {};
  if (o.items && typeof o.items === "object") {
    for (const [key, val] of Object.entries(o.items as Record<string, unknown>)) {
      if (!val || typeof val !== "object") continue;
      const v = val as Record<string, unknown>;
      if (
        typeof v.x !== "number" ||
        typeof v.y !== "number" ||
        typeof v.w !== "number" ||
        typeof v.h !== "number"
      ) {
        continue;
      }
      items[key] = clampItem({ x: v.x, y: v.y, w: v.w, h: v.h }, cols);
    }
  }
  return { v: 1, cols, rowHeight, items };
}

/** Ensure every key has a non-overlapping item; drop stale keys. */
export function reconcileCanvasItems(
  layout: CanvasGridLayout,
  keys: string[],
  span2ForKey: (key: string) => boolean
): CanvasGridLayout {
  const keep = new Set(keys);
  const items: Record<string, CanvasGridItem> = {};
  for (const [k, v] of Object.entries(layout.items)) {
    if (keep.has(k)) items[k] = clampItem(v, layout.cols);
  }
  for (const key of keys) {
    if (items[key]) continue;
    const def = defaultItemForKey(key, span2ForKey(key));
    const slot = findFreeSlot(items, def.w, def.h, layout.cols);
    items[key] = { ...slot, ...def };
  }
  return { ...layout, items };
}

export function resizeCanvasItem(
  layout: CanvasGridLayout,
  key: string,
  next: Partial<CanvasGridItem>
): CanvasGridLayout {
  const cur = layout.items[key];
  if (!cur) return layout;
  const merged = clampItem({ ...cur, ...next }, layout.cols);
  return {
    ...layout,
    items: { ...layout.items, [key]: merged },
  };
}

export function gridStyleForItem(
  item: CanvasGridItem,
  rowHeight: number
): Record<string, string> {
  return {
    gridColumn: `${item.x + 1} / span ${item.w}`,
    gridRow: `${item.y + 1} / span ${item.h}`,
    minHeight: `${item.h * rowHeight}px`,
  };
}
