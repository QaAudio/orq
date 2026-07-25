<script setup lang="ts">
import { QaBadge, QaPanel } from "@quantumaudio/ableton-extension-sdk/vue";
import { resolveSrc, clampHeight } from "@/lib/format";
import StatusBadge from "./StatusBadge.vue";
import TimeBadge from "./TimeBadge.vue";
import CanvasMdBody from "./CanvasMdBody.vue";
import type { PoiRow } from "@/lib/types";

defineProps<{
  poi: PoiRow;
  canvasKey: string;
  desc: Record<string, unknown>;
  kind: string;
  kindLower: string;
  mdHtml: string;
}>();
</script>

<template>
  <QaPanel
    class="canvas-card"
    :data-key="canvasKey"
    :data-kind="kind"
    :label="String(desc.title || canvasKey || 'canvas')"
  >
    <template #header>
      <StatusBadge :state="poi.state || 'live'" />
    </template>
    <template #subtitle>
      <span class="canvas-sub">
        <QaBadge :label="kind" />
        <span class="canvas-sub-sep">·</span>
        <span>v{{ poi.version }}</span>
        <span class="canvas-sub-sep">·</span>
        <TimeBadge :id="'canvas-updated:' + canvasKey" :iso="poi.updated_at" />
      </span>
    </template>
    <div class="canvas-body scroll-themed">
      <CanvasMdBody
        v-if="kindLower === 'markdown'"
        :md-html="mdHtml"
        :canvas-key="canvasKey"
      />
      <img
        v-else-if="kindLower === 'image'"
        :src="resolveSrc(desc.src)"
        :alt="String(desc.alt || desc.title || 'canvas image')"
        loading="lazy"
      />
      <iframe
        v-else-if="kindLower === 'url'"
        :src="resolveSrc(desc.src)"
        :height="clampHeight(desc.height, 360)"
        sandbox="allow-scripts"
        referrerpolicy="no-referrer"
        :title="String(desc.title || 'url canvas')"
      />
      <iframe
        v-else-if="kindLower === 'html'"
        :srcdoc="String(desc.body || '')"
        :height="clampHeight(desc.height, 280)"
        sandbox=""
        referrerpolicy="no-referrer"
        :title="String(desc.title || 'html canvas')"
      />
      <pre v-else class="canvas-fallback">{{ JSON.stringify(desc, null, 2) }}</pre>
    </div>
  </QaPanel>
</template>
