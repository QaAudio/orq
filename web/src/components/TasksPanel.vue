<script setup lang="ts">
import { computed, inject } from "vue";
import { QaPanel } from "@quantumaudio/ableton-extension-sdk/vue";
import type { DashStore } from "@/composables/useDashStore";
import { DEFAULT_PANEL_HEIGHTS } from "@/composables/useDashStore";
import { asArray } from "@/lib/format";
import StatusBadge from "./StatusBadge.vue";

defineProps<{ focused?: boolean }>();
const emit = defineEmits<{ "focus-panel": [] }>();
const store = inject("dash") as DashStore;

const tasks = computed(() => asArray(store.snapshot.value?.tasks));
const wrapStyle = computed(() => ({ maxHeight: (store.panelHeights.tasks || 260) + "px" }));

function open(id?: string) {
  if (id) store.drawerTaskId.value = id;
}

function onResizeDown(e: PointerEvent) {
  e.preventDefault();
  const startY = e.clientY;
  const startH = store.panelHeights.tasks || 260;
  const onMove = (ev: PointerEvent) => store.setPanelHeight("tasks", startH + (ev.clientY - startY));
  const onUp = () => {
    document.removeEventListener("pointermove", onMove);
    document.removeEventListener("pointerup", onUp);
  };
  document.addEventListener("pointermove", onMove);
  document.addEventListener("pointerup", onUp);
}

function onHeadDblClick() {
  const base = DEFAULT_PANEL_HEIGHTS.tasks;
  const cur = store.panelHeights.tasks || base;
  store.setPanelHeight("tasks", cur >= base * 1.8 ? base : Math.round(base * 2.2));
}
</script>

<template>
  <QaPanel
    class="panel-section"
    :class="{ 'panel-focused': focused }"
    label="Tasks"
    tabindex="0"
    data-height-panel="tasks"
    @focusin="emit('focus-panel')"
  >
    <template #header>
      <span class="count" id="count-tasks" @dblclick="onHeadDblClick">{{ tasks.length }}</span>
    </template>
    <div id="tasks">
      <p v-if="!tasks.length" class="placeholder">none</p>
      <div v-else class="table-wrap" :style="wrapStyle">
        <table>
          <thead>
            <tr>
              <th>id</th>
              <th>name</th>
              <th>status</th>
              <th>model</th>
              <th>cmd</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="t in tasks"
              :key="t.id"
              class="task-row"
              tabindex="0"
              role="button"
              :data-task-id="t.id"
              @click="open(t.id)"
              @keydown.enter.prevent="open(t.id)"
              @keydown.space.prevent="open(t.id)"
            >
              <td class="mono">{{ (t.id || "").slice(0, 8) }}</td>
              <td>{{ t.name }}</td>
              <td><StatusBadge :state="t.status" /></td>
              <td class="mono">{{ t.model_id || t.profile || "" }}</td>
              <td class="mono cmd-cell">{{ (t.command || "").slice(0, 48) }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
    <div class="panel-resize-y" data-height-panel="tasks" @pointerdown="onResizeDown" />
  </QaPanel>
</template>
