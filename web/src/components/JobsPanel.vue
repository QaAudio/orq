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
    <div id="jobs" class="panel-fill scroll-themed">
      <p v-if="!jobs.length" class="placeholder">none</p>
      <div v-else class="table-wrap">
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
  </QaPanel>
</template>
