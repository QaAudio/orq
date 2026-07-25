<script setup lang="ts">
import { computed, inject } from "vue";
import type { DashStore } from "@/composables/useDashStore";
import { useMermaid } from "@/composables/useMermaid";
import { asArray, archivedCount, filterArchivable } from "@/lib/format";
import { renderMarkdown } from "@/lib/markdown";
import StateFilter from "./StateFilter.vue";
import CanvasGridHost from "./CanvasGridHost.vue";
import CanvasCard from "./CanvasCard.vue";
import type { PoiRow } from "@/lib/types";

const store = inject("dash") as DashStore;

function canvasDescriptor(poi: PoiRow): Record<string, unknown> {
  let v = poi.value;
  if (typeof v === "string") {
    try {
      v = JSON.parse(v);
    } catch {
      return { kind: "unknown", raw: v };
    }
  }
  if (!v || typeof v !== "object") return { kind: "unknown", raw: v };
  return v as Record<string, unknown>;
}

const allCanvases = computed(() => asArray(store.snapshot.value?.canvases));
const visible = computed(() => filterArchivable(allCanvases.value, store.filterMode.value));
const hiddenArchived = computed(() => archivedCount(allCanvases.value));
const visibleKeys = computed(() => visible.value.map((p) => String(p.key || "")).filter(Boolean));

type CanvasCardModel = {
  poi: PoiRow;
  key: string;
  desc: Record<string, unknown>;
  kind: string;
  kindLower: string;
  mdHtml: string;
  span2: boolean;
};

const cards = computed((): CanvasCardModel[] =>
  visible.value.map((poi) => {
    const desc = canvasDescriptor(poi);
    const kind = String(desc.kind || "unknown");
    const span2 =
      poi.key === "loop-roadmap" ||
      poi.columns?.span === 2 ||
      poi.columns?.span === "2" ||
      desc.span === 2;
    return {
      poi,
      key: String(poi.key || ""),
      desc,
      kind,
      kindLower: kind.toLowerCase(),
      mdHtml:
        kind.toLowerCase() === "markdown"
          ? renderMarkdown(String(desc.body || ""))
          : "",
      span2: !!span2,
    };
  })
);

const cardByKey = computed(() => {
  const m = new Map<string, CanvasCardModel>();
  for (const c of cards.value) {
    if (c.key) m.set(c.key, c);
  }
  return m;
});

function span2ForKey(key: string): boolean {
  return cardByKey.value.get(key)?.span2 || key === "loop-roadmap";
}

const mermaidSource = computed(() =>
  cards.value
    .filter((c) => c.kindLower === "markdown")
    .map((c) => c.mdHtml)
    .join("\n---\n")
);

useMermaid(mermaidSource, store.qaTheme);
</script>

<template>
  <div class="canvases-head">
    <h2>Canvases</h2>
    <div class="panel-toolbar">
      <StateFilter />
      <span class="count" id="count-canvases">{{ visible.length }}</span>
    </div>
  </div>
  <div id="canvases">
    <div v-if="!visible.length" class="canvas-empty">
      <p v-if="!allCanvases.length">
        No canvases yet — publish with <code>porq canvas set</code>
      </p>
      <p v-else>
        No {{ store.filterMode.value }} canvases
        <template v-if="store.filterMode.value === 'active' && hiddenArchived">
          ({{ hiddenArchived }} archived hidden) — switch filter to All or Archived.
        </template>
      </p>
      <p>Canvases are the primary view. Ops panels live under <strong>Details</strong>.</p>
    </div>
    <CanvasGridHost v-else :keys="visibleKeys" :span2-for-key="span2ForKey">
      <template #default="{ canvasKey }">
        <CanvasCard
          v-if="cardByKey.get(canvasKey)"
          :poi="cardByKey.get(canvasKey)!.poi"
          :canvas-key="canvasKey"
          :desc="cardByKey.get(canvasKey)!.desc"
          :kind="cardByKey.get(canvasKey)!.kind"
          :kind-lower="cardByKey.get(canvasKey)!.kindLower"
          :md-html="cardByKey.get(canvasKey)!.mdHtml"
        />
      </template>
    </CanvasGridHost>
  </div>
</template>
