import type { DockLayout, DockLeaf, PanelId } from "./types";

export const ALL_PANEL_IDS: PanelId[] = [
  "ops-health",
  "running-tasks",
  "board",
  "tasks",
  "jobs",
  "aff",
  "events",
  "files",
];

export const PANEL_LABELS: Record<PanelId, string> = {
  "ops-health": "Loop ops",
  "running-tasks": "Running tasks",
  board: "Board",
  tasks: "Tasks",
  jobs: "Jobs",
  aff: "Affinities",
  events: "Events",
  files: "Files",
};

export const DEFAULT_PANEL_HEIGHTS: Record<PanelId, number> = {
  "ops-health": 320,
  "running-tasks": 220,
  board: 260,
  tasks: 260,
  jobs: 260,
  aff: 260,
  events: 300,
  files: 180,
};

export function defaultDockLayout(colSplitPct = 54.5): DockLayout {
  return {
    v: 1,
    colSplitPct,
    columns: [
      [
        { tabs: ["ops-health"], active: 0, height: DEFAULT_PANEL_HEIGHTS["ops-health"] },
        { tabs: ["board"], active: 0, height: DEFAULT_PANEL_HEIGHTS.board },
        { tabs: ["jobs"], active: 0, height: DEFAULT_PANEL_HEIGHTS.jobs },
        { tabs: ["events"], active: 0, height: DEFAULT_PANEL_HEIGHTS.events },
      ],
      [
        { tabs: ["running-tasks"], active: 0, height: DEFAULT_PANEL_HEIGHTS["running-tasks"] },
        { tabs: ["tasks"], active: 0, height: DEFAULT_PANEL_HEIGHTS.tasks },
        { tabs: ["aff"], active: 0, height: DEFAULT_PANEL_HEIGHTS.aff },
        { tabs: ["files"], active: 0, height: DEFAULT_PANEL_HEIGHTS.files },
      ],
    ],
  };
}

export function isPanelId(v: unknown): v is PanelId {
  return typeof v === "string" && (ALL_PANEL_IDS as string[]).includes(v);
}

export function normalizeDockLayout(raw: unknown, fallbackCols = 54.5): DockLayout {
  const base = defaultDockLayout(fallbackCols);
  if (!raw || typeof raw !== "object") return base;
  const o = raw as Record<string, unknown>;
  if (o.v !== 1 || !Array.isArray(o.columns) || o.columns.length < 2) return base;
  const colSplitPct =
    typeof o.colSplitPct === "number" ? Math.max(20, Math.min(80, o.colSplitPct)) : fallbackCols;

  function normLeaf(leaf: unknown): DockLeaf | null {
    if (!leaf || typeof leaf !== "object") return null;
    const L = leaf as Record<string, unknown>;
    const tabs = Array.isArray(L.tabs) ? L.tabs.filter(isPanelId) : [];
    if (!tabs.length) return null;
    const active =
      typeof L.active === "number" ? Math.max(0, Math.min(tabs.length - 1, L.active)) : 0;
    const height =
      typeof L.height === "number"
        ? Math.max(120, Math.min(720, L.height))
        : DEFAULT_PANEL_HEIGHTS[tabs[0]!];
    return { tabs, active, height };
  }

  const left = (Array.isArray(o.columns[0]) ? o.columns[0] : [])
    .map(normLeaf)
    .filter((x): x is DockLeaf => !!x);
  const right = (Array.isArray(o.columns[1]) ? o.columns[1] : [])
    .map(normLeaf)
    .filter((x): x is DockLeaf => !!x);

  const seen = new Set<PanelId>();
  for (const leaf of [...left, ...right]) {
    for (const t of leaf.tabs) seen.add(t);
  }
  const missing = ALL_PANEL_IDS.filter((id) => !seen.has(id));
  for (const id of missing) {
    left.push({ tabs: [id], active: 0, height: DEFAULT_PANEL_HEIGHTS[id] });
  }

  if (!left.length && !right.length) return base;
  return {
    v: 1,
    colSplitPct,
    columns: [left.length ? left : base.columns[0], right.length ? right : base.columns[1]],
  };
}

export function migrateDockFromLegacy(
  cols: { left?: number } | null,
  heights: Record<string, number> | null
): DockLayout {
  const layout = defaultDockLayout(cols && typeof cols.left === "number" ? cols.left : 54.5);
  if (!heights) return layout;
  for (const col of layout.columns) {
    for (const leaf of col) {
      const id = leaf.tabs[0];
      if (id && typeof heights[id] === "number") {
        leaf.height = Math.max(120, Math.min(720, heights[id]!));
      }
    }
  }
  return layout;
}

export type DockDropTarget =
  | { kind: "reorder"; col: 0 | 1; index: number }
  | { kind: "stack"; col: 0 | 1; index: number }
  | { kind: "gap"; col: 0 | 1; index: number };

/** Move or stack a panel identified by source leaf location + tab index. */
export function applyDockDrag(
  layout: DockLayout,
  from: { col: 0 | 1; leaf: number; tab: number },
  target: DockDropTarget
): DockLayout {
  const next: DockLayout = {
    v: 1,
    colSplitPct: layout.colSplitPct,
    columns: [
      layout.columns[0].map((l) => ({ ...l, tabs: [...l.tabs] })),
      layout.columns[1].map((l) => ({ ...l, tabs: [...l.tabs] })),
    ],
  };

  const srcCol = next.columns[from.col];
  const srcLeaf = srcCol[from.leaf];
  if (!srcLeaf) return layout;
  const panel = srcLeaf.tabs[from.tab];
  if (!panel) return layout;

  srcLeaf.tabs.splice(from.tab, 1);
  let removedLeaf = false;
  if (srcLeaf.tabs.length === 0) {
    srcCol.splice(from.leaf, 1);
    removedLeaf = true;
  } else {
    srcLeaf.active = Math.max(0, Math.min(srcLeaf.tabs.length - 1, srcLeaf.active));
  }

  const destCol = next.columns[target.col];

  if (target.kind === "stack") {
    let idx = target.index;
    if (from.col === target.col && removedLeaf && from.leaf < idx) idx -= 1;
    const leaf = destCol[idx];
    if (!leaf) {
      destCol.push({ tabs: [panel], active: 0, height: DEFAULT_PANEL_HEIGHTS[panel] });
    } else if (!leaf.tabs.includes(panel)) {
      leaf.tabs.push(panel);
      leaf.active = leaf.tabs.length - 1;
    }
    return next;
  }

  // reorder or gap — insert as new leaf
  let insertAt = target.index;
  if (from.col === target.col && removedLeaf && from.leaf < insertAt) insertAt -= 1;
  insertAt = Math.max(0, Math.min(destCol.length, insertAt));
  destCol.splice(insertAt, 0, {
    tabs: [panel],
    active: 0,
    height: DEFAULT_PANEL_HEIGHTS[panel],
  });
  return next;
}
