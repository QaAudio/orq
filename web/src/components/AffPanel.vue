<script setup lang="ts">
import { computed, inject } from "vue";
import { QaPanel } from "@quantumaudio/ableton-extension-sdk/vue";
import type { DashStore } from "@/composables/useDashStore";
import { asArray } from "@/lib/format";

defineProps<{ focused?: boolean }>();
const emit = defineEmits<{ "focus-panel": [] }>();
const store = inject("dash") as DashStore;
const aff = computed(() => asArray(store.snapshot.value?.affinities));
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
    <div id="aff" class="panel-fill scroll-themed">
      <p v-if="!aff.length" class="placeholder">none</p>
      <div v-else class="table-wrap">
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
  </QaPanel>
</template>
