<script setup lang="ts">
import { computed, inject } from "vue";
import type { DashStore } from "@/composables/useDashStore";
import { formatTime, nextTimeFormatMode } from "@/lib/format";

const props = defineProps<{
  /** Stable localStorage key for this badge's format mode. */
  id: string;
  iso?: string | null;
}>();
const store = inject("dash") as DashStore;

const mode = computed(() => store.getTimeFormat(props.id));
const label = computed(() => formatTime(props.iso, mode.value));
const nextHint = computed(() => nextTimeFormatMode(mode.value));
const aria = computed(
  () =>
    `Time ${label.value}. Click for ${nextHint.value} format. Raw: ${props.iso || "none"}`
);
</script>

<template>
  <button
    type="button"
    class="time-badge"
    :data-time-id="id"
    :title="iso || ''"
    :aria-label="aria"
    :disabled="!iso"
    @click.stop="store.cycleTimeFormat(id)"
  >
    {{ label }}
  </button>
</template>
