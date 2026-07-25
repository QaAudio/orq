<script setup lang="ts">
import { computed, inject, onMounted, ref } from "vue";
import type { DashStore } from "@/composables/useDashStore";
import DockHost from "./DockHost.vue";

const store = inject("dash") as DashStore;
const detailsEl = ref<HTMLElement | null>(null);
const focused = ref<string | null>(null);

const colStyle = computed(() => ({
  "--details-col-left": store.detailsColLeftPct.value + "%",
  "--details-col-right": 100 - store.detailsColLeftPct.value + "%",
  "--details-split-pct": store.detailsColLeftPct.value + "%",
}));

function onSplitterDown(e: PointerEvent) {
  if (window.matchMedia("(max-width: 900px)").matches) return;
  e.preventDefault();
  const el = detailsEl.value;
  if (!el) return;
  const splitter = e.currentTarget as HTMLElement;
  splitter.classList.add("dragging");
  const onMove = (ev: PointerEvent) => {
    const rect = el.getBoundingClientRect();
    if (rect.width <= 0) return;
    store.applyCols(((ev.clientX - rect.left) / rect.width) * 100);
  };
  const onUp = () => {
    splitter.classList.remove("dragging");
    document.removeEventListener("pointermove", onMove);
    document.removeEventListener("pointerup", onUp);
  };
  document.addEventListener("pointermove", onMove);
  document.addEventListener("pointerup", onUp);
}

function onSplitterKey(e: KeyboardEvent) {
  if (window.matchMedia("(max-width: 900px)").matches) return;
  if (e.key === "ArrowLeft") {
    e.preventDefault();
    store.applyCols(store.detailsColLeftPct.value - 2);
  } else if (e.key === "ArrowRight") {
    e.preventDefault();
    store.applyCols(store.detailsColLeftPct.value + 2);
  }
}

onMounted(() => {
  /* layout prefs already loaded in store */
});
</script>

<template>
  <main
    id="view-details"
    ref="detailsEl"
    class="view"
    :class="{ active: store.view.value === 'details' }"
    role="tabpanel"
    :style="colStyle"
  >
    <button
      type="button"
      class="details-col-splitter"
      id="details-col-splitter"
      role="separator"
      aria-orientation="vertical"
      aria-label="Resize Details columns"
      tabindex="0"
      @pointerdown="onSplitterDown"
      @keydown="onSplitterKey"
    />
    <DockHost v-model:focused="focused" />
  </main>
</template>
