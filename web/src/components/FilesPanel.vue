<script setup lang="ts">
import { computed, inject } from "vue";
import { QaPanel } from "@quantumaudio/ableton-extension-sdk/vue";
import type { DashStore } from "@/composables/useDashStore";
import { asArray } from "@/lib/format";

defineProps<{ focused?: boolean }>();
const emit = defineEmits<{ "focus-panel": [] }>();
const store = inject("dash") as DashStore;
const files = computed(() => asArray(store.snapshot.value?.files));
const listStyle = computed(() => ({ maxHeight: (store.panelHeights.files || 180) + "px" }));

function onResizeDown(e: PointerEvent) {
  e.preventDefault();
  const startY = e.clientY;
  const startH = store.panelHeights.files || 180;
  const onMove = (ev: PointerEvent) => store.setPanelHeight("files", startH + (ev.clientY - startY));
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
    label="Workspace files"
    tabindex="0"
    data-height-panel="files"
    @focusin="emit('focus-panel')"
  >
    <template #header>
      <span class="count" id="count-files">{{ files.length }}</span>
    </template>
    <div id="files">
      <p v-if="!files.length" class="placeholder">none</p>
      <div v-else class="file-grid" :style="listStyle">
        <span v-for="f in files" :key="f" class="file-chip" :title="f">{{ f }}</span>
      </div>
    </div>
    <div class="panel-resize-y" data-height-panel="files" @pointerdown="onResizeDown" />
  </QaPanel>
</template>
