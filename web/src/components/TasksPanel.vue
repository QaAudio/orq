<script setup lang="ts">
import { computed, inject } from "vue";
import { QaPanel } from "@quantumaudio/ableton-extension-sdk/vue";
import type { DashStore } from "@/composables/useDashStore";
import { asArray } from "@/lib/format";
import StatusBadge from "./StatusBadge.vue";

defineProps<{ focused?: boolean }>();
const emit = defineEmits<{ "focus-panel": [] }>();
const store = inject("dash") as DashStore;

const tasks = computed(() => asArray(store.snapshot.value?.tasks));

function open(id?: string) {
  if (id) store.drawerTaskId.value = id;
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
      <span class="count" id="count-tasks">{{ tasks.length }}</span>
    </template>
    <div id="tasks" class="panel-fill scroll-themed">
      <p v-if="!tasks.length" class="placeholder">none</p>
      <div v-else class="table-wrap">
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
  </QaPanel>
</template>
