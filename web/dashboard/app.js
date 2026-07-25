(function () {
  const stampEl = document.getElementById("stamp");
  const workspaceEl = document.getElementById("workspace");
  const errorEl = document.getElementById("error");
  const emptyHint = document.getElementById("empty-hint");

  function asArray(v) {
    if (v == null) return [];
    if (Array.isArray(v)) return v;
    return [v];
  }

  function pill(s) {
    const cls = (s || "").toString().replace(/[^a-z0-9_-]/gi, "");
    return `<span class="pill ${cls}">${esc(s || "-")}</span>`;
  }

  function showError(msg) {
    errorEl.textContent = msg;
    errorEl.classList.add("visible");
  }

  function clearError() {
    errorEl.textContent = "";
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

  function eventFamily(kind) {
    const k = (kind || "").toString().toLowerCase();
    const head = k.split(".")[0] || "other";
    if (head === "task") return "task";
    if (head === "poi") return "poi";
    if (head === "job" || head === "route") return head === "route" ? "route" : "job";
    if (head === "affinity") return "affinity";
    if (head === "model") return "model";
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
    const prefer = ["name", "key", "table", "status", "id", "state", "model_id", "class"];
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

    for (const line of lines) {
      if (line.startsWith("```")) {
        if (inCode) {
          out.push("<pre><code>" + codeBuf.join("\n") + "</code></pre>");
          codeBuf = [];
          inCode = false;
        } else {
          flushList();
          inCode = true;
        }
        continue;
      }
      if (inCode) {
        codeBuf.push(esc(line));
        continue;
      }

      const heading = /^(#{1,3})\s+(.*)$/.exec(line);
      if (heading) {
        flushList();
        const level = heading[1].length;
        out.push(`<h${level}>${inlineFormat(esc(heading[2]))}</h${level}>`);
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
        continue;
      }

      flushList();
      if (!line.trim()) {
        out.push("");
      } else {
        out.push("<p>" + inlineFormat(esc(line)) + "</p>");
      }
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

  function renderCanvases(canvases) {
    if (!canvases.length) {
      return `<p class="placeholder">none — publish with <code>orq canvas set</code></p>`;
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
          `<span class="canvas-title">${esc(title)}</span>` +
          `<div class="canvas-meta">` +
          `<span class="mono">${esc(kind)}</span>` +
          `${pill(poi.state || "live")}` +
          `<span title="v${esc(poi.version)} · ${esc(poi.updated_at || "")}">v${esc(
            poi.version
          )} · ${esc(relTime(poi.updated_at))}</span>` +
          `</div></div>` +
          `<div class="canvas-body">${renderCanvasBody(desc)}</div>` +
          `</article>`
        );
      })
      .join("");
    return `<div class="canvas-grid">${cards}</div>`;
  }

  function renderEvents(events) {
    const rows = events
      .slice()
      .reverse()
      .slice(0, 25);
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

  function render(d) {
    const updated = d.updated || "—";
    stampEl.textContent = updated;
    stampEl.classList.remove("error", "stale");
    workspaceEl.textContent = "ws " + (d.workspace || "—");

    const board = asArray(d.board);
    const tasks = asArray(d.tasks);
    const jobs = asArray(d.jobs);
    const aff = asArray(d.affinities);
    const events = asArray(d.events);
    const files = asArray(d.files);
    const canvases = asArray(d.canvases);

    setCount("count-board", board.length);
    setCount("count-tasks", tasks.length);
    setCount("count-jobs", jobs.length);
    setCount("count-aff", aff.length);
    setCount("count-events", events.length);
    setCount("count-files", files.length);
    setCount("count-canvases", canvases.length);

    const boardRows = board
      .map(
        (p) =>
          `<tr><td class="mono">${esc(p.key)}</td><td>${pill(p.state)}</td><td>${esc(
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
      .map(
        (t) =>
          `<tr><td class="mono">${esc((t.id || "").slice(0, 8))}</td><td>${esc(
            t.name
          )}</td><td>${pill(t.status)}</td><td class="mono">${esc(
            t.model_id || t.profile || ""
          )}</td><td class="mono">${esc((t.command || "").slice(0, 48))}</td></tr>`
      )
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

    document.getElementById("canvases").innerHTML = renderCanvases(canvases);
    document.getElementById("events").innerHTML = renderEvents(events);
    document.getElementById("files").innerHTML = renderFiles(files);

    const hasAny =
      board.length +
        tasks.length +
        jobs.length +
        aff.length +
        events.length +
        canvases.length >
      0;
    emptyHint.classList.toggle("visible", !hasAny);
  }

  async function tick() {
    try {
      const res = await fetch("data.json?ts=" + Date.now(), {
        cache: "no-store",
      });
      if (!res.ok) {
        throw new Error("data.json HTTP " + res.status);
      }
      const d = await res.json();
      clearError();
      render(d);
    } catch (e) {
      showError("Failed to load data.json: " + (e && e.message ? e.message : e));
      stampEl.textContent = "error";
      stampEl.classList.add("error");
    }
  }

  tick();
  setInterval(tick, 1000);
})();
