<script setup lang="ts">
import { inject } from "vue";
import { QaSegmented } from "@quantumaudio/ableton-extension-sdk/vue";
import type { DashStore } from "@/composables/useDashStore";
import type { FilterMode } from "@/lib/types";

const store = inject("dash") as DashStore;

const options = [
  { value: "active", label: "Active" },
  { value: "all", label: "All" },
  { value: "archived", label: "Archived" },
];

function onUpdate(v: string) {
  if (v === "active" || v === "all" || v === "archived") {
    store.setFilterMode(v as FilterMode);
  }
}
</script>

<template>
  <QaSegmented
    class="state-filter"
    :model-value="store.filterMode.value"
    :options="options"
    aria-label="State filter"
    @update:model-value="onUpdate"
  />
</template>
