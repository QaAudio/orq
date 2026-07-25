<script setup lang="ts">
import { computed, inject, ref, type Component } from "vue";
import type { DashStore } from "@/composables/useDashStore";
import type { PanelId } from "@/lib/types";
import { PANEL_LABELS } from "@/lib/dock";
import OpsHealth from "./OpsHealth.vue";
import RunningTasksPanel from "./RunningTasksPanel.vue";
import BoardPanel from "./BoardPanel.vue";
import TasksPanel from "./TasksPanel.vue";
import JobsPanel from "./JobsPanel.vue";
import AffPanel from "./AffPanel.vue";
import EventsPanel from "./EventsPanel.vue";
import FilesPanel from "./FilesPanel.vue";

const store = inject("dash") as DashStore;
const focused = defineModel<string | null>("focused", { default: null });

const PANEL_MAP: Record<PanelId, Component> = {
  "ops-health": OpsHealth,
  "running-tasks": RunningTasksPanel,
  board: BoardPanel,
  tasks: TasksPanel,
  jobs: JobsPanel,
  aff: AffPanel,
  events: EventsPanel,
  files: FilesPanel,
};

const layout = computed(() => store.dockLayout.value);
const dragOver = ref<string | null>(null);

const dragFrom = { col: 0 as 0 | 1, leaf: 0, tab: 0 };

function onTabDragStart(col: 0 | 1, leaf: number, tab: number, e: DragEvent) {
  dragFrom.col = col;
  dragFrom.leaf = leaf;
  dragFrom.tab = tab;
  e.dataTransfer?.setData("text/porq-dock", JSON.stringify({ col, leaf, tab }));
  if (e.dataTransfer) e.dataTransfer.effectAllowed = "move";
}

function onLeafDrop(col: 0 | 1, leafIndex: number, e: DragEvent, mode: "stack" | "gap") {
  e.preventDefault();
  e.stopPropagation();
  dragOver.value = null;
  const raw = e.dataTransfer?.getData("text/porq-dock");
  if (!raw) return;
  let from: { col: 0 | 1; leaf: number; tab: number };
  try {
    from = JSON.parse(raw);
  } catch {
    return;
  }
  if (mode === "stack") {
    store.dockDrag(from, { kind: "stack", col, index: leafIndex });
  } else {
    store.dockDrag(from, { kind: "gap", col, index: leafIndex });
  }
}

function onColGapDrop(col: 0 | 1, index: number, e: DragEvent) {
  e.preventDefault();
  dragOver.value = null;
  const raw = e.dataTransfer?.getData("text/porq-dock");
  if (!raw) return;
  let from: { col: 0 | 1; leaf: number; tab: number };
  try {
    from = JSON.parse(raw);
  } catch {
    return;
  }
  store.dockDrag(from, { kind: "gap", col, index });
}

function allowDrop(e: DragEvent) {
  e.preventDefault();
  if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
}

function setDragOver(id: string | null) {
  dragOver.value = id;
}

function onResizeDown(col: 0 | 1, leafIndex: number, e: PointerEvent) {
  e.preventDefault();
  e.stopPropagation();
  const startY = e.clientY;
  const startH = layout.value.columns[col][leafIndex]?.height || 260;
  const target = e.currentTarget as HTMLElement;
  target.setPointerCapture?.(e.pointerId);
  const onMove = (ev: PointerEvent) =>
    store.setLeafHeight(col, leafIndex, startH + (ev.clientY - startY));
  const onUp = () => {
    document.removeEventListener("pointermove", onMove);
    document.removeEventListener("pointerup", onUp);
  };
  document.addEventListener("pointermove", onMove);
  document.addEventListener("pointerup", onUp);
}

function activePanel(col: 0 | 1, leafIndex: number): PanelId | null {
  const leaf = layout.value.columns[col][leafIndex];
  if (!leaf) return null;
  return leaf.tabs[leaf.active] || leaf.tabs[0] || null;
}
</script>

<template>
  <div class="dock-host">
    <div
      v-for="(colLeaves, colIdx) in layout.columns"
      :key="colIdx"
      class="dock-column"
      :data-dock-col="colIdx"
      @dragover="allowDrop"
    >
      <div
        class="dock-gap"
        data-testid="dock-gap"
        :class="{ 'drag-over': dragOver === `gap-${colIdx}-0` }"
        @dragover="
          allowDrop($event);
          setDragOver(`gap-${colIdx}-0`);
        "
        @dragleave="setDragOver(null)"
        @drop="onColGapDrop(colIdx as 0 | 1, 0, $event)"
      />
      <div
        v-for="(leaf, leafIndex) in colLeaves"
        :key="colIdx + '-' + leafIndex + '-' + leaf.tabs.join(',')"
        class="dock-leaf"
        :class="{ 'drag-over': dragOver === `leaf-${colIdx}-${leafIndex}` }"
        :data-dock-leaf="leafIndex"
        :style="{ '--dock-leaf-height': leaf.height + 'px' }"
        @dragover="
          allowDrop($event);
          setDragOver(`leaf-${colIdx}-${leafIndex}`);
        "
        @dragleave="setDragOver(null)"
        @drop="onLeafDrop(colIdx as 0 | 1, leafIndex, $event, 'stack')"
      >
        <div v-if="leaf.tabs.length > 1" class="dock-tabs" role="tablist">
          <button
            v-for="(tabId, tabIndex) in leaf.tabs"
            :key="tabId"
            type="button"
            class="dock-tab"
            role="tab"
            draggable="true"
            :aria-selected="leaf.active === tabIndex ? 'true' : 'false'"
            :class="{ active: leaf.active === tabIndex }"
            :data-panel-tab="tabId"
            @click="store.setLeafActive(colIdx as 0 | 1, leafIndex, tabIndex)"
            @dragstart="onTabDragStart(colIdx as 0 | 1, leafIndex, tabIndex, $event)"
          >
            {{ PANEL_LABELS[tabId] }}
          </button>
        </div>
        <div
          v-else
          class="dock-drag-handle"
          draggable="true"
          title="Drag to move panel"
          @dragstart="onTabDragStart(colIdx as 0 | 1, leafIndex, leaf.active, $event)"
        >
          ⋮⋮
        </div>
        <div class="dock-leaf-body">
          <component
            :is="PANEL_MAP[activePanel(colIdx as 0 | 1, leafIndex)!]"
            v-if="activePanel(colIdx as 0 | 1, leafIndex)"
            :focused="focused === activePanel(colIdx as 0 | 1, leafIndex)"
            @focus-panel="focused = activePanel(colIdx as 0 | 1, leafIndex)"
          />
        </div>
        <div
          class="dock-leaf-resize"
          data-dock-resize
          title="Drag edge to resize height"
          @pointerdown="onResizeDown(colIdx as 0 | 1, leafIndex, $event)"
        />
        <div
          class="dock-gap"
          :class="{ 'drag-over': dragOver === `gap-${colIdx}-${leafIndex + 1}` }"
          @dragover="
            allowDrop($event);
            setDragOver(`gap-${colIdx}-${leafIndex + 1}`);
          "
          @dragleave="setDragOver(null)"
          @drop="onColGapDrop(colIdx as 0 | 1, leafIndex + 1, $event)"
        />
      </div>
    </div>
  </div>
</template>
