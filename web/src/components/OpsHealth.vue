<script setup lang="ts">
import { computed, inject } from "vue";
import { QaLed, QaPanel } from "@quantumaudio/ableton-extension-sdk/vue";
import type { DashStore } from "@/composables/useDashStore";
import { asArray, eventFamily, payloadSummary } from "@/lib/format";
import StatusBadge from "./StatusBadge.vue";
import TimeBadge from "./TimeBadge.vue";

defineProps<{ focused?: boolean }>();
const emit = defineEmits<{ "focus-panel": [] }>();
const store = inject("dash") as DashStore;

function leaseKey(l: { table?: string; key?: string }) {
  return `${l.table || ""}/${l.key || ""}`;
}
function isRelatedLease(l: { table?: string; key?: string; holder?: string }) {
  if (!store.selectedTaskId.value) return false;
  if (l.holder === store.selectedTaskId.value) return true;
  return store.relatedLeaseKeys.value.has(leaseKey(l));
}

const data = computed(() => {
  const d = store.snapshot.value;
  if (!d) {
    return {
      daemonUp: false,
      leases: [],
      triggers: [],
      blocked: [],
      failures: [],
      sessions: [],
      models: [],
    };
  }
  return {
    daemonUp: !!(d.daemon && d.daemon.running),
    leases: asArray(d.leases),
    triggers: asArray(d.triggers),
    blocked: asArray(d.blocked_pois),
    failures: asArray(d.trigger_failures).slice().reverse().slice(0, 10),
    sessions: asArray(d.active_sessions),
    models: asArray(d.models).slice(0, 12),
  };
});
</script>

<template>
  <QaPanel
    class="panel-section"
    :class="{ 'panel-focused': focused }"
    label="Loop ops"
    tabindex="0"
    data-height-panel="ops-health"
    @focusin="emit('focus-panel')"
  >
    <div id="ops-health" class="panel-fill scroll-themed">
      <div class="ops-grid">
        <div class="ops-card">
          <div class="ops-label">Daemon</div>
          <div class="ops-value" style="display: flex; gap: 0.4rem; align-items: center">
            <QaLed :on="data.daemonUp" :color="data.daemonUp ? 'green' : 'red'" />
            <StatusBadge :state="data.daemonUp ? 'daemon up' : 'daemon down'" />
          </div>
        </div>
        <div class="ops-card">
          <div class="ops-label">Active sessions</div>
          <div v-if="data.sessions.length" class="chip-row">
            <span v-for="s in data.sessions" :key="s" class="mini-chip mono">{{ s }}</span>
          </div>
          <span v-else class="placeholder">none</span>
        </div>
        <div class="ops-card">
          <div class="ops-label">Models</div>
          <div v-if="data.models.length" class="chip-row">
            <span
              v-for="m in data.models"
              :key="m.id"
              class="mini-chip mono"
              :title="m.display_name || m.id"
            >
              {{ m.id }}
            </span>
          </div>
          <span v-else class="placeholder">none</span>
        </div>
        <div class="ops-card ops-wide">
          <div class="ops-label">Leases ({{ data.leases.length }})</div>
          <p v-if="!data.leases.length" class="placeholder">none</p>
          <div v-else class="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>poi</th>
                  <th>kind</th>
                  <th>holder</th>
                  <th>expires</th>
                </tr>
              </thead>
              <tbody>
                <tr
                  v-for="(l, i) in data.leases"
                  :key="i"
                  :class="{ 'related-to-selection': isRelatedLease(l) }"
                >
                  <td class="mono">{{ l.table }}/{{ l.key }}</td>
                  <td>{{ l.kind }}</td>
                  <td class="mono">{{ l.holder }}</td>
                  <td>
                    <TimeBadge
                      :id="'lease-exp:' + (l.table || '') + '/' + (l.key || '')"
                      :iso="l.expires_at"
                    />
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
        <div class="ops-card ops-wide">
          <div class="ops-label">Triggers ({{ data.triggers.length }})</div>
          <p v-if="!data.triggers.length" class="placeholder">none</p>
          <div v-else class="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>id</th>
                  <th>name</th>
                  <th>pattern</th>
                  <th>state</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="t in data.triggers" :key="t.id">
                  <td class="mono">{{ (t.id || "").slice(0, 8) }}</td>
                  <td>{{ t.name }}</td>
                  <td class="mono">{{ t.event_pattern }}</td>
                  <td>
                    <StatusBadge :state="t.enabled === false ? 'disabled' : 'enabled'" />
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
        <div class="ops-card ops-wide">
          <div class="ops-label">Blocked POIs ({{ data.blocked.length }})</div>
          <p v-if="!data.blocked.length" class="placeholder">none</p>
          <div v-else class="table-wrap">
            <table>
              <thead>
                <tr>
                  <th>poi</th>
                  <th>state</th>
                  <th>reason</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="(p, i) in data.blocked" :key="i">
                  <td class="mono">{{ p.table }}/{{ p.key }}</td>
                  <td><StatusBadge :state="p.state" /></td>
                  <td>{{ p.blocker_reason || "—" }}</td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
        <div class="ops-card ops-wide">
          <div class="ops-label">Trigger failures ({{ data.failures.length }})</div>
          <p v-if="!data.failures.length" class="placeholder">none</p>
          <div v-else class="event-list">
            <div
              v-for="e in data.failures"
              :key="String(e.id)"
              class="event-row"
              :class="'fam-' + eventFamily(e.kind)"
            >
              <span class="event-id mono">#{{ e.id != null ? e.id : "—" }}</span>
              <div class="event-main">
                <span class="event-kind">{{ e.kind || "event" }}</span>
                <div class="event-payload">{{ payloadSummary(e.payload) || "—" }}</div>
              </div>
              <TimeBadge
                class="event-time"
                :id="'ops-event:' + String(e.id ?? e.created_at ?? '')"
                :iso="e.created_at"
              />
            </div>
          </div>
        </div>
      </div>
    </div>
  </QaPanel>
</template>
