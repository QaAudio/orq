<script setup lang="ts">
import { computed, inject } from "vue";
import { QaButton, QaPanel } from "@quantumaudio/ableton-extension-sdk/vue";
import type { DashStore } from "@/composables/useDashStore";
import { asArray } from "@/lib/format";
import StatusBadge from "./StatusBadge.vue";

defineProps<{ focused?: boolean }>();
const emit = defineEmits<{ "focus-panel": [] }>();
const store = inject("dash") as DashStore;

const selected = computed(() => {
  const id = store.selectedTaskId.value;
  if (!id) return null;
  return asArray(store.snapshot.value?.tasks).find((t) => t.id === id) || null;
});

const selectedLeases = computed(() => {
  const id = store.selectedTaskId.value;
  if (!id) return [];
  return asArray(store.snapshot.value?.leases).filter((l) => l.holder === id);
});

function select(id?: string) {
  if (!id) return;
  store.selectTask(store.selectedTaskId.value === id ? null : id);
}

function openDrawer() {
  if (selected.value?.id) store.drawerTaskId.value = selected.value.id;
}

function clear() {
  store.selectTask(null);
}
</script>

<template>
  <QaPanel
    class="panel-section"
    :class="{ 'panel-focused': focused }"
    label="Running tasks"
    tabindex="0"
    data-height-panel="running-tasks"
    data-panel-id="running-tasks"
    @focusin="emit('focus-panel')"
  >
    <template #header>
      <span class="count" id="count-running-tasks">{{ store.runningTasks.value.length }}</span>
    </template>
    <div id="running-tasks" class="panel-fill scroll-themed">
      <p v-if="!store.runningTasks.value.length" class="placeholder">No running tasks</p>
      <div v-else class="running-strip">
        <button
          v-for="t in store.runningTasks.value"
          :key="t.id"
          type="button"
          class="running-card"
          :class="{ selected: store.selectedTaskId.value === t.id }"
          :aria-pressed="store.selectedTaskId.value === t.id ? 'true' : 'false'"
          :data-task-id="t.id"
          @click="select(t.id)"
        >
          <span class="running-card-name">{{ t.name || "task" }}</span>
          <span class="mono running-card-id">{{ (t.id || "").slice(0, 8) }}</span>
          <StatusBadge :state="t.status" />
          <span class="mini-chip" title="claims">{{ asArray(t.claims).length }} claims</span>
        </button>
      </div>

      <div v-if="selected" class="running-inspector" data-testid="running-inspector">
        <div class="ops-label">Selected</div>
        <p class="mono">{{ selected.id }}</p>
        <div class="ops-label">Claims</div>
        <p v-if="!asArray(selected.claims).length" class="placeholder">none</p>
        <div v-else class="chip-row">
          <code v-for="c in asArray(selected.claims)" :key="c" class="mini-chip">{{ c }}</code>
        </div>
        <div class="ops-label">Leases</div>
        <p v-if="!selectedLeases.length" class="placeholder">none</p>
        <ul v-else class="lease-list">
          <li v-for="(l, i) in selectedLeases" :key="i" class="mono">
            {{ l.table }}/{{ l.key }}
          </li>
        </ul>
        <div class="running-actions">
          <QaButton type="button" @click="openDrawer">Open drawer</QaButton>
          <QaButton type="button" @click="clear">Clear</QaButton>
        </div>
      </div>
    </div>
  </QaPanel>
</template>
