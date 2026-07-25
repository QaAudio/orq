<script setup lang="ts">
import { computed, inject } from "vue";
import { QaPanel } from "@quantumaudio/ableton-extension-sdk/vue";
import type { DashStore } from "@/composables/useDashStore";
import { asArray, eventFamily, payloadSummary } from "@/lib/format";
import TimeBadge from "./TimeBadge.vue";

defineProps<{ focused?: boolean }>();
const emit = defineEmits<{ "focus-panel": [] }>();
const store = inject("dash") as DashStore;

const events = computed(() => {
  const rows = asArray(store.snapshot.value?.events).slice().reverse().slice(0, 25);
  return rows;
});
</script>

<template>
  <QaPanel
    class="panel-section"
    :class="{ 'panel-focused': focused }"
    label="Recent events"
    tabindex="0"
    data-height-panel="events"
    @focusin="emit('focus-panel')"
  >
    <template #header>
      <span class="count" id="count-events">{{ events.length }}</span>
    </template>
    <div id="events" class="panel-fill scroll-themed">
      <p v-if="!events.length" class="placeholder">none</p>
      <div v-else class="event-list">
        <div
          v-for="e in events"
          :key="String(e.id) + (e.created_at || '')"
          class="event-row"
          :class="'fam-' + eventFamily(e.kind)"
        >
          <span class="event-id mono">#{{ e.id != null ? e.id : "—" }}</span>
          <div class="event-main">
            <span class="event-kind">{{ e.kind || "event" }}</span>
            <div class="event-payload">{{ payloadSummary(e.payload) || "—" }}</div>
          </div>
          <TimeBadge
            class="event-time"
            :id="'event:' + String(e.id ?? e.created_at ?? '')"
            :iso="e.created_at"
          />
        </div>
      </div>
    </div>
  </QaPanel>
</template>
