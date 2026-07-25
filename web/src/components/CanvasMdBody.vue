<script setup lang="ts">
import {
  createApp,
  inject,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
  type App,
} from "vue";
import type { DashStore } from "@/composables/useDashStore";
import { htmlWithIsoPlaceholders } from "@/lib/format";
import { runMermaidIn } from "@/composables/useMermaid";
import TimeBadge from "./TimeBadge.vue";

const props = defineProps<{
  mdHtml: string;
  canvasKey: string;
}>();

const store = inject("dash") as DashStore;
const root = ref<HTMLElement | null>(null);
const apps: App[] = [];

function teardown() {
  while (apps.length) {
    apps.pop()!.unmount();
  }
}

async function mountBody() {
  teardown();
  const el = root.value;
  if (!el) return;
  const prefix = `md:${props.canvasKey}`;
  const { html } = htmlWithIsoPlaceholders(props.mdHtml, prefix);
  el.innerHTML = html;
  await nextTick();
  for (const slot of el.querySelectorAll<HTMLElement>("[data-porq-time]")) {
    const iso = slot.getAttribute("data-porq-time") || "";
    const id = slot.getAttribute("data-porq-time-id") || `${prefix}:0`;
    const app = createApp(TimeBadge, { id, iso });
    app.provide("dash", store);
    app.mount(slot);
    apps.push(app);
  }
  await runMermaidIn(el, store.qaTheme.value);
}

watch(
  () => [props.mdHtml, props.canvasKey, store.qaTheme.value] as const,
  () => {
    void mountBody();
  },
  { flush: "post" }
);

onMounted(() => {
  void mountBody();
});

onBeforeUnmount(teardown);
</script>

<template>
  <div ref="root" class="canvas-md" />
</template>
