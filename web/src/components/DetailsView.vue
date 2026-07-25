<script setup lang="ts">
import { computed, inject, ref, type Component } from "vue";
import type { DashStore } from "@/composables/useDashStore";
import type { PanelId } from "@/lib/types";
import { DETAILS_PANEL_IDS } from "@/lib/canvasGrid";
import CanvasGridHost from "./CanvasGridHost.vue";
import OpsHealth from "./OpsHealth.vue";
import RunningTasksPanel from "./RunningTasksPanel.vue";
import BoardPanel from "./BoardPanel.vue";
import TasksPanel from "./TasksPanel.vue";
import JobsPanel from "./JobsPanel.vue";
import AffPanel from "./AffPanel.vue";
import EventsPanel from "./EventsPanel.vue";
import FilesPanel from "./FilesPanel.vue";

const store = inject("dash") as DashStore;
const focused = ref<string | null>(null);

const PANEL_MAP: Record<PanelId, Component> = {
  "ops-health": OpsHealth,
  "running-tasks": RunningTasksPanel,
  board: BoardPanel,
  tasks: TasksPanel,
  jobs: JobsPanel,
  aff: AffPanel,
  events: EventsPanel,
  files: FilesPanel,
};

const keys = computed(() => [...DETAILS_PANEL_IDS]);
</script>

<template>
  <main
    id="view-details"
    class="view"
    :class="{ active: store.view.value === 'details' }"
    role="tabpanel"
  >
    <CanvasGridHost :keys="keys" surface="details">
      <template #default="{ canvasKey }">
        <component
          :is="PANEL_MAP[canvasKey as PanelId]"
          v-if="PANEL_MAP[canvasKey as PanelId]"
          :focused="focused === canvasKey"
          @focus-panel="focused = canvasKey"
        />
      </template>
    </CanvasGridHost>
  </main>
</template>
