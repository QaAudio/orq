<script setup lang="ts">
import { computed, provide } from "vue";
import { QaButton, QaSelect } from "@quantumaudio/ableton-extension-sdk/vue";
import { useDashStore } from "@/composables/useDashStore";
import { asArray } from "@/lib/format";
import CanvasesView from "@/components/CanvasesView.vue";
import DetailsView from "@/components/DetailsView.vue";
import ComputerFocus from "@/components/ComputerFocus.vue";
import TaskDrawer from "@/components/TaskDrawer.vue";

const store = useDashStore();
provide("dash", store);

const workspace = computed(() => "ws " + (store.snapshot.value?.workspace || "—"));
const daemonUp = computed(() => !!(store.snapshot.value?.daemon && store.snapshot.value.daemon.running));
const pulse = computed(() => {
  const d = store.snapshot.value;
  if (!d) {
    return { tasks: 0, pois: 0, jobs: 0, events: 0, event: "waiting for pulse…" };
  }
  const events = asArray(d.events);
  const last = events.length ? events[events.length - 1] : null;
  return {
    tasks: asArray(d.tasks).length,
    pois: asArray(d.board).length,
    jobs: asArray(d.jobs).length,
    events: events.length,
    event: last
      ? `${last.id != null ? "#" + last.id : ""} ${last.kind || "event"}`
      : "waiting for pulse…",
  };
});

const themeOptions = [
  { value: "dark", label: "dark" },
  { value: "light", label: "light" },
];

function onTheme(v: string) {
  store.applyTheme(v === "light" ? "light" : "dark");
}
</script>

<template>
  <header class="dash-header">
    <div class="brand">
      <div class="brand-text">
        <span class="brand-mark">porq</span>
        <span class="brand-sub">dashboard</span>
      </div>
    </div>
    <nav class="view-tabs" role="tablist" aria-label="Dashboard view">
      <QaButton
        id="tab-canvases"
        :highlight="store.view.value === 'canvases'"
        :active="store.view.value === 'canvases'"
        role="tab"
        :aria-selected="store.view.value === 'canvases' ? 'true' : 'false'"
        @click="store.setView('canvases', { user: true })"
      >
        Canvases
      </QaButton>
      <QaButton
        id="tab-details"
        :highlight="store.view.value === 'details'"
        :active="store.view.value === 'details'"
        role="tab"
        :aria-selected="store.view.value === 'details' ? 'true' : 'false'"
        @click="store.setView('details', { user: true })"
      >
        Details
      </QaButton>
    </nav>
    <div class="header-meta">
      <label class="theme-picker" for="theme-select">
        <span class="theme-picker-label">theme</span>
        <QaSelect
          id="theme-select"
          :model-value="store.qaTheme.value"
          :options="themeOptions"
          aria-label="Dashboard theme"
          @update:model-value="onTheme"
        />
      </label>
      <span id="workspace">{{ workspace }}</span>
      <span id="stamp" :class="store.stampClass.value" aria-live="polite">{{ store.stampText.value }}</span>
    </div>
  </header>

  <div
    id="pulse"
    class="pulse"
    title="Open Details view"
    role="button"
    tabindex="0"
    @click="store.setView('details', { user: true })"
    @keydown.enter.prevent="store.setView('details', { user: true })"
    @keydown.space.prevent="store.setView('details', { user: true })"
  >
    <span id="pulse-daemon" :class="daemonUp ? 'status-done' : 'status-failed'">
      {{ daemonUp ? "daemon up" : "daemon down" }}
    </span>
    <span class="pulse-chip"><strong id="pulse-tasks">{{ pulse.tasks }}</strong> tasks</span>
    <span class="pulse-chip"><strong id="pulse-pois">{{ pulse.pois }}</strong> pois</span>
    <span class="pulse-chip"><strong id="pulse-jobs">{{ pulse.jobs }}</strong> jobs</span>
    <span class="pulse-chip"><strong id="pulse-events">{{ pulse.events }}</strong> events</span>
    <span class="pulse-event" id="pulse-event">{{ pulse.event }}</span>
  </div>

  <div id="error" class="error-banner" :class="{ visible: !!store.errorMsg.value }" role="alert">
    <span id="error-msg">{{ store.errorMsg.value }}</span>
    <QaButton id="error-retry" @click="store.tick()">Retry</QaButton>
  </div>

  <div v-if="store.staticDemo.value" id="static-demo-banner" class="static-demo-banner" role="status">
    Static demo — observe only (frozen snapshot, no mutate).
    <a href="https://github.com/QaAudio/porq">Source</a>
  </div>

  <div
    id="view-canvases"
    class="view"
    :class="{ active: store.view.value === 'canvases' }"
    role="tabpanel"
  >
    <ComputerFocus />
    <CanvasesView />
  </div>

  <DetailsView />
  <TaskDrawer />
</template>
