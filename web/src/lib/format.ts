import type { FilterMode, PoiRow } from "./types";

export function asArray<T>(v: T | T[] | null | undefined): T[] {
  if (v == null) return [];
  return Array.isArray(v) ? v : [v];
}

export function truncate(s: string, n: number): string {
  return s.length > n ? s.slice(0, n - 1) + "…" : s;
}

export type TimeFormatMode = "relative" | "abs24" | "abs12";

export const TIME_FORMAT_MODES: TimeFormatMode[] = ["relative", "abs24", "abs12"];

export function isTimeFormatMode(v: unknown): v is TimeFormatMode {
  return v === "relative" || v === "abs24" || v === "abs12";
}

export function nextTimeFormatMode(mode: TimeFormatMode): TimeFormatMode {
  const i = TIME_FORMAT_MODES.indexOf(mode);
  return TIME_FORMAT_MODES[(i + 1) % TIME_FORMAT_MODES.length]!;
}

/** ISO-8601 wall-clock tokens agents/scripts embed in canvas markdown bodies. */
export const ISO_TIMESTAMP_RE =
  /\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,9})?(?:Z|[+-]\d{2}:?\d{2})/g;

/**
 * Replace ISO timestamps outside `<pre>` / `<code>` with mount slots for TimeBadge.
 * `idPrefix` becomes `idPrefix:0`, `idPrefix:1`, …
 */
export function htmlWithIsoPlaceholders(
  html: string,
  idPrefix: string
): { html: string; count: number } {
  const parts = String(html || "").split(/(<pre[\s\S]*?<\/pre>|<code[\s\S]*?<\/code>)/gi);
  let count = 0;
  const out = parts.map((part) => {
    if (/^<(pre|code)[\s>]/i.test(part)) return part;
    return part.replace(ISO_TIMESTAMP_RE, (iso) => {
      const idx = count;
      count += 1;
      const safe = iso.replace(/&/g, "&amp;").replace(/"/g, "&quot;");
      const id = `${idPrefix}:${idx}`.replace(/"/g, "");
      return `<span class="porq-time-slot" data-porq-time="${safe}" data-porq-time-id="${id}"></span>`;
    });
  });
  return { html: out.join(""), count };
}

export function pad2(n: number): string {
  return n < 10 ? "0" + n : String(n);
}

export function relTime(iso?: string | null): string {
  if (!iso) return "—";
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return String(iso).slice(0, 19);
  const sec = Math.round((Date.now() - t) / 1000);
  const abs = Math.abs(sec);
  const ago = sec >= 0;
  const suffix = ago ? " ago" : " from now";
  if (abs < 5) return ago ? "just now" : "soon";
  if (abs < 60) return abs + "s" + suffix;
  const min = Math.round(abs / 60);
  if (min < 60) return min + "m" + suffix;
  const hr = Math.round(min / 60);
  if (hr < 48) return hr + "h" + suffix;
  const day = Math.round(hr / 24);
  return day + "d" + suffix;
}

function formatAbs24(d: Date): string {
  return (
    d.getFullYear() +
    "-" +
    pad2(d.getMonth() + 1) +
    "-" +
    pad2(d.getDate()) +
    " " +
    pad2(d.getHours()) +
    ":" +
    pad2(d.getMinutes()) +
    ":" +
    pad2(d.getSeconds())
  );
}

function formatAbs12(d: Date): string {
  return d.toLocaleString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit",
    second: "2-digit",
    hour12: true,
  });
}

export function formatTime(iso?: string | null, mode: TimeFormatMode = "relative"): string {
  if (!iso) return "—";
  const t = Date.parse(iso);
  if (Number.isNaN(t)) return String(iso).slice(0, 19);
  const d = new Date(t);
  if (mode === "abs24") return formatAbs24(d);
  if (mode === "abs12") return formatAbs12(d);
  return relTime(iso);
}

/** ACTIVE supervised tasks shown in Running tasks strip. */
export const ACTIVE_TASK_STATUSES = new Set([
  "running",
  "starting",
  "reconciling",
  "interrupting",
]);

export function isActiveTaskStatus(status?: string | null): boolean {
  return ACTIVE_TASK_STATUSES.has(String(status || "").toLowerCase());
}

export function relatedForTask(
  d: { board?: { key?: string }[]; leases?: { holder?: string; table?: string; key?: string }[] } | null | undefined,
  taskId: string | null | undefined,
  claims: string[] | null | undefined
): { keys: Set<string>; leaseKeys: Set<string> } {
  const keys = new Set<string>();
  const leaseKeys = new Set<string>();
  if (!taskId) return { keys, leaseKeys };
  for (const c of claims || []) {
    if (c) keys.add(c);
  }
  for (const l of asArray(d?.leases)) {
    if (l.holder === taskId) {
      const k = `${l.table || ""}/${l.key || ""}`;
      leaseKeys.add(k);
      if (l.key) keys.add(l.key);
    }
  }
  return { keys, leaseKeys };
}

export function payloadSummary(payload: unknown): string {
  if (payload == null) return "";
  let obj: unknown = payload;
  if (typeof payload === "string") {
    try {
      obj = JSON.parse(payload);
    } catch {
      return truncate(payload, 80);
    }
  }
  if (typeof obj !== "object" || obj === null) {
    return truncate(String(obj), 80);
  }
  const prefer = ["name", "key", "table", "status", "id", "state", "model_id", "class", "error", "note"];
  const bits: string[] = [];
  const rec = obj as Record<string, unknown>;
  for (const key of prefer) {
    if (rec[key] != null && rec[key] !== "") {
      bits.push(key + "=" + String(rec[key]));
    }
    if (bits.length >= 3) break;
  }
  if (bits.length) return truncate(bits.join(" · "), 96);
  try {
    return truncate(JSON.stringify(obj), 80);
  } catch {
    return "";
  }
}

export function prettyJson(v: unknown): string {
  try {
    return JSON.stringify(v, null, 2);
  } catch {
    return String(v);
  }
}

export function valueNeedsExpand(v: unknown): boolean {
  if (v != null && typeof v === "object") return true;
  if (typeof v === "string" && v.length > 80) return true;
  try {
    const s = JSON.stringify(v);
    return !!s && s.length > 80;
  } catch {
    return false;
  }
}

export function isArchived(row: { state?: string; status?: string } | null | undefined): boolean {
  const s = (row?.state || row?.status || "").toString().toLowerCase();
  return s === "archived";
}

export function filterArchivable<T extends { state?: string; status?: string }>(
  rows: T[],
  mode: FilterMode
): T[] {
  if (mode === "all") return rows;
  if (mode === "archived") return rows.filter(isArchived);
  return rows.filter((r) => !isArchived(r));
}

export function archivedCount(rows: PoiRow[]): number {
  return rows.filter(isArchived).length;
}

export function badgeAccent(
  state: string | undefined
): "green" | "red" | "blue" | "purple" | undefined {
  const k = (state || "").toLowerCase();
  if (["done", "complete", "success", "enabled", "approved", "live", "held"].includes(k)) {
    return "green";
  }
  if (["failed", "cancelled", "killed", "error", "blocked"].includes(k)) return "red";
  if (["running", "starting", "pending", "waiting", "queued", "proposed"].includes(k)) {
    return "blue";
  }
  if (["archived", "disabled", "aggregated"].includes(k)) return "purple";
  return undefined;
}

export function resolveSrc(src: unknown): string {
  const s = String(src ?? "");
  if (s.startsWith("canvas:")) {
    return "canvas/" + encodeURIComponent(s.slice("canvas:".length));
  }
  return s;
}

export function clampHeight(v: unknown, fallback: number): number {
  const n = Number(v);
  if (!Number.isFinite(n) || n < 80) return fallback;
  return Math.min(1200, Math.round(n));
}

export function eventFamily(kind?: string): string {
  const k = (kind || "").toString().toLowerCase();
  const head = k.split(".")[0] || "other";
  if (head === "task") return "task";
  if (head === "poi") return "poi";
  if (head === "job" || head === "route") return head === "route" ? "route" : "job";
  if (head === "affinity") return "affinity";
  if (head === "model") return "model";
  if (head === "trigger") return "trigger";
  return "other";
}
