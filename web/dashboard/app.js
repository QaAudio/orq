(function () {
  const POLL_MS = 1000;
  const STALE_AFTER_MISSED = 3;
  const LOG_TAIL_BYTES = 8192;

  const stampEl = document.getElementById("stamp");
  const workspaceEl = document.getElementById("workspace");
  const errorEl = document.getElementById("error");
  const errorMsgEl = document.getElementById("error-msg");
  const errorRetryEl = document.getElementById("error-retry");
  const emptyHint = document.getElementById("empty-hint");

  let lastSnapshot = null;
  let lastSuccessAt = null;
  let missedPolls = 0;
  let fetchInFlight = false;
  let freshnessTimer = null;
  let taskById = new Map();
  let childTasksById = new Map();
  let drawerTaskId = null;

  function asArray(v) {
    if (v == null) return [];
    if (Array.isArray(v)) return v;
    return [v];
  }

  function statusClass(s) {
    const k = (s || "").toString().toLowerCase().replace(/[^a-z0-9_-]/g, "");
    const map = {
      done: "status-done",
      complete: "status-done",
      success: "status-done",
      live: "status-active",
      running: "status-active",
      starting: "status-active",
      reconciling: "status-active",
      interrupting: "status-active",
      queued: "status-waiting",
      pending: "status-waiting",
      ready: "status-waiting",
      planned: "status-waiting",
      waiting: "status-waiting",
      blocked: "status-blocked",
      failed: "status-failed",
      cancelled: "status-failed",
      killed: "status-failed",
      error: "status-failed",
      proposed: "status-proposed",
      approved: "status-approved",
      archived: "status-muted",
      aggregated: "status-muted",
      enabled: "status-done",
      disabled: "status-muted",
    };
    return map[k] || "status-muted";
  }

  function pill(s) {
    const raw = (s || "-").toString();
    const cls = statusClass(raw);
    return `<span class="pill ${cls}" aria-label="status ${esc(raw)}">${esc(raw)}</span>`;
  }

  function showError(msg) {
    if (errorMsgEl) errorMsgEl.textContent = msg;
    else errorEl.textContent = msg;
    errorEl.classList.add("visible");
  }

  function clearError() {
    if (errorMsgEl) errorMsgEl.textContent = "";
    else errorEl.textContent = "";
    errorEl.classList.remove("visible");
  }

  function esc(s) {
    return String(s ?? "")
      .replace(/&/g, "&amp;")
      .replace(/</g, "&lt;")
      .replace(/>/g, "&gt;")
      .replace(/"/g, "&quot;");
  }

  function setCount(id, n) {
    const el = document.getElementById(id);
    if (el) el.textContent = String(n);
  }

  function relTime(iso) {
    if (!iso) return "—";
    const t = Date.parse(iso);
    if (Number.isNaN(t)) return String(iso).slice(0, 19);
    const sec = Math.round((Date.now() - t) / 1000);
    if (sec < 5) return "just now";
    if (sec < 60) return sec + "s ago";
    const min = Math.round(sec / 60);
    if (min < 60) return min + "m ago";
    const hr = Math.round(min / 60);
    if (hr < 48) return hr + "h ago";
    return new Date(t).toLocaleString();
  }

  function fmtTime(iso) {
    if (!iso) return "—";
    const t = Date.parse(iso);
    if (Number.isNaN(t)) return String(iso);
    return new Date(t).toLocaleString();
  }

  function updateFreshnessUI() {
    if (!lastSuccessAt) {
      stampEl.textContent = "connecting…";
      stampEl.classList.remove("stale", "error", "live");
      return;
    }
    const sec = Math.max(0, Math.round((Date.now() - lastSuccessAt) / 1000));
    const stale = missedPolls >= STALE_AFTER_MISSED;
    stampEl.textContent = stale ? "stale · " + sec + "s ago" : "updated " + sec + "s ago";
    stampEl.title = lastSnapshot && lastSnapshot.updated ? "snapshot " + lastSnapshot.updated : "";
    stampEl.classList.toggle("stale", stale);
    stampEl.classList.toggle("live", !stale);
    stampEl.classList.remove("error");
  }

  function startFreshnessTimer() {
    if (freshnessTimer) return;
    freshnessTimer = setInterval(updateFreshnessUI, 250);
  }

  function indexTasks(tasks) {
    taskById = new Map();
    childTasksById = new Map();
    for (const t of tasks) {
      if (!t || !t.id) continue;
      taskById.set(t.id, t);
    }
    for (const t of tasks) {
      if (!t || !t.id) continue;
      for (const dep of asArray(t.depends_on)) {
        if (!dep) continue;
        if (!childTasksById.has(dep)) childTasksById.set(dep, []);
        childTasksById.get(dep).push(t);
      }
    }
  }

  async function copyText(text, btn) {
    const value = String(text ?? "");
    if (!value) return;
    try {
      await navigator.clipboard.writeText(value);
    } catch {
      const ta = document.createElement("textarea");
      ta.value = value;
      ta.style.position = "fixed";
      ta.style.opacity = "0";
      document.body.appendChild(ta);
      ta.select();
      try {
        document.execCommand("copy");
      } catch {
        /* ignore */
      }
      document.body.removeChild(ta);
    }
    if (btn) {
      const orig = btn.textContent;
      btn.textContent = "Copied";
      btn.disabled = true;
      setTimeout(() => {
        btn.textContent = orig;
        btn.disabled = false;
      }, 1200);
    }
  }

  function eventFamily(kind) {
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

  function payloadSummary(payload) {
    if (payload == null) return "";
    let obj = payload;
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
    const prefer = ["name", "key", "table", "status", "id", "state", "model_id", "class", "error"];
    const bits = [];
    for (const key of prefer) {
      if (obj[key] != null && obj[key] !== "") {
        bits.push(key + "=" + String(obj[key]));
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

  function truncate(s, n) {
    const str = String(s);
    return str.length > n ? str.slice(0, n - 1) + "…" : str;
  }

  function tableOrEmpty(rowsHtml, emptyLabel) {
    if (!rowsHtml) {
      return `<p class="placeholder">${esc(emptyLabel)}</p>`;
    }
    return `<div class="table-wrap"><table>${rowsHtml}</table></div>`;
  }

  function chipList(items, emptyLabel) {
    if (!items.length) {
      return `<p class="placeholder">${esc(emptyLabel)}</p>`;
    }
    return (
      `<div class="chip-row">` +
      items.map((x) => `<span class="mini-chip mono">${esc(x)}</span>`).join("") +
      `</div>`
    );
  }

  function resolveSrc(src) {
    const s = String(src || "");
    if (s.startsWith("canvas:")) {
      return "/canvas/" + encodeURIComponent(s.slice("canvas:".length));
    }
    return s;
  }

  function clampHeight(h, fallback) {
    const n = Number(h);
    if (!Number.isFinite(n)) return fallback;
    return Math.max(120, Math.min(800, Math.round(n)));
  }

  /** Escape-first markdown subset (headings, bold/italic, code, lists, links). */
  function renderMarkdown(src) {
    const raw = String(src ?? "");
    const lines = raw.replace(/\r\n/g, "\n").split("\n");
    const out = [];
    let inCode = false;
    let codeBuf = [];
    let listType = null;

    function flushList() {
      if (!listType) return;
      out.push(listType === "ol" ? "</ol>" : "</ul>");
      listType = null;
    }

    function flushTable(rows) {
      if (!rows.length) return;
      const head = rows[0];
      const body = rows.slice(1);
      out.push('<div class="table-wrap"><table class="md-table"><thead><tr>');
      for (const cell of head) {
        out.push("<th>" + inlineFormat(esc(cell)) + "</th>");
      }
      out.push("</tr></thead><tbody>");
      for (const row of body) {
        out.push("<tr>");
        for (const cell of row) {
          out.push("<td>" + inlineFormat(esc(cell)) + "</td>");
        }
        out.push("</tr>");
      }
      out.push("</tbody></table></div>");
    }

    function parseTableRow(line) {
      const trimmed = line.trim();
      if (!trimmed.startsWith("|") || !trimmed.includes("|", 1)) return null;
      const parts = trimmed.split("|");
      // drop leading/trailing empties from edge pipes
      if (parts[0].trim() === "") parts.shift();
      if (parts.length && parts[parts.length - 1].trim() === "") parts.pop();
      if (!parts.length) return null;
      return parts.map((c) => c.trim());
    }

    function isSeparatorRow(cells) {
      return (
        cells.length > 0 &&
        cells.every((c) => /^:?-{3,}:?$/.test(c.replace(/\s+/g, "")))
      );
    }

    function inlineFormat(escaped) {
      return escaped
        .replace(/`([^`]+)`/g, "<code>$1</code>")
        .replace(/\*\*([^*]+)\*\*/g, "<strong>$1</strong>")
        .replace(/\*([^*]+)\*/g, "<em>$1</em>")
        .replace(
          /\[([^\]]+)\]\((https?:[^)\s]+)\)/g,
          '<a href="$2" target="_blank" rel="noopener noreferrer">$1</a>'
        );
    }

    let i = 0;
    while (i < lines.length) {
      const line = lines[i];
      if (line.startsWith("```")) {
        if (inCode) {
          out.push("<pre><code>" + codeBuf.join("\n") + "</code></pre>");
          codeBuf = [];
          inCode = false;
        } else {
          flushList();
          inCode = true;
        }
        i += 1;
        continue;
      }
      if (inCode) {
        codeBuf.push(esc(line));
        i += 1;
        continue;
      }

      // GFM-ish tables: header | --- | rows
      const tableRow = parseTableRow(line);
      if (tableRow) {
        const next = i + 1 < lines.length ? parseTableRow(lines[i + 1]) : null;
        if (next && isSeparatorRow(next)) {
          flushList();
          const rows = [tableRow];
          i += 2;
          while (i < lines.length) {
            const r = parseTableRow(lines[i]);
            if (!r || isSeparatorRow(r)) break;
            rows.push(r);
            i += 1;
          }
          flushTable(rows);
          continue;
        }
      }

      const heading = /^(#{1,3})\s+(.*)$/.exec(line);
      if (heading) {
        flushList();
        const level = heading[1].length;
        out.push(`<h${level}>${inlineFormat(esc(heading[2]))}</h${level}>`);
        i += 1;
        continue;
      }

      const ul = /^[-*]\s+(.*)$/.exec(line);
      if (ul) {
        if (listType !== "ul") {
          flushList();
          out.push("<ul>");
          listType = "ul";
        }
        out.push("<li>" + inlineFormat(esc(ul[1])) + "</li>");
        i += 1;
        continue;
      }

      const ol = /^(\d+)\.\s+(.*)$/.exec(line);
      if (ol) {
        if (listType !== "ol") {
          flushList();
          out.push("<ol>");
          listType = "ol";
        }
        out.push("<li>" + inlineFormat(esc(ol[2])) + "</li>");
        i += 1;
        continue;
      }

      flushList();
      if (!line.trim()) {
        out.push("");
      } else {
        out.push("<p>" + inlineFormat(esc(line)) + "</p>");
      }
      i += 1;
    }
    if (inCode) {
      out.push("<pre><code>" + codeBuf.join("\n") + "</code></pre>");
    }
    flushList();
    return out.join("\n");
  }

  function canvasDescriptor(poi) {
    let v = poi && poi.value;
    if (typeof v === "string") {
      try {
        v = JSON.parse(v);
      } catch {
        return { kind: "unknown", raw: v };
      }
    }
    if (!v || typeof v !== "object") {
      return { kind: "unknown", raw: v };
    }
    return v;
  }

  function renderCanvasBody(desc) {
    const kind = (desc.kind || "unknown").toString().toLowerCase();
    if (kind === "markdown") {
      return `<div class="canvas-md">${renderMarkdown(desc.body || "")}</div>`;
    }
    if (kind === "image") {
      const src = resolveSrc(desc.src);
      const alt = esc(desc.alt || desc.title || "canvas image");
      return `<img src="${esc(src)}" alt="${alt}" loading="lazy" />`;
    }
    if (kind === "url") {
      const src = resolveSrc(desc.src);
      const h = clampHeight(desc.height, 360);
      return `<iframe src="${esc(src)}" height="${h}" sandbox="allow-scripts" referrerpolicy="no-referrer" title="${esc(
        desc.title || "url canvas"
      )}"></iframe>`;
    }
    if (kind === "html") {
      const h = clampHeight(desc.height, 280);
      const body = String(desc.body || "");
      return `<iframe srcdoc="${esc(body)}" height="${h}" sandbox="" referrerpolicy="no-referrer" title="${esc(
        desc.title || "html canvas"
      )}"></iframe>`;
    }
    let pretty;
    try {
      pretty = JSON.stringify(desc, null, 2);
    } catch {
      pretty = String(desc);
    }
    return `<pre class="canvas-fallback">${esc(pretty)}</pre>`;
  }

  function renderEvents(events, limit) {
    const cap = limit == null ? 25 : limit;
    const rows = events.slice().reverse().slice(0, cap);
    if (!rows.length) {
      return `<p class="placeholder">none</p>`;
    }
    const html = rows
      .map((e) => {
        const fam = eventFamily(e.kind);
        const id = e.id != null ? String(e.id) : "—";
        const summary = payloadSummary(e.payload);
        return (
          `<div class="event-row fam-${fam}">` +
          `<span class="event-id mono">#${esc(id)}</span>` +
          `<div class="event-main">` +
          `<span class="event-kind">${esc(e.kind || "event")}</span>` +
          `<div class="event-payload">${esc(summary || "—")}</div>` +
          `</div>` +
          `<span class="event-time" title="${esc(e.created_at || "")}">${esc(
            relTime(e.created_at)
          )}</span>` +
          `</div>`
        );
      })
      .join("");
    return `<div class="event-list">${html}</div>`;
  }

  function renderFiles(files) {
    if (!files.length) {
      return `<p class="placeholder">none</p>`;
    }
    const chips = files
      .map((f) => `<span class="file-chip" title="${esc(f)}">${esc(f)}</span>`)
      .join("");
    return `<div class="file-grid">${chips}</div>`;
  }

  function renderCanvases(canvases) {
    if (!canvases.length) {
      return (
        `<div class="canvas-empty">` +
        `<p>No canvases yet — publish with <code>porq canvas set</code></p>` +
        `<p>Canvases are the primary view. Ops panels live under <strong>Details</strong>.</p>` +
        `</div>`
      );
    }
    const cards = canvases
      .map((poi) => {
        const desc = canvasDescriptor(poi);
        const title = desc.title || poi.key || "canvas";
        const span =
          (poi.columns && (poi.columns.span === 2 || poi.columns.span === "2")) ||
          desc.span === 2
            ? " span-2"
            : "";
        const kind = (desc.kind || "unknown").toString();
        return (
          `<article class="canvas-card${span}" data-key="${esc(poi.key)}" data-kind="${esc(kind)}">` +
          `<div class="canvas-head">` +
          `<div class="canvas-head-row">` +
          `<span class="canvas-title">${esc(title)}</span>` +
          `${pill(poi.state || "live")}` +
          `</div>` +
          `<div class="canvas-meta">` +
          `<span class="canvas-kind">${esc(kind)}</span>` +
          `<span class="canvas-ver" title="v${esc(poi.version)} · ${esc(
            poi.updated_at || ""
          )}">v${esc(poi.version)} · ${esc(relTime(poi.updated_at))}</span>` +
          `</div></div>` +
          `<div class="canvas-body scroll-themed">${renderCanvasBody(desc)}</div>` +
          `</article>`
        );
      })
      .join("");
    return `<div class="canvas-grid">${cards}</div>`;
  }

  function renderOpsHealth(d) {
    const daemonRunning = !!(d.daemon && d.daemon.running);
    const leases = asArray(d.leases);
    const triggers = asArray(d.triggers);
    const blocked = asArray(d.blocked_pois);
    const triggerFailures = asArray(d.trigger_failures);
    const sessions = asArray(d.active_sessions);
    const models = asArray(d.models);

    const daemonPill = daemonRunning
      ? `<span class="pill status-done">daemon up</span>`
      : `<span class="pill status-failed">daemon down</span>`;

    const leaseRows = leases
      .map(
        (l) =>
          `<tr>` +
          `<td class="mono">${esc(l.table)}/${esc(l.key)}</td>` +
          `<td>${esc(l.kind)}</td>` +
          `<td class="mono">${esc(l.holder)}</td>` +
          `<td title="${esc(l.expires_at || "")}">${esc(relTime(l.expires_at))}</td>` +
          `</tr>`
      )
      .join("");

    const triggerRows = triggers
      .map(
        (t) =>
          `<tr>` +
          `<td class="mono">${esc((t.id || "").slice(0, 8))}</td>` +
          `<td>${esc(t.name)}</td>` +
          `<td class="mono">${esc(t.event_pattern)}</td>` +
          `<td>${t.enabled === false ? pill("disabled") : pill("enabled")}</td>` +
          `</tr>`
      )
      .join("");

    const blockedRows = blocked
      .map(
        (p) =>
          `<tr>` +
          `<td class="mono">${esc(p.table)}/${esc(p.key)}</td>` +
          `<td>${pill(p.state)}</td>` +
          `<td>${esc(p.blocker_reason || "—")}</td>` +
          `</tr>`
      )
      .join("");

    const modelChips = models
      .slice(0, 12)
      .map((m) => `<span class="mini-chip mono" title="${esc(m.display_name || m.id)}">${esc(m.id)}</span>`)
      .join("");

    return (
      `<div class="ops-grid">` +
      `<div class="ops-card">` +
      `<div class="ops-label">Daemon</div>` +
      `<div class="ops-value">${daemonPill}</div>` +
      `</div>` +
      `<div class="ops-card">` +
      `<div class="ops-label">Active sessions</div>` +
      `<div class="ops-value">${sessions.length ? chipList(sessions, "") : `<span class="placeholder inline">none</span>`}</div>` +
      `</div>` +
      `<div class="ops-card">` +
      `<div class="ops-label">Models</div>` +
      `<div class="ops-value">${models.length ? `<div class="chip-row">${modelChips}</div>` : `<span class="placeholder inline">none</span>`}</div>` +
      `</div>` +
      `<div class="ops-card ops-wide">` +
      `<div class="ops-head"><span class="ops-label">Leases</span><span class="count">${leases.length}</span></div>` +
      tableOrEmpty(
        leaseRows
          ? `<thead><tr><th>poi</th><th>kind</th><th>holder</th><th>expires</th></tr></thead><tbody>${leaseRows}</tbody>`
          : "",
        "none"
      ) +
      `</div>` +
      `<div class="ops-card ops-wide">` +
      `<div class="ops-head"><span class="ops-label">Triggers</span><span class="count">${triggers.length}</span></div>` +
      tableOrEmpty(
        triggerRows
          ? `<thead><tr><th>id</th><th>name</th><th>pattern</th><th>state</th></tr></thead><tbody>${triggerRows}</tbody>`
          : "",
        "none"
      ) +
      `</div>` +
      `<div class="ops-card ops-wide">` +
      `<div class="ops-head"><span class="ops-label">Blocked POIs</span><span class="count">${blocked.length}</span></div>` +
      tableOrEmpty(
        blockedRows
          ? `<thead><tr><th>poi</th><th>state</th><th>reason</th></tr></thead><tbody>${blockedRows}</tbody>`
          : "",
        "none"
      ) +
      `</div>` +
      `<div class="ops-card ops-wide">` +
      `<div class="ops-head"><span class="ops-label">Trigger failures</span><span class="count">${triggerFailures.length}</span></div>` +
      renderEvents(triggerFailures, 10) +
      `</div>` +
      `</div>`
    );
  }

  function drawerField(label, valueHtml) {
    return (
      `<div class="drawer-field">` +
      `<div class="drawer-label">${esc(label)}</div>` +
      `<div class="drawer-value">${valueHtml}</div>` +
      `</div>`
    );
  }

  function renderTaskDrawer(task) {
    if (!task) return "";
    const status =
      typeof task.status === "string" ? task.status : task.status && task.status.toString();
    const parents = asArray(task.depends_on);
    const children = (childTasksById.get(task.id) || []).map((c) => c.id);
    const claims = asArray(task.claims);
    const needsPoi = asArray(task.needs_poi);

    const parentLinks = parents.length
      ? parents
          .map(
            (id) =>
              `<button type="button" class="linkish mono drawer-open-task" data-task-id="${esc(id)}">${esc(
                id.slice(0, 8)
              )}</button>`
          )
          .join(" ")
      : `<span class="placeholder inline">none</span>`;

    const childLinks = children.length
      ? children
          .map(
            (id) =>
              `<button type="button" class="linkish mono drawer-open-task" data-task-id="${esc(id)}">${esc(
                id.slice(0, 8)
              )}</button>`
          )
          .join(" ")
      : `<span class="placeholder inline">none</span>`;

    const claimsHtml = claims.length
      ? claims.map((c) => `<code>${esc(c)}</code>`).join(" ")
      : `<span class="placeholder inline">none</span>`;

    return (
      `<div class="drawer-section">` +
      drawerField("Status", pill(status)) +
      drawerField(
        "Task id",
        `<span class="mono">${esc(task.id)}</span> ` +
          `<button type="button" class="copy-btn" data-copy="${esc(task.id)}">Copy ID</button>`
      ) +
      drawerField("Name", esc(task.name || "—")) +
      drawerField("Session", `<span class="mono">${esc(task.session || "—")}</span>`) +
      drawerField("Job (parent)", `<span class="mono">${esc(task.job_id || "—")}</span>`) +
      drawerField("Depends on", parentLinks) +
      drawerField("Children", childLinks) +
      drawerField(
        "Exit",
        `<span class="mono">${task.exit_code == null ? "—" : esc(task.exit_code)}</span>` +
          (task.pid != null ? ` · pid ${esc(task.pid)}` : "")
      ) +
      drawerField(
        "Command",
        `<pre class="drawer-cmd">${esc(task.command || "—")}</pre>` +
          `<button type="button" class="copy-btn" data-copy="${esc(task.command || "")}">Copy command</button>`
      ) +
      drawerField("Claims", claimsHtml) +
      (needsPoi.length ? drawerField("Needs POI", needsPoi.map((p) => `<code>${esc(p)}</code>`).join(" ")) : "") +
      drawerField("Profile / model", `<span class="mono">${esc(task.profile || "—")} · ${esc(task.model_id || "—")}</span>`) +
      drawerField("Attempt", esc(task.attempt) + " / " + esc(task.max_attempts)) +
      drawerField("Created", `<span title="${esc(task.created_at || "")}">${esc(fmtTime(task.created_at))}</span>`) +
      drawerField("Updated", `<span title="${esc(task.updated_at || "")}">${esc(fmtTime(task.updated_at))}</span>`) +
      `</div>` +
      `<div class="drawer-section">` +
      `<div class="drawer-label">Log tail</div>` +
      `<button type="button" class="ghost-btn" id="drawer-load-log" data-task-id="${esc(task.id)}">Load log tail</button>` +
      `<pre class="drawer-log" id="drawer-log">—</pre>` +
      `</div>`
    );
  }

  function openTaskDrawer(taskId) {
    const task = taskById.get(taskId);
    if (!task) return;
    drawerTaskId = taskId;
    const drawer = document.getElementById("task-drawer");
    const backdrop = document.getElementById("drawer-backdrop");
    const title = document.getElementById("drawer-title");
    const body = document.getElementById("drawer-body");
    if (!drawer || !backdrop || !body) return;
    if (title) title.textContent = task.name || "Task";
    body.innerHTML = renderTaskDrawer(task);
    drawer.classList.remove("hidden");
    backdrop.classList.remove("hidden");
    drawer.setAttribute("aria-hidden", "false");
    backdrop.setAttribute("aria-hidden", "false");
    document.body.classList.add("drawer-open");
    const closeBtn = document.getElementById("drawer-close");
    if (closeBtn) closeBtn.focus();
  }

  function closeTaskDrawer() {
    drawerTaskId = null;
    const drawer = document.getElementById("task-drawer");
    const backdrop = document.getElementById("drawer-backdrop");
    if (!drawer || !backdrop) return;
    drawer.classList.add("hidden");
    backdrop.classList.add("hidden");
    drawer.setAttribute("aria-hidden", "true");
    backdrop.setAttribute("aria-hidden", "true");
    document.body.classList.remove("drawer-open");
  }

  async function loadDrawerLog(taskId) {
    const logEl = document.getElementById("drawer-log");
    const btn = document.getElementById("drawer-load-log");
    if (!logEl) return;
    logEl.textContent = "loading…";
    if (btn) btn.disabled = true;
    try {
      const res = await fetch(
        `/api/v1/tasks/${encodeURIComponent(taskId)}/logs?tailBytes=${LOG_TAIL_BYTES}&ts=${Date.now()}`,
        { cache: "no-store" }
      );
      const data = await res.json();
      if (!res.ok || !data.ok) {
        throw new Error((data && data.error) || "HTTP " + res.status);
      }
      const prefix = data.truncated ? "… (truncated)\n" : "";
      logEl.textContent = prefix + (data.content || "");
    } catch (e) {
      logEl.textContent = "log unavailable: " + (e && e.message ? e.message : e);
    } finally {
      if (btn) btn.disabled = false;
    }
  }

  const VIEW_KEY = "porq.dash.view";
  const viewCanvases = document.getElementById("view-canvases");
  const viewDetails = document.getElementById("view-details");
  const tabCanvases = document.getElementById("tab-canvases");
  const tabDetails = document.getElementById("tab-details");
  const pulseEl = document.getElementById("pulse");
  let userPickedView = false;
  let currentView = "canvases";

  function applyView(view) {
    currentView = view === "details" ? "details" : "canvases";
    viewCanvases.classList.toggle("active", currentView === "canvases");
    viewDetails.classList.toggle("active", currentView === "details");
    tabCanvases.classList.toggle("active", currentView === "canvases");
    tabDetails.classList.toggle("active", currentView === "details");
    tabCanvases.setAttribute("aria-selected", currentView === "canvases" ? "true" : "false");
    tabDetails.setAttribute("aria-selected", currentView === "details" ? "true" : "false");
  }

  function setView(view, { persist = true, user = false } = {}) {
    if (user) userPickedView = true;
    applyView(view);
    if (persist) {
      try {
        localStorage.setItem(VIEW_KEY, currentView);
      } catch {
        /* ignore */
      }
    }
  }

  try {
    const saved = localStorage.getItem(VIEW_KEY);
    if (saved === "details" || saved === "canvases") {
      userPickedView = true;
      applyView(saved);
    }
  } catch {
    /* ignore */
  }

  tabCanvases.addEventListener("click", () => setView("canvases", { user: true }));
  tabDetails.addEventListener("click", () => setView("details", { user: true }));
  pulseEl.addEventListener("click", () => setView("details", { user: true }));
  pulseEl.addEventListener("keydown", (e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      setView("details", { user: true });
    }
  });

  document.getElementById("drawer-close")?.addEventListener("click", closeTaskDrawer);
  document.getElementById("drawer-backdrop")?.addEventListener("click", closeTaskDrawer);
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && drawerTaskId) closeTaskDrawer();
  });

  document.getElementById("drawer-body")?.addEventListener("click", (e) => {
    const t = e.target;
    if (!(t instanceof HTMLElement)) return;
    if (t.classList.contains("copy-btn")) {
      copyText(t.getAttribute("data-copy"), t);
      return;
    }
    if (t.classList.contains("drawer-open-task")) {
      const id = t.getAttribute("data-task-id");
      if (id) openTaskDrawer(id);
      return;
    }
    if (t.id === "drawer-load-log" || t.closest("#drawer-load-log")) {
      const btn = t.id === "drawer-load-log" ? t : t.closest("#drawer-load-log");
      const id = btn && btn.getAttribute("data-task-id");
      if (id) loadDrawerLog(id);
    }
  });

  document.getElementById("tasks")?.addEventListener("click", (e) => {
    const row = e.target.closest("tr[data-task-id]");
    if (!row) return;
    openTaskDrawer(row.getAttribute("data-task-id"));
  });

  document.getElementById("tasks")?.addEventListener("keydown", (e) => {
    if (e.key !== "Enter" && e.key !== " ") return;
    const row = e.target.closest("tr[data-task-id]");
    if (!row) return;
    e.preventDefault();
    openTaskDrawer(row.getAttribute("data-task-id"));
  });

  if (errorRetryEl) {
    errorRetryEl.addEventListener("click", () => {
      missedPolls = 0;
      tick();
    });
  }

  function updatePulse(board, tasks, jobs, events, d) {
    document.getElementById("pulse-tasks").textContent = String(tasks.length);
    document.getElementById("pulse-pois").textContent = String(board.length);
    document.getElementById("pulse-jobs").textContent = String(jobs.length);
    document.getElementById("pulse-events").textContent = String(events.length);

    const daemonEl = document.getElementById("pulse-daemon");
    if (daemonEl) {
      const up = !!(d && d.daemon && d.daemon.running);
      daemonEl.textContent = up ? "daemon up" : "daemon down";
      daemonEl.classList.toggle("status-done", up);
      daemonEl.classList.toggle("status-failed", !up);
    }

    const last = events.length ? events[events.length - 1] : null;
    const pulseEvent = document.getElementById("pulse-event");
    if (!last) {
      pulseEvent.textContent = "waiting for pulse…";
      return;
    }
    const id = last.id != null ? "#" + last.id : "";
    pulseEvent.textContent = id + " " + (last.kind || "event") + " · " + relTime(last.created_at);
    pulseEvent.title = last.created_at || "";
  }

  function updateEmptyHint(d, hasAny) {
    const daemonRunning = !!(d.daemon && d.daemon.running);
    if (hasAny) {
      emptyHint.classList.remove("visible", "daemon-down", "empty-ws");
      return;
    }
    emptyHint.classList.add("visible");
    emptyHint.classList.toggle("daemon-down", !daemonRunning);
    emptyHint.classList.toggle("empty-ws", daemonRunning);
    if (!daemonRunning) {
      emptyHint.innerHTML =
        "Daemon not running — tasks and triggers will not progress. Start the porq daemon or the parent <code>porq dash serve</code> process.";
    } else {
      emptyHint.innerHTML =
        "Workspace is empty — seed data with <code>porq</code> or wait for snapshot refresh.";
    }
  }

  function render(d) {
    const board = asArray(d.board);
    const tasks = asArray(d.tasks);
    const jobs = asArray(d.jobs);
    const aff = asArray(d.affinities);
    const events = asArray(d.events);
    const files = asArray(d.files);
    const canvases = asArray(d.canvases);

    indexTasks(tasks);

    workspaceEl.textContent = "ws " + (d.workspace || "—");

    setCount("count-board", board.length);
    setCount("count-tasks", tasks.length);
    setCount("count-jobs", jobs.length);
    setCount("count-aff", aff.length);
    setCount("count-events", events.length);
    setCount("count-files", files.length);

    updatePulse(board, tasks, jobs, events, d);

    if (!userPickedView) {
      applyView(canvases.length === 0 ? "details" : "canvases");
    }

    const boardRows = board
      .map(
        (p) =>
          `<tr><td class="mono">${esc(p.key)}</td><td>${pill(p.blocked ? "blocked" : p.state)}</td><td>${esc(
            (p.columns && p.columns.owner) || ""
          )}</td><td><code>${esc(JSON.stringify(p.value))}</code></td></tr>`
      )
      .join("");
    document.getElementById("board").innerHTML = tableOrEmpty(
      boardRows
        ? `<thead><tr><th>key</th><th>state</th><th>owner</th><th>value</th></tr></thead><tbody>${boardRows}</tbody>`
        : "",
      "empty"
    );

    const taskRows = tasks
      .map((t) => {
        const status = typeof t.status === "string" ? t.status : String(t.status || "");
        return (
          `<tr class="task-row" tabindex="0" role="button" data-task-id="${esc(t.id)}" ` +
          `aria-label="Task ${esc(t.name || t.id)} status ${esc(status)}">` +
          `<td class="mono">${esc((t.id || "").slice(0, 8))}</td>` +
          `<td>${esc(t.name)}</td>` +
          `<td>${pill(status)}</td>` +
          `<td class="mono">${esc(t.model_id || t.profile || "")}</td>` +
          `<td class="mono cmd-cell">${esc((t.command || "").slice(0, 48))}</td>` +
          `</tr>`
        );
      })
      .join("");
    document.getElementById("tasks").innerHTML = tableOrEmpty(
      taskRows
        ? `<thead><tr><th>id</th><th>name</th><th>status</th><th>model</th><th>cmd</th></tr></thead><tbody>${taskRows}</tbody>`
        : "",
      "none"
    );

    const jobRows = jobs
      .map(
        (j) =>
          `<tr><td class="mono">${esc((j.id || "").slice(0, 8))}</td><td>${esc(
            j.name
          )}</td><td>${pill(j.status)}</td><td>${esc(
            j.strategy
          )}</td><td class="mono">${esc(j.route_reason || "")}</td></tr>`
      )
      .join("");
    document.getElementById("jobs").innerHTML = tableOrEmpty(
      jobRows
        ? `<thead><tr><th>id</th><th>name</th><th>status</th><th>strategy</th><th>route</th></tr></thead><tbody>${jobRows}</tbody>`
        : "",
      "none"
    );

    const affRows = aff
      .map(
        (a) =>
          `<tr><td class="mono">${esc(a.class)}</td><td>${esc(
            a.model_id
          )}</td><td>${(+a.score || 0).toFixed(3)}</td><td>n=${esc(a.n)}</td></tr>`
      )
      .join("");
    document.getElementById("aff").innerHTML = tableOrEmpty(
      affRows
        ? `<thead><tr><th>class</th><th>model</th><th>score</th><th>samples</th></tr></thead><tbody>${affRows}</tbody>`
        : "",
      "none"
    );

    const opsEl = document.getElementById("ops-health");
    if (opsEl) opsEl.innerHTML = renderOpsHealth(d);

    document.getElementById("canvases").innerHTML = renderCanvases(canvases);
    document.getElementById("events").innerHTML = renderEvents(events);
    document.getElementById("files").innerHTML = renderFiles(files);

    const hasAny =
      board.length + tasks.length + jobs.length + aff.length + events.length + canvases.length > 0;
    updateEmptyHint(d, hasAny);
    emptyHint.classList.toggle("visible", !hasAny && currentView === "details");

    if (drawerTaskId && taskById.has(drawerTaskId)) {
      const body = document.getElementById("drawer-body");
      const title = document.getElementById("drawer-title");
      const t = taskById.get(drawerTaskId);
      if (body && t) {
        if (title) title.textContent = t.name || "Task";
        body.innerHTML = renderTaskDrawer(t);
      }
    }
  }

  async function tick() {
    if (fetchInFlight) return;
    fetchInFlight = true;
    try {
      const res = await fetch("data.json?ts=" + Date.now(), { cache: "no-store" });
      if (!res.ok) {
        throw new Error("data.json HTTP " + res.status);
      }
      const d = await res.json();
      lastSnapshot = d;
      lastSuccessAt = Date.now();
      missedPolls = 0;
      clearError();
      render(d);
      startFreshnessTimer();
      updateFreshnessUI();
    } catch (e) {
      missedPolls += 1;
      const msg = e && e.message ? e.message : String(e);
      if (lastSnapshot) {
        showError("Fetch failed — showing last snapshot. " + msg);
        updateFreshnessUI();
      } else {
        showError("Failed to load data.json: " + msg);
        stampEl.textContent = "error";
        stampEl.classList.add("error");
        stampEl.classList.remove("stale", "live");
      }
    } finally {
      fetchInFlight = false;
    }
  }

  tick();
  setInterval(tick, POLL_MS);
})();
