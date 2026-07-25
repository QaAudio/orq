<script setup lang="ts">
import { computed, inject } from "vue";
import { QaPanel } from "@quantumaudio/ableton-extension-sdk/vue";
import type { DashStore } from "@/composables/useDashStore";
import { asArray } from "@/lib/format";

defineProps<{ focused?: boolean }>();
const emit = defineEmits<{ "focus-panel": [] }>();
const store = inject("dash") as DashStore;
const aff = computed(() => asArray(store.snapshot.value?.affinities));
const wrapStyle = computed(() => ({ maxHeight: (store.panelHeights.aff || 260) + "px" }));

function onResizeDown(e: PointerEvent) {
  e.preventDefault();
  const startY = e.clientY;
  const startH = store.panelHeights.aff || 260;
  const onMove = (ev: PointerEvent) => store.setPanelHeight("aff", startH + (ev.clientY - startY));
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
    label="Affinities"
    tabindex="0"
    data-height-panel="aff"
    @focusin="emit('focus-panel')"
  >
    <template #header>
      <span class="count" id="count-aff">{{ aff.length }}</span>
    </template>
    <div id="aff">
      <p v-if="!aff.length" class="placeholder">none</p>
      <div v-else class="table-wrap" :style="wrapStyle">
        <table>
          <thead>
            <tr>
              <th>class</th>
              <th>model</th>
              <th>score</th>
              <th>samples</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="(a, i) in aff" :key="i">
              <td class="mono">{{ a.class }}</td>
              <td>{{ a.model_id }}</td>
              <td>{{ Number(a.score || 0).toFixed(3) }}</td>
              <td>n={{ a.n }}</td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
    <div class="panel-resize-y" data-height-panel="aff" @pointerdown="onResizeDown" />
  </QaPanel>
</template>
