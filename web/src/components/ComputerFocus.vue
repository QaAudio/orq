<script setup lang="ts">
import { computed, inject } from "vue";
import { QaButton, QaLed, QaPanel } from "@quantumaudio/ableton-extension-sdk/vue";
import type { DashStore } from "@/composables/useDashStore";
import { asArray } from "@/lib/format";
import StatusBadge from "./StatusBadge.vue";
import TimeBadge from "./TimeBadge.vue";

const store = inject("dash") as DashStore;

const info = computed(() => {
  const d = store.snapshot.value;
  if (!d) {
    return {
      held: false,
      holder: "—",
      purpose: "—",
      expiresIso: null as string | null,
      yieldReq: false,
      state: "—",
    };
  }
  const leases = asArray(d.leases);
  const lease = leases.find((l) => l.table === "computer" && l.key === "focus") || null;
  const poi = d.computer_focus || null;
  const val =
    poi && poi.value && typeof poi.value === "object"
      ? (poi.value as Record<string, unknown>)
      : {};
  const held = !!lease;
  return {
    held,
    holder: lease ? lease.holder : String(val.session || "—"),
    purpose: String((lease && lease.reason) || val.purpose || "—"),
    expiresIso: lease ? lease.expires_at || null : null,
    yieldReq: !!val.yield_requested,
    state: held ? "held" : poi ? poi.state || "idle" : "—",
  };
});

async function claim() {
  store.cfBusy.value = true;
  store.cfStatus.value = "waiting for lease…";
  store.cfError.value = false;
  try {
    await store.postPoiApi("/api/v1/poi/lock", {
      table: "computer",
      key: "focus",
      holder: "user",
      reason: "dash claim",
      ttl: 600,
      wait: true,
      timeout_ms: 120000,
    });
    store.cfStatus.value = "acquired";
    await store.tick();
  } catch (e) {
    store.cfStatus.value = String(e instanceof Error ? e.message : e);
    store.cfError.value = true;
  } finally {
    store.cfBusy.value = false;
  }
}

async function yieldReq() {
  store.cfBusy.value = true;
  store.cfStatus.value = "requesting yield…";
  store.cfError.value = false;
  try {
    await store.postPoiApi("/api/v1/poi/yield-request", {
      table: "computer",
      key: "focus",
      yield_by: "user",
    });
    store.cfStatus.value = "yield requested";
    await store.tick();
  } catch (e) {
    store.cfStatus.value = String(e instanceof Error ? e.message : e);
    store.cfError.value = true;
  } finally {
    store.cfBusy.value = false;
  }
}

async function release() {
  store.cfBusy.value = true;
  store.cfStatus.value = "releasing…";
  store.cfError.value = false;
  try {
    await store.postPoiApi("/api/v1/poi/unlock", {
      table: "computer",
      key: "focus",
      holder: "user",
    });
    store.cfStatus.value = "released";
    await store.tick();
  } catch (e) {
    store.cfStatus.value = String(e instanceof Error ? e.message : e);
    store.cfError.value = true;
  } finally {
    store.cfBusy.value = false;
  }
}

async function steal() {
  if (!window.confirm("Steal computer/focus lease? Prefer Request yield when an agent holds it.")) {
    return;
  }
  store.cfBusy.value = true;
  store.cfStatus.value = "stealing…";
  store.cfError.value = false;
  try {
    await store.postPoiApi("/api/v1/poi/steal", {
      table: "computer",
      key: "focus",
      holder: "user",
      reason: "dash steal",
      ttl: 600,
    });
    store.cfStatus.value = "stolen";
    await store.tick();
  } catch (e) {
    store.cfStatus.value = String(e instanceof Error ? e.message : e);
    store.cfError.value = true;
  } finally {
    store.cfBusy.value = false;
  }
}
</script>

<template>
  <QaPanel
    id="computer-focus-panel"
    class="computer-focus-panel"
    label="Computer focus"
    aria-label="Computer focus"
  >
    <template #header>
      <span class="count" id="cf-state">{{ info.state }}</span>
      <QaLed :on="info.held" :color="info.held ? 'green' : 'accent'" />
    </template>
    <div id="computer-focus-body">
      <div class="cf-grid">
        <div>
          <div class="ops-label">holder</div>
          <div class="mono">{{ info.holder }}</div>
        </div>
        <div>
          <div class="ops-label">purpose</div>
          <div>{{ info.purpose }}</div>
        </div>
        <div>
          <div class="ops-label">expires</div>
          <div>
            <TimeBadge id="computer-focus-expires" :iso="info.expiresIso" />
          </div>
        </div>
        <div>
          <div class="ops-label">yield</div>
          <StatusBadge :state="info.yieldReq ? 'requested' : 'clear'" />
        </div>
      </div>
      <div v-if="store.staticDemo.value" class="cf-actions cf-actions-static">
        <span class="cf-status" id="cf-status">observe-only — mutate disabled in static demo</span>
      </div>
      <div v-else class="cf-actions">
        <QaButton id="cf-claim" :disabled="store.cfBusy.value" @click="claim">
          Take ownership (wait)
        </QaButton>
        <QaButton id="cf-yield" :disabled="store.cfBusy.value || !info.held" @click="yieldReq">
          Request yield
        </QaButton>
        <QaButton id="cf-release" :disabled="store.cfBusy.value" @click="release">Release</QaButton>
        <QaButton id="cf-steal" :disabled="store.cfBusy.value" accent="red" @click="steal">
          Steal
        </QaButton>
        <span
          class="cf-status"
          id="cf-status"
          aria-live="polite"
          :class="{ 'cf-error': store.cfError.value }"
        >
          {{ store.cfStatus.value }}
        </span>
      </div>
    </div>
  </QaPanel>
</template>
