<script setup lang="ts">
import { computed, inject } from "vue";
import { QaPanel } from "@quantumaudio/ableton-extension-sdk/vue";
import type { DashStore } from "@/composables/useDashStore";
import { asArray } from "@/lib/format";
import StatusBadge from "./StatusBadge.vue";

defineProps<{ focused?: boolean }>();
const emit = defineEmits<{ "focus-panel": [] }>();
const store = inject("dash") as DashStore;
const jobs = computed(() => asArray(store.snapshot.value?.jobs));
const wrapStyle = computed(() => ({ maxHeight: (store.panelHeights.jobs || 260) + "px" }));

function onResizeDown(e: PointerEvent) {
  e.preventDefault();
  const startY = e.clientY;
  const startH = store.panelHeights.jobs || 260;
  const onMove = (ev: PointerEvent) => store.setPanelHeight("jobs", startH + (ev.clientY - startY));
  const onUp = () => {
    document.removeEventListener("pointermove", onMove);
    document.removeEventListener("pointerup", onUp);
  };
  document.addEventListener("pointermove", onMove);
  document.addEventListener("pointerup", onUp);
}
</script>

<template>
  <QaPanel
    class="panel-section"
    :class="{ 'panel-focused': focused }"
    label="Jobs / routing"
    tabindex="0"
    data-height-panel="jobs"
    @focusin="emit('focus-panel')"
  >
    <template #header>
      <span class="count" id="count-jobs">{{ jobs.length }}</span>
    </template>
    <div id="jobs">
      <p v-if="!jobs.length" class="placeholder">none</p>
      <div v-else class="table-wrap" :style="wrapStyle">
        <table>
          <thead>
            <tr>
              <th>id</th>
              <th>name</th>
              <th>status</th>
              <th>strategy</th>
              <th>route</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="j in jobs" :key="j.id">
              <td class="mono">{{ (j.id || "").slice(0, 8) }}</td>
              <td>{{ j.name }}</td>
              <td><StatusBadge :state="j.status" /></td>
              <td>{{ j.strategy }}</td>
              <td class="mono">{{ j.route_reason || "" }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
    <div class="panel-resize-y" data-height-panel="jobs" @pointerdown="onResizeDown" />
  </QaPanel>
</template>
