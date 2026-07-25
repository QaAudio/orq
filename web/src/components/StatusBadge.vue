<script setup lang="ts">
import { computed } from "vue";
import { QaBadge } from "@quantumaudio/ableton-extension-sdk/vue";
import { badgeAccent } from "@/lib/format";

const props = defineProps<{ state?: string | number | null }>();
const label = computed(() => String(props.state ?? "—"));
const accent = computed(() => badgeAccent(label.value));
</script>

<template>
  <QaBadge :label="label" :accent="accent" />
</template>
