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

    setCount("count-board", board.length);
    setCount("count-tasks", tasks.length);
    setCount("count-jobs", jobs.length);
    setCount("count-aff", aff.length);
    setCount("count-events", events.length);
    setCount("count-files", files.length);

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

    document.getElementById("events").innerHTML = renderEvents(events);
    document.getElementById("files").innerHTML = renderFiles(files);

    const hasAny =
      board.length + tasks.length + jobs.length + aff.length + events.length > 0;
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
