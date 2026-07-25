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

export const DETAILS_GRID_MIN_W = 3;
export const DETAILS_GRID_MIN_H = 3;
export const DETAILS_GRID_ROW_HEIGHT = 48;

/** Details panel ids — kept here so the grid engine owns Details defaults. */
export const DETAILS_PANEL_IDS = [
  "ops-health",
  "running-tasks",
  "board",
  "tasks",
  "jobs",
  "aff",
  "events",
  "files",
] as const;

export type DetailsPanelId = (typeof DETAILS_PANEL_IDS)[number];

export const DETAILS_PANEL_LABELS: Record<DetailsPanelId, string> = {
  "ops-health": "Loop ops",
  "running-tasks": "Running tasks",
  board: "Board",
  tasks: "Tasks",
  jobs: "Jobs",
  aff: "Affinities",
  events: "Events",
  files: "Files",
};

/** Default pixel heights used only when migrating legacy dock layouts. */
export const DEFAULT_PANEL_HEIGHTS: Record<DetailsPanelId, number> = {
  "ops-health": 320,
  "running-tasks": 220,
  board: 260,
  tasks: 260,
  jobs: 260,
  aff: 260,
  events: 300,
  files: 180,
};

export function defaultCanvasGridLayout(): CanvasGridLayout {
  return {
    v: 1,
    cols: CANVAS_GRID_COLS,
    rowHeight: CANVAS_GRID_ROW_HEIGHT,
    items: {},
  };
}

export function clampItem(
  item: CanvasGridItem,
  cols: number,
  minW = CANVAS_GRID_MIN_W,
  minH = CANVAS_GRID_MIN_H
): CanvasGridItem {
  const w = Math.max(minW, Math.min(cols, Math.round(item.w)));
  const h = Math.max(minH, Math.round(item.h));
  const x = Math.max(0, Math.min(cols - w, Math.round(item.x)));
  const y = Math.max(0, Math.round(item.y));
  return { x, y, w, h };
}

export function overlaps(a: CanvasGridItem, b: CanvasGridItem): boolean {
  return a.x < b.x + b.w && a.x + a.w > b.x && a.y < b.y + b.h && a.y + a.h > b.y;
}

export function layoutHasOverlaps(items: Record<string, CanvasGridItem>): boolean {
  const entries = Object.entries(items);
  for (let i = 0; i < entries.length; i += 1) {
    for (let j = i + 1; j < entries.length; j += 1) {
      if (overlaps(entries[i]![1], entries[j]![1])) return true;
    }
  }
  return false;
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

/**
 * Push colliding items downward (Grafana / react-grid-layout style).
 * Mutates `items` in place. Active key stays put; others move down until clear.
 */
export function resolveCollisions(
  items: Record<string, CanvasGridItem>,
  activeKey: string,
  cols: number
): void {
  let changed = true;
  let guard = 0;
  while (changed && guard < 2000) {
    changed = false;
    guard += 1;
    const keys = Object.keys(items);
    for (const key of keys) {
      if (key === activeKey) continue;
      const item = items[key]!;
      for (const otherKey of keys) {
        if (otherKey === key) continue;
        const other = items[otherKey]!;
        if (!overlaps(item, other)) continue;
        // Never move the active item; push this one below the other when:
        // - other is the active item, or
        // - this item is strictly below / later in reading order.
        const otherIsActive = otherKey === activeKey;
        const itemIsLower =
          item.y > other.y ||
          (item.y === other.y && (item.x > other.x || (item.x === other.x && key > otherKey)));
        if (!otherIsActive && !itemIsLower) continue;
        const nextY = other.y + other.h;
        if (item.y < nextY) {
          items[key] = clampItem({ ...item, y: nextY }, cols);
          changed = true;
        }
      }
    }
  }
}

/**
 * Vertical compact: pack each item as high as possible without overlap.
 * Stable sort by y, then x, then key.
 */
export function compactLayout(
  items: Record<string, CanvasGridItem>,
  cols: number
): Record<string, CanvasGridItem> {
  const sorted = Object.entries(items).sort(([, a], [, b]) => {
    if (a.y !== b.y) return a.y - b.y;
    if (a.x !== b.x) return a.x - b.x;
    return 0;
  });
  const next: Record<string, CanvasGridItem> = {};
  for (const [key, item] of sorted) {
    let y = 0;
    for (;;) {
      const cand = clampItem({ ...item, y }, cols);
      if (!Object.values(next).some((p) => overlaps(cand, p))) {
        next[key] = cand;
        break;
      }
      y += 1;
      if (y > 500) {
        next[key] = clampItem({ ...item, y: 0 }, cols);
        break;
      }
    }
  }
  return next;
}

function cloneItems(items: Record<string, CanvasGridItem>): Record<string, CanvasGridItem> {
  const out: Record<string, CanvasGridItem> = {};
  for (const [k, v] of Object.entries(items)) out[k] = { ...v };
  return out;
}

/** Place/move/resize one item, push collisions down, then compact. */
export function applyItemChange(
  layout: CanvasGridLayout,
  key: string,
  next: Partial<CanvasGridItem>,
  opts: { compact?: boolean; minW?: number; minH?: number } = {}
): CanvasGridLayout {
  const cur = layout.items[key];
  if (!cur) return layout;
  const minW = opts.minW ?? CANVAS_GRID_MIN_W;
  const minH = opts.minH ?? CANVAS_GRID_MIN_H;
  const items = cloneItems(layout.items);
  items[key] = clampItem({ ...cur, ...next }, layout.cols, minW, minH);
  resolveCollisions(items, key, layout.cols);
  const packed = opts.compact === false ? items : compactLayout(items, layout.cols);
  return { ...layout, items: packed };
}

export function moveCanvasItem(
  layout: CanvasGridLayout,
  key: string,
  x: number,
  y: number
): CanvasGridLayout {
  return applyItemChange(layout, key, { x, y });
}

export function resizeCanvasItem(
  layout: CanvasGridLayout,
  key: string,
  next: Partial<CanvasGridItem>
): CanvasGridLayout {
  return applyItemChange(layout, key, next);
}

export function normalizeCanvasGridLayout(
  raw: unknown,
  defaults: Partial<CanvasGridLayout> = {}
): CanvasGridLayout {
  const base = {
    ...defaultCanvasGridLayout(),
    ...defaults,
    items: {},
  };
  if (!raw || typeof raw !== "object") return base;
  const o = raw as Record<string, unknown>;
  const cols =
    typeof o.cols === "number" && o.cols > 0 ? Math.round(o.cols) : base.cols || CANVAS_GRID_COLS;
  const rowHeight =
    typeof o.rowHeight === "number" && o.rowHeight >= 24
      ? Math.round(o.rowHeight)
      : base.rowHeight || CANVAS_GRID_ROW_HEIGHT;
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
  // Repair any overlapping saved layouts.
  let repaired = items;
  if (layoutHasOverlaps(repaired)) {
    const keys = Object.keys(repaired).sort((a, b) => {
      const A = repaired[a]!;
      const B = repaired[b]!;
      if (A.y !== B.y) return A.y - B.y;
      if (A.x !== B.x) return A.x - B.x;
      return a.localeCompare(b);
    });
    const staged: Record<string, CanvasGridItem> = {};
    for (const key of keys) {
      staged[key] = repaired[key]!;
      resolveCollisions(staged, key, cols);
    }
    repaired = compactLayout(staged, cols);
  }
  return { v: 1, cols, rowHeight, items: repaired };
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
  if (layoutHasOverlaps(items)) {
    const packed = compactLayout(items, layout.cols);
    return { ...layout, items: packed };
  }
  return { ...layout, items };
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

/** Default Details layout: two columns of independent tiles. */
export function defaultDetailsGridLayout(): CanvasGridLayout {
  const items: Record<string, CanvasGridItem> = {
    "ops-health": { x: 0, y: 0, w: 6, h: 7 },
    "running-tasks": { x: 6, y: 0, w: 6, h: 5 },
    board: { x: 0, y: 7, w: 6, h: 6 },
    tasks: { x: 6, y: 5, w: 6, h: 6 },
    jobs: { x: 0, y: 13, w: 6, h: 6 },
    aff: { x: 6, y: 11, w: 6, h: 6 },
    events: { x: 0, y: 19, w: 6, h: 7 },
    files: { x: 6, y: 17, w: 6, h: 5 },
  };
  return {
    v: 1,
    cols: CANVAS_GRID_COLS,
    rowHeight: DETAILS_GRID_ROW_HEIGHT,
    items: compactLayout(items, CANVAS_GRID_COLS),
  };
}

function pxToRows(px: number, rowHeight: number): number {
  return Math.max(DETAILS_GRID_MIN_H, Math.round(px / rowHeight));
}

/**
 * One-time migration from legacy dock (`porq.dash.layout.dock`) into a Details grid.
 * Left column → x=0 w=6; right → x=6 w=6; heights → row spans; tab stacks become sibling tiles.
 */
export function migrateDockToDetailsGrid(raw: unknown): CanvasGridLayout | null {
  if (!raw || typeof raw !== "object") return null;
  const o = raw as Record<string, unknown>;
  if (!Array.isArray(o.columns) || o.columns.length < 1) return null;
  const rowHeight = DETAILS_GRID_ROW_HEIGHT;
  const items: Record<string, CanvasGridItem> = {};
  const colsArr = o.columns as unknown[];

  for (let colIdx = 0; colIdx < Math.min(2, colsArr.length); colIdx += 1) {
    const leaves = Array.isArray(colsArr[colIdx]) ? (colsArr[colIdx] as unknown[]) : [];
    let y = 0;
    for (const leaf of leaves) {
      if (!leaf || typeof leaf !== "object") continue;
      const L = leaf as Record<string, unknown>;
      const tabs = Array.isArray(L.tabs)
        ? L.tabs.filter((t): t is DetailsPanelId =>
            typeof t === "string" && (DETAILS_PANEL_IDS as readonly string[]).includes(t)
          )
        : [];
      if (!tabs.length) continue;
      const heightPx =
        typeof L.height === "number"
          ? L.height
          : DEFAULT_PANEL_HEIGHTS[tabs[0]!] ?? 260;
      const h = pxToRows(heightPx, rowHeight);
      for (const tab of tabs) {
        if (items[tab]) continue;
        items[tab] = {
          x: colIdx === 0 ? 0 : 6,
          y,
          w: 6,
          h,
        };
        y += h;
      }
    }
  }

  for (const id of DETAILS_PANEL_IDS) {
    if (items[id]) continue;
    const slot = findFreeSlot(items, 6, pxToRows(DEFAULT_PANEL_HEIGHTS[id], rowHeight), 12);
    items[id] = {
      ...slot,
      w: 6,
      h: pxToRows(DEFAULT_PANEL_HEIGHTS[id], rowHeight),
    };
  }

  if (!Object.keys(items).length) return null;
  return {
    v: 1,
    cols: CANVAS_GRID_COLS,
    rowHeight,
    items: compactLayout(items, CANVAS_GRID_COLS),
  };
}

/** Pixel→grid helpers (gap-aware). */
export function colWidthPx(hostWidth: number, cols: number, gapPx: number): number {
  if (cols <= 0) return hostWidth;
  const usable = Math.max(0, hostWidth - gapPx * (cols - 1));
  return usable / cols;
}

export function clientToGrid(
  clientX: number,
  clientY: number,
  hostRect: DOMRect,
  cols: number,
  rowHeight: number,
  gapPx: number
): { col: number; row: number } {
  const colW = colWidthPx(hostRect.width, cols, gapPx);
  const x = clientX - hostRect.left;
  const y = clientY - hostRect.top;
  const col = Math.max(0, Math.min(cols - 1, Math.floor(x / (colW + gapPx))));
  const row = Math.max(0, Math.floor(y / (rowHeight + gapPx)));
  return { col, row };
}
