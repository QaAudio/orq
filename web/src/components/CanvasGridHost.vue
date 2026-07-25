<script setup lang="ts">
import { computed, inject, watch } from "vue";
import type { DashStore } from "@/composables/useDashStore";
import {
  CANVAS_GRID_COLS,
  CANVAS_GRID_MIN_H,
  CANVAS_GRID_MIN_W,
  gridStyleForItem,
  type CanvasGridItem,
} from "@/lib/canvasGrid";

const props = defineProps<{
  keys: string[];
  span2ForKey: (key: string) => boolean;
}>();

const store = inject("dash") as DashStore;

watch(
  () => props.keys.slice(),
  (keys) => {
    store.reconcileCanvasGrid(keys, props.span2ForKey);
  },
  { immediate: true, deep: true }
);

const layout = computed(() => store.canvasGrid.value);
const rowHeight = computed(() => layout.value.rowHeight);

function itemFor(key: string): CanvasGridItem {
  return (
    layout.value.items[key] || {
      x: 0,
      y: 0,
      w: CANVAS_GRID_MIN_W,
      h: CANVAS_GRID_MIN_H,
    }
  );
}

function cardStyle(key: string) {
  return gridStyleForItem(itemFor(key), rowHeight.value);
}

function onResizeDown(key: string, edge: "e" | "s" | "se", e: PointerEvent) {
  e.preventDefault();
  e.stopPropagation();
  const el = (e.currentTarget as HTMLElement).closest(".canvas-grid-cell") as HTMLElement | null;
  const host = el?.parentElement;
  if (!host) return;
  const startX = e.clientX;
  const startY = e.clientY;
  const start = { ...itemFor(key) };
  const hostRect = host.getBoundingClientRect();
  const colW = hostRect.width / CANVAS_GRID_COLS;

  const onMove = (ev: PointerEvent) => {
    const dx = ev.clientX - startX;
    const dy = ev.clientY - startY;
    let w = start.w;
    let h = start.h;
    if (edge === "e" || edge === "se") {
      w = Math.max(CANVAS_GRID_MIN_W, Math.round(start.w + dx / colW));
    }
    if (edge === "s" || edge === "se") {
      h = Math.max(CANVAS_GRID_MIN_H, Math.round(start.h + dy / rowHeight.value));
    }
    store.setCanvasGridItem(key, { w, h });
  };
  const onUp = () => {
    document.removeEventListener("pointermove", onMove);
    document.removeEventListener("pointerup", onUp);
  };
  document.addEventListener("pointermove", onMove);
  document.addEventListener("pointerup", onUp);
}
</script>

<template>
  <div
    class="canvas-grid"
    :style="{
      gridTemplateColumns: `repeat(${CANVAS_GRID_COLS}, 1fr)`,
      gridAutoRows: `${rowHeight}px`,
    }"
  >
    <div
      v-for="key in keys"
      :key="key"
      class="canvas-grid-cell"
      :data-canvas-key="key"
      :style="cardStyle(key)"
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
