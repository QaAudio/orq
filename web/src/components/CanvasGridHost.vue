<script setup lang="ts">
import { computed, inject, ref, watch } from "vue";
import type { DashStore } from "@/composables/useDashStore";
import {
  CANVAS_GRID_COLS,
  CANVAS_GRID_MIN_H,
  CANVAS_GRID_MIN_W,
  applyItemChange,
  colWidthPx,
  gridStyleForItem,
  type CanvasGridItem,
  type CanvasGridLayout,
} from "@/lib/canvasGrid";

const props = withDefaults(
  defineProps<{
    keys: string[];
    /** Which store layout to bind. */
    surface?: "canvases" | "details";
    span2ForKey?: (key: string) => boolean;
    minW?: number;
    minH?: number;
  }>(),
  {
    surface: "canvases",
    span2ForKey: () => false,
    minW: CANVAS_GRID_MIN_W,
    minH: CANVAS_GRID_MIN_H,
  }
);

const store = inject("dash") as DashStore;
const hostEl = ref<HTMLElement | null>(null);
const draggingKey = ref<string | null>(null);
const resizingKey = ref<string | null>(null);

watch(
  () => props.keys.slice(),
  (keys) => {
    if (props.surface === "canvases") {
      store.reconcileCanvasGrid(keys, props.span2ForKey);
    } else {
      store.reconcileDetailsGrid();
    }
  },
  { immediate: true, deep: true }
);

const layout = computed((): CanvasGridLayout =>
  props.surface === "details" ? store.detailsGrid.value : store.canvasGrid.value
);
const rowHeight = computed(() => layout.value.rowHeight);
const cols = computed(() => layout.value.cols || CANVAS_GRID_COLS);

function itemFor(key: string): CanvasGridItem {
  return (
    layout.value.items[key] || {
      x: 0,
      y: 0,
      w: props.minW,
      h: props.minH,
    }
  );
}

function cardStyle(key: string) {
  return gridStyleForItem(itemFor(key), rowHeight.value);
}

function gapPx(host: HTMLElement): number {
  const g = getComputedStyle(host).gap || getComputedStyle(host).columnGap || "0";
  const n = parseFloat(g);
  return Number.isFinite(n) ? n : 0;
}

function applyPreview(key: string, patch: Partial<CanvasGridItem>) {
  if (props.surface === "details") store.previewDetailsGridItem(key, patch);
  else store.previewCanvasGridItem(key, patch);
}

function persistLayout() {
  if (props.surface === "details") store.persistDetailsGrid();
  else store.persistCanvasGrid();
}

function replaceLayout(next: CanvasGridLayout) {
  if (props.surface === "details") store.replaceDetailsGrid(next, false);
  else store.replaceCanvasGrid(next, false);
}

function isDragExcluded(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return true;
  return !!target.closest(
    "button, a, input, select, textarea, .qa-segmented, .qa-badge, .time-badge, [data-canvas-resize], .canvas-body, .panel-toolbar, .table-wrap, .event-list, .running-strip, .ops-grid"
  );
}

function isTitleDragTarget(target: EventTarget | null): boolean {
  if (!(target instanceof Element)) return false;
  if (isDragExcluded(target)) return false;
  // QaPanel title text lives in .qa-label__text / header row label
  return !!target.closest(
    ".qa-panel__header .qa-label__text, .qa-panel__header-row > .qa-label, .qa-panel__header .qa-label"
  );
}

function onCellPointerDown(key: string, e: PointerEvent) {
  if (e.button !== 0) return;
  if (!isTitleDragTarget(e.target)) return;
  e.preventDefault();
  e.stopPropagation();
  const host = hostEl.value;
  if (!host) return;

  const startItem = { ...itemFor(key) };
  const startX = e.clientX;
  const startY = e.clientY;
  const gap = gapPx(host);
  const hostRect = host.getBoundingClientRect();
  const colW = colWidthPx(hostRect.width, cols.value, gap);
  const pitchX = colW + gap;
  const pitchY = rowHeight.value + gap;
  draggingKey.value = key;
  host.classList.add("is-interacting");

  const onMove = (ev: PointerEvent) => {
    const dx = ev.clientX - startX;
    const dy = ev.clientY - startY;
    const nx = Math.round(startItem.x + dx / pitchX);
    const ny = Math.round(startItem.y + dy / pitchY);
    applyPreview(key, { x: nx, y: ny });
  };
  const onUp = () => {
    draggingKey.value = null;
    host.classList.remove("is-interacting");
    document.removeEventListener("pointermove", onMove);
    document.removeEventListener("pointerup", onUp);
    document.removeEventListener("pointercancel", onUp);
    const cur = layout.value.items[key];
    if (cur) {
      replaceLayout(
        applyItemChange(layout.value, key, cur, {
          compact: true,
          minW: props.minW,
          minH: props.minH,
        })
      );
    }
    persistLayout();
  };
  document.addEventListener("pointermove", onMove);
  document.addEventListener("pointerup", onUp);
  document.addEventListener("pointercancel", onUp);
}

function onResizeDown(key: string, edge: "e" | "s" | "se", e: PointerEvent) {
  e.preventDefault();
  e.stopPropagation();
  const host = hostEl.value;
  if (!host) return;
  const startX = e.clientX;
  const startY = e.clientY;
  const start = { ...itemFor(key) };
  const gap = gapPx(host);
  const hostRect = host.getBoundingClientRect();
  const colW = colWidthPx(hostRect.width, cols.value, gap);
  const pitchX = colW + gap;
  const pitchY = rowHeight.value + gap;
  resizingKey.value = key;
  host.classList.add("is-interacting");

  const onMove = (ev: PointerEvent) => {
    const dx = ev.clientX - startX;
    const dy = ev.clientY - startY;
    let w = start.w;
    let h = start.h;
    if (edge === "e" || edge === "se") {
      w = Math.max(props.minW, Math.round(start.w + dx / pitchX));
    }
    if (edge === "s" || edge === "se") {
      h = Math.max(props.minH, Math.round(start.h + dy / pitchY));
    }
    // Keep active item pinned (no compact mid-gesture for smoother feel) then compact on up.
    const next = applyItemChange(
      layout.value,
      key,
      { w, h },
      { compact: false, minW: props.minW, minH: props.minH }
    );
    replaceLayout(next);
  };
  const onUp = () => {
    resizingKey.value = null;
    host.classList.remove("is-interacting");
    document.removeEventListener("pointermove", onMove);
    document.removeEventListener("pointerup", onUp);
    document.removeEventListener("pointercancel", onUp);
    // Final compact pass
    const keyNow = key;
    const cur = layout.value.items[keyNow];
    if (cur) {
      const next = applyItemChange(layout.value, keyNow, cur, {
        compact: true,
        minW: props.minW,
        minH: props.minH,
      });
      replaceLayout(next);
    }
    persistLayout();
  };
  document.addEventListener("pointermove", onMove);
  document.addEventListener("pointerup", onUp);
  document.addEventListener("pointercancel", onUp);
}
</script>

<template>
  <div
    ref="hostEl"
    class="canvas-grid"
    :data-grid-surface="surface"
    :style="{
      gridTemplateColumns: `repeat(${cols}, 1fr)`,
      gridAutoRows: `${rowHeight}px`,
    }"
  >
    <div
      v-for="key in keys"
      :key="key"
      class="canvas-grid-cell"
      :class="{
        'is-dragging': draggingKey === key,
        'is-resizing': resizingKey === key,
      }"
      :data-canvas-key="key"
      :style="cardStyle(key)"
      @pointerdown="onCellPointerDown(key, $event)"
    >
      <slot :canvas-key="key" :item="itemFor(key)" />
      <div
        class="canvas-resize-e"
        data-canvas-resize="e"
        title="Drag to resize width"
        @pointerdown="onResizeDown(key, 'e', $event)"
      />
      <div
        class="canvas-resize-s"
        data-canvas-resize="s"
        title="Drag to resize height"
        @pointerdown="onResizeDown(key, 's', $event)"
      />
      <div
        class="canvas-resize-se"
        data-canvas-resize="se"
        title="Drag to resize"
        @pointerdown="onResizeDown(key, 'se', $event)"
      />
    </div>
  </div>
</template>
