<script setup lang="ts">
import { computed, inject, ref } from "vue";
import { QaButton } from "@quantumaudio/ableton-extension-sdk/vue";
import type { DashStore } from "@/composables/useDashStore";
import { LOG_TAIL_BYTES } from "@/composables/useDashStore";
import { asArray } from "@/lib/format";
import StatusBadge from "./StatusBadge.vue";

const store = inject("dash") as DashStore;
const logText = ref("");
const logBusy = ref(false);

const task = computed(() => {
  const id = store.drawerTaskId.value;
  if (!id || !store.snapshot.value) return null;
  return asArray(store.snapshot.value.tasks).find((t) => t.id === id) || null;
});

const open = computed(() => !!store.drawerTaskId.value && !!task.value);

function close() {
  store.drawerTaskId.value = null;
  logText.value = "";
}

async function copy(text?: string) {
  if (!text) return;
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    /* ignore */
  }
}

async function loadLog() {
  const id = store.drawerTaskId.value;
  if (!id) return;
  logBusy.value = true;
  try {
    const res = await fetch(
      `api/v1/tasks/${encodeURIComponent(id)}/logs?tailBytes=${LOG_TAIL_BYTES}&ts=${Date.now()}`,
      { cache: "no-store" }
    );
    const data = await res.json();
    if (!res.ok || !data.ok) throw new Error((data && data.error) || "HTTP " + res.status);
    const prefix = data.truncated ? "… (truncated)\n" : "";
    logText.value = prefix + (data.content || "");
  } catch (e) {
    logText.value = "log unavailable: " + (e instanceof Error ? e.message : e);
  } finally {
    logBusy.value = false;
  }
}
</script>

<template>
  <div
    id="drawer-backdrop"
    class="drawer-backdrop"
    :class="{ hidden: !open }"
    aria-hidden="true"
    @click="close"
  />
  <aside
    id="task-drawer"
    class="task-drawer"
    :class="{ hidden: !open }"
    aria-labelledby="drawer-title"
    :aria-hidden="open ? 'false' : 'true'"
    role="dialog"
  >
    <div class="drawer-header">
      <h3 id="drawer-title">{{ task?.name || "Task" }}</h3>
      <QaButton id="drawer-close" aria-label="Close task details" @click="close">Close</QaButton>
    </div>
    <div v-if="task" id="drawer-body">
      <p><StatusBadge :state="task.status" /></p>
      <p class="mono">id: {{ task.id }}</p>
      <p class="mono">model: {{ task.model_id || task.profile || "—" }}</p>
      <p>
        cmd:
        <code class="drawer-cmd">{{ task.command || "—" }}</code>
        <QaButton class="copy-btn" @click="copy(task.command)">Copy</QaButton>
      </p>
      <QaButton id="drawer-load-log" :disabled="logBusy" :data-task-id="task.id" @click="loadLog">
        Load log tail
      </QaButton>
      <pre class="drawer-log">{{ logText }}</pre>
    </div>
  </aside>
</template>
