import { computed, onMounted, onUnmounted, reactive, ref } from "vue";
import type { DashSnapshot, FilterMode } from "@/lib/types";
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
  defaultCanvasGridLayout,
  defaultDetailsGridLayout,
  DETAILS_PANEL_IDS,
  migrateDockToDetailsGrid,
  normalizeCanvasGridLayout,
  reconcileCanvasItems,
  applyItemChange,
  type CanvasGridItem,
  type CanvasGridLayout,
} from "@/lib/canvasGrid";

const POLL_MS = 1000;
const STALE_AFTER_MISSED = 3;
export const LOG_TAIL_BYTES = 8192;

const VIEW_KEY = "porq.dash.view";
const FILTER_KEY = "porq.dash.filter.state";
const THEME_KEY = "porq.dash.qa-theme";
/** @deprecated migrated into details grid */
const LAYOUT_DOCK_KEY = "porq.dash.layout.dock";
const LAYOUT_CANVASES_KEY = "porq.dash.layout.canvases";
const LAYOUT_DETAILS_KEY = "porq.dash.layout.details";
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
  const canvasGrid = ref<CanvasGridLayout>(defaultCanvasGridLayout());
  const detailsGrid = ref<CanvasGridLayout>(defaultDetailsGridLayout());
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

  function persistCanvasGrid() {
    saveJson(LAYOUT_CANVASES_KEY, canvasGrid.value);
  }

  function persistDetailsGrid() {
    saveJson(LAYOUT_DETAILS_KEY, detailsGrid.value);
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

  function reconcileDetailsGrid() {
    const keys = [...DETAILS_PANEL_IDS];
    const next = reconcileCanvasItems(detailsGrid.value, keys, () => false);
    const same =
      Object.keys(next.items).length === Object.keys(detailsGrid.value.items).length &&
      keys.every((k) => {
        const a = next.items[k];
        const b = detailsGrid.value.items[k];
        return a && b && a.x === b.x && a.y === b.y && a.w === b.w && a.h === b.h;
      });
    if (same) return;
    detailsGrid.value = next;
    persistDetailsGrid();
  }

  /** Live gesture update (push collisions, defer compact + persist). */
  function previewCanvasGridItem(key: string, patch: Partial<CanvasGridItem>) {
    canvasGrid.value = applyItemChange(canvasGrid.value, key, patch, { compact: false });
  }

  function previewDetailsGridItem(key: string, patch: Partial<CanvasGridItem>) {
    detailsGrid.value = applyItemChange(detailsGrid.value, key, patch, { compact: false });
  }

  function setCanvasGridItem(key: string, patch: Partial<CanvasGridItem>) {
    canvasGrid.value = applyItemChange(canvasGrid.value, key, patch);
    persistCanvasGrid();
  }

  function setDetailsGridItem(key: string, patch: Partial<CanvasGridItem>) {
    detailsGrid.value = applyItemChange(detailsGrid.value, key, patch);
    persistDetailsGrid();
  }

  function replaceCanvasGrid(layout: CanvasGridLayout, persist = true) {
    canvasGrid.value = layout;
    if (persist) persistCanvasGrid();
  }

  function replaceDetailsGrid(layout: CanvasGridLayout, persist = true) {
    detailsGrid.value = layout;
    if (persist) persistDetailsGrid();
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

    canvasGrid.value = normalizeCanvasGridLayout(loadJson<unknown>(LAYOUT_CANVASES_KEY, null));

    const detailsRaw = loadJson<unknown>(LAYOUT_DETAILS_KEY, null);
    if (detailsRaw) {
      detailsGrid.value = normalizeCanvasGridLayout(detailsRaw, defaultDetailsGridLayout());
    } else {
      const dockRaw = loadJson<unknown>(LAYOUT_DOCK_KEY, null);
      const migrated = migrateDockToDetailsGrid(dockRaw);
      detailsGrid.value = migrated || defaultDetailsGridLayout();
      persistDetailsGrid();
      try {
        if (dockRaw) localStorage.removeItem(LAYOUT_DOCK_KEY);
      } catch {
        /* ignore */
      }
    }
    reconcileDetailsGrid();
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
    canvasGrid,
    detailsGrid,
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
    reconcileCanvasGrid,
    reconcileDetailsGrid,
    previewCanvasGridItem,
    previewDetailsGridItem,
    setCanvasGridItem,
    setDetailsGridItem,
    replaceCanvasGrid,
    replaceDetailsGrid,
    persistCanvasGrid,
    persistDetailsGrid,
    selectTask,
    getTimeFormat,
    cycleTimeFormat,
    tick,
    postPoiApi,
  };
}

export type DashStore = ReturnType<typeof useDashStore>;
