<script setup lang="ts">
import { computed, inject } from "vue";
import { QaPanel } from "@quantumaudio/ableton-extension-sdk/vue";
import type { DashStore } from "@/composables/useDashStore";
import {
  asArray,
  archivedCount,
  filterArchivable,
  payloadSummary,
  prettyJson,
  valueNeedsExpand,
} from "@/lib/format";
import StatusBadge from "./StatusBadge.vue";
import StateFilter from "./StateFilter.vue";

defineProps<{ focused?: boolean }>();
const emit = defineEmits<{ "focus-panel": [] }>();
const store = inject("dash") as DashStore;

const allBoard = computed(() => asArray(store.snapshot.value?.board));
const rows = computed(() => filterArchivable(allBoard.value, store.filterMode.value));
const hiddenArchived = computed(() => archivedCount(allBoard.value));

function expandId(key?: string) {
  return "board:" + (key || "");
}

function onToggle(e: Event, key?: string) {
  const t = e.target as HTMLDetailsElement;
  if (!(t instanceof HTMLDetailsElement)) return;
  const id = expandId(key);
  if (t.open) store.boardExpanded.add(id);
  else store.boardExpanded.delete(id);
}
</script>

<template>
  <QaPanel
    class="panel-section"
    :class="{ 'panel-focused': focused }"
    label="Board (POIs)"
    tabindex="0"
    data-height-panel="board"
    @focusin="emit('focus-panel')"
  >
    <template #header>
      <div class="panel-toolbar">
        <StateFilter />
        <span class="count" id="count-board">{{ rows.length }}</span>
      </div>
    </template>
    <div id="board" class="panel-fill scroll-themed">
      <p v-if="!rows.length" class="placeholder">
        <template v-if="!allBoard.length">empty</template>
        <template v-else>
          No {{ store.filterMode.value }} POIs
          <template v-if="store.filterMode.value === 'active' && hiddenArchived">
            ({{ hiddenArchived }} archived hidden)
          </template>
        </template>
      </p>
      <div v-else class="table-wrap">
        <table>
          <thead>
            <tr>
              <th>key</th>
              <th>state</th>
              <th>owner</th>
              <th>value</th>
            </tr>
          </thead>
          <tbody>
            <tr
              v-for="p in rows"
              :key="p.key"
              :class="{ 'related-to-selection': !!(p.key && store.relatedKeys.value.has(p.key)) }"
              :data-poi-key="p.key"
            >
              <td class="mono">{{ p.key }}</td>
              <td>
                <StatusBadge :state="p.blocked ? 'blocked' : p.state" />
              </td>
              <td>{{ (p.columns && p.columns.owner) || "" }}</td>
              <td class="value-cell">
                <code v-if="!valueNeedsExpand(p.value)" class="value-summary">
                  {{ payloadSummary(p.value) || "—" }}
                </code>
                <details
                  v-else
                  class="value-details"
                  :open="store.boardExpanded.has(expandId(p.key))"
                  :data-expand-key="expandId(p.key)"
                  @toggle="onToggle($event, p.key)"
                >
                  <summary>
                    <code class="value-summary">{{ payloadSummary(p.value) || "—" }}</code>
                  </summary>
                  <pre class="json-pretty">{{ prettyJson(p.value) }}</pre>
                </details>
              </td>
            </tr>
          </tbody>
        </table>
      </div>
    </div>
  </QaPanel>
</template>
