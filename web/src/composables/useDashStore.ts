import { computed, onMounted, onUnmounted, reactive, ref } from "vue";
import type { DashSnapshot, DockLayout, FilterMode, PanelId } from "@/lib/types";
import type { TimeFormatMode } from "@/lib/format";
import {
  asArray,
  isActiveTaskStatus,
  isTimeFormatMode,
  nextTimeFormatMode,
  relatedForTask,
} from "@/lib/format";
import {
  DEFAULT_PANEL_HEIGHTS,
  applyDockDrag,
  defaultDockLayout,
  migrateDockFromLegacy,
  normalizeDockLayout,
  type DockDropTarget,
} from "@/lib/dock";
import {
  defaultCanvasGridLayout,
  normalizeCanvasGridLayout,
  reconcileCanvasItems,
  resizeCanvasItem,
  type CanvasGridItem,
  type CanvasGridLayout,
} from "@/lib/canvasGrid";

const POLL_MS = 1000;
const STALE_AFTER_MISSED = 3;
export const LOG_TAIL_BYTES = 8192;

const VIEW_KEY = "porq.dash.view";
const FILTER_KEY = "porq.dash.filter.state";
const THEME_KEY = "porq.dash.qa-theme";
const LAYOUT_COLS_KEY = "porq.dash.layout.cols";
const LAYOUT_HEIGHTS_KEY = "porq.dash.layout.heights";
const LAYOUT_DOCK_KEY = "porq.dash.layout.dock";
const LAYOUT_CANVASES_KEY = "porq.dash.layout.canvases";
const TIME_FORMATS_KEY = "porq.dash.time.formats";
/** @deprecated migrated once into TIME_FORMATS_KEY default seed */
const TIME_FORMAT_LEGACY_KEY = "porq.dash.time.format";

function loadJson<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    if (!raw) return fallback;
    return JSON.parse(raw) as T;
  } catch {
    return fallback;
  }
}

function saveJson(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    /* ignore */
  }
}

export { DEFAULT_PANEL_HEIGHTS };

export function useDashStore() {
  const snapshot = ref<DashSnapshot | null>(null);
  const lastSuccessAt = ref<number | null>(null);
  const missedPolls = ref(0);
  const errorMsg = ref("");
  const fetchInFlight = ref(false);
  const stampText = ref("connecting…");
  const stampClass = ref("");

  const view = ref<"canvases" | "details">("canvases");
  const userPickedView = ref(false);
  const filterMode = ref<FilterMode>("active");
  const qaTheme = ref<"dark" | "light">("dark");
  const detailsColLeftPct = ref(54.5);
  const panelHeights = reactive<Record<string, number>>({ ...DEFAULT_PANEL_HEIGHTS });
  const dockLayout = ref<DockLayout>(defaultDockLayout());
  const canvasGrid = ref<CanvasGridLayout>(defaultCanvasGridLayout());
  /** Per-badge modes; unknown ids fall back to timeFormatDefault (relative unless legacy migrated). */
  const timeFormats = ref<Record<string, TimeFormatMode>>({});
  const timeFormatDefault = ref<TimeFormatMode>("relative");
  const selectedTaskId = ref<string | null>(null);
  const drawerTaskId = ref<string | null>(null);
  const boardExpanded = reactive(new Set<string>());
  const cfBusy = ref(false);
  const cfStatus = ref("");
  const cfError = ref(false);

  const staticDemo = computed(() => !!(snapshot.value && snapshot.value.static_demo));

  const runningTasks = computed(() =>
    asArray(snapshot.value?.tasks).filter((t) => isActiveTaskStatus(t.status))
  );

  const relatedKeys = computed(() => {
    const id = selectedTaskId.value;
    if (!id || !snapshot.value) return new Set<string>();
    const task = asArray(snapshot.value.tasks).find((t) => t.id === id);
    return relatedForTask(snapshot.value, id, asArray(task?.claims)).keys;
  });

  const relatedLeaseKeys = computed(() => {
    const id = selectedTaskId.value;
    if (!id || !snapshot.value) return new Set<string>();
    const task = asArray(snapshot.value.tasks).find((t) => t.id === id);
    return relatedForTask(snapshot.value, id, asArray(task?.claims)).leaseKeys;
  });

  function persistDock() {
    saveJson(LAYOUT_DOCK_KEY, dockLayout.value);
    detailsColLeftPct.value = dockLayout.value.colSplitPct;
    saveJson(LAYOUT_COLS_KEY, { left: detailsColLeftPct.value });
    const heights: Record<string, number> = {};
    for (const col of dockLayout.value.columns) {
      for (const leaf of col) {
        const id = leaf.tabs[leaf.active] || leaf.tabs[0];
        if (id) heights[id] = leaf.height;
      }
    }
    Object.assign(panelHeights, heights);
    saveJson(LAYOUT_HEIGHTS_KEY, { ...panelHeights });
  }

  function applyTheme(theme: "dark" | "light") {
    qaTheme.value = theme;
    document.documentElement.setAttribute("data-qa-theme", theme);
    try {
      localStorage.setItem(THEME_KEY, theme);
    } catch {
      /* ignore */
    }
  }

  function setView(next: "canvases" | "details", opts: { user?: boolean; persist?: boolean } = {}) {
    if (opts.user) userPickedView.value = true;
    view.value = next;
    if (opts.persist !== false) {
      try {
        localStorage.setItem(VIEW_KEY, next);
      } catch {
        /* ignore */
      }
    }
  }

  function setFilterMode(mode: FilterMode) {
    filterMode.value = mode;
    try {
      localStorage.setItem(FILTER_KEY, mode);
    } catch {
      /* ignore */
    }
  }

  function getTimeFormat(id: string): TimeFormatMode {
    const key = String(id || "");
    const mapped = timeFormats.value[key];
    if (mapped) return mapped;
    return timeFormatDefault.value;
  }

  function cycleTimeFormat(id: string) {
    const key = String(id || "");
    if (!key) return;
    const next = nextTimeFormatMode(getTimeFormat(key));
    timeFormats.value = { ...timeFormats.value, [key]: next };
    saveJson(TIME_FORMATS_KEY, timeFormats.value);
  }

  function applyCols(pct: number) {
    const clamped = Math.max(20, Math.min(80, pct));
    detailsColLeftPct.value = clamped;
    dockLayout.value = { ...dockLayout.value, colSplitPct: clamped };
    persistDock();
  }

  function setPanelHeight(id: string, h: number) {
    const height = Math.max(120, Math.min(720, h));
    panelHeights[id] = height;
    const next = {
      ...dockLayout.value,
      columns: dockLayout.value.columns.map((col) =>
        col.map((leaf) =>
          leaf.tabs.includes(id as PanelId) ? { ...leaf, height } : leaf
        )
      ) as DockLayout["columns"],
    };
    dockLayout.value = next;
    persistDock();
  }

  function setLeafHeight(col: 0 | 1, leafIndex: number, h: number) {
    const height = Math.max(120, Math.min(720, h));
    const cols = dockLayout.value.columns.map((c) => c.map((l) => ({ ...l, tabs: [...l.tabs] }))) as DockLayout["columns"];
    const leaf = cols[col][leafIndex];
    if (!leaf) return;
    leaf.height = height;
    dockLayout.value = { ...dockLayout.value, columns: cols };
    persistDock();
  }

  function setLeafActive(col: 0 | 1, leafIndex: number, active: number) {
    const cols = dockLayout.value.columns.map((c) => c.map((l) => ({ ...l, tabs: [...l.tabs] }))) as DockLayout["columns"];
    const leaf = cols[col][leafIndex];
    if (!leaf) return;
    leaf.active = Math.max(0, Math.min(leaf.tabs.length - 1, active));
    dockLayout.value = { ...dockLayout.value, columns: cols };
    persistDock();
  }

  function dockDrag(
    from: { col: 0 | 1; leaf: number; tab: number },
    target: DockDropTarget
  ) {
    dockLayout.value = applyDockDrag(dockLayout.value, from, target);
    persistDock();
  }

  function persistCanvasGrid() {
    saveJson(LAYOUT_CANVASES_KEY, canvasGrid.value);
  }

  function reconcileCanvasGrid(keys: string[], span2ForKey: (key: string) => boolean) {
    const next = reconcileCanvasItems(canvasGrid.value, keys, span2ForKey);
    const same =
      Object.keys(next.items).length === Object.keys(canvasGrid.value.items).length &&
      keys.every((k) => {
        const a = next.items[k];
        const b = canvasGrid.value.items[k];
        return a && b && a.x === b.x && a.y === b.y && a.w === b.w && a.h === b.h;
      });
    if (same) return;
    canvasGrid.value = next;
    persistCanvasGrid();
  }

  function setCanvasGridItem(key: string, patch: Partial<CanvasGridItem>) {
    canvasGrid.value = resizeCanvasItem(canvasGrid.value, key, patch);
    persistCanvasGrid();
  }

  function selectTask(id: string | null) {
    selectedTaskId.value = id;
  }

  function loadPrefs() {
    try {
      const savedView = localStorage.getItem(VIEW_KEY);
      if (savedView === "details" || savedView === "canvases") {
        userPickedView.value = true;
        view.value = savedView;
      }
    } catch {
      /* ignore */
    }
    try {
      const f = localStorage.getItem(FILTER_KEY);
      if (f === "active" || f === "all" || f === "archived") filterMode.value = f;
    } catch {
      /* ignore */
    }
    const rawMap = loadJson<Record<string, unknown>>(TIME_FORMATS_KEY, {});
    const cleaned: Record<string, TimeFormatMode> = {};
    for (const [k, v] of Object.entries(rawMap || {})) {
      if (k && isTimeFormatMode(v)) cleaned[k] = v;
    }
    timeFormats.value = cleaned;
    try {
      const legacy = localStorage.getItem(TIME_FORMAT_LEGACY_KEY);
      if (isTimeFormatMode(legacy)) {
        timeFormatDefault.value = legacy;
        localStorage.removeItem(TIME_FORMAT_LEGACY_KEY);
      }
    } catch {
      /* ignore */
    }
    const boot = document.documentElement.getAttribute("data-theme-boot");
    const mapped =
      boot === "light" || boot === "system"
        ? "light"
        : boot === "dark" || boot === "default" || boot === "dracula"
          ? "dark"
          : null;
    let theme: "dark" | "light" = mapped || "dark";
    try {
      const stored = localStorage.getItem(THEME_KEY);
      if (stored === "dark" || stored === "light") theme = stored;
    } catch {
      /* ignore */
    }
    applyTheme(theme);

    const cols = loadJson<{ left?: number } | null>(LAYOUT_COLS_KEY, null);
    if (cols && typeof cols.left === "number") detailsColLeftPct.value = cols.left;
    const heights = loadJson<Record<string, number> | null>(LAYOUT_HEIGHTS_KEY, null);
    if (heights) Object.assign(panelHeights, DEFAULT_PANEL_HEIGHTS, heights);

    const dockRaw = loadJson<unknown>(LAYOUT_DOCK_KEY, null);
    if (dockRaw) {
      dockLayout.value = normalizeDockLayout(dockRaw, detailsColLeftPct.value);
    } else {
      dockLayout.value = migrateDockFromLegacy(cols, heights);
    }
    detailsColLeftPct.value = dockLayout.value.colSplitPct;

    canvasGrid.value = normalizeCanvasGridLayout(loadJson<unknown>(LAYOUT_CANVASES_KEY, null));
  }

  function updateFreshness() {
    if (!lastSuccessAt.value) {
      stampText.value = "connecting…";
      stampClass.value = "";
      return;
    }
    const sec = Math.max(0, Math.round((Date.now() - lastSuccessAt.value) / 1000));
    const stale = missedPolls.value >= STALE_AFTER_MISSED;
    stampText.value = stale ? `stale · ${sec}s ago` : `updated ${sec}s ago`;
    stampClass.value = stale ? "stale" : "live";
  }

  async function tick() {
    if (fetchInFlight.value) return;
    fetchInFlight.value = true;
    try {
      const res = await fetch("data.json?ts=" + Date.now(), { cache: "no-store" });
      if (!res.ok) throw new Error("data.json HTTP " + res.status);
      const d = (await res.json()) as DashSnapshot;
      snapshot.value = d;
      lastSuccessAt.value = Date.now();
      missedPolls.value = 0;
      errorMsg.value = "";
      if (selectedTaskId.value) {
        const still = asArray(d.tasks).some(
          (t) => t.id === selectedTaskId.value && isActiveTaskStatus(t.status)
        );
        if (!still) selectedTaskId.value = null;
      }
      if (!userPickedView.value) {
        const canvases = asArray(d.canvases);
        view.value = canvases.length === 0 ? "details" : "canvases";
      }
      updateFreshness();
    } catch (e) {
      missedPolls.value += 1;
      const msg = e instanceof Error ? e.message : String(e);
      if (snapshot.value) {
        errorMsg.value = "Fetch failed — showing last snapshot. " + msg;
        updateFreshness();
      } else {
        errorMsg.value = "Failed to load data.json: " + msg;
        stampText.value = "error";
        stampClass.value = "error";
      }
    } finally {
      fetchInFlight.value = false;
    }
  }

  async function postPoiApi(path: string, body: Record<string, unknown>) {
    if (staticDemo.value) throw new Error("static demo — mutate disabled");
    const res = await fetch(path, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify(body || {}),
    });
    const data = await res.json().catch(() => ({}));
    if (!res.ok || data.ok === false) {
      throw new Error((data && data.error) || res.statusText || "request failed");
    }
    return data;
  }

  let pollTimer: ReturnType<typeof setInterval> | null = null;
  let freshnessTimer: ReturnType<typeof setInterval> | null = null;

  onMounted(() => {
    loadPrefs();
    tick();
    pollTimer = setInterval(tick, POLL_MS);
    freshnessTimer = setInterval(updateFreshness, 250);
  });

  onUnmounted(() => {
    if (pollTimer) clearInterval(pollTimer);
    if (freshnessTimer) clearInterval(freshnessTimer);
  });

  return {
    snapshot,
    lastSuccessAt,
    missedPolls,
    errorMsg,
    stampText,
    stampClass,
    view,
    filterMode,
    qaTheme,
    detailsColLeftPct,
    panelHeights,
    dockLayout,
    canvasGrid,
    timeFormats,
    timeFormatDefault,
    selectedTaskId,
    runningTasks,
    relatedKeys,
    relatedLeaseKeys,
    drawerTaskId,
    boardExpanded,
    cfBusy,
    cfStatus,
    cfError,
    staticDemo,
    setView,
    setFilterMode,
    applyTheme,
    applyCols,
    setPanelHeight,
    setLeafHeight,
    setLeafActive,
    dockDrag,
    reconcileCanvasGrid,
    setCanvasGridItem,
    selectTask,
    getTimeFormat,
    cycleTimeFormat,
    tick,
    postPoiApi,
  };
}

export type DashStore = ReturnType<typeof useDashStore>;
